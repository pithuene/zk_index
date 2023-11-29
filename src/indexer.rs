use std::{path::PathBuf, sync::mpsc::Receiver};

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
    index_event_receiver: Receiver<watcher::IndexEvent>,
}

impl Indexer {
    pub fn new(
        vault_root_path: PathBuf,
        extensions: Vec<Box<dyn IndexExt>>,
        index_event_receiver: Receiver<watcher::IndexEvent>,
    ) -> Self {
        Self {
            vault_root_path,
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
                    let mut note = Note::new(&rel_path);

                    note.set::<PathBuf>("absolute_path", self.vault_root_path.join(&rel_path));

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
}
