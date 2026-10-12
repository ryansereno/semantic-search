mod crawl;
mod db;
mod text;

use rusqlite::Connection;
use std::env;
use std::error::Error;
use std::fs::{self, Metadata};
use std::io::{self, IsTerminal};
use std::path::Path;
use std::process;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();

    let result = match args.get(1).map(|arg| arg.as_str()) {
        Some("crawl") => {
            let dir = args.get(2).map(|arg| arg.as_str()).unwrap_or(".");
            crawl_command(Path::new(dir))
        }
        Some("search") if args.len() > 2 => search_command(&args[2..]),
        _ => {
            eprintln!("usage: tracker crawl [DIR]");
            eprintln!("       tracker search WORD...");
            process::exit(2);
        }
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn crawl_command(dir: &Path) -> Result<(), Box<dyn Error>> {
    let root = match fs::canonicalize(dir) {
        Ok(root) => root,
        Err(err) => {
            eprintln!("can't crawl {}: {err}", dir.display());
            process::exit(1);
        }
    };

    let started = Instant::now();
    let mut conn = db::open()?;

    let tx = conn.transaction()?;
    let crawl: i64 = db::next_crawl(&tx)?;

    let mut new = 0;
    let mut changed = 0;
    let mut unchanged = 0;
    let mut read = 0;

    let unreadable = crawl::walk(&root, |path: &Path, meta: &Metadata| {
        let change = db::save_file(&tx, path, meta, crawl)?;
        match change {
            db::Change::New => new += 1,
            db::Change::Changed => changed += 1,
            db::Change::Unchanged => unchanged += 1,
        }
        if change != db::Change::Unchanged && text::worth_reading(path, meta) {
            index_text(&tx, path)?;
            read += 1;
        }
        Ok(())
    })?;

    for dir in &unreadable {
        db::keep_folder(&tx, dir, crawl)?;
    }
    let deleted = db::delete_missing(&tx, &root, crawl)?;
    db::delete_orphan_chunks(&tx)?;
    tx.commit()?;

    println!(
        "crawl {crawl}: {new} new, {changed} changed, {unchanged} unchanged, {deleted} deleted, {read} read as text in {:.2?}",
        started.elapsed()
    );
    Ok(())
}

fn index_text(conn: &Connection, path: &Path) -> Result<(), Box<dyn Error>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("skipping {}: {err}", path.display());
            return Ok(());
        }
    };
    let hash = blake3::hash(&bytes);
    db::set_hash(conn, path, hash.as_bytes())?;

    if db::has_chunks(conn, hash.as_bytes())? {
        return Ok(()); //a copy of this file is already indexed
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return Ok(()), 
    };
    db::save_chunks(conn, hash.as_bytes(), &text::chunks(&text))?;
    Ok(())
}

fn search_command(words: &[String]) -> Result<(), Box<dyn Error>> {
    let mut query = String::new();
    for word in words {
        query.push_str(&format!("\"{}\" ", word.replace('"', "\"\"")));
    }

    //bold the matched words, unless the output is going to a file or another program
    let (bold, plain) = if io::stdout().is_terminal() {
        ("\x1b[1m", "\x1b[0m")
    } else {
        ("", "")
    };

    let conn = db::open()?;
    let hits = db::search(&conn, &query, bold, plain)?;
    if hits.is_empty() {
        println!("no matches");
    }
    for hit in &hits {
        println!("{}:{}", hit.path.display(), hit.line);
        println!("    {}", hit.snippet.replace('\n', " "));
    }
    Ok(())
}
