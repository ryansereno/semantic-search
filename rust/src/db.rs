use rusqlite::{Connection, params};
use std::env;
use std::error::Error;
use std::fs::{self, Metadata};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub enum Change {
    New,
    Changed,
    Unchanged,
}

pub fn open() -> Result<Connection, Box<dyn Error>> {
    let home = env::var_os("HOME").expect("HOME is not set");
    let dir = Path::new(&home).join(".tracker");
    fs::create_dir_all(&dir)?;

    let conn = Connection::open(dir.join("tracker.db"))?;
    conn.execute_batch(include_str!("schema.sql"))?;
    Ok(conn)
}

pub fn next_crawl(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COALESCE(MAX(last_crawl), 0) + 1 FROM files",
        [],
        |row| row.get(0),
    )
}

//records that crawl number `crawl` found this file
pub fn save_file(
    conn: &Connection,
    path: &Path,
    meta: &Metadata,
    crawl: i64,
) -> rusqlite::Result<Change> {
    let path_bytes = path.as_os_str().as_bytes();
    let byte_count = meta.len() as i64;
    let last_modified = meta.mtime() * 1_000_000_000 + meta.mtime_nsec();
    let values = params![path_bytes, byte_count, last_modified, crawl]; //?1 ?2 ?3 ?4 below

    //known and unchanged: just stamp it with this crawl
    let updated = conn
        .prepare_cached(
            "UPDATE files SET last_crawl = ?4
             WHERE path = ?1 AND byte_count = ?2 AND last_modified = ?3",
        )?
        .execute(values)?;
    if updated == 1 {
        return Ok(Change::Unchanged);
    }

    //known but different: store the new size and time
    let updated = conn
        .prepare_cached(
            "UPDATE files SET byte_count = ?2, last_modified = ?3, last_crawl = ?4
             WHERE path = ?1",
        )?
        .execute(values)?;
    if updated == 1 {
        return Ok(Change::Changed);
    }

    //never seen before
    conn.prepare_cached(
        "INSERT INTO files (path, byte_count, last_modified, last_crawl)
         VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(values)?;
    Ok(Change::New)
}

//marks everything under `dir` as found by `crawl`. used for dirs the crawl
//couldn't read: we couldn't look, so keep what we knew.
pub fn keep_folder(conn: &Connection, dir: &Path, crawl: i64) -> rusqlite::Result<usize> {
    let (start, end) = folder_range(dir);
    conn.execute(
        "UPDATE files SET last_crawl = ?1 WHERE path >= ?2 AND path < ?3",
        params![crawl, start, end],
    )
}

//deletes rows under `dir` that crawl number `crawl` didn't find
pub fn delete_missing(conn: &Connection, dir: &Path, crawl: i64) -> rusqlite::Result<usize> {
    let (start, end) = folder_range(dir);
    conn.execute(
        "DELETE FROM files WHERE path >= ?1 AND path < ?2 AND last_crawl < ?3",
        params![start, end, crawl],
    )
}

//every path inside `dir` sorts between "dir/" and "dir0",
//because '0' is the character right after '/'
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
