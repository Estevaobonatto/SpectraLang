PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS reference_items(
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    score REAL NOT NULL,
    payload BLOB,
    note TEXT
);
