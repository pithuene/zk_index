//! The Watcher module is responsible for listening to filesystem changes and
//! adds the necessary IndexEvents to a channel, which can then be consumed by
//! the Indexer.

use notify::{recommended_watcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::mpsc::{channel, Receiver, Sender},
    time::UNIX_EPOCH,
};

use crate::sqlite::with_db_conn;

#[derive(Clone, Debug)]
pub enum IndexEvent {
    // Remove must be idempotent, it may be called for non-existing notes.
    Remove(PathBuf),
    Add(PathBuf),
}

pub struct DirWatcher {
    vault_root_path: Box<Path>,
    watcher: RecommendedWatcher,
    file_event_receiver: Receiver<Result<notify::Event, notify::Error>>,
    index_event_sender: Sender<IndexEvent>,
    /// Function which is passed to the watcher to configure a file filter.
    file_filter: Box<dyn Fn(&Path) -> bool>,
}

pub fn file_has_no_hidden_component(path: &Path) -> bool {
    !path.components().any(|c| {
        c.as_os_str().to_str().unwrap().starts_with('.')
            && c.as_os_str().to_str().unwrap().len() > 1
    })
}

impl DirWatcher {
    pub fn new(
        path: &str,
        index_event_sender: Sender<IndexEvent>,
        file_filter: Box<dyn Fn(&Path) -> bool>,
    ) -> Self {
        let path = Path::new(path);

        let (tx, rx) = channel();
        let config = notify::Config::default().with_compare_contents(false);
        let mut watcher: RecommendedWatcher = recommended_watcher(tx).unwrap();
        watcher.configure(config).unwrap();

        Self {
            vault_root_path: Box::from(path),
            watcher,
            file_event_receiver: rx,
            index_event_sender,
            file_filter,
        }
    }

    /// Sync the filesystem and the index.
    /// Iterate over all files in the vault and check if they have been modified
    /// since the last run.
    fn sync_fs_and_index(&mut self) {
        use diesel::RunQueryDsl;

        // Get the last run times from the database.
        // As the directory is traversed, the entries are removed from this map.
        // At the end, all remaining entries must have been deleted while the
        // indexer was not running and are therefore removed from the index.
        let mut file_map: HashMap<String, i32> = HashMap::new();
        with_db_conn(|conn| {
            for f in crate::sqlite::schema::file::dsl::file
                .load::<crate::sqlite::models::File>(conn)
                .unwrap()
                .into_iter()
            {
                file_map.insert(f.path, f.last_indexed);
            }
        });

        // Recursively walk the vault directory and find all files.
        for entry in walkdir::WalkDir::new(&self.vault_root_path) {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            if metadata.is_file() && (self.file_filter)(entry.path()) {
                let modified = metadata
                    .modified()
                    .unwrap()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let rel_path = self.relative_path_from_absolute_path(entry.path());
                let last_indexed = file_map.remove(rel_path.to_str().unwrap()).unwrap_or(0);

                if last_indexed < modified.try_into().unwrap() {
                    log::debug!(
                        "File {:?} has been modified at {} which is after the last index time {}.",
                        entry.path(),
                        modified,
                        last_indexed
                    );
                    self.emit_index_event(IndexEvent::Remove(rel_path.clone()));
                    self.emit_index_event(IndexEvent::Add(rel_path));
                }
            }
        }

        // All remaining entries in the file map have been deleted while the
        // indexer was not running and are now removed from the index.
        for (rel_path, _) in file_map {
            self.emit_index_event(IndexEvent::Remove(rel_path.into()));
        }
    }

    pub fn start(&mut self) {
        self.watcher
            .watch(&self.vault_root_path, RecursiveMode::Recursive)
            .unwrap();

        self.sync_fs_and_index();

        loop {
            match self.file_event_receiver.recv().unwrap() {
                Ok(event) => self.handle_event(event),
                Err(e) => log::error!("INotifyWatcher error: {:?}", e),
            }
        }
    }

    fn emit_index_event(&self, event: IndexEvent) {
        log::debug!("Adding {:?} event to index event channel.", event);
        self.index_event_sender.send(event).unwrap();
    }

    fn handle_event(&self, event: notify::event::Event) {
        use notify::event::EventKind::{Create, Modify, Remove};
        use notify::event::ModifyKind;
        use notify::event::RenameMode;
        match event.kind {
            /* Apparently, the RenameMode::Both event is emitted **in addition** to the
               RenameMode::From and RenameMode::To events. So we don't need to handle it
               here.

            Modify(ModifyKind::Name(RenameMode::Both)) => {
                log::debug!("Handling event: {:?}", event);
                assert!(event.paths.len() == 2);
                let from_path = event.paths.first().unwrap();
                let to_path = event.paths.last().unwrap();
                if (self.file_filter)(from_path) {
                    let rel_path = self.relative_path_from_absolute_path(from_path);
                    self.emit_index_event(IndexEvent::Remove(rel_path));
                }
                if (self.file_filter)(to_path) {
                    let rel_path = self.relative_path_from_absolute_path(to_path);
                    self.emit_index_event(IndexEvent::Add(rel_path));
                }
            }*/
            Modify(ModifyKind::Name(RenameMode::From)) => {
                assert!(event.paths.len() == 1);
                let from_path = event.paths.first().unwrap();
                if (self.file_filter)(from_path) {
                    log::debug!("Handling event: {:?}", event);
                    let rel_path = self.relative_path_from_absolute_path(from_path);
                    self.emit_index_event(IndexEvent::Remove(rel_path));
                }
            }
            Modify(ModifyKind::Name(RenameMode::To)) => {
                assert!(event.paths.len() == 1);
                let to_path = event.paths.first().unwrap();
                if (self.file_filter)(to_path) {
                    log::debug!("Handling event: {:?}", event);
                    let rel_path = self.relative_path_from_absolute_path(to_path);
                    self.emit_index_event(IndexEvent::Add(rel_path));
                }
            }
            Modify(ModifyKind::Data(_)) => {
                assert!(event.paths.len() == 1);
                event.paths.iter().for_each(|path| {
                    if (self.file_filter)(path) {
                        log::debug!("Handling event: {:?}", event);
                        let rel_path = self.relative_path_from_absolute_path(path);
                        self.emit_index_event(IndexEvent::Remove(rel_path.clone()));
                        self.emit_index_event(IndexEvent::Add(rel_path));
                    }
                });
            }
            Create(_) => {
                assert!(event.paths.len() == 1);
                event.paths.iter().for_each(|path| {
                    if (self.file_filter)(path) {
                        log::debug!("Handling event: {:?}", event);
                        self.emit_index_event(IndexEvent::Add(
                            self.relative_path_from_absolute_path(path),
                        ));
                    }
                });
            }
            Remove(_) => {
                assert!(event.paths.len() == 1);
                event.paths.iter().for_each(|path| {
                    if (self.file_filter)(path) {
                        log::debug!("Handling event: {:?}", event);
                        self.emit_index_event(IndexEvent::Remove(
                            self.relative_path_from_absolute_path(path),
                        ));
                    }
                });
            }
            _ => {
                log::debug!("Unhandled event: {:?}", event);
            }
        }
    }

    fn relative_path_from_absolute_path(&self, absolute_path: &Path) -> PathBuf {
        assert!(
            absolute_path.starts_with(&self.vault_root_path),
            "Path {:?} is not a subpath of the vault root path {:?}.",
            absolute_path,
            self.vault_root_path
        );
        absolute_path
            .strip_prefix(&self.vault_root_path)
            .unwrap()
            .to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Sender;

    use crate::watcher::file_has_no_hidden_component;
    use proptest::prelude::*;

    use super::IndexEvent;

    fn setup_watcher(
        index_event_sender: Sender<IndexEvent>,
    ) -> (tempfile::TempDir, super::DirWatcher) {
        // Create a temporary directory.
        let temp_dir = tempfile::tempdir().unwrap();

        let watcher = super::DirWatcher::new(
            temp_dir.path().to_str().unwrap(),
            index_event_sender,
            Box::new(file_has_no_hidden_component),
        );
        (temp_dir, watcher)
    }

    fn cleanup_watcher(dir: tempfile::TempDir) {
        tempfile::TempDir::close(dir).unwrap();
    }

    #[test]
    fn test_false() {
        assert!(false);
    }

    #[test]
    fn test_relative_path_from_absolute_path() {
        use std::path::PathBuf;
        let (dir, watcher) = setup_watcher(std::sync::mpsc::channel().0);

        assert_eq!(
            watcher.relative_path_from_absolute_path(&dir.path().join(".zk_index/")),
            PathBuf::from(".zk_index/")
        );

        proptest!(|(name in "[^/\0]+")| {
            if name != "." && name != ".." {
                let path = dir.path().join(&name);
                assert_eq!(
                    watcher.relative_path_from_absolute_path(&path),
                    PathBuf::from(name)
                );
            }
        });

        cleanup_watcher(dir);
    }

    #[test]
    fn test_file_has_hidden_component() {
        use std::path::Path;

        // Hidden files.
        assert!(!file_has_no_hidden_component(Path::new("./.test")));
        assert!(!file_has_no_hidden_component(Path::new("/tmp/.test")));
        assert!(!file_has_no_hidden_component(Path::new("/tmp/.test/test")));
        assert!(!file_has_no_hidden_component(Path::new("/tmp/test/.test")));
        assert!(!file_has_no_hidden_component(Path::new(
            "/tmp/test/.test/test"
        )));

        proptest!(|(components in prop::collection::vec("[^/\0.][^/\0]*", 1..20))| {
            // Random component is hidden.
            let mut components = components;
            let random_index = rand::random::<usize>() % components.len();
            components[random_index] = format!(".{}", components[random_index]);

            let str_path = components.join("/");
            let path = Path::new(&str_path);
            assert!(!file_has_no_hidden_component(path));
        });

        // Non-hidden files.
        assert!(file_has_no_hidden_component(Path::new("./test")));
        assert!(file_has_no_hidden_component(Path::new("/tmp/test")));
        assert!(file_has_no_hidden_component(Path::new("/tmp/test/test")));

        proptest!(|(components in prop::collection::vec("[^/\0.][^/\0]*", 1..20))| {
            let str_path = components.join("/");
            let path = Path::new(&str_path);
            assert!(file_has_no_hidden_component(path));
        });
    }
}
