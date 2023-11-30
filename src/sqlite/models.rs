use super::schema::{file, link, note};
use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = file)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct File {
    pub path: String,
    pub last_indexed: i32,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = note)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Note {
    pub vault_path: String,
    pub file: String,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = link)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Link {
    pub from: String,
    pub to: String,
    pub text: Option<String>,
    pub start: i32,
    pub end: i32,
}
