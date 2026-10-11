-- One row per regular file the crawler has found.
CREATE TABLE IF NOT EXISTS files (
    id            INTEGER PRIMARY KEY,     
    path          BLOB    NOT NULL UNIQUE, 
    byte_count    INTEGER NOT NULL,        
    last_modified INTEGER NOT NULL,       
    last_crawl    INTEGER NOT NULL        
);
