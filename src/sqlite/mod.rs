use diesel::RunQueryDsl;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::indexer::IndexExt;
use crate::note;
use diesel::connection::Connection;
use diesel::prelude::*;
use once_cell::sync::Lazy;

pub mod models;
pub mod note_index;
pub mod schema;

static CONNECTION: Lazy<Arc<Mutex<Option<SqliteConnection>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub struct SqliteIndex {
    db_path: PathBuf,
}

impl SqliteIndex {
    pub fn new(index_dir: PathBuf) -> Self {
        Self {
            db_path: index_dir.join("index.db"),
        }
    }
}

impl IndexExt for SqliteIndex {
    fn init(&mut self) {
        log::info!("SqliteIndex init");
        let mut conn: SqliteConnection =
            Connection::establish(self.db_path.to_str().unwrap()).unwrap();

        diesel::sql_query(
            r#"
            CREATE TABLE IF NOT EXISTS file (
                path TEXT NOT NULL,
                PRIMARY KEY(path)
            )
        "#,
        )
        .execute(&mut conn)
        .unwrap();

        CONNECTION.lock().unwrap().replace(conn);
    }

    fn index(&mut self, new_note: &mut note::Note) {
        if let Some(conn) = CONNECTION.lock().unwrap().as_mut() {
            let new_file = models::File {
                path: new_note.rel_path.to_str().unwrap().to_owned(),
            };

            diesel::insert_into(schema::file::table)
                .values(&new_file)
                .execute(conn)
                .unwrap();

            // TODO: Remove this. It's just for testing the data passing between extension.
            new_note.set::<PathBuf>(
                "vault_path",
                vault_path_from_relative_path(&new_note.rel_path),
            );
        }
    }

    fn remove(&mut self, rel_path: PathBuf) {
        if let Some(conn) = CONNECTION.lock().unwrap().as_mut() {
            use schema::file::dsl::*;
            diesel::delete(schema::file::table)
                .filter(path.eq(rel_path.to_str().unwrap()))
                .execute(conn)
                .unwrap();
        }
    }
}

// TODO: This should not stay here
fn vault_path_from_relative_path(rel_path: &Path) -> PathBuf {
    match rel_path.extension() {
        // with_extension("") removes the extension
        Some(ext) if ext == "md" => rel_path.with_extension(""),
        _ => rel_path.to_path_buf(),
    }
}
