import os
import re
from typing import Any, List
from index import IndexExtension, IndexManager
import marko

url_regex = re.compile(
    r"^(?:http|ftp)s?://",
    re.IGNORECASE,
)

# Traverse the Document tree and find all links
def find_nodes_of_type(document: marko.block.Document, type) -> List:
    links = []
    for node in document.children:
        if isinstance(node, type):
            links.append(node)
        elif hasattr(node, "children") and len(node.children) > 0:
            links.extend(find_nodes_of_type(node, type))
    return links

class WikiLink(marko.inline.InlineElement):

    pattern = r'\[\[([^|\]]*)\|?([^\]]*?)\]\]'
    parse_children = True

    def __init__(self, match):
        self.dest = match.group(1)

class IndexLinks(IndexExtension):
    def __init__(self):
        self.parser = marko.Markdown(
            extensions=[
                marko.MarkoExtension(
                    elements=[WikiLink],
                ),
            ],
        )

    def init(self, cursor):
        cursor.execute(
            """
            CREATE TABLE IF NOT EXISTS link (
                "from" TEXT,
                "to" TEXT,
                PRIMARY KEY ("from", "to"),
                FOREIGN KEY ("from") REFERENCES note(path)
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
        document = self.parser.parse(content)
        
        links = find_nodes_of_type(document, marko.inline.Link)
        links.extend(find_nodes_of_type(document, WikiLink))

        # Add all links to the links table
        for link in links:
            # Link destination URL decoded
            decodedDest: str = link.dest.replace("%20", " ")
            decodedDest = decodedDest.removeprefix("./")
            
            # If the destination is not a url and has no file extension, add .md
            if not url_regex.match(decodedDest) and not os.path.splitext(decodedDest)[1]:
                decodedDest += ".md"

            cursor.execute(
                "INSERT OR IGNORE INTO link VALUES (?, ?)",
                (vault_path, decodedDest),
            )

    def remove_file(
        self,
        cursor,
        absolute_path: str,
        vault_path: str,
    ):
        cursor.execute(
            'DELETE FROM link WHERE "from" = ?',
            (vault_path,),
        )
