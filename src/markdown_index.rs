use diesel::{ExpressionMethods, RunQueryDsl};
use markdown_it::Node;

use crate::{
    indexer::IndexExt,
    note,
    sqlite::{models, schema, with_db_conn},
};

pub struct MarkdownIndex {
    parser: markdown_it::MarkdownIt,
}

impl MarkdownIndex {
    pub fn new() -> Self {
        let mut parser = markdown_it::MarkdownIt::new();
        markdown_it::plugins::cmark::add(&mut parser);
        markdown_it::plugins::extra::add(&mut parser);

        Self { parser }
    }
}

impl IndexExt for MarkdownIndex {
    fn init(&mut self) {}

    /// Read markdown files and parse them into a markdown AST.
    fn index(&mut self, new_note: &mut note::Note) {
        // If the note is a markdown file
        match new_note.get::<Option<String>>("extension").unwrap() {
            Some(ext) if ext == "md" => {
                let absolute_path = new_note.get::<std::path::PathBuf>("absolute_path").unwrap();
                let content = std::fs::read_to_string(absolute_path).unwrap();
                let md = self.parser.parse(&content);
                new_note.set::<String>("content", content);
                new_note.set::<Node>("markdown", md);
            }
            _ => {}
        }
    }

    fn remove(&mut self, _: std::path::PathBuf) {}
}

pub struct LinkIndex {}

/// Link urls may be internal links, in which case this function cleans them, or external links (like webpages), in which case this function leaves them unchanged.
/// Internal links may start with `./` which is undesired and are often times url encoded.
fn link_url_to_rel_path(link_url: &str) -> String {
    let url_decoded = &*urlencoding::decode(link_url).unwrap();
    let without_prefix = url_decoded.trim_start_matches("./");
    without_prefix.to_owned()
}

impl IndexExt for LinkIndex {
    fn init(&mut self) {
        let _ = with_db_conn(|conn| {
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
            .execute(conn)
            .unwrap();
            Ok(())
        });
    }

    fn index(&mut self, new_note: &mut note::Note) {
        use markdown_it::plugins::cmark::inline;

        if let Some(md) = new_note.get::<Node>("markdown") {
            let mut links = Vec::new();
            md.walk(|node, _| {
                if node.is::<inline::link::Link>() {
                    let link = node.cast::<inline::link::Link>().unwrap();
                    let (start, end) = node.srcmap.unwrap().get_byte_offsets();
                    links.push(models::Link {
                        from: new_note.rel_path.to_str().unwrap().to_owned(),
                        to: link_url_to_rel_path(&link.url),
                        text: None, // TODO
                        start: start.try_into().unwrap(),
                        end: end.try_into().unwrap(),
                    });
                }
            });

            // Insert all links into the database.
            let _ = with_db_conn(|conn| {
                diesel::insert_into(schema::link::table)
                    .values(links)
                    .execute(conn)
                    .unwrap();
                Ok(())
            });
        }
    }

    fn remove(&mut self, rel_path: std::path::PathBuf) {
        let _ = with_db_conn(|conn| {
            use schema::link::dsl::*;
            diesel::delete(schema::link::table)
                .filter(from.eq(rel_path.to_str().unwrap()))
                .execute(conn)
                .unwrap();
            Ok(())
        });
    }
}
