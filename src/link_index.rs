use crate::{
    indexer::IndexExt,
    markdown_index::MarkdownNote,
    sqlite::{models, schema, SqliteInitConfig},
    wikilink_parser::Wikilink,
};
use diesel::{ExpressionMethods, RunQueryDsl, SqliteConnection};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub struct LinkIndex {
    conn: Option<Arc<Mutex<SqliteConnection>>>,
}

impl LinkIndex {
    pub fn new() -> Self {
        Self { conn: None }
    }
}

/// Link urls may be internal links, in which case this function cleans them, or external links (like webpages), in which case this function leaves them unchanged.
/// Internal links may start with `./` which is undesired and are often times url encoded.
fn link_url_to_rel_path(link_url: &str) -> String {
    let url_decoded = &*urlencoding::decode(link_url).unwrap();
    let without_prefix = url_decoded.trim_start_matches("./");
    without_prefix.to_owned()
}

impl<'a> IndexExt<'a> for LinkIndex {
    type InitCfg = SqliteInitConfig;
    type NoteIn = MarkdownNote<'a>;

    fn init(&mut self, config: &Self::InitCfg) {
        self.conn = Some(Arc::clone(&config.conn));

        let mut conn = self.conn.as_ref().unwrap().lock().unwrap();
        diesel::sql_query(
            r#"
                    CREATE TABLE IF NOT EXISTS link (
                        "from" TEXT NOT NULL,
                        "to" TEXT NOT NULL,
                        "text" TEXT,
                        "start" INTEGER,
                        "end" INTEGER,
                        PRIMARY KEY("from", "start"),
                        FOREIGN KEY("from") REFERENCES note (file)
                    )
                "#,
        )
        .execute(&mut *conn)
        .unwrap();
        log::info!("Index extension LinkIndex initialized.");
    }

    fn index(&mut self, md_note: &MarkdownNote<'a>) {
        use markdown_it::plugins::cmark::inline;

        let mut links = Vec::new();
        md_note.markdown.walk(|node, _| {
            if node.is::<inline::link::Link>() {
                let link = node.cast::<inline::link::Link>().unwrap();
                let (start, end) = node.srcmap.unwrap().get_byte_offsets();
                links.push(models::Link {
                    from: md_note.note.rel_path.to_str().unwrap().to_owned(),
                    to: link_url_to_rel_path(&link.url),
                    text: None, // TODO
                    start: start.try_into().unwrap(),
                    end: end.try_into().unwrap(),
                });
            } else if node.is::<Wikilink>() {
                let wikilink = node.cast::<Wikilink>().unwrap();
                let (start, end) = node.srcmap.unwrap().get_byte_offsets();
                log::debug!("Found wikilink: {:?}", wikilink);
                links.push(models::Link {
                    from: md_note.note.rel_path.to_str().unwrap().to_owned(),
                    to: wikilink.target.to_owned(),
                    text: None, // TODO
                    start: start.try_into().unwrap(),
                    end: end.try_into().unwrap(),
                });
            }
        });

        // Insert all links into the database.
        let mut conn = self.conn.as_ref().unwrap().lock().unwrap();
        diesel::insert_into(schema::link::table)
            .values(links)
            .execute(&mut *conn)
            .unwrap();
    }

    fn remove(&mut self, rel_path: &Path) {
        let mut conn = self.conn.as_ref().unwrap().lock().unwrap();
        use schema::link::dsl::*;
        diesel::delete(schema::link::table)
            .filter(from.eq(rel_path.to_str().unwrap()))
            .execute(&mut *conn)
            .unwrap();
    }
}
