use crate::package::{build_dictionary, make_manifest};
use crate::{ImportReport, PackageError, PackageManifest};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use tinydict_core::{Dictionary, Entry};

const SCHEMA_VERSION: u32 = 2;
// Bound each decoded JSON record, including a malformed manifest before memory admission.
const MAX_RECORD_BYTES: u64 = 16 * 1024 * 1024;
// An 8 MiB history keeps decompression bounded while improving cross-entry compression.
const WINDOW_LOG: u32 = 23;
// Level 6 balances one-time packing cost against the size of distributed datasets.
const COMPRESSION_LEVEL: i32 = 6;

pub(crate) fn write(
    file: &mut File,
    dictionary: &Dictionary,
    report: ImportReport,
) -> Result<(), PackageError> {
    let mut encoder = zstd::stream::write::Encoder::new(BufWriter::new(file), COMPRESSION_LEVEL)?;
    encoder.include_checksum(true)?;
    encoder.window_log(WINDOW_LOG)?;
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .clamp(1, 24)
        .saturating_sub(2);
    encoder.multithread(workers as u32)?;
    let manifest = make_manifest(dictionary, report, SCHEMA_VERSION);
    let mut line = Vec::new();
    write_record(&mut encoder, &mut line, &manifest)?;
    for entry in dictionary.groups().iter().flat_map(|group| &group.entries) {
        write_record(&mut encoder, &mut line, entry)?;
    }
    encoder.finish()?.flush()?;
    Ok(())
}

fn write_record(
    output: &mut impl Write,
    line: &mut Vec<u8>,
    value: &impl serde::Serialize,
) -> Result<(), PackageError> {
    line.clear();
    serde_json::to_writer(&mut *line, value)?;
    line.push(b'\n');
    if line.len() as u64 > MAX_RECORD_BYTES {
        return Err(PackageError::Invalid("JSON record exceeds 16 MiB".into()));
    }
    output.write_all(line)?;
    Ok(())
}

fn reader(path: &Path) -> Result<impl BufRead, PackageError> {
    let mut decoder = zstd::stream::read::Decoder::new(File::open(path)?)?;
    decoder.window_log_max(WINDOW_LOG)?;
    Ok(BufReader::new(decoder))
}

fn read_record(input: &mut impl BufRead, line: &mut String) -> Result<usize, PackageError> {
    line.clear();
    let bytes = input.take(MAX_RECORD_BYTES + 1).read_line(line)?;
    if bytes as u64 > MAX_RECORD_BYTES {
        return Err(PackageError::Invalid("JSON record exceeds 16 MiB".into()));
    }
    if bytes != 0 && !line.ends_with('\n') {
        return Err(PackageError::Invalid("unterminated JSON record".into()));
    }
    Ok(bytes)
}

fn manifest(input: &mut impl BufRead, line: &mut String) -> Result<PackageManifest, PackageError> {
    read_record(input, line)?;
    let metadata: PackageManifest = serde_json::from_str(line)?;
    if metadata.schema_version != SCHEMA_VERSION {
        return Err(PackageError::Invalid(format!(
            "unsupported compressed schema {}",
            metadata.schema_version
        )));
    }
    Ok(metadata)
}

pub(crate) fn inspect(path: &Path) -> Result<PackageManifest, PackageError> {
    manifest(&mut reader(path)?, &mut String::new())
}

pub(crate) fn load(path: &Path) -> Result<Dictionary, PackageError> {
    let mut input = reader(path)?;
    let mut line = String::new();
    let metadata = manifest(&mut input, &mut line)?;
    // Grow from actual records, never preallocate from an untrusted header count.
    let mut entries = Vec::new();
    while read_record(&mut input, &mut line)? != 0 {
        if entries.len() >= metadata.entry_count {
            return Err(PackageError::Invalid(
                "more entries than manifest declares".into(),
            ));
        }
        entries.push(serde_json::from_str::<Entry>(&line)?);
    }
    // Reading through EOF also verifies the zstd frame checksum and detects truncation.
    build_dictionary(metadata, entries)
}
