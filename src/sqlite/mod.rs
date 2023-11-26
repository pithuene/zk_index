use diesel::RunQueryDsl;
use std::path::PathBuf;

use self::models::Note;
use crate::indexer::IndexExt;
use crate::watcher;
use diesel::connection::Connection;
use diesel::prelude::*;

pub mod models;
pub mod schema;

pub struct SqliteIndex {
    connection: Option<SqliteConnection>,
    db_path: PathBuf,
}

impl SqliteIndex {
    pub fn new(index_dir: PathBuf) -> Self {
        Self {
            connection: None,
            db_path: index_dir.join("index.db"),
        }
    }
}

impl IndexExt for SqliteIndex {
    fn init(&mut self) {
        log::info!("SqliteIndex init");
        let mut conn: SqliteConnection =
            Connection::establish(self.db_path.to_str().unwrap()).unwrap();

        // Make sure the note table exists.
        diesel::sql_query("CREATE TABLE IF NOT EXISTS note (path TEXT NOT NULL)")
            .execute(&mut conn)
            .unwrap();

        self.connection = Some(conn);
    }

    fn index(&mut self, new_note: &watcher::Note) {
        if let Some(ref mut conn) = self.connection {
            let new_note = Note {
                path: new_note.vault_path.to_str().unwrap().to_string(),
            };
            diesel::insert_into(schema::note::table)
                .values(&new_note)
                .execute(conn)
                .unwrap();
        }
    }

    fn remove(&mut self, old_note: &watcher::Note) {
        if let Some(ref mut conn) = self.connection {
            use schema::note::dsl::*;
            diesel::delete(schema::note::table)
                .filter(path.eq(old_note.vault_path.to_str().unwrap()))
                .execute(conn)
                .unwrap();
        }
    }
}
