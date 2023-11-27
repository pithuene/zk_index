use clap::{Arg, Command};
use env_logger::Builder;
use std::{path::Path, sync::mpsc::channel, thread};
use watcher::file_has_no_hidden_component;

mod indexer;
pub mod note;
mod sqlite;
use sqlite::{db_connect, SqliteIndex};
mod watcher;

fn main() {
    // Initialize the logger.
    Builder::new().filter(None, log::LevelFilter::Debug).init();

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
    // The name of the directory where the index is stored.
    let index_dir_name = ".zk_index";
    let index_dir = Path::new(&root_dir).join(index_dir_name);
    // Make sure the index directory exists, and create it if it doesn't.
    if !index_dir.exists() {
        std::fs::create_dir(&index_dir).unwrap();
    }

    db_connect(&index_dir.join("index.db"));

    let (index_event_sender, index_event_receiver) = channel::<watcher::IndexEvent>();

    let indexer_task = thread::spawn(|| {
        let mut indexer = indexer::Indexer::new(
            vec![
                Box::new(SqliteIndex::new()),
                Box::new(sqlite::note_index::NoteIndex {}),
            ],
            index_event_receiver,
        );
        log::info!("Indexer starting.");
        indexer.start();
        log::info!("Indexer stopped.");
    });

    // Ignore hidden files or files in hidden directories.
    let file_filter = Box::new(file_has_no_hidden_component);

    let watcher_task = thread::spawn(move || {
        let mut watcher = watcher::DirWatcher::new(&root_dir, index_event_sender, file_filter);
        log::info!("Watcher starting.");
        watcher.start();
        log::info!("Watcher stopped.");
    });

    indexer_task.join().unwrap();
    watcher_task.join().unwrap();
}
