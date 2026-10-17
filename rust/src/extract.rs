use rusqlite::Connection;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File, Metadata};
use std::io;
use std::path::Path;

use crate::config::Config;
use crate::db;
use crate::plugin::{self, Outcome};
use crate::text;

const TEXT_READER: &str = "text";

pub struct Extractor {
    pub name: String,
    pub command: Option<Vec<String>>,
}

pub enum Status {
    Extracted,
    Failed(String),
    Skipped(Option<String>),
}

pub fn extractors(config: Config) -> HashMap<String, Extractor> {
    let mut table = HashMap::new();
    for extension in text::TEXT_EXTENSIONS {
        let reader = Extractor {
            name: String::from(TEXT_READER),
            command: None,
        };
        table.insert(extension.to_string(), reader);
    }
    for (extension, command) in config.extract {
        //TODO- keep command without {path} running between files;
        if !command.iter().any(|arg| arg.contains("{path}")) {
            eprintln!("extract.{extension}: commands without {{path}} aren't supported yet");
            continue;
        }
        let tool = Extractor {
            name: command.join(" "),
            command: Some(command),
        };
        table.insert(extension, tool);
    }
    table
}

//"IMG_2041.HEIC" -> "heic"; "" if there's none
pub fn parse_extension(path: &Path) -> String {
    match path.extension() {
        Some(extension) => extension.to_string_lossy().to_lowercase(),
        None => String::new(),
    }
}

//broken: tools that couldn't be started this run
pub fn extract_file(
    conn: &Connection,
    path: &Path,
    extractor: &Extractor,
    broken: &mut Vec<String>,
) -> Result<Status, Box<dyn Error>> {
    if broken.contains(&extractor.name) {
        return Ok(Status::Skipped(None));
    }
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) => return Ok(skipping(path, err)),
    };
    //skip cloud dir's
    if is_cloud_only(&meta) {
        return Ok(Status::Skipped(None));
    }
    match &extractor.command {
        None => read_text(conn, path, &meta),
        Some(command) => run_tool(conn, path, command, &extractor.name, broken),
    }
}

fn read_text(conn: &Connection, path: &Path, meta: &Metadata) -> Result<Status, Box<dyn Error>> {
    if meta.len() > text::MAX_BYTES {
        return Ok(Status::Skipped(None));
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => return Ok(skipping(path, err)),
    };
    let hash = blake3::hash(&bytes);
    db::set_hash(conn, path, hash.as_bytes())?;
    if let Some(status) = already_done(conn, hash.as_bytes(), TEXT_READER)? {
        return Ok(status);
    }

    let outcome = match String::from_utf8(bytes) {
        Ok(text) => Outcome::Text(text),
        Err(_) => Outcome::Failed(String::from("not UTF-8 text")),
    };
    save(conn, hash.as_bytes(), TEXT_READER, outcome)
}

fn run_tool(
    conn: &Connection,
    path: &Path,
    command: &[String],
    name: &str,
    broken: &mut Vec<String>,
) -> Result<Status, Box<dyn Error>> {
    let hash = match hash_file(path) {
        Ok(hash) => hash,
        Err(err) => return Ok(skipping(path, err)),
    };
    db::set_hash(conn, path, hash.as_bytes())?;
    if let Some(status) = already_done(conn, hash.as_bytes(), name)? {
        return Ok(status);
    }

    let outcome = match plugin::run_once(command, path) {
        Ok(outcome) => outcome,
        Err(err) => {
            broken.push(name.to_string());
            let message = format!("can't start {}: {err}; skipping its files", command[0]);
            return Ok(Status::Skipped(Some(message)));
        }
    };
    save(conn, hash.as_bytes(), name, outcome)
}

fn already_done(conn: &Connection, hash: &[u8], name: &str) -> rusqlite::Result<Option<Status>> {
    match db::extraction(conn, hash)? {
        Some(done) if done.command == name => match done.error {
            None => Ok(Some(Status::Extracted)),
            Some(error) => Ok(Some(Status::Failed(error))),
        },
        _ => Ok(None),
    }
}

fn save(
    conn: &Connection,
    hash: &[u8],
    name: &str,
    outcome: Outcome,
) -> Result<Status, Box<dyn Error>> {
    match outcome {
        Outcome::Text(text) => {
            db::save_extraction(conn, hash, name, &text::chunks(&text), None)?;
            Ok(Status::Extracted)
        }
        Outcome::Failed(error) => {
            db::save_extraction(conn, hash, name, &[], Some(&error))?;
            Ok(Status::Failed(error))
        }
    }
}

fn skipping(path: &Path, err: io::Error) -> Status {
    Status::Skipped(Some(format!("skipping {}: {err}", path.display())))
}

fn hash_file(path: &Path) -> io::Result<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(File::open(path)?)?;
    Ok(hasher.finalize())
}

#[cfg(target_os = "macos")]
fn is_cloud_only(meta: &Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    const SF_DATALESS: u32 = 0x40000000;
    (meta.st_flags() & SF_DATALESS) != 0
}

#[cfg(not(target_os = "macos"))]
fn is_cloud_only(_meta: &Metadata) -> bool {
    false
}
