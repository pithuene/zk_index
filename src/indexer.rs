use std::{
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
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
    index_extensions: Vec<Box<dyn IndexExt>>,
    index_event_receiver: Receiver<watcher::IndexEvent>,
}

impl Indexer {
    pub fn new(
        extensions: Vec<Box<dyn IndexExt>>,
        index_event_receiver: Receiver<watcher::IndexEvent>,
    ) -> Self {
        Self {
            index_extensions: extensions,
            index_event_receiver,
        }
    }

    pub fn start(&mut self) {
        // Initialize all extensions
        self.index_extensions.iter_mut().for_each(|index| {
            index.init();
        });
        loop {
            match self.index_event_receiver.recv().unwrap() {
                watcher::IndexEvent::Add(rel_path) => {
                    log::debug!("Adding note to index: {:?}", rel_path);
                    let mut note = self.read_note_file(&rel_path);
                    self.index_extensions.iter_mut().for_each(|index| {
                        index.index(&mut note);
                    });
                }
                watcher::IndexEvent::Remove(rel_path) => {
                    log::debug!("Removing note from index: {:?}", rel_path);
                    self.index_extensions.iter_mut().for_each(|index| {
                        index.remove(rel_path.clone());
                    });
                }
            }
        }
    }

    fn read_note_file(&self, rel_path: &Path) -> Note {
        Note::new(rel_path)
    }
}
