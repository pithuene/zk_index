use std::sync::mpsc::Receiver;

use crate::watcher::{self, Note};

pub trait IndexExt {
    // Called to initialize the index if it doesn't exist yet.
    fn init(&mut self);
    // Called to add a note to the index.
    fn index(&mut self, note: &Note);
    // Called to remove a note from the index.
    fn remove(&mut self, note: &Note);
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
                watcher::IndexEvent::Add(note) => {
                    log::debug!("Adding note to index: {:?}", note.absolute_path);
                    self.index_extensions.iter_mut().for_each(|index| {
                        index.index(&note);
                    });
                }
                watcher::IndexEvent::Remove(note) => {
                    log::debug!("Removing note from index: {:?}", note.absolute_path);
                    self.index_extensions.iter_mut().for_each(|index| {
                        index.remove(&note);
                    });
                }
            }
        }
    }
}
