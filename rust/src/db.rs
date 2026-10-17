use rusqlite::{Connection, params};
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::{self, Metadata};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::text::Chunk;

const SCHEMA_VERSION: i64 = 3;

#[derive(PartialEq)]
pub enum Change {
    New,
    Changed,
    Unchanged,
}

pub struct Hit {
    pub path: PathBuf,
    pub line: i64,
    pub snippet: String,
}

pub struct Extraction {
    pub command: String,
    pub error: Option<String>,
}

pub fn open() -> Result<Connection, Box<dyn Error>> {
    let home = env::var_os("HOME").expect("HOME is not set");
    let dir = Path::new(&home).join(".tracker");
    fs::create_dir_all(&dir)?;
    let path = dir.join("tracker.db");

    let conn = Connection::open(&path)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let tables: i64 = conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))?;
    if tables > 0 && version != SCHEMA_VERSION {
        let message = format!(
            "{} is from an older version; delete it and run tracker update again",
            path.display()
        );
        return Err(message.into());
    }

    conn.execute_batch(include_str!("schema.sql"))?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(conn)
}

pub fn next_crawl(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COALESCE(MAX(last_crawl), 0) + 1 FROM files",
        [],
        |row| row.get(0),
    )
}

pub fn save_file(
    conn: &Connection,
    path: &Path,
    meta: &Metadata,
    crawl: i64,
) -> rusqlite::Result<Change> {
    let path_bytes = path.as_os_str().as_bytes();
    let byte_count = meta.len() as i64;
    let last_modified = meta.mtime() * 1_000_000_000 + meta.mtime_nsec();
    let values = params![path_bytes, byte_count, last_modified, crawl];

    //known and unchanged: only increment the last_crawl value
    let updated = conn
        .prepare_cached(
            "UPDATE files SET last_crawl = ?4
             WHERE path = ?1 AND byte_count = ?2 AND last_modified = ?3",
        )?
        .execute(values)?;
    if updated == 1 {
        return Ok(Change::Unchanged);
    }

    //known file but changed since last crawl: store the new size and time and increment the
    //last_crawl, clear hash
    let updated = conn
        .prepare_cached(
            "UPDATE files SET byte_count = ?2, last_modified = ?3, last_crawl = ?4, hash = NULL
             WHERE path = ?1",
        )?
        .execute(values)?;
    if updated == 1 {
        return Ok(Change::Changed);
    }

    //never seen/ new file, insert new row
    conn.prepare_cached(
        "INSERT INTO files (path, byte_count, last_modified, last_crawl)
         VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(values)?;
    Ok(Change::New)
}

pub fn set_hash(conn: &Connection, path: &Path, hash: &[u8]) -> rusqlite::Result<()> {
    conn.prepare_cached("UPDATE files SET hash = ?2 WHERE path = ?1")?
        .execute(params![path.as_os_str().as_bytes(), hash])?;
    Ok(())
}

pub fn each_file(
    conn: &Connection,
    mut on_file: impl FnMut(PathBuf, Option<String>),
) -> rusqlite::Result<()> {
    let mut statement = conn.prepare(
        "SELECT files.path, contents.command
         FROM files LEFT JOIN contents ON contents.hash = files.hash",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let path_bytes: Vec<u8> = row.get(0)?;
        let command: Option<String> = row.get(1)?;
        on_file(PathBuf::from(OsStr::from_bytes(&path_bytes)), command);
    }
    Ok(())
}

pub fn extraction(conn: &Connection, hash: &[u8]) -> rusqlite::Result<Option<Extraction>> {
    let mut statement =
        conn.prepare_cached("SELECT command, error FROM contents WHERE hash = ?1")?;
    let mut rows = statement.query(params![hash])?;
    match rows.next()? {
        Some(row) => Ok(Some(Extraction {
            command: row.get(0)?,
            error: row.get(1)?,
        })),
        None => Ok(None),
    }
}

pub fn save_extraction(
    conn: &Connection,
    hash: &[u8],
    command: &str,
    chunks: &[Chunk],
    error: Option<&str>,
) -> rusqlite::Result<()> {
    conn.prepare_cached("DELETE FROM chunks WHERE hash = ?1")?
        .execute(params![hash])?;
    let mut insert =
        conn.prepare_cached("INSERT INTO chunks (hash, start_line, text) VALUES (?1, ?2, ?3)")?;
    for chunk in chunks {
        insert.execute(params![hash, chunk.start_line, chunk.text])?;
    }
    conn.prepare_cached(
        "INSERT OR REPLACE INTO contents (hash, command, error) VALUES (?1, ?2, ?3)",
    )?
    .execute(params![hash, command, error])?;
    Ok(())
}

pub fn clear_failures(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM contents WHERE error IS NOT NULL", [])
}

pub fn delete_orphans(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM chunks WHERE NOT EXISTS (SELECT 1 FROM files WHERE files.hash = chunks.hash)",
        [],
    )?;
    conn.execute(
        "DELETE FROM contents WHERE NOT EXISTS (SELECT 1 FROM files WHERE files.hash = contents.hash)",
        [],
    )?;
    Ok(())
}

pub fn search(
    conn: &Connection,
    query: &str,
    bold: &str,
    plain: &str,
) -> rusqlite::Result<Vec<Hit>> {
    let mut statement = conn.prepare(
        "SELECT files.path, chunks.start_line, snippet(chunks_fts, 0, ?2, ?3, '…', 16),
                highlight(chunks_fts, 0, char(1), '')
         FROM chunks_fts
         JOIN chunks ON chunks.id = chunks_fts.rowid
         JOIN files ON files.hash = chunks.hash
         WHERE chunks_fts MATCH ?1
         ORDER BY rank
         LIMIT 20",
    )?;
    let mut rows = statement.query(params![query, bold, plain])?;

    let mut hits = Vec::new();
    while let Some(row) = rows.next()? {
        let path_bytes: Vec<u8> = row.get(0)?;
        let start_line: i64 = row.get(1)?;
        let marked: String = row.get(3)?;
        let before_match = match marked.find('\u{1}') {
            Some(index) => &marked[..index],
            None => "",
        };
        hits.push(Hit {
            path: PathBuf::from(OsStr::from_bytes(&path_bytes)),
            line: start_line + before_match.matches('\n').count() as i64,
            snippet: row.get(2)?,
        });
    }
    Ok(hits)
}

pub fn keep_folder(conn: &Connection, dir: &Path, crawl: i64) -> rusqlite::Result<usize> {
    let (start, end) = folder_range(dir);
    conn.execute(
        "UPDATE files SET last_crawl = ?1 WHERE path >= ?2 AND path < ?3",
        params![crawl, start, end],
    )
}

pub fn delete_missing(conn: &Connection, dir: &Path, crawl: i64) -> rusqlite::Result<usize> {
    let (start, end) = folder_range(dir);
    conn.execute(
        "DELETE FROM files WHERE path >= ?1 AND path < ?2 AND last_crawl < ?3",
        params![start, end, crawl],
    )
}

fn folder_range(dir: &Path) -> (Vec<u8>, Vec<u8>) {
    let mut start = dir.as_os_str().as_bytes().to_vec();
    if !start.ends_with(b"/") {
        start.push(b'/');
    }
    let mut end = start.clone();
    end.pop();
    end.push(b'0');
    (start, end)
}
