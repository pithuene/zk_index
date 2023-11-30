use anyhow::Result;
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, RecvError, RecvTimeoutError},
};

use crate::{
    note::Note,
    watcher::{self},
};

pub trait IndexExt {
    // Called to initialize the index if it doesn't exist yet.
    fn init(&mut self);
    // Called to add a note to the index.
    fn index(&mut self, note: &mut Note);
    // Called to remove a note from the index.
    fn remove(&mut self, path: PathBuf);
}

pub struct Indexer {
    vault_root_path: PathBuf,
    index_extensions: Vec<Box<dyn IndexExt>>,
    pub index_event_receiver: Receiver<watcher::IndexEvent>,
}

impl Indexer {
    pub fn new(
        vault_root_path: PathBuf,
        mut extensions: Vec<Box<dyn IndexExt>>,
        index_event_receiver: Receiver<watcher::IndexEvent>,
    ) -> Self {
        // Initialize all extensions
        extensions.iter_mut().for_each(|index| {
            index.init();
        });
        Self {
            vault_root_path,
            index_extensions: extensions,
            index_event_receiver,
        }
    }

    fn handle_single_event(&mut self, event: watcher::IndexEvent) {
        match event {
            watcher::IndexEvent::Add(rel_path) => {
                let mut note = Note::new(&rel_path);

                note.set::<PathBuf>("absolute_path", self.vault_root_path.join(&rel_path));

                self.index_extensions.iter_mut().for_each(|index| {
                    index.index(&mut note);
                });
                log::info!("Indexed file: {:?}", rel_path);
            }
            watcher::IndexEvent::Remove(rel_path) => {
                self.index_extensions.iter_mut().for_each(|index| {
                    index.remove(rel_path.clone());
                });
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
