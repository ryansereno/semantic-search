<img src="icon/matrix-search-icon.png" alt="Treasure Trove logo" width="110" />

Semantic search is an information retrieval technique that uses the underlying meaning/ intent of a user's query rather than just simple keyword matching.

Ever since the introduction of semantic search to Google Photos, iCloud, and now MacOs- this single modality has been life changing for me, in regard to interacting with computers.

Find: "photos of my daughter's first birthday", "last year's tax return documents", "the final version of the CAD model", etc.

This tool brings semantic search to your whole file system- search for files (images, text, video, etc ) with casual, natural language, via vector similarity.

This is a lightweight (rust & sqlite) tool to achieve semantic search on any unix file system.

This tool is designed to be extensible, so you can add your own file parsing tools to expand support beyond the default supported file types.

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
