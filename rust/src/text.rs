use std::fs::Metadata;
use std::path::Path;

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "rst", "org", "tex", "csv", "tsv", "json", "yaml", "yml", "toml",
    "xml", "html", "htm", "css", "js", "ts", "py", "rs", "c", "h", "cpp", "hpp", "go", "java",
    "swift", "sh", "sql",
];

const MAX_BYTES: u64 = 1_000_000;

const CHUNK_BYTES: usize = 1500;

pub struct Chunk {
    pub start_line: i64,
    pub text: String,
}

pub fn worth_reading(path: &Path, meta: &Metadata) -> bool {
    let extension = match path.extension() {
        Some(ext) => ext.to_string_lossy().to_lowercase(),
        None => return false,
    };
    TEXT_EXTENSIONS.contains(&extension.as_str()) && meta.len() <= MAX_BYTES && !is_cloud_only(meta)
}

pub fn chunks(text: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut start_line = 1;

    for (index, line) in text.lines().enumerate() {
        if current.len() + line.len() > CHUNK_BYTES {
            push_chunk(&mut chunks, start_line, current);
            current = String::new();
            start_line = index as i64 + 1;
        }
        current.push_str(line);
        current.push('\n');
    }
    push_chunk(&mut chunks, start_line, current);
    chunks
}

fn push_chunk(chunks: &mut Vec<Chunk>, start_line: i64, text: String) {
    if !text.trim().is_empty() {
        chunks.push(Chunk { start_line, text });
    }
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
