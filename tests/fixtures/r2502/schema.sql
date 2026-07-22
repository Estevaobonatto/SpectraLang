PRAGMA foreign_keys = ON;
CREATE TABLE items (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    score REAL NOT NULL,
    active INTEGER NOT NULL,
    payload BLOB,
    note TEXT
);
INSERT INTO items(id, name, score, active, payload, note)
VALUES (1, 'seed', 1.5, 1, X'0102', NULL);
