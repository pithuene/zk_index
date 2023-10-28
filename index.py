import sqlite3
from typing import List, cast
from pprint import pprint
import frontmatter


def abs_to_vault_path(path: str, directory_path: str) -> str:
    return path[len(directory_path) + 1 :]


# An indexing extension.
# Every indexing operation is implemented as an extension.
# The extension implements `init`, `index_file` and `remove_file` methods.
class IndexExtension:
    def init(self, cursor: sqlite3.Cursor):
        raise NotImplementedError

    def index_file(
        self,
        cursor: sqlite3.Cursor,
        content: str,
        absolute_path: str,
        vault_path: str,
    ):
        raise NotImplementedError

    def remove_file(
        self,
        cursor: sqlite3.Cursor,
        absolute_path: str,
        vault_path: str,
    ):
        raise NotImplementedError


class IndexManager:
    conn: sqlite3.Connection

    def __init__(
        self,
        db_path: str,
        extensions: List[IndexExtension],
    ):
        self.db_path = db_path
        self.extensions = extensions

    # Connect to database
    def connect(self):
        self.conn = sqlite3.connect(self.db_path)

    # Setup database tables and indices if they don't exist
    def init(self):
        cursor = self.conn.cursor()
        for extension in self.extensions:
            extension.init(cursor)
        self.conn.commit()
        cursor.close()

    # Add a file to the index
    def index_file(self, path: str, directory_path: str):
        with open(path, "r") as f:
            content = f.read()
        vault_path = abs_to_vault_path(path, directory_path)
        print("Adding file to index: " + vault_path)
        cursor = self.conn.cursor()
        for extension in self.extensions:
            extension.index_file(
                cursor,
                content,
                absolute_path=path,
                vault_path=vault_path,
            )
        self.conn.commit()
        cursor.close()

    # Remove a file from the index
    def remove_file(self, path: str, directory_path: str):
        vault_path = abs_to_vault_path(path, directory_path)
        print("Removing file from index: " + vault_path)
        cursor = self.conn.cursor()
        for extension in self.extensions:
            extension.remove_file(
                cursor,
                absolute_path=path,
                vault_path=vault_path,
            )
        self.conn.commit()
        cursor.close()

    def close(self):
        self.conn.close()
