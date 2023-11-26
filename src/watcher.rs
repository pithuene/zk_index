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
pub struct Note {
    pub content: String,
    pub absolute_path: Box<Path>,
    pub vault_path: Box<Path>,
}

#[derive(Clone, Debug)]
pub enum IndexEvent {
    Remove(Note),
    Add(Note),
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
                    let note = self.read_note_file(entry.path());
                    self.emit_index_events(vec![
                        IndexEvent::Remove(note.clone()),
                        IndexEvent::Add(note),
                    ]);
                }
            }
        }

        loop {
            match self.file_event_receiver.recv().unwrap() {
                Ok(event) => self.handle_event(event),
                Err(e) => log::error!("INotifyWatcher error: {:?}", e),
            }
        }
    }

    fn emit_index_events(&self, events: Vec<IndexEvent>) {
        events.iter().for_each(|event| {
            log::debug!("Adding {:?} event to index event channel.", event);
            self.index_event_sender.send(event.clone()).unwrap();
        });
    }

    fn handle_event(&self, event: notify::event::Event) {
        match event.kind {
            notify::event::EventKind::Modify(_) => {
                event.paths.iter().for_each(|path| {
                    if (self.file_filter)(path) {
                        let note = self.read_note_file(path);
                        self.emit_index_events(vec![
                            IndexEvent::Remove(note.clone()),
                            IndexEvent::Add(note),
                        ])
                    }
                });
            }
            notify::event::EventKind::Create(_) => {
                event.paths.iter().for_each(|path| {
                    if (self.file_filter)(path) {
                        let note = self.read_note_file(path);
                        self.emit_index_events(vec![IndexEvent::Add(note)])
                    }
                });
            }
            notify::event::EventKind::Remove(_) => {
                event.paths.iter().for_each(|path| {
                    if (self.file_filter)(path) {
                        let note = self.read_note_file(path);
                        self.emit_index_events(vec![IndexEvent::Remove(note)])
                    }
                });
            }
            _ => {}
        }
    }

    fn vault_path_from_absolute_path(&self, path: &Path) -> PathBuf {
        let relative_path = path.strip_prefix(&self.vault_root_path).unwrap();
        let mut vault_path = PathBuf::from("");
        if relative_path.extension().unwrap() == "md" {
            vault_path.push(relative_path.file_stem().unwrap());
        } else {
            vault_path.push(relative_path);
        }
        vault_path
    }

    fn read_note_file(&self, path: &Path) -> Note {
        Note {
            content: String::from(""),
            absolute_path: Box::from(path),
            vault_path: Box::from(self.vault_path_from_absolute_path(path)),
        }
    }
}
