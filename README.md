# Zettelkasten Index

![GitHub license](https://img.shields.io/badge/license-MIT-blue.svg)

This is a Python tool designed to manage and maintain an index of a markdown note graph as used by systems like [Obsidian](https://obsidian.md). It continuously monitors the notes and updates the index, which is stored as an SQLite database. 
This makes searching your notes easier and more flexible (because you can use SQL) as well as much faster (because you don't have to process all note files for each query).

## Features

- **Continuous Monitoring**: Automatically updates the index as new notes are added or existing notes are modified. Changes made while the indexer is not running will be indexed the next time it is started.
- **SQLite Database**: Utilizes a robust and versatile SQLite database for efficient storage and retrieval of index data.
- **Task Management**: Integrates with the [Obsidian tasks extension](https://publish.obsidian.md/tasks/Introduction) and allows querying of tasks by status, due date, and completion date.

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