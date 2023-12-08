use std::path::Path;

use fastembed::{EmbeddingBase, EmbeddingModel, FlagEmbedding, InitOptions};

use crate::{indexer::IndexExt, markdown_index::MarkdownNote};

pub struct EmbeddingIndex {
    model: FlagEmbedding,
}

impl EmbeddingIndex {
    pub fn new() -> Self {
        let model: FlagEmbedding = FlagEmbedding::try_new(InitOptions {
            model_name: EmbeddingModel::MLE5Large,
            show_download_message: true,
            ..Default::default()
        })
        .unwrap();

        Self { model }
    }
}

impl<'a> IndexExt<MarkdownNote<'a>> for EmbeddingIndex {
    fn init(&mut self) {
        // TODO
    }

    fn index(&mut self, new_note: &MarkdownNote<'a>) {
        // Get the sentences from the markdown AST.
        let passages: Vec<String> = new_note
            .markdown
            .children
            .iter()
            .map(|child| child.collect_text())
            .collect();
        println!("{:?}", passages);

        // Get the embeddings for each sentence.
        let embeddings = self.model.passage_embed(passages, None).unwrap();
        println!("{:?}", embeddings);

        // TODO
    }

    fn remove(&mut self, rel_path: &Path) {
        // TODO
    }
}
