pub const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "rst", "org", "tex", "csv", "tsv", "json", "yaml", "yml", "toml",
    "xml", "html", "htm", "css", "js", "ts", "py", "rs", "c", "h", "cpp", "hpp", "go", "java",
    "swift", "sh", "sql",
];

pub const MAX_BYTES: u64 = 1_000_000;

const CHUNK_BYTES: usize = 1500;

pub struct Chunk {
    pub start_line: i64,
    pub text: String,
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
