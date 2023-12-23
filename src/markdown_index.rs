use crate::{
    indexer::IndexExt, link_index::LinkIndex, note, sqlite::SqliteInitConfig, wikilink_parser,
};
use markdown_it::Node;
use std::path::Path;

pub struct MarkdownIndex {
    parser: markdown_it::MarkdownIt,
    child_extensions:
        Vec<Box<dyn for<'a> IndexExt<'a, InitCfg = SqliteInitConfig, NoteIn = MarkdownNote<'a>>>>,
}

impl MarkdownIndex {
    pub fn new() -> Self {
        let mut parser = markdown_it::MarkdownIt::new();
        markdown_it::plugins::cmark::add(&mut parser);
        markdown_it::plugins::extra::add(&mut parser);
        wikilink_parser::add(&mut parser);

        Self {
            parser,
            child_extensions: vec![Box::new(LinkIndex::new()), Box::new(EmbeddingIndex::new())],
        }
    }
}

pub struct MarkdownNote<'a> {
    pub note: &'a note::Note,
    pub markdown: Node,
}

impl IndexExt<'_> for MarkdownIndex {
    type InitCfg = SqliteInitConfig;
    type NoteIn = note::Note;

    fn init(&mut self, config: &Self::InitCfg) {
        log::info!("Index extension MarkdownIndex initialized.");
        self.child_extensions
            .iter_mut()
            .for_each(|ext| ext.init(config));
    }

    /// Read markdown files and parse them into a markdown AST.
    fn index(&mut self, new_note: &note::Note) {
        // If the note is a markdown file
        match &new_note.extension {
            Some(ext) if ext == "md" => {
                let content = std::fs::read_to_string(&new_note.absolute_path).unwrap();
                let md = self.parser.parse(&content);
                let md_note = MarkdownNote {
                    note: new_note,
                    markdown: md,
                };
                self.child_extensions
                    .iter_mut()
                    .for_each(|ext| ext.index(&md_note));
            }
            _ => {}
        }
    }

    fn remove(&mut self, rel_path: &Path) {
        self.child_extensions
            .iter_mut()
            .for_each(|ext| ext.remove(rel_path));
    }
}
