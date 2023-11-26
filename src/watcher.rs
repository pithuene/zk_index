//! The Watcher module is responsible for listening to filesystem changes and
//! adds the necessary IndexEvents to a channel, which can then be consumed by
//! the Indexer.

use notify::{recommended_watcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc::{channel, Receiver, Sender},
    time::UNIX_EPOCH,
};

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
    last_run_time_file: std::fs::File,
    /// Function which is passed to the watcher to configure a file filter.
    file_filter: Box<dyn Fn(&Path) -> bool>,
}

impl DirWatcher {
    pub fn new(
        path: &str,
        index_event_sender: Sender<IndexEvent>,
        last_run_time_file_path: PathBuf,
        file_filter: Box<dyn Fn(&Path) -> bool>,
    ) -> Self {
        let path = Path::new(path);

        let (tx, rx) = channel();
        let config = notify::Config::default()
            .with_poll_interval(std::time::Duration::from_secs(1))
            .with_compare_contents(false);
        let mut watcher: RecommendedWatcher = recommended_watcher(tx).unwrap();
        watcher.configure(config).unwrap();

        // Open the last run time file.
        let last_run_time_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(last_run_time_file_path)
            .unwrap();

        Self {
            vault_root_path: Box::from(path),
            watcher,
            file_event_receiver: rx,
            index_event_sender,
            last_run_time_file,
            file_filter,
        }
    }

    /// Read last run time from file.
    fn read_last_run_time(&mut self) -> u64 {
        let mut last_run_file_content = String::new();
        self.last_run_time_file
            .read_to_string(&mut last_run_file_content)
            .ok()
            .map_or(0, |_| last_run_file_content.parse::<u64>().unwrap_or(0))
    }

    pub fn start(&mut self) {
        self.watcher
            .watch(&self.vault_root_path, RecursiveMode::Recursive)
            .unwrap();

        // Find all files which have been modified since the last run.

        // Unix timestamp of the last run.
        let last_run_time = self.read_last_run_time();

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

                if modified > last_run_time {
                    let rel_path = self.relative_path_from_absolute_path(entry.path());
                    self.emit_index_event(IndexEvent::Remove(rel_path.clone()));
                    self.emit_index_event(IndexEvent::Add(rel_path));
                }
            }
        }
        // TODO: You need to iterate over all files in the index too, and check, if they still exist.

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
        absolute_path
            .strip_prefix(&self.vault_root_path)
            .unwrap()
            .to_path_buf()
    }
}
