use anyhow::Result;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, RecvError, RecvTimeoutError},
};

use crate::{
    note::Note,
    sqlite::SqliteIndex,
    watcher::{self},
};

pub trait IndexExt<'a> {
    type InitCfg;
    type NoteIn;
    // Called to initialize the index if it doesn't exist yet.
    fn init(&mut self, config: &Self::InitCfg);
    // Called to add a note to the index.
    fn index(&mut self, note: &Self::NoteIn);
    // Called to remove a note from the index.
    fn remove(&mut self, path: &Path);
}

#[derive(Debug, Clone)]
pub struct IndexerInitConfig {
    pub vault_root_path: PathBuf,
    pub index_dir: PathBuf,
}

pub struct Indexer {
    pub vault_root_path: PathBuf,
    pub index_event_receiver: Receiver<watcher::IndexEvent>,
    pub child_extensions:
        Vec<Box<dyn for<'a> IndexExt<'a, InitCfg = IndexerInitConfig, NoteIn = Note>>>,
}

impl IndexExt<'_> for Indexer {
    type InitCfg = IndexerInitConfig;
    type NoteIn = Note;

    fn init(&mut self, config: &Self::InitCfg) {
        log::info!("Index extension Indexer initialized.");
        self.child_extensions
            .iter_mut()
            .for_each(|ext| ext.init(config));
    }

    fn index(&mut self, new_note: &Note) {
        self.child_extensions
            .iter_mut()
            .for_each(|ext| ext.index(new_note));
    }

    fn remove(&mut self, path: &Path) {
        self.child_extensions
            .iter_mut()
            .for_each(|ext| ext.remove(path));
    }
}

impl Indexer {
    pub fn new(
        vault_root_path: PathBuf,
        index_event_receiver: Receiver<watcher::IndexEvent>,
    ) -> Self {
        Self {
            vault_root_path,
            index_event_receiver,
            child_extensions: vec![Box::from(SqliteIndex::new())],
        }
    }

    fn handle_single_event(&mut self, event: watcher::IndexEvent) {
        match event {
            watcher::IndexEvent::Add(rel_path) => {
                let note = Note::new(&self.vault_root_path, &rel_path);

                self.index(&note);

                log::info!("Indexed file: {:?}", rel_path);
            }
            watcher::IndexEvent::Remove(rel_path) => {
                self.remove(&rel_path);
                log::info!("Removed file: {:?}", rel_path);
            }
        }
    }

    /// Handle index events in an infinite loop.
    /// If you only want to handle the current events, use `process` instead.
    pub fn start(&mut self) {
        loop {
            self.handle_single_event(self.index_event_receiver.recv().unwrap());
        }
    }

    /// Used for testing.
    /// Handle all remaining events until the queue is empty.
    ///
    /// If you want to handle events continuously, use `start` instead.
    #[allow(dead_code)]
    pub fn process(&mut self) -> Result<()> {
        loop {
            match self
                .index_event_receiver
                .recv_timeout(std::time::Duration::from_millis(100))
            {
                Ok(event) => self.handle_single_event(event),
                Err(RecvTimeoutError::Timeout) => return Ok(()),
                Err(RecvTimeoutError::Disconnected) => return Err(RecvError.into()),
            }
        }
    }
}
