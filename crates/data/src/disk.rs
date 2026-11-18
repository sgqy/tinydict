use crate::{PackageError, PackageManifest};
use fst::{Automaton, IntoStreamer, Streamer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use tinydict_core::{DictionaryInfo, Entry, normalize};

const MAGIC: &[u8; 8] = b"TDSEEK01";
const HEADER_BYTES: u64 = 32;
// Small independent frames bound each lookup; oversized single headwords get their own block.
const BLOCK_BYTES: usize = 256 * 1024;
const MAX_BLOCK_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
struct Word {
    spelling: Box<str>,
    block: u32,
    start: u32,
    len: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Block {
    offset: u64,
    compressed: u32,
    decoded: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Index {
    source_sha256: String,
    manifest: PackageManifest,
    words: Vec<Word>,
    blocks: Vec<Block>,
}

/// Fully resident headword index with definitions stored in independently compressed disk blocks.
#[derive(Debug)]
pub struct DiskDictionary {
    path: PathBuf,
    index: Index,
    fst: fst::Map<Vec<u8>>,
}

impl DiskDictionary {
    /// Source identity, license and revision, retained in memory.
    pub fn info(&self) -> &DictionaryInfo {
        &self.index.manifest.dictionary
    }
    /// Number of indexed normalized headwords.
    pub fn headword_count(&self) -> usize {
        self.index.words.len()
    }
    /// Original entry count.
    pub fn entry_count(&self) -> usize {
        self.index.manifest.entry_count
    }
    /// Original sense count.
    pub fn sense_count(&self) -> usize {
        self.index.manifest.sense_count
    }
    /// Original spelling for a suggestion, without accessing disk.
    pub fn headword(&self, id: usize) -> Option<&str> {
        self.index.words.get(id).map(|word| word.spelling.as_ref())
    }
    /// Estimated resident index bytes, excluding transient query buffers and allocator overhead.
    pub fn estimated_bytes(&self) -> usize {
        std::mem::size_of_val(self.index.words.as_slice())
            + self
                .index
                .words
                .iter()
                .map(|word| word.spelling.len())
                .sum::<usize>()
            + std::mem::size_of_val(self.index.blocks.as_slice())
            + self.fst.as_fst().as_bytes().len()
            + self.info().license.len()
    }
    /// Exact lookup uses only the resident index.
    pub fn exact(&self, query: &str) -> Option<usize> {
        self.fst.get(normalize(query)).map(|id| id as usize)
    }
    /// Prefix lookup uses only the resident index.
    pub fn prefix(&self, query: &str, limit: usize) -> Vec<usize> {
        let key = normalize(query);
        if key.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut stream = self
            .fst
            .search(fst::automaton::Str::new(&key).starts_with())
            .into_stream();
        let mut ids = Vec::with_capacity(limit.min(100));
        while ids.len() < limit {
            let Some((_, id)) = stream.next() else {
                break;
            };
            ids.push(id as usize);
        }
        ids
    }
    /// Read and decompress only the block containing a selected headword.
    ///
    /// # Errors
    /// Fails for invalid identifiers, missing cache files, damaged blocks or invalid entries.
    pub fn entries(&self, id: usize) -> Result<Vec<Entry>, PackageError> {
        let word = self
            .index
            .words
            .get(id)
            .ok_or_else(|| invalid("unknown headword identifier"))?;
        let block = &self.index.blocks[word.block as usize];
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(block.offset))?;
        let mut decoder = zstd::stream::read::Decoder::new(file.take(u64::from(block.compressed)))?;
        decoder.window_log_max(23)?;
        let mut bytes = Vec::with_capacity(block.decoded as usize);
        decoder
            .take(u64::from(block.decoded) + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() != block.decoded as usize {
            return Err(invalid("decoded block length mismatch"));
        }
        let end = word.start as usize + word.len as usize;
        let entries: Vec<Entry> = serde_json::from_slice(&bytes[word.start as usize..end])?;
        let key = normalize(&word.spelling);
        if entries.is_empty() || entries.iter().any(|entry| normalize(&entry.word) != key) {
            return Err(invalid("disk block does not match its indexed headword"));
        }
        Ok(entries)
    }
}

fn invalid(message: &str) -> PackageError {
    PackageError::Invalid(message.into())
}

/// Open a disk-backed index, streaming a bounded-memory cache build on first use.
///
/// # Errors
/// Fails for unreadable sources, unwritable caches, malformed records or unsorted packages.
/// Packages produced by TinyDict are ordered by normalized headword.
pub fn load_disk_package(
    source: &Path,
    cache_directory: &Path,
) -> Result<DiskDictionary, PackageError> {
    let mut hash = Sha256::new();
    let mut input = File::open(source)?;
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let bytes = input.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        hash.update(&buffer[..bytes]);
    }
    let digest = format!("{:x}", hash.finalize());
    let path = cache_directory.join(format!("{digest}.tdseek"));
    fs::create_dir_all(cache_directory)?;
    if !path.exists() {
        build(source, &path, &digest)?;
    }
    open(&path, &digest)
}

fn build(source: &Path, destination: &Path, digest: &str) -> Result<(), PackageError> {
    let mut temporary =
        tempfile::NamedTempFile::new_in(destination.parent().unwrap_or(Path::new(".")))?;
    let output = temporary.as_file_mut();
    output.write_all(&[0; HEADER_BYTES as usize])?;
    let mut builder = Builder {
        output,
        buffer: Vec::with_capacity(BLOCK_BYTES),
        words: Vec::new(),
        blocks: Vec::new(),
        fst: fst::MapBuilder::memory(),
        previous: String::new(),
    };
    let mut group = Vec::new();
    let mut group_bytes = 0_usize;
    let mut key = String::new();
    let manifest = crate::package::visit_entries(source, |entry| {
        let next = normalize(&entry.word);
        if next.is_empty()
            || entry.senses.is_empty()
            || entry
                .senses
                .iter()
                .any(|sense| sense.definition.trim().is_empty())
        {
            return Err(invalid("empty headword or sense in package"));
        }
        if !group.is_empty() && next != key {
            builder.group(&key, &group)?;
            group.clear();
            group_bytes = 0;
        }
        group_bytes = group_bytes.saturating_add(entry_bytes(&entry));
        if group_bytes > MAX_BLOCK_BYTES {
            return Err(invalid("headword exceeds bounded cache build memory"));
        }
        key = next;
        group.push(entry);
        Ok(())
    })?;
    if !group.is_empty() {
        builder.group(&key, &group)?;
    }
    builder.flush()?;
    let index_offset = builder.output.stream_position()?;
    let fst = builder
        .fst
        .into_inner()
        .map_err(tinydict_core::BuildError::Index)?;
    let index = Index {
        source_sha256: digest.into(),
        manifest,
        words: builder.words,
        blocks: builder.blocks,
    };
    {
        let mut buffered = BufWriter::new(&mut *builder.output);
        serde_json::to_writer(&mut buffered, &index)?;
        buffered.flush()?;
    }
    let fst_offset = builder.output.stream_position()?;
    builder.output.write_all(&fst)?;
    let end = builder.output.stream_position()?;
    builder.output.seek(SeekFrom::Start(0))?;
    builder.output.write_all(MAGIC)?;
    for value in [index_offset, fst_offset, end] {
        builder.output.write_all(&value.to_le_bytes())?;
    }
    builder.output.sync_all()?;
    match temporary.persist_noclobber(destination) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.error.into()),
    }
}

fn entry_bytes(entry: &Entry) -> usize {
    let strings = |values: &[String]| {
        std::mem::size_of_val(values) + values.iter().map(String::len).sum::<usize>()
    };
    size_of::<Entry>()
        + entry.id.len()
        + entry.word.len()
        + strings(&entry.labels)
        + strings(&entry.contributors)
        + std::mem::size_of_val(entry.senses.as_slice())
        + entry
            .senses
            .iter()
            .map(|sense| {
                sense.id.len()
                    + sense.definition.len()
                    + sense.source.len()
                    + strings(&sense.examples)
                    + strings(&sense.synonyms)
                    + strings(&sense.labels)
                    + [
                        &sense.part_of_speech,
                        &sense.original_wordtype,
                        &sense.synset_id,
                    ]
                    .into_iter()
                    .flatten()
                    .map(String::len)
                    .sum::<usize>()
            })
            .sum::<usize>()
}

struct Builder<'a> {
    output: &'a mut File,
    buffer: Vec<u8>,
    words: Vec<Word>,
    blocks: Vec<Block>,
    fst: fst::MapBuilder<Vec<u8>>,
    previous: String,
}

impl Builder<'_> {
    fn group(&mut self, key: &str, entries: &[Entry]) -> Result<(), PackageError> {
        if !self.words.is_empty() && key <= self.previous.as_str() {
            return Err(invalid(
                "package is not sorted by normalized headword; repack it before disk-backed loading",
            ));
        }
        let bytes = serde_json::to_vec(entries)?;
        if bytes.len() > MAX_BLOCK_BYTES {
            return Err(invalid("headword exceeds 64 MiB disk block limit"));
        }
        if !self.buffer.is_empty() && self.buffer.len() + bytes.len() > BLOCK_BYTES {
            self.flush()?;
        }
        self.fst
            .insert(key, self.words.len() as u64)
            .map_err(tinydict_core::BuildError::Index)?;
        self.words.push(Word {
            spelling: entries[0].word.clone().into_boxed_str(),
            block: self.blocks.len() as u32,
            start: self.buffer.len() as u32,
            len: bytes.len() as u32,
        });
        self.buffer.extend_from_slice(&bytes);
        self.previous.clear();
        self.previous.push_str(key);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), PackageError> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 3)?;
        encoder.include_checksum(true)?;
        encoder.write_all(&self.buffer)?;
        let bytes = encoder.finish()?;
        let offset = self.output.stream_position()?;
        self.output.write_all(&bytes)?;
        self.blocks.push(Block {
            offset,
            compressed: bytes.len() as u32,
            decoded: self.buffer.len() as u32,
        });
        self.buffer.clear();
        Ok(())
    }
}

fn open(path: &Path, digest: &str) -> Result<DiskDictionary, PackageError> {
    let mut file = File::open(path)?;
    let mut header = [0; HEADER_BYTES as usize];
    file.read_exact(&mut header)?;
    if &header[..8] != MAGIC {
        return Err(invalid("unsupported disk index cache"));
    }
    let number = |offset| {
        u64::from_le_bytes(
            header[offset..offset + 8]
                .try_into()
                .expect("fixed header field"),
        )
    };
    let (index_offset, fst_offset, end) = (number(8), number(16), number(24));
    if !(HEADER_BYTES <= index_offset
        && index_offset <= fst_offset
        && fst_offset <= end
        && end == file.metadata()?.len())
    {
        return Err(invalid("invalid disk index offsets"));
    }
    file.seek(SeekFrom::Start(index_offset))?;
    let mut index: Index =
        serde_json::from_reader(BufReader::new((&mut file).take(fst_offset - index_offset)))?;
    if index.source_sha256 != digest {
        return Err(invalid("disk index belongs to a different source snapshot"));
    }
    let mut fst_bytes = Vec::new();
    file.read_to_end(&mut fst_bytes)?;
    let fst = fst::Map::new(fst_bytes).map_err(tinydict_core::BuildError::Index)?;
    if fst.len() != index.words.len() {
        return Err(invalid("disk index headword count mismatch"));
    }
    for (id, word) in index.words.iter().enumerate() {
        let Some(block) = index.blocks.get(word.block as usize) else {
            return Err(invalid("invalid headword block"));
        };
        if u64::from(word.start) + u64::from(word.len) > u64::from(block.decoded)
            || fst.get(normalize(&word.spelling)) != Some(id as u64)
        {
            return Err(invalid("invalid headword location"));
        }
    }
    let mut offset = HEADER_BYTES;
    for block in &index.blocks {
        if block.offset != offset
            || block.decoded as usize > MAX_BLOCK_BYTES
            || block.compressed == 0
        {
            return Err(invalid("invalid compressed block range"));
        }
        offset = offset
            .checked_add(u64::from(block.compressed))
            .ok_or_else(|| invalid("block offset overflow"))?;
    }
    if offset != index_offset {
        return Err(invalid("compressed blocks do not match index boundary"));
    }
    index.words.shrink_to_fit();
    index.blocks.shrink_to_fit();
    Ok(DiskDictionary {
        path: path.into(),
        index,
        fst,
    })
}
