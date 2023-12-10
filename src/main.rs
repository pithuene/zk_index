use clap::{Arg, Command};
use env_logger::Builder;
use indexer::IndexerInitConfig;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{channel, Receiver, Sender},
    thread,
};
use watcher::{file_has_no_hidden_component, IndexEvent};

mod indexer;
pub mod note;
mod sqlite;

use crate::indexer::IndexExt;
mod markdown_index;
mod watcher;

/// Create the indexer, but don't start it.
fn indexer_create(
    vault_root_path: PathBuf,
    index_event_receiver: Receiver<IndexEvent>,
) -> indexer::Indexer {
    indexer::Indexer::new(vault_root_path, index_event_receiver)
}

/// Create and start the indexer in a new thread.
fn indexer_start(
    config: &IndexerInitConfig,
    index_event_receiver: Receiver<IndexEvent>,
) -> thread::JoinHandle<()> {
    {
        let config = config.clone();
        thread::spawn(move || {
            let mut indexer = indexer_create(config.vault_root_path.clone(), index_event_receiver);
            log::info!("Initializing index extensions.");

            indexer.init(&config);

            log::info!("Index extensions initialized.");
            log::info!("Indexer starting.");
            indexer.start();
            log::info!("Indexer stopped.");
        })
    }
}

fn watcher_start<F>(
    config: &IndexerInitConfig,
    index_event_sender: Sender<IndexEvent>,
    file_filter: F,
    timeout: Option<std::time::Duration>,
) -> thread::JoinHandle<()>
where
    F: Fn(&Path) -> bool + Send + 'static,
{
    {
        let config = config.clone();
        thread::spawn(move || {
            let mut watcher =
                watcher::DirWatcher::new(&config, index_event_sender, Box::from(file_filter));
            log::info!("Watcher starting.");
            watcher.start(timeout);
            log::info!("Watcher stopped.");
        })
    }
}

// The name of the directory where the index is stored.
const INDEX_DIR_NAME: &str = ".zk_index";

fn main() {
    // Initialize the logger.
    Builder::new().filter(None, log::LevelFilter::Info).init();

    // Parse the command line arguments.
    let matches = Command::new("zk_index")
        .version("0.0.0")
        .author("Pit Hüne")
        .arg(
            Arg::new("dir")
                .short('d')
                .long("directory")
                .help("The directory which is indexed.")
                .default_value("/home/pit/Downloads"),
        )
        .get_matches();

    // The directory which is indexed.
    let root_dir = matches.get_one::<String>("dir").unwrap().to_owned();
    let index_dir = Path::new(&root_dir).join(INDEX_DIR_NAME);
    // Make sure the index directory exists, and create it if it doesn't.
    if !index_dir.exists() {
        std::fs::create_dir(&index_dir).unwrap();
    }

    // TODO: Technically a set would be better here to avoid reindexing
    // the same file multiple times when there are frequent changes.
    let (index_event_sender, index_event_receiver) = channel::<watcher::IndexEvent>();

    let config = IndexerInitConfig {
        vault_root_path: PathBuf::from(&root_dir),
        index_dir,
    };
    let indexer_task = indexer_start(&config, index_event_receiver);

    let watcher_task = watcher_start(
        &config,
        index_event_sender,
        file_has_no_hidden_component,
        None,
    );
    indexer_task.join().unwrap();
    watcher_task.join().unwrap();
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::Write,
        path::{Path, PathBuf},
        sync::mpsc::channel,
    };

    use anyhow::{anyhow, Result};
    use diesel::{Connection, SqliteConnection};
    use fastembed::{EmbeddingModel, FlagEmbedding, InitOptions};

    use crate::{
        indexer::{IndexExt, IndexerInitConfig},
        indexer_create,
        sqlite::{
            embedding_index::{EMBEDDING_MODEL_DIR, EMBEDDING_MODEL_NAME},
            models::Link,
            SQL_INDEX_NAME,
        },
        watcher::{self},
        watcher_start, INDEX_DIR_NAME,
    };

    fn model_directory() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("embeddings")
    }

    /// The embedding model is large and takes a long time to download.
    /// This function downloads it to the source directory if it doesn't
    /// exist yet.
    /// During testing, the file is then linked to the temp directory.
    fn download_embedding_model() {
        let _: FlagEmbedding = FlagEmbedding::try_new(InitOptions {
            model_name: EmbeddingModel::MLE5Large,
            show_download_message: true,
            cache_dir: model_directory(),
            ..Default::default()
        })
        .unwrap();
    }

    #[test]
    fn test_indexing() -> Result<()> {
        // Download the embedding model if it doesn't exist yet.
        download_embedding_model();

        let temp_dir = tempfile::tempdir()?;
        log::info!("Temp dir: {:?}", temp_dir.path());
        let index_dir = Path::new(temp_dir.path()).join(INDEX_DIR_NAME);
        std::fs::create_dir(&index_dir)?;
        log::info!("Index dir: {:?}", index_dir);

        // Link the embedding model to the temp directory.
        let model_target_dir = index_dir.join(EMBEDDING_MODEL_DIR);
        std::fs::create_dir(&model_target_dir)?;
        let model_source_path = model_directory().join(EMBEDDING_MODEL_NAME);
        let model_target_path = model_target_dir.join(EMBEDDING_MODEL_NAME);
        std::os::unix::fs::symlink(model_source_path, model_target_path)?;

        // Create initial files
        let mut file1 = File::create(temp_dir.path().join("file1.md"))?;
        let mut file2 = File::create(temp_dir.path().join("file2.md"))?;
        write!(file1, "Hello File1, a [link](file2).")?;
        write!(file2, "Hello File2, another [link](file1) back.")?;
        log::info!("Created files.");

        let (index_event_sender, index_event_receiver) = channel::<watcher::IndexEvent>();

        let config = IndexerInitConfig {
            index_dir: index_dir.to_owned(),
            vault_root_path: temp_dir.path().to_owned(),
        };

        let watcher_task = {
            let index_dir = index_dir.clone();
            watcher_start(
                &config,
                index_event_sender,
                move |path| {
                    // Path is not in the index directory.
                    !path.starts_with(&index_dir)
                },
                Some(std::time::Duration::from_millis(5000)),
            )
        };
        log::info!("Started watcher.");
        let mut indexer = indexer_create(temp_dir.path().to_owned(), index_event_receiver);
        log::info!("Created indexer.");
        log::info!("Initializing index extensions.");

        indexer.init(&config);

        // Open a connection to the database.
        let mut conn: SqliteConnection =
            Connection::establish(index_dir.join(SQL_INDEX_NAME).to_str().unwrap()).unwrap();

        log::info!("Index extensions initialized.");

        indexer.process()?;
        log::info!("Processed initial events.");

        // Assert that the index includes the expected entries.
        {
            use crate::sqlite::schema::link::dsl::*;
            use diesel::RunQueryDsl;

            let links = link.load::<Link>(&mut conn).unwrap();

            assert_eq!(links.len(), 2);
            assert!(links
                .iter()
                .any(|l| l.from == "file1.md" && l.to == "file2"));
            assert!(links
                .iter()
                .any(|l| l.from == "file2.md" && l.to == "file1"));
            log::info!("Checked index correctness.");
        }

        // Change the files
        write!(file1, "Another link to myself [file1](file1).")?;
        indexer.process()?;
        log::info!("Changed file1.");

        {
            use crate::sqlite::schema::link::dsl::*;
            use diesel::RunQueryDsl;

            let links = link.load::<Link>(&mut conn).unwrap();

            assert_eq!(links.len(), 3);
            assert!(links
                .iter()
                .any(|l| l.from == "file1.md" && l.to == "file2"));
            assert!(links
                .iter()
                .any(|l| l.from == "file1.md" && l.to == "file1"));
            assert!(links
                .iter()
                .any(|l| l.from == "file2.md" && l.to == "file1"));
            log::info!("Checked that index was updated correctly.");
        }

        drop(indexer.index_event_receiver);
        watcher_task
            .join()
            .map_err(|_| anyhow!("Watcher thread panicked."))?;
        log::info!("Watcher stopped.");

        // Remove temp dir
        temp_dir.close()?;
        log::info!("Temp dir removed.");

        Ok(())
    }
}
