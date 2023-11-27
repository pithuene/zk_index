use super::schema::{file, note};
use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = file)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct File {
    pub path: String,
    pub last_indexed: i32,
}

/// A note, has a vault path and a foreign key to a file.
#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = note)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Note {
    pub vault_path: String,
    pub file: String,
}
