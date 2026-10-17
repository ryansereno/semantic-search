mod config;
mod crawl;
mod db;
mod extract;
mod plugin;
mod text;

use extract::Status;
use std::env;
use std::error::Error;
use std::fs::{self, Metadata};
use std::io::{self, IsTerminal};
use std::path::Path;
use std::process;
use std::time::{Duration, Instant};

//how much extract work Ctrl-C can lose
const COMMIT_EVERY: Duration = Duration::from_secs(2);

fn main() {
    let args: Vec<String> = env::args().collect();

    let result = match args.get(1).map(|arg| arg.as_str()) {
        Some("crawl") => {
            let dir = args.get(2).map(|arg| arg.as_str()).unwrap_or(".");
            crawl_command(Path::new(dir))
        }
        Some("extract") if args.len() == 2 => extract_command(false),
        Some("extract") if args.len() == 3 && args[2] == "--retry" => extract_command(true),
        Some("update") => {
            let dir = args.get(2).map(|arg| arg.as_str()).unwrap_or(".");
            match crawl_command(Path::new(dir)) {
                Ok(()) => extract_command(false),
                Err(err) => Err(err),
            }
        }
        Some("search") if args.len() > 2 => search_command(&args[2..]),
        _ => {
            eprintln!("usage: tracker crawl [DIR]       record the files under DIR");
            eprintln!(
                "       tracker extract [--retry] read their text; --retry tries failed files again"
            );
            eprintln!("       tracker update [DIR]      crawl, then extract");
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

fn extract_command(retry: bool) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let extractors = extract::extractors(config::load_config()?);
    let mut conn = db::open()?;
    if retry {
        db::clear_failures(&conn)?;
    }

    //files that have an extractor, and whose content wasn't extracted with it yet
    let mut queue = Vec::new();
    db::each_file(&conn, |path, extracted_with| {
        if let Some(extractor) = extractors.get(&extract::parse_extension(&path)) {
            let done = match extracted_with {
                Some(command) => command == extractor.name,
                None => false,
            };
            if !done {
                queue.push(path);
            }
        }
    })?;

    //a progress line that updates in place, unless stderr goes to a file or another program
    let terminal = io::stderr().is_terminal();
    let mut extracted = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut broken = Vec::new();

    let mut tx = conn.transaction()?;
    let mut last_commit = Instant::now();
    for (index, path) in queue.iter().enumerate() {
        if terminal {
            eprint!("\r\x1b[Kextract: {} of {}", index + 1, queue.len());
        }
        //every file in the queue has an extractor
        let extractor = &extractors[&extract::parse_extension(path)];
        let message = match extract::extract_file(&tx, path, extractor, &mut broken)? {
            Status::Extracted => {
                extracted += 1;
                None
            }
            Status::Failed(error) => {
                failed += 1;
                Some(format!("{}: {error}", path.display()))
            }
            Status::Skipped(why) => {
                skipped += 1;
                why
            }
        };
        if let Some(message) = message {
            if terminal {
                eprint!("\r\x1b[K");
            }
            eprintln!("{message}");
        }

        if last_commit.elapsed() >= COMMIT_EVERY {
            tx.commit()?;
            tx = conn.transaction()?;
            last_commit = Instant::now();
        }
    }
    //only now: a moved or renamed file has been hashed by this point, so its content isn't
    //mistaken for an orphan and extracted again
    db::delete_orphans(&tx)?;
    tx.commit()?;
    if terminal {
        eprint!("\r\x1b[K");
    }

    println!(
        "extract: {extracted} extracted, {failed} failed, {skipped} skipped in {:.2?}",
        started.elapsed()
    );
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
