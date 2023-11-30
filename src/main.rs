use clap::{Arg, Command};
use env_logger::Builder;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{channel, Receiver, Sender},
    thread,
};
use watcher::{file_has_no_hidden_component, IndexEvent};

mod indexer;
pub mod note;
mod sqlite;
use sqlite::{db_connect, SqliteIndex};
mod markdown_index;
mod watcher;

/// Create the indexer, but don't start it.
fn indexer_create(
    vault_root_path: PathBuf,
    index_event_receiver: Receiver<IndexEvent>,
) -> indexer::Indexer {
    indexer::Indexer::new(
        vault_root_path,
        vec![
            Box::new(SqliteIndex::new()),
            Box::new(sqlite::note_index::NoteIndex {}),
            Box::new(markdown_index::MarkdownIndex::new()),
            Box::new(markdown_index::LinkIndex {}),
        ],
        index_event_receiver,
    )
}

/// Create and start the indexer in a new thread.
fn indexer_start(
    vault_root_path: PathBuf,
    index_event_receiver: Receiver<IndexEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut indexer = indexer_create(vault_root_path, index_event_receiver);
        log::info!("Indexer starting.");
        indexer.start();
        log::info!("Indexer stopped.");
    })
}

fn watcher_start<F>(
    vault_root_path: PathBuf,
    index_event_sender: Sender<IndexEvent>,
    file_filter: F,
    timeout: Option<std::time::Duration>,
) -> thread::JoinHandle<()>
where
    F: Fn(&Path) -> bool + Send + 'static,
{
    thread::spawn(move || {
        let mut watcher =
            watcher::DirWatcher::new(&vault_root_path, index_event_sender, Box::from(file_filter));
        log::info!("Watcher starting.");
        watcher.start(timeout);
        log::info!("Watcher stopped.");
    })
}

// The name of the directory where the index is stored.
const INDEX_DIR_NAME: &str = ".zk_index";
const SQL_INDEX_NAME: &str = "index.db";

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

    db_connect(&index_dir.join(SQL_INDEX_NAME));

    let (index_event_sender, index_event_receiver) = channel::<watcher::IndexEvent>();

    let indexer_task = indexer_start(PathBuf::from(&root_dir), index_event_receiver);

    let watcher_task = watcher_start(
        PathBuf::from(&root_dir),
        index_event_sender,
        file_has_no_hidden_component,
        None,
    );
    indexer_task.join().unwrap();
    watcher_task.join().unwrap();
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write, path::Path, sync::mpsc::channel};

    use anyhow::{anyhow, Result};

    use crate::{
        indexer_create,
        sqlite::{db_connect, with_db_conn},
        watcher::{self},
        watcher_start, INDEX_DIR_NAME, SQL_INDEX_NAME,
    };

    #[test]
    fn test_indexing() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        log::info!("Temp dir: {:?}", temp_dir.path());
        let index_dir = Path::new(temp_dir.path()).join(INDEX_DIR_NAME);
        std::fs::create_dir(&index_dir)?;
        log::info!("Index dir: {:?}", index_dir);

        // Create initial files
        let mut file1 = File::create(temp_dir.path().join("file1.md"))?;
        let mut file2 = File::create(temp_dir.path().join("file2.md"))?;
        write!(file1, "Hello File1, a [link](file2).")?;
        write!(file2, "Hello File2, another [link](file1) back.")?;
        log::info!("Created files.");

        db_connect(&index_dir.join(SQL_INDEX_NAME));
        log::info!("Connected to db.");

        let (index_event_sender, index_event_receiver) = channel::<watcher::IndexEvent>();

        let watcher_task = watcher_start(
            temp_dir.path().to_owned(),
            index_event_sender,
            move |path| {
                // Path is not in the index directory.
                !path.starts_with(&index_dir)
            },
            Some(std::time::Duration::from_millis(1000)),
        );
        log::info!("Started watcher.");
        let mut indexer = indexer_create(temp_dir.path().to_owned(), index_event_receiver);
        log::info!("Created indexer.");

        indexer.process()?;
        indexer.process()?;
        log::info!("Processed initial events.");

        // Assert that the index includes the expected entries.
        with_db_conn(|conn| {
            use crate::sqlite::schema::link::dsl::*;
            use diesel::RunQueryDsl;

            let links = link.load::<crate::sqlite::models::Link>(conn).unwrap();

            assert_eq!(links.len(), 2);
            assert!(links
                .iter()
                .any(|l| l.from == "file1.md" && l.to == "file2"));
            assert!(links
                .iter()
                .any(|l| l.from == "file2.md" && l.to == "file1"));
            Ok(())
        })?;
        log::info!("Checked index correctness.");

        // Change the files
        write!(file1, "Another link to myself [file1](file1).")?;
        indexer.process()?;
        log::info!("Changed file1.");

        with_db_conn(|conn| {
            use crate::sqlite::schema::link::dsl::*;
            use diesel::RunQueryDsl;

            let links = link.load::<crate::sqlite::models::Link>(conn).unwrap();

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
            Ok(())
        })?;
        log::info!("Checked that index was updated correctly.");

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
