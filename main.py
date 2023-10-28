import asyncio
import index
import argparse
from file_change_watcher import FileChangeWatcher
from index_extensions.index_links import IndexLinks
from index_extensions.index_notes import IndexNote
from index_extensions.index_properties import IndexProperties
from index_extensions.index_tasks import IndexTasks

parser = argparse.ArgumentParser(description="Zettelkasten Index creates and maintains an index of your markdown notes.")
parser.add_argument("-d", "--directory", dest="directory_path", help="The notes directory to index")
parser.add_argument("-i", "--index", dest="index_path", help="The path to the index database")

async def main():
    args = parser.parse_args()

    if not args.directory_path:
        print("Please specify a directory to index with -d")
        exit(1)

    if not args.index_path:
        print("Please specify a path to the index database with -i")
        exit(1)

    index_manager = index.IndexManager(
        db_path=args.index_path,
        extensions=[
            IndexNote(),
            IndexLinks(),
            IndexProperties(),
            IndexTasks()
        ],
    )
    index_manager.connect()
    
    cursor = index_manager.conn.cursor()
    cursor.execute("CREATE TABLE IF NOT EXISTS _config (key TEXT PRIMARY KEY, value TEXT)")
    index_manager.conn.commit()
    cursor.close()
    
    def read_last_run() -> float:
        cursor = index_manager.conn.cursor()
        cursor.execute("SELECT value FROM _config WHERE key = 'last_run_time'")
        result = cursor.fetchone()
        cursor.close()
        return result[0] if result else 0

    def write_last_run(time: float):
        cursor = index_manager.conn.cursor()
        cursor.execute("INSERT OR REPLACE INTO _config VALUES ('last_run_time', ?)", (time,))
        index_manager.conn.commit()
        cursor.close()

    index_manager.init()
    

    directory_path = args.directory_path.rstrip("/")
    change_watcher = FileChangeWatcher(
        read_last_modified=read_last_run,
        write_last_modified=write_last_run,
        directory_path=directory_path,
        event_loop=asyncio.get_event_loop(),
        index_callback=lambda file_path: index_manager.index_file(
            file_path, directory_path
        ),
        remove_callback=lambda file_path: index_manager.remove_file(
            file_path, directory_path
        ),
    )
    change_watcher.start()
    try:
        while True:
            await asyncio.sleep(1)
    except:
        change_watcher.stop()
        index_manager.close()


if __name__ == "__main__":
    asyncio.run(main())
