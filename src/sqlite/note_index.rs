use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::note;
use crate::{indexer::IndexExt, markdown_index::MarkdownIndex};
use diesel::prelude::*;

use super::{models, schema, SqliteInitConfig};

pub struct NoteIndex {
    conn: Option<Arc<Mutex<SqliteConnection>>>,
    child_extensions:
        Vec<Box<dyn for<'a> IndexExt<'a, InitCfg = SqliteInitConfig, NoteIn = note::Note>>>,
}

impl NoteIndex {
    pub fn new() -> Self {
        Self {
            conn: None,
            child_extensions: vec![Box::new(MarkdownIndex::new())],
        }
    }
}

impl IndexExt<'_> for NoteIndex {
    type InitCfg = SqliteInitConfig;
    type NoteIn = note::Note;
    fn init(&mut self, config: &Self::InitCfg) {
        self.conn = Some(Arc::clone(&config.conn));
        let mut conn = self.conn.as_ref().unwrap().lock().unwrap();
        diesel::sql_query(
            r#"
                    CREATE TABLE IF NOT EXISTS note (
                        vault_path TEXT NOT NULL,
                        file TEXT NOT NULL,
                        PRIMARY KEY(file),
                        FOREIGN KEY(file) REFERENCES file(path)
                    )
                "#,
        )
        .execute(&mut *conn)
        .unwrap();

        drop(conn);
        log::info!("Index extension NoteIndex initialized.");
        self.child_extensions
            .iter_mut()
            .for_each(|ext| ext.init(config));
    }

    fn index(&mut self, new_note: &note::Note) {
        let mut conn = self.conn.as_ref().unwrap().lock().unwrap();
        let new_row = models::Note {
            file: new_note.rel_path.to_str().unwrap().to_owned(),
            vault_path: new_note.vault_path.to_str().unwrap().to_owned(),
        };

        diesel::insert_into(schema::note::table)
            .values(&new_row)
            .execute(&mut *conn)
            .unwrap();

        drop(conn);
        self.child_extensions.iter_mut().for_each(|ext| {
            ext.index(new_note);
        });
    }

    fn remove(&mut self, path: &Path) {
        let mut conn = self.conn.as_ref().unwrap().lock().unwrap();
        use schema::note::dsl::*;
        diesel::delete(schema::note::table)
            .filter(file.eq(path.to_str().unwrap()))
            .execute(&mut *conn)
            .unwrap();

        drop(conn);
        self.child_extensions.iter_mut().for_each(|ext| {
            ext.remove(path);
        });
    }
}
