use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Note {
    pub rel_path: PathBuf,
    data: HashMap<String, (TypeId, Box<dyn Any>)>,
}

impl Note {
    pub fn new(rel_path: &Path) -> Self {
        Self {
            rel_path: rel_path.to_path_buf(),
            data: HashMap::new(),
        }
    }

    pub fn set<T: 'static>(&mut self, key: &str, value: T) {
        assert!(
            !self.data.contains_key(key),
            "Key {} already exists in note.",
            key
        );
        self.data
            .insert(key.to_owned(), (TypeId::of::<T>(), Box::new(value)));
    }

    pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
        if let Some((type_id, value)) = self.data.get(key) {
            if *type_id == TypeId::of::<T>() {
                return value.downcast_ref::<T>();
            }
            unreachable!("Type mismatch expected {:?}", std::any::type_name::<T>());
        }
        None
    }
}
