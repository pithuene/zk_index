from index import IndexExtension

class IndexNote(IndexExtension):
    def init(self, cursor):
        cursor.execute(
            """
            CREATE TABLE IF NOT EXISTS note (
                path TEXT PRIMARY KEY
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
        cursor.execute("INSERT INTO note VALUES (?)", (vault_path,))

    def remove_file(
        self,
        cursor,
        absolute_path: str,
        vault_path: str,
    ):
        cursor.execute(
            "DELETE FROM note WHERE path = ?",
            (vault_path,),
        )