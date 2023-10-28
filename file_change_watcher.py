import os
import time
import asyncio
from typing import Callable
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler

class FileChangeWatcher(FileSystemEventHandler):
    """
    This class is used to maintain an index of the files in a directory.
    """

    debounce_time = 0.2

    def __init__(
        self,
        write_last_modified: Callable[[float], None],
        read_last_modified: Callable[[], float],
        directory_path: str,
        event_loop: asyncio.AbstractEventLoop,
        index_callback: Callable[[str], None],
        remove_callback: Callable[[str], None],
    ):
        super().__init__()
        self.write_last_modified = write_last_modified
        self.read_last_modified = read_last_modified
        self.directory_path = directory_path
        self.event_loop = event_loop
        self.index_callback = index_callback
        self.remove_callback = remove_callback
        
    def start(self):
        self.observer = Observer()
        self.observer.schedule(self, path=self.directory_path, recursive=True)
        self.observer.start()
        self.detect_changes()
        print("Started file change watcher")

    def stop(self):
        self.write_last_modified(time.time())
        self.observer.stop()
        print("Stopped file change watcher")
        
        
    def should_be_indexed(self, file_path) -> bool:
        # File is hidden or in a hidden directory
        if "/." in file_path:
            return False

        # Ignore non .md files
        if not file_path.endswith(".md"):
            return False
            
        return True

    last_index_call_time = {}

    def index(self, file_path):
        if not self.should_be_indexed(file_path):
            return

        call_time = time.time()
        self.last_index_call_time[file_path] = call_time

        def index_later():
            if self.last_index_call_time[file_path] != call_time:
                # This call was superseded by a more recent call
                return
            self.last_index_call_time.pop(file_path)
            self.index_callback(file_path)

        self.event_loop.call_later(self.debounce_time, index_later)

    last_remove_call_time = {}

    def remove(self, file_path):
        if not self.should_be_indexed(file_path):
            return

        call_time = time.time()
        self.last_remove_call_time[file_path] = call_time

        def remove_later():
            if self.last_remove_call_time[file_path] != call_time:
                # This call was superseded by a more recent call
                return
            self.last_remove_call_time.pop(file_path)
            self.remove_callback(file_path)

        self.event_loop.call_later(self.debounce_time, remove_later)

    def on_modified(self, event):
        if event.is_directory:
            return
        # Remove the old index entry
        self.remove(event.src_path)
        # Add the new index entry
        self.index(event.src_path)

    def on_created(self, event):
        self.index(event.src_path)

    def on_deleted(self, event):
        self.remove(event.src_path)

    def on_moved(self, event):
        self.remove(event.src_path)
        self.index(event.dest_path)

    def detect_changes(self):
        self.last_modified_time = self.read_last_modified()
        start_time = time.time()
        # TODO: If a file gets deleted while the program is not running, it will not be removed from the index

        # Get the last modified time of each file in the directory
        for root,dirs, files in os.walk(self.directory_path):
            for file in files:
                file_path = os.path.join(root, file)

                if not self.should_be_indexed(file_path):
                    continue

                modified_time = os.path.getmtime(file_path)
                # Compare the last modified time to the last modified time stored in the file
                if modified_time > self.last_modified_time:
                    self.remove(file_path)
                    self.index(file_path)
        self.write_last_modified(time.time())
        print("Indexing took", time.time() - start_time, "seconds")
