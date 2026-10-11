mod crawl;
mod db;

use std::env;
use std::error::Error;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let arg = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    //absolute, symlinks resolved, so a dir is always stored under the same path
    let root = match fs::canonicalize(&arg) {
        Ok(root) => root,
        Err(err) => {
            eprintln!("can't crawl {}: {err}", arg.display());
            process::exit(1);
        }
    };

    let started = Instant::now();
    let mut conn = db::open()?;

    let tx = conn.transaction()?;
    let crawl:i64 = db::next_crawl(&tx)?;

    let mut new = 0;
    let mut changed = 0;
    let mut unchanged = 0;

    let unreadable = crawl::walk(&root, |path: &Path, meta: &Metadata| {
        match db::save_file(&tx, path, meta, crawl)? {
            db::Change::New => new += 1,
            db::Change::Changed => changed += 1,
            db::Change::Unchanged => unchanged += 1,
        }
        Ok(())
    })?;

    for dir in &unreadable {
        db::keep_folder(&tx, dir, crawl)?;
    }
    let deleted = db::delete_missing(&tx, &root, crawl)?;
    tx.commit()?;

    println!(
        "crawl {crawl}: {new} new, {changed} changed, {unchanged} unchanged, {deleted} deleted in {:.2?}",
        started.elapsed()
    );
    Ok(())
}
