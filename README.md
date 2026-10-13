Semantic search- the ability to search for files (images, text, video, etc ) with casual, natural language.

This singular modality/ interface has been a life change in regard to interacting with computers.

This is a lightweight (rust & sqlite) tool to achieve semantic search on any unix system.

## How to use

catalog a directory:

```bash
cd rust
cargo run --release -- crawl ~/Documents
```

search for text matches:

```bash
cargo run --release -- search quarterly estimate
```

To view the database:

```bash
sqlite3 ~/.tracker/tracker.db "SELECT * FROM files LIMIT 10"
```


## Roadmap

1. [x] **Crawl**: walk a directory tree and print file metadata
2. [x] **Store**: keep that metadata in SQLite; detect new, changed and deleted files between runs
3. [x] **Keyword search**: extract text from plain-text files, split it into chunks, index it with FTS5
4. [ ] **Extract**: plugins for PDFs, EPUB, OCR and audio; `tracker extract` and `tracker update`
5. [ ] **Embeddings**: `tracker embed` sends text, images and file names to an embedding plugin; search by meaning
6. [ ] **Later?**: metadata (EXIF, GPS, macOS attributes), one ranked list, ignore rules, file watching
