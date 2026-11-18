use anyhow::Context;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

pub(crate) fn run(
    directory: &Path,
    lookup: Option<&str>,
    mode: tinydict_data::LoadMode,
) -> anyhow::Result<()> {
    let started = Instant::now();
    let dictionaries = crate::paths::packages(directory)?
        .iter()
        .map(|path| {
            tinydict_data::load_for_query(path, &directory.join(".tinydict-cache"), mode)
                .with_context(|| format!("loading {}", path.display()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    anyhow::ensure!(
        !dictionaries.is_empty(),
        "no .tdict or .tdict.zst packages in {}",
        directory.display()
    );
    println!(
        "Loaded {} dictionaries in {:.3}s; estimated resident payload {:.1} MiB",
        dictionaries.len(),
        started.elapsed().as_secs_f64(),
        dictionaries
            .iter()
            .map(|d| d.estimated_bytes())
            .sum::<usize>() as f64
            / 1048576.0
    );
    for dictionary in &dictionaries {
        println!(
            "{}: {} headwords, {} entries, {} senses, {:.1} MiB estimated resident",
            dictionary.info().name,
            dictionary.headword_count(),
            dictionary.entry_count(),
            dictionary.sense_count(),
            dictionary.estimated_bytes() as f64 / 1048576.0
        );
    }
    if let Some(query) = lookup {
        for dictionary in &dictionaries {
            println!("\n[{}]", dictionary.info().name);
            if let Some(id) = dictionary.exact(query) {
                for entry in dictionary.entries(id)?.iter() {
                    for sense in &entry.senses {
                        println!(
                            "{} ({}) [{}]: {}",
                            entry.word,
                            sense.part_of_speech.as_deref().unwrap_or("—"),
                            sense.source,
                            sense.definition
                        );
                        for example in &sense.examples {
                            println!("  Example: {example}");
                        }
                    }
                }
            } else {
                println!("No exact match");
            }
        }
        return Ok(());
    }
    let mut queries: Vec<String> = dictionaries
        .iter()
        .flat_map(|d| {
            (0..d.headword_count())
                .step_by((d.headword_count() / 1000).max(1))
                .filter_map(|id| d.headword(id).map(str::to_owned))
        })
        .collect();
    queries.extend(["NOT-A-REAL-WORD-XYZ", "ice cream", "café", "US", "can't"].map(str::to_owned));
    let mut exact = Vec::new();
    let mut prefix = Vec::new();
    for query in queries.iter().cycle().take(20000) {
        let start = Instant::now();
        for dictionary in &dictionaries {
            if let Some(id) = black_box(dictionary.exact(black_box(query))) {
                black_box(id);
            }
        }
        exact.push(start.elapsed().as_nanos() as u64);
        let prefix_query: String = query.chars().take(3).collect();
        let start = Instant::now();
        for dictionary in &dictionaries {
            black_box(dictionary.prefix(black_box(&prefix_query), 50));
        }
        prefix.push(start.elapsed().as_nanos() as u64);
    }
    for (name, mut samples) in [("exact", exact), ("prefix top 50 per dictionary", prefix)] {
        samples.sort_unstable();
        println!(
            "{name}: {} samples, p50 {:.3} ms, p95 {:.3} ms, p99 {:.3} ms",
            samples.len(),
            samples[samples.len() / 2] as f64 / 1e6,
            samples[samples.len() * 95 / 100] as f64 / 1e6,
            samples[samples.len() * 99 / 100] as f64 / 1e6
        );
    }
    println!("Resident index only; excludes definition reads, GUI, startup, and cold disk access.");
    if mode == tinydict_data::LoadMode::LowMemory {
        let mut reads = Vec::new();
        for dictionary in &dictionaries {
            for id in
                (0..dictionary.headword_count()).step_by((dictionary.headword_count() / 250).max(1))
            {
                let started = Instant::now();
                black_box(dictionary.entries(id)?);
                reads.push(started.elapsed().as_secs_f64() * 1000.0);
            }
        }
        reads.sort_by(f64::total_cmp);
        if !reads.is_empty() {
            println!(
                "Disk text lookup: {} samples, p50 {:.3} ms, p95 {:.3} ms, p99 {:.3} ms",
                reads.len(),
                reads[reads.len() / 2],
                reads[reads.len() * 95 / 100],
                reads[reads.len() * 99 / 100]
            );
        }
    }
    Ok(())
}
