use diesel::{ExpressionMethods, RunQueryDsl};
use markdown::mdast::Node;

use crate::{
    indexer::IndexExt,
    note,
    sqlite::{schema, with_db_conn},
};

pub struct MarkdownIndex {}

impl IndexExt for MarkdownIndex {
    fn init(&mut self) {}

    /// Read markdown files and parse them into a markdown AST.
    fn index(&mut self, new_note: &mut note::Note) {
        // If the note is a markdown file
        match new_note.get::<Option<String>>("extension").unwrap() {
            Some(ext) if ext == "md" => {
                let absolute_path = new_note.get::<std::path::PathBuf>("absolute_path").unwrap();
                let content = std::fs::read_to_string(absolute_path).unwrap();
                let md = markdown::to_mdast(&content, &markdown::ParseOptions::default()).unwrap();
                new_note.set::<String>("content", content);
                new_note.set::<Node>("markdown", md);
            }
            _ => {}
        }
    }

    fn remove(&mut self, _: std::path::PathBuf) {}
}

pub fn find_all_nodes<'a, F>(node: &'a Node, predicate: &'a F) -> Vec<&'a Node>
where
    F: Fn(&Node) -> bool,
{
    if predicate(node) {
        vec![node]
    } else {
        node.children().map_or(vec![], |children| {
            children
                .iter()
                .flat_map(|child| find_all_nodes(child, predicate))
                .collect()
        })
    }
}

pub struct LinkIndex {}

impl IndexExt for LinkIndex {
    fn init(&mut self) {
        with_db_conn(|conn| {
            diesel::sql_query(
                r#"
                    CREATE TABLE IF NOT EXISTS link (
                        "from" TEXT NOT NULL,
                        "to" TEXT NOT NULL,
                        "text" TEXT,
                        PRIMARY KEY("from", "to", "text")
                    )
                "#,
            )
            .execute(conn)
            .unwrap();
        });
    }

    fn index(&mut self, new_note: &mut note::Note) {
        if let Some(md) = new_note.get::<Node>("markdown") {
            let links = find_all_nodes(md, &|span| matches!(span, markdown::mdast::Node::Link(_)));

            // Insert all links into the database.
            with_db_conn(|conn| {
                use crate::sqlite::models;
                diesel::insert_into(schema::link::table)
                    .values(
                        links
                            .iter()
                            .map(|link| match link {
                                markdown::mdast::Node::Link(link) => models::Link {
                                    from: new_note.rel_path.to_str().unwrap().to_owned(),
                                    to: link.url.to_owned(),
                                    text: None, // TODO
                                },
                                _ => unreachable!(),
                            })
                            .collect::<Vec<models::Link>>(),
                    )
                    .execute(conn)
                    .unwrap();
            });
        }
    }

    fn remove(&mut self, rel_path: std::path::PathBuf) {
        with_db_conn(|conn| {
            use schema::link::dsl::*;
            diesel::delete(schema::link::table)
                .filter(from.eq(rel_path.to_str().unwrap()))
                .execute(conn)
                .unwrap();
        });
    }
}
