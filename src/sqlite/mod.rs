use diesel::RunQueryDsl;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use crate::indexer::{IndexExt, IndexerInitConfig};
use crate::note;
use diesel::connection::Connection;
use diesel::prelude::*;

pub mod embedding_index;
pub mod models;
pub mod note_index;
pub mod schema;

pub const SQL_INDEX_NAME: &str = "index.db";

pub struct SqliteInitConfig {
    pub vault_root_path: PathBuf,
    pub index_dir: PathBuf,
    pub db_path: PathBuf,
    pub conn: Arc<Mutex<SqliteConnection>>,
}

pub struct SqliteIndex {
    // Use an Arc<Mutex<SqliteConnection>> instead of a RefCell<SqliteConnection>
    // in case we want to handle multiple index events in parallel.
    pub conn: Option<Arc<Mutex<SqliteConnection>>>,
    pub child_extensions:
        Vec<Box<dyn for<'a> IndexExt<'a, InitCfg = SqliteInitConfig, NoteIn = note::Note>>>,
}

impl SqliteIndex {
    pub fn new() -> Self {
        Self {
            conn: None,
            child_extensions: vec![Box::new(note_index::NoteIndex::new())],
        }
    }
}

impl IndexExt<'_> for SqliteIndex {
    type InitCfg = IndexerInitConfig;
    type NoteIn = note::Note;

    fn init(&mut self, config: &Self::InitCfg) {
        log::info!("SqliteIndex init");

        let db_path = config.index_dir.join(SQL_INDEX_NAME);

        self.conn = Some(Arc::new(Mutex::new(
            Connection::establish(db_path.to_str().unwrap()).unwrap(),
        )));
        let mut conn = self.conn.as_ref().unwrap().lock().unwrap();

        diesel::sql_query(
            r#"
            CREATE TABLE IF NOT EXISTS file (
                path TEXT NOT NULL,
                last_indexed INTEGER NOT NULL,
                PRIMARY KEY(path)
            )
            "#,
        )
        .execute(&mut *conn)
        .unwrap();

        let child_config = SqliteInitConfig {
            vault_root_path: config.vault_root_path.clone(),
            index_dir: config.index_dir.clone(),
            db_path,
            conn: Arc::clone(self.conn.as_ref().unwrap()),
        };

        log::info!("Index extension SqliteIndex initialized.");

        // TODO: I have to come up with a better abstraction around the database connection.
        // This pattern occurs in every extension that uses the database and has children.
        // If I ever forget to drop the connection, I'll have a deadlock.
        drop(conn);
        self.child_extensions
            .iter_mut()
            .for_each(|ext| ext.init(&child_config));
    }

    fn index(&mut self, new_note: &note::Note) {
        let mut conn = self.conn.as_ref().unwrap().lock().unwrap();

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
            .execute(&mut *conn)
            .unwrap();

        drop(conn);
        self.child_extensions
            .iter_mut()
            .for_each(|ext| ext.index(new_note));
    }

    fn remove(&mut self, rel_path: &Path) {
        use schema::file::dsl::*;
        let mut conn = self.conn.as_ref().unwrap().lock().unwrap();
        diesel::delete(schema::file::table)
            .filter(path.eq(rel_path.to_str().unwrap()))
            .execute(&mut *conn)
            .unwrap();

        drop(conn);
        self.child_extensions
            .iter_mut()
            .for_each(|ext| ext.remove(rel_path));
    }
}
