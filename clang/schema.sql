
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    file_size INTEGER,
    last_modified INTEGER, 
    content_hash TEXT,    
    file_type TEXT,       
    indexed_at INTEGER, 
    last_changed INTEGER,
    parent_dir TEXT       
);

CREATE TABLE IF NOT EXISTS descriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    description TEXT,     
    keywords TEXT,       
    generated_at INTEGER, 
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_files_hash ON files(content_hash);
CREATE INDEX IF NOT EXISTS idx_files_path ON files(file_path);
CREATE INDEX IF NOT EXISTS idx_files_type ON files(file_type);
CREATE INDEX IF NOT EXISTS idx_files_parent ON files(parent_dir);
CREATE INDEX IF NOT EXISTS idx_files_changed ON files(last_changed);

CREATE VIRTUAL TABLE IF NOT EXISTS description_fts USING fts5(
    file_path,
    description,
    keywords,
    content='descriptions',
    content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS descriptions_ai AFTER INSERT ON descriptions BEGIN
    INSERT INTO description_fts(rowid, file_path, description, keywords)
    SELECT new.id, f.file_path, new.description, new.keywords
    FROM files f WHERE f.id = new.file_id;
END;

CREATE TRIGGER IF NOT EXISTS descriptions_ad AFTER DELETE ON descriptions BEGIN
    DELETE FROM description_fts WHERE rowid = old.id;
END;

CREATE TRIGGER IF NOT EXISTS descriptions_au AFTER UPDATE ON descriptions BEGIN
    DELETE FROM description_fts WHERE rowid = old.id;
    INSERT INTO description_fts(rowid, file_path, description, keywords)
    SELECT new.id, f.file_path, new.description, new.keywords
    FROM files f WHERE f.id = new.file_id;
END;
