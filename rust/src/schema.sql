CREATE TABLE IF NOT EXISTS files (
    id            INTEGER PRIMARY KEY,
    path          BLOB    NOT NULL UNIQUE,
    byte_count    INTEGER NOT NULL,
    last_modified INTEGER NOT NULL,
    last_crawl    INTEGER NOT NULL,
    hash          BLOB
);
CREATE INDEX IF NOT EXISTS files_by_hash ON files(hash);

CREATE TABLE IF NOT EXISTS chunks (
    id         INTEGER PRIMARY KEY,
    hash       BLOB    NOT NULL,
    start_line INTEGER NOT NULL,
    text       TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS chunks_by_hash ON chunks(hash);

--one row per content that was extracted: error is NULL if it worked
CREATE TABLE IF NOT EXISTS contents (
    hash    BLOB PRIMARY KEY,
    command TEXT NOT NULL,
    error   TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(text, content='chunks', content_rowid='id');

CREATE TRIGGER IF NOT EXISTS chunks_added AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;

CREATE TRIGGER IF NOT EXISTS chunks_removed AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
