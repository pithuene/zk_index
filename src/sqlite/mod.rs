use diesel::RunQueryDsl;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use crate::indexer::IndexExt;
use crate::note;
use anyhow::{anyhow, Result};
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

pub fn with_db_conn<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut SqliteConnection) -> Result<T>,
{
    let mut maybe_conn = match crate::sqlite::CONNECTION.lock() {
        Ok(guard) => guard,
        Err(_) => Err(anyhow!("Failed to lock connection mutex"))?,
    };
    let conn = maybe_conn
        .as_mut()
        .ok_or(anyhow!("Connection not initialized"))?;
    f(conn)
}

pub struct SqliteIndex {}

impl SqliteIndex {
    pub fn new() -> Self {
        Self {}
    }
}

impl IndexExt<note::Note> for SqliteIndex {
    fn init(&mut self) {
        log::info!("SqliteIndex init");
        let _ = with_db_conn(|conn| {
            diesel::sql_query(
                r#"
                    CREATE TABLE IF NOT EXISTS file (
                        path TEXT NOT NULL,
                        last_indexed INTEGER NOT NULL,
                        PRIMARY KEY(path)
                    )
                "#,
            )
            .execute(conn)?;
            Ok(())
        });
    }

    fn index(&mut self, new_note: &note::Note) {
        let _ = with_db_conn(|conn| {
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
                .execute(conn)?;
            Ok(())
        });
    }

    fn remove(&mut self, rel_path: &Path) {
        let _ = with_db_conn(|conn| {
            use schema::file::dsl::*;
            diesel::delete(schema::file::table)
                .filter(path.eq(rel_path.to_str().unwrap()))
                .execute(conn)?;
            Ok(())
        });
    }
}
