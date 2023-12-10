use std::path::Path;

use fastembed::{EmbeddingBase, EmbeddingModel, FlagEmbedding, InitOptions};

use crate::{indexer::IndexExt, markdown_index::MarkdownNote};

use super::SqliteInitConfig;

pub struct EmbeddingIndex {
    model: Option<FlagEmbedding>,
}

impl EmbeddingIndex {
    pub fn new() -> Self {
        Self { model: None }
    }
}

pub const EMBEDDING_MODEL_DIR: &str = "embedding_models";

#[cfg(test)]
pub const EMBEDDING_MODEL_NAME: &str = "fast-multilingual-e5-large";

impl<'a> IndexExt<'a> for EmbeddingIndex {
    type InitCfg = SqliteInitConfig;
    type NoteIn = MarkdownNote<'a>;

    fn init(&mut self, config: &Self::InitCfg) {
        log::info!("Initializing embedding index");
        let model: FlagEmbedding = FlagEmbedding::try_new(InitOptions {
            model_name: EmbeddingModel::MLE5Large,
            show_download_message: true,
            cache_dir: config.index_dir.join(EMBEDDING_MODEL_DIR),
            ..Default::default()
        })
        .unwrap();
        self.model = Some(model);
        log::info!("Embedding model initialized");
        log::info!("Index extension EmbeddingIndex initialized.");
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
        let embeddings = self
            .model
            .as_mut()
            .unwrap()
            .passage_embed(passages, None)
            .unwrap();
        println!("{:?}", embeddings);

        // TODO
    }

    fn remove(&mut self, _rel_path: &Path) {
        // TODO
    }
}
