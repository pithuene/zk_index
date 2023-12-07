use std::path::Path;

use diesel::{ExpressionMethods, RunQueryDsl};
use markdown_it::Node;

use crate::{
    indexer::IndexExt,
    note,
    sqlite::{models, schema, with_db_conn},
};

pub struct MarkdownIndex {
    parser: markdown_it::MarkdownIt,
    child_extensions: Vec<Box<dyn for<'a> IndexExt<MarkdownNote<'a>>>>,
}

impl MarkdownIndex {
    pub fn new() -> Self {
        let mut parser = markdown_it::MarkdownIt::new();
        markdown_it::plugins::cmark::add(&mut parser);
        markdown_it::plugins::extra::add(&mut parser);

        Self {
            parser,
            child_extensions: vec![Box::new(LinkIndex::new())],
        }
    }
}

pub struct MarkdownNote<'a> {
    pub note: &'a note::Note,
    pub markdown: Node,
}

impl IndexExt<note::Note> for MarkdownIndex {
    fn init(&mut self) {
        for ext in self.child_extensions.iter_mut() {
            ext.init();
        }
    }

    /// Read markdown files and parse them into a markdown AST.
    fn index<'b>(&mut self, new_note: &'b note::Note) {
        // If the note is a markdown file
        match &new_note.extension {
            Some(ext) if ext == "md" => {
                let content = std::fs::read_to_string(&new_note.absolute_path).unwrap();
                let md = self.parser.parse(&content);
                let md_note = MarkdownNote {
                    note: new_note,
                    markdown: md,
                };
                for ext in self.child_extensions.iter_mut() {
                    ext.index(&md_note);
                }
            }
            _ => {}
        }
    }

    fn remove(&mut self, rel_path: &Path) {
        for ext in self.child_extensions.iter_mut() {
            ext.remove(rel_path);
        }
    }
}

pub struct LinkIndex {}

impl LinkIndex {
    pub fn new() -> Self {
        Self {}
    }
}

/// Link urls may be internal links, in which case this function cleans them, or external links (like webpages), in which case this function leaves them unchanged.
/// Internal links may start with `./` which is undesired and are often times url encoded.
fn link_url_to_rel_path(link_url: &str) -> String {
    let url_decoded = &*urlencoding::decode(link_url).unwrap();
    let without_prefix = url_decoded.trim_start_matches("./");
    without_prefix.to_owned()
}

impl IndexExt<MarkdownNote<'_>> for LinkIndex {
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
            .execute(conn)?;
            Ok(())
        });
    }

    fn index<'b>(&mut self, md_note: &'b MarkdownNote<'b>) {
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
            }
        });

        // Insert all links into the database.
        let _ = with_db_conn(|conn| {
            diesel::insert_into(schema::link::table)
                .values(links)
                .execute(conn)?;
            Ok(())
        });
    }

    fn remove(&mut self, rel_path: &Path) {
        let _ = with_db_conn(|conn| {
            use schema::link::dsl::*;
            diesel::delete(schema::link::table)
                .filter(from.eq(rel_path.to_str().unwrap()))
                .execute(conn)?;
            Ok(())
        });
    }
}
