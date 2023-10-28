from index import IndexExtension
import frontmatter

class IndexProperties(IndexExtension):
    def init(self, cursor):
        cursor.execute(
            """
            CREATE TABLE IF NOT EXISTS property (
                note TEXT,
                key TEXT,
                value TEXT,
                PRIMARY KEY (note, key, value),
                FOREIGN KEY (note) REFERENCES note(path)
            )
            """
        )

    def index_file(
        self,
        cursor,
        content: str,
        absolute_path: str,
        vault_path: str,
    ):
        metadata, _ = frontmatter.parse(content)
        for key, value in metadata.items():
            # Iterate over all values, if the value is a list
            if isinstance(value, list):
                for item in value:
                    cursor.execute(
                        "INSERT OR IGNORE INTO property VALUES (?, ?, ?)",
                        (vault_path, str(key), item),
                    )
            else:
                cursor.execute(
                    "INSERT OR IGNORE INTO property VALUES (?, ?, ?)",
                    (vault_path, key, str(value)),
                )

    def remove_file(
        self,
        cursor,
        absolute_path: str,
        vault_path: str,
    ):
        cursor.execute(
            "DELETE FROM property WHERE note = ?",
            (vault_path,),
        )