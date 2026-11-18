use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};
use tinydict_core::{Entry, ResourceBudget};
use tinydict_data::{LoadMode, LoadedDictionary};

pub(crate) enum LoadEvent {
    Started(String),
    Ready(Arc<LoadedDictionary>, Duration),
    Failed(String),
    Finished,
}

pub(crate) fn start_loader(
    directory: PathBuf,
    ctx: egui::Context,
    mode: LoadMode,
) -> mpsc::Receiver<LoadEvent> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let send = |event| {
            let alive = sender.send(event).is_ok();
            ctx.request_repaint();
            alive
        };
        let paths = match crate::paths::packages(&directory) {
            Ok(paths) => paths,
            Err(error) => {
                send(LoadEvent::Failed(format!(
                    "{}: {error}",
                    directory.display()
                )));
                send(LoadEvent::Finished);
                return;
            }
        };
        let mut system = sysinfo::System::new();
        system.refresh_memory();
        let cpus = std::thread::available_parallelism().map_or(1, usize::from);
        let budget = ResourceBudget::detect(system.total_memory(), system.available_memory(), cpus);
        let mut resident = 0_u64;
        let mut ids = std::collections::HashSet::new();
        for path in paths {
            if !send(LoadEvent::Started(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            )) {
                return;
            }
            let start = Instant::now();
            let result = (|| -> anyhow::Result<LoadedDictionary> {
                let metadata = tinydict_data::inspect_package(&path)?;
                anyhow::ensure!(
                    !ids.contains(&metadata.dictionary.id),
                    "duplicate dictionary identity: {}",
                    metadata.dictionary.id
                );
                system.refresh_memory();
                let estimate = if mode == LoadMode::LowMemory {
                    (metadata.entry_count as u64)
                        .saturating_mul(96)
                        .saturating_add(metadata.dictionary.license.len() as u64)
                } else {
                    metadata.estimated_bytes
                };
                let working_space = estimate.saturating_mul(2);
                let reserve = (system.total_memory() / 6).max(1024_u64.pow(3));
                anyhow::ensure!(
                    resident.saturating_add(estimate) <= budget.resident_bytes
                        && working_space <= system.available_memory().saturating_sub(reserve),
                    "insufficient memory for this mode; try --low-memory or close other applications"
                );
                let dictionary =
                    tinydict_data::load_for_query(&path, &directory.join(".tinydict-cache"), mode)?;
                anyhow::ensure!(
                    resident.saturating_add(dictionary.estimated_bytes() as u64)
                        <= budget.resident_bytes,
                    "resident index exceeds available memory budget"
                );
                Ok(dictionary)
            })();
            match result {
                Ok(dictionary) => {
                    ids.insert(dictionary.info().id.clone());
                    resident += dictionary.estimated_bytes() as u64;
                    if !send(LoadEvent::Ready(Arc::new(dictionary), start.elapsed())) {
                        return;
                    }
                }
                Err(error) => {
                    if !send(LoadEvent::Failed(format!("{}: {error:#}", path.display()))) {
                        return;
                    }
                }
            }
        }
        send(LoadEvent::Finished);
    });
    receiver
}

pub(crate) struct SearchRequest {
    pub generation: u64,
    pub query: String,
    pub dictionaries: Vec<Arc<LoadedDictionary>>,
}

#[derive(Debug)]
pub(crate) struct BookMatches {
    pub dictionary: Arc<LoadedDictionary>,
    pub exact: Option<usize>,
    pub suggestions: Vec<usize>,
    disk_entries: Option<Vec<Entry>>,
    pub error: Option<String>,
}

impl BookMatches {
    pub fn entries(&self) -> Option<&[Entry]> {
        self.disk_entries
            .as_deref()
            .or_else(|| self.dictionary.resident_entries(self.exact?))
    }
}

#[derive(Debug)]
pub(crate) struct SearchResult {
    pub generation: u64,
    pub query: String,
    pub books: Vec<BookMatches>,
    pub elapsed: Duration,
}

#[derive(Default)]
struct Inbox {
    request: Option<SearchRequest>,
    closed: bool,
}

pub(crate) struct SearchWorker {
    inbox: Arc<(Mutex<Inbox>, Condvar)>,
    latest: Arc<AtomicU64>,
    pub results: mpsc::Receiver<SearchResult>,
}

impl SearchWorker {
    pub fn new(ctx: egui::Context) -> Self {
        let inbox = Arc::new((Mutex::new(Inbox::default()), Condvar::new()));
        let latest = Arc::new(AtomicU64::new(0));
        let (sender, results) = mpsc::sync_channel(1);
        let worker_inbox = Arc::clone(&inbox);
        let worker_latest = Arc::clone(&latest);
        std::thread::spawn(move || {
            loop {
                let (mutex, wake) = &*worker_inbox;
                let mut state = mutex.lock().expect("search inbox poisoned");
                while state.request.is_none() && !state.closed {
                    state = wake
                        .wait(state)
                        .expect("search inbox poisoned while waiting");
                }
                if state.closed {
                    break;
                }
                let request = state
                    .request
                    .take()
                    .expect("awakened search inbox contains a request");
                drop(state);
                let started = Instant::now();
                let mut books = Vec::new();
                for dictionary in request.dictionaries {
                    if worker_latest.load(Ordering::Relaxed) != request.generation {
                        break;
                    }
                    let exact = dictionary.exact(&request.query);
                    let suggestions = dictionary.prefix(&request.query, 50);
                    let (disk_entries, error) =
                        if let Some(id) = exact.filter(|_| dictionary.is_disk_backed()) {
                            match dictionary.entries(id) {
                                Ok(entries) => (Some(entries.into_owned()), None),
                                Err(error) => (None, Some(error.to_string())),
                            }
                        } else {
                            (None, None)
                        };
                    books.push(BookMatches {
                        dictionary,
                        exact,
                        suggestions,
                        disk_entries,
                        error,
                    });
                }
                if worker_latest.load(Ordering::Relaxed) == request.generation {
                    let result = SearchResult {
                        generation: request.generation,
                        query: request.query,
                        books,
                        elapsed: started.elapsed(),
                    };
                    if sender.send(result).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            }
        });
        Self {
            inbox,
            latest,
            results,
        }
    }

    pub fn submit(&self, request: SearchRequest) {
        self.latest.store(request.generation, Ordering::Relaxed);
        let (mutex, wake) = &*self.inbox;
        mutex.lock().expect("search inbox poisoned").request = Some(request);
        wake.notify_one();
    }
}

impl Drop for SearchWorker {
    fn drop(&mut self) {
        let (mutex, wake) = &*self.inbox;
        let mut state = mutex.lock().expect("search inbox poisoned");
        state.closed = true;
        state.request = None;
        wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinydict_core::{Dictionary, DictionaryInfo, Entry, Sense};

    fn book(id: &str, definition: &str) -> Arc<LoadedDictionary> {
        Arc::new(
            Dictionary::build(
                DictionaryInfo {
                    id: id.into(),
                    name: id.into(),
                    source_url: String::new(),
                    revision: "test".into(),
                    license: "test".into(),
                },
                vec![Entry {
                    id: "entry".into(),
                    word: "word".into(),
                    labels: vec![],
                    contributors: vec![],
                    senses: vec![Sense {
                        definition: definition.into(),
                        ..Sense::default()
                    }],
                }],
            )
            .unwrap()
            .into(),
        )
    }

    #[test]
    fn latest_query_wins_and_shared_books_remain_independent() {
        let worker = SearchWorker::new(egui::Context::default());
        let a = book("a", "first source");
        let b = book("b", "second source");
        for generation in 1..=100 {
            worker.submit(SearchRequest {
                generation,
                query: if generation == 100 {
                    "word".into()
                } else {
                    format!("missing-{generation}")
                },
                dictionaries: vec![Arc::clone(&a), Arc::clone(&b)],
            });
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            let result = worker
                .results
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            if result.generation == 100 {
                break result;
            }
        };
        assert_eq!(result.books.len(), 2);
        assert!(Arc::ptr_eq(&result.books[0].dictionary, &a));
        assert!(Arc::ptr_eq(&result.books[1].dictionary, &b));
        assert_eq!(result.books[0].exact, Some(0));
        assert_eq!(
            result.books[1].entries().unwrap()[0].senses[0].definition,
            "second source"
        );
        worker.submit(SearchRequest {
            generation: 101,
            query: "word".into(),
            dictionaries: vec![a],
        });
        assert_eq!(
            worker
                .results
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .books
                .len(),
            1
        );
    }

    #[test]
    fn missing_package_directory_reports_error_and_finishes() {
        let receiver = start_loader(
            PathBuf::from("a-directory-that-does-not-exist-for-tinydict-tests"),
            egui::Context::default(),
            LoadMode::Full,
        );
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            LoadEvent::Failed(_)
        ));
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            LoadEvent::Finished
        ));
    }
}
