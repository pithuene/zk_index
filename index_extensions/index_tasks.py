import re
from typing import List, Tuple
from index import IndexExtension


# Find the optional due date.
# The due date is noted in this format: "📅 YYYY-MM-DD" where the emoji acts as a marker and the date is the due date.
# The due date may appear anywhere in the line, so we need to find it.
#
# Example:
# "This is a task 📅 2021-10-10 that must be done" => ("This is a task that must be done", "2021-10-10")
def find_date_with_marker(marker: str, content: str) -> Tuple[str, str]:
    # Find the index of the due date marker
    due_date_marker_index = content.find(marker + " ")
    # If no marker is found, return None
    if due_date_marker_index == -1:
        return (content, None)
    # Get the content before the marker
    content_before_marker = content[:due_date_marker_index].rstrip()
    # Get the content after the marker
    content_after_marker = content[due_date_marker_index + 2 :]

    # Check whether the marker is followed by a string of the format "YYYY-MM-DD"
    potential_date = content_after_marker[:10]
    # Use a regex to the potential date has the correct format
    if not re.match(r"\d\d\d\d-\d\d-\d\d", potential_date):
        # If the potential date is not of the correct format, return None
        return (content, None)
    else:
        # Get the due date
        due_date = potential_date
        content_after_due_date = content_after_marker[10:]
        if len(content_after_due_date) == 0:
            return (content_before_marker.rstrip(), due_date)
        if len(content_before_marker) == 0:
            return (content_after_due_date.lstrip(), due_date)
        return (content_before_marker + content_after_due_date, due_date)


class IndexTasks(IndexExtension):
    def init(self, cursor):
        cursor.execute(
            """
            CREATE TABLE IF NOT EXISTS task (
                note TEXT,
                status TEXT,
                content TEXT,
                due_date TEXT,
                done_date TEXT,
                PRIMARY KEY (note, content, due_date, done_date),
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
        # A task is a line that starts with "- [ ]",  "- [-]" or "- [x]" (possibly with whitespace in front)
        # Find all tasks in the file
        tasks: List = []
        # Create a new list of tasks
        tasks = []
        # Split the content into lines
        for line in content.splitlines():
            # Remove extra whitespace
            line = line.strip()
            # If the line starts with a task marker, add it to the list of tasks
            if (
                line.startswith("- [ ]")
                or line.startswith("- [-]")
                or line.startswith("- [x]")
            ):
                # Get the status of the task
                status = line[3:4]

                # Get the content of the task
                content = line[5:].strip()

                # Find the optional due date
                content, due_date = find_date_with_marker("📅", content)

                # Find the optional done date
                content, done_date = find_date_with_marker("✅", content)

                # Add the task to the list of tasks
                tasks.append(
                    {
                        "status": status,
                        "content": content,
                        "due_date": due_date,
                        "done_date": done_date,
                    }
                )
        # Add all tasks to the database
        for task in tasks:
            cursor.execute(
                "INSERT OR IGNORE INTO task VALUES (?, ?, ?, ?, ?)",
                (
                    vault_path,
                    task["status"],
                    task["content"],
                    task["due_date"],
                    task["done_date"],
                ),
            )

    def remove_file(
        self,
        cursor,
        absolute_path: str,
        vault_path: str,
    ):
        cursor.execute(
            "DELETE FROM task WHERE note = ?",
            (vault_path,),
        )
