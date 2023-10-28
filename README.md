# Zettelkasten Index

![GitHub license](https://img.shields.io/badge/license-MIT-blue.svg)

This is a Python tool designed to manage and maintain an index of a markdown note graph as used by systems like [Obsidian](https://obsidian.md). It continuously monitors the notes and updates the index, which is stored as an SQLite database. 
This makes searching your notes easier and more flexible (because you can use SQL) as well as much faster (because you don't have to process all note files for each query).

## Features

- **Continuous Monitoring**: Automatically updates the index as new notes are added or existing notes are modified. Changes made while the indexer is not running will be indexed the next time it is started.
- **SQLite Database**: Utilizes a robust and versatile SQLite database for efficient storage and retrieval of index data.
- **Modular Design**: The indexer is designed to be easily extended with new features. An extension only needs to implement a setup function to create the necessary tables, a function to add a note to the index and a function to remove a note from the index.
- **Task Management**: Integrates with the [Obsidian tasks extension](https://publish.obsidian.md/tasks/Introduction) and allows querying of tasks by status, due date, and completion date.

## Examples

Here are some examples of queries that can be run on the index database.

### Backlinks

Find all backlinks to the note `Example.md`.
Without an index, this would require searching through all notes for links to `Example.md`.
With an index, this only takes a couple of milliseconds.

```sql
SELECT * FROM link WHERE link."to" = 'Example.md';
```

### Unfinished Tasks

Find all unfinished tasks that are due before the end of the year.

```sql
SELECT * FROM task
WHERE task.status = ' '
  AND task.due_date <= '2023-12-31';
```

### Books by Property

Find all notes that have the property `type: book`.

```sql
SELECT note FROM property
WHERE property.key = 'type' AND property.value = 'book';
```

## Schema

The index database is structured as follows:

```mermaid
erDiagram
    note {
        path TEXT
    }
    link {
        from TEXT
        to TEXT
    }
    property {
        note TEXT
        key TEXT
        value TEXT
    }
    task {
        note TEXT
        status TEXT
        content TEXT
        due_date TEXT
        done_date TEXT
    }

    link 1 -- 1 note : "from"
    link 1 -- 1 note : "to"
    property zero or more -- 1 note : note
    task zero or more -- 1 note : note
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.