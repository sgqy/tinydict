use anyhow::Context;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

pub(crate) fn run(directory: &Path, lookup: Option<&str>) -> anyhow::Result<()> {
    let started = Instant::now();
    let dictionaries = crate::paths::packages(directory)?
        .iter()
        .map(|path| {
            tinydict_data::load_package(path).with_context(|| format!("loading {}", path.display()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    anyhow::ensure!(
        !dictionaries.is_empty(),
        "no .tdict packages in {}",
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
            dictionary.groups().len(),
            dictionary.entry_count(),
            dictionary.sense_count(),
            dictionary.estimated_bytes() as f64 / 1048576.0
        );
    }
    if let Some(query) = lookup {
        for dictionary in &dictionaries {
            println!("\n[{}]", dictionary.info().name);
            if let Some(id) = dictionary.exact(query) {
                for entry in &dictionary.groups()[id].entries {
                    for sense in &entry.senses {
                        println!(
                            "{} ({}) [{}]: {}",
                            entry.word,
                            sense.part_of_speech.as_deref().unwrap_or("—"),
                            sense.source,
                            sense.definition
                        );
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
            d.groups()
                .iter()
                .step_by((d.groups().len() / 1000).max(1))
                .map(|g| g.key.clone())
        })
        .collect();
    queries.extend(["NOT-A-REAL-WORD-XYZ", "ice cream", "café", "US", "can't"].map(str::to_owned));
    let mut exact = Vec::new();
    let mut prefix = Vec::new();
    for query in queries.iter().cycle().take(20000) {
        let start = Instant::now();
        for dictionary in &dictionaries {
            if let Some(id) = black_box(dictionary.exact(black_box(query))) {
                black_box(&dictionary.groups()[id].entries);
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
    println!("Warm resident engine only; excludes GUI, startup, and cold disk access.");
    Ok(())
}
