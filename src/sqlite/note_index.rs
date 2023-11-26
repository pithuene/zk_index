use std::path::PathBuf;

use crate::indexer::IndexExt;
use crate::note;
use diesel::prelude::*;

use super::{models, schema, CONNECTION};

pub struct NoteIndex {}

impl IndexExt for NoteIndex {
    fn init(&mut self) {
        if let Some(conn) = CONNECTION.lock().unwrap().as_mut() {
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
            .execute(conn)
            .unwrap();
        }
    }

    fn index(&mut self, new_note: &mut note::Note) {
        if let Some(conn) = CONNECTION.lock().unwrap().as_mut() {
            let new_row = models::Note {
                file: new_note.rel_path.to_str().unwrap().to_owned(),
                vault_path: new_note
                    .get::<PathBuf>("vault_path")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
            };

            diesel::insert_into(schema::note::table)
                .values(&new_row)
                .execute(conn)
                .unwrap();
        }
    }

    fn remove(&mut self, path: PathBuf) {
        if let Some(conn) = CONNECTION.lock().unwrap().as_mut() {
            use schema::note::dsl::*;
            diesel::delete(schema::note::table)
                .filter(file.eq(path.to_str().unwrap()))
                .execute(conn)
                .unwrap();
        }
    }
}
