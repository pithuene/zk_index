use diesel::RunQueryDsl;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use crate::indexer::IndexExt;
use crate::note;
use diesel::connection::Connection;
use diesel::prelude::*;
use once_cell::sync::Lazy;

pub mod models;
pub mod note_index;
pub mod schema;

pub static CONNECTION: Lazy<Arc<Mutex<Option<SqliteConnection>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub fn db_connect(db_path: &Path) {
    let conn: SqliteConnection = Connection::establish(db_path.to_str().unwrap()).unwrap();
    CONNECTION.lock().unwrap().replace(conn);
}

pub fn with_db_conn<F, T>(f: F) -> T
where
    F: FnOnce(&mut SqliteConnection) -> T,
{
    let mut conn_opt = crate::sqlite::CONNECTION.lock().unwrap();
    let conn = conn_opt.as_mut().unwrap();
    f(conn)
}

pub struct SqliteIndex {}

impl SqliteIndex {
    pub fn new() -> Self {
        Self {}
    }
}

impl IndexExt for SqliteIndex {
    fn init(&mut self) {
        log::info!("SqliteIndex init");
        with_db_conn(|conn| {
            diesel::sql_query(
                r#"
                    CREATE TABLE IF NOT EXISTS file (
                        path TEXT NOT NULL,
                        last_indexed INTEGER NOT NULL,
                        PRIMARY KEY(path)
                    )
                "#,
            )
            .execute(conn)
            .unwrap();
        });
    }

    fn index(&mut self, new_note: &mut note::Note) {
        if let Some(conn) = CONNECTION.lock().unwrap().as_mut() {
            let now = std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let new_file = models::File {
                path: new_note.rel_path.to_str().unwrap().to_owned(),
                // Set last_indexed to the current time.
                last_indexed: now.try_into().unwrap(),
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
