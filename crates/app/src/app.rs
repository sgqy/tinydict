use crate::worker::{LoadEvent, SearchRequest, SearchResult, SearchWorker, start_loader};
use eframe::egui::{self, Color32, RichText};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use tinydict_core::Dictionary;

const ACCENT: Color32 = Color32::from_rgb(83, 202, 184);
const SENSE_PAGE: usize = 24;

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct Preferences {
    disabled: Vec<String>,
    history: Vec<String>,
    favorites: Vec<String>,
    light: bool,
}

struct LoadedBook {
    dictionary: Arc<Dictionary>,
    enabled: bool,
}
struct Suggestion {
    book: String,
    word: String,
}

pub(crate) struct TinyDict {
    directory: PathBuf,
    preferences: Preferences,
    books: Vec<LoadedBook>,
    loader: mpsc::Receiver<LoadEvent>,
    loading: Option<String>,
    errors: Vec<String>,
    worker: SearchWorker,
    generation: u64,
    query: String,
    result: Option<SearchResult>,
    searching: bool,
    suggestions: Vec<Suggestion>,
    selected: Option<usize>,
    scroll_selection: bool,
    focus_search: bool,
    visible_senses: HashMap<String, usize>,
    show_licenses: bool,
    load_seconds: f64,
}

impl TinyDict {
    pub fn new(creation: &eframe::CreationContext<'_>, directory: PathBuf, query: String) -> Self {
        let preferences: Preferences = creation
            .storage
            .and_then(|storage| eframe::get_value(storage, "preferences"))
            .unwrap_or_default();
        let ctx = &creation.egui_ctx;
        ctx.set_theme(if preferences.light {
            egui::Theme::Light
        } else {
            egui::Theme::Dark
        });
        ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(17.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
        });
        Self {
            loader: start_loader(directory.clone(), ctx.clone()),
            worker: SearchWorker::new(ctx.clone()),
            directory,
            preferences,
            books: Vec::new(),
            loading: Some("Discovering dictionaries".into()),
            errors: Vec::new(),
            generation: 0,
            query,
            result: None,
            searching: false,
            suggestions: Vec::new(),
            selected: None,
            scroll_selection: false,
            focus_search: true,
            visible_senses: HashMap::new(),
            show_licenses: false,
            load_seconds: 0.0,
        }
    }

    fn request_search(&mut self) {
        self.generation += 1;
        self.result = None;
        self.selected = None;
        self.visible_senses.clear();
        self.suggestions.clear();
        self.searching = true;
        self.worker.submit(SearchRequest {
            generation: self.generation,
            query: self.query.clone(),
            dictionaries: self
                .books
                .iter()
                .filter(|book| book.enabled)
                .map(|book| Arc::clone(&book.dictionary))
                .collect(),
        });
    }

    fn receive(&mut self) {
        let mut loaded = false;
        while let Ok(event) = self.loader.try_recv() {
            match event {
                LoadEvent::Started(name) => self.loading = Some(name),
                LoadEvent::Ready(dictionary, duration) => {
                    let enabled = !self.preferences.disabled.contains(&dictionary.info().id);
                    self.books.push(LoadedBook {
                        dictionary,
                        enabled,
                    });
                    self.load_seconds += duration.as_secs_f64();
                    loaded = true;
                }
                LoadEvent::Failed(error) => self.errors.push(error),
                LoadEvent::Finished => self.loading = None,
            }
        }
        if loaded {
            self.request_search();
        }
        while let Ok(result) = self.worker.results.try_recv() {
            if result.generation != self.generation {
                continue;
            }
            self.suggestions = result
                .books
                .iter()
                .flat_map(|book| {
                    book.suggestions.iter().map(|&id| Suggestion {
                        book: book.dictionary.info().name.clone(),
                        word: book.dictionary.groups()[id].entries[0].word.clone(),
                    })
                })
                .collect();
            self.result = Some(result);
            self.searching = false;
        }
    }

    fn remember(&mut self) {
        let query = self.query.trim();
        if query.is_empty() {
            return;
        }
        self.preferences.history.retain(|word| word != query);
        self.preferences.history.insert(0, query.to_owned());
        self.preferences.history.truncate(30);
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut changed = false;
        let mut choose = None;
        egui::Panel::top("search")
            .frame(egui::Frame::default().inner_margin(20))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("TinyDict").size(27.0).strong());
                    ui.label(RichText::new("ENGLISH / OFFLINE").size(12.0).color(ACCENT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Sources & licenses").clicked() {
                            self.show_licenses = true;
                        }
                        if ui
                            .button(if self.preferences.light {
                                "Dark"
                            } else {
                                "Light"
                            })
                            .clicked()
                        {
                            self.preferences.light = !self.preferences.light;
                            ctx.set_theme(if self.preferences.light {
                                egui::Theme::Light
                            } else {
                                egui::Theme::Dark
                            });
                        }
                    });
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let search = ui.add_sized(
                        [ui.available_width() - 100.0, 38.0],
                        egui::TextEdit::singleline(&mut self.query)
                            .id(egui::Id::unique("search-input"))
                            .hint_text("Search a word or phrase...")
                            .font(egui::TextStyle::Heading),
                    );
                    if self.focus_search
                        || ctx.input(|input| {
                            input.modifiers.command && input.key_pressed(egui::Key::L)
                        })
                    {
                        search.request_focus();
                        self.focus_search = false;
                    }
                    changed |= search.changed();
                    if ctx.input(|input| input.key_pressed(egui::Key::ArrowDown))
                        && !self.suggestions.is_empty()
                    {
                        self.selected = Some(
                            self.selected
                                .map_or(0, |index| (index + 1).min(self.suggestions.len() - 1)),
                        );
                        self.scroll_selection = true;
                    }
                    if ctx.input(|input| input.key_pressed(egui::Key::ArrowUp)) {
                        self.selected = self.selected.map(|index| index.saturating_sub(1));
                        self.scroll_selection = true;
                    }
                    if ctx.input(|input| input.key_pressed(egui::Key::Enter)) {
                        if let Some(index) = self.selected {
                            choose = self
                                .suggestions
                                .get(index)
                                .map(|suggestion| suggestion.word.clone());
                        }
                        self.remember();
                        search.request_focus();
                    }
                    let saved = self.preferences.favorites.contains(&self.query);
                    if ui
                        .add_enabled(
                            !self.query.trim().is_empty(),
                            egui::Button::new(if saved { "Saved" } else { "Save word" }),
                        )
                        .clicked()
                    {
                        if saved {
                            self.preferences
                                .favorites
                                .retain(|word| word != &self.query);
                        } else {
                            self.preferences.favorites.push(self.query.clone());
                        }
                    }
                });
            });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(name) = &self.loading {
                    ui.spinner();
                    ui.label(format!("Loading {name}"));
                } else {
                    ui.label(RichText::new("LOCAL / IN MEMORY").color(ACCENT).size(12.0));
                }
                ui.separator();
                let memory = self
                    .books
                    .iter()
                    .map(|book| book.dictionary.estimated_bytes())
                    .sum::<usize>();
                ui.small(format!(
                    "{} dictionaries · {:.1} MiB payload estimate · {:.2}s load",
                    self.books.len(),
                    memory as f64 / 1048576.0,
                    self.load_seconds
                ));
                if self.searching {
                    ui.spinner();
                } else if let Some(result) = &self.result {
                    ui.small(format!(
                        "Query {:.3} ms",
                        result.elapsed.as_secs_f64() * 1000.0
                    ));
                }
            });
        });
        egui::Panel::left("library")
            .default_size(280.0)
            .min_size(220.0)
            .resizable(true)
            .frame(egui::Frame::default().inner_margin(16))
            .show(ui, |ui| {
                ui.label(RichText::new("DICTIONARIES").size(12.0).color(ACCENT));
                for book in &mut self.books {
                    if ui
                        .checkbox(&mut book.enabled, &book.dictionary.info().name)
                        .changed()
                    {
                        changed = true;
                    }
                    ui.small(format!(
                        "{} headwords · {} senses",
                        book.dictionary.groups().len(),
                        book.dictionary.sense_count()
                    ));
                }
                ui.add_space(8.0);
                ui.separator();
                if self.query.trim().is_empty() {
                    egui::ScrollArea::vertical()
                        .id_salt("saved-words")
                        .show(ui, |ui| {
                            for (title, words) in [
                                ("SAVED WORDS", &self.preferences.favorites),
                                ("RECENT LOOKUPS", &self.preferences.history),
                            ] {
                                ui.label(RichText::new(title).size(12.0).color(ACCENT));
                                for word in words {
                                    if ui.selectable_label(false, word).clicked() {
                                        choose = Some(word.clone());
                                    }
                                }
                                ui.add_space(12.0);
                            }
                        });
                } else {
                    ui.small(format!(
                        "{} suggestions · up to 50 per dictionary",
                        self.suggestions.len()
                    ));
                    let mut scroll = egui::ScrollArea::vertical().id_salt("suggestions");
                    if self.scroll_selection
                        && let Some(index) = self.selected
                    {
                        // Offscreen rows are never rendered, so scroll before virtualization.
                        scroll = scroll.vertical_scroll_offset(
                            index as f32 * (44.0 + ui.spacing().item_spacing.y),
                        );
                    }
                    scroll.show_rows(ui, 44.0, self.suggestions.len(), |ui, rows| {
                        for index in rows {
                            let suggestion = &self.suggestions[index];
                            let response = ui.add_sized(
                                [ui.available_width(), 44.0],
                                egui::Button::new(format!(
                                    "{}\n{}",
                                    suggestion.word, suggestion.book
                                ))
                                .selected(self.selected == Some(index)),
                            );
                            if response.clicked() {
                                choose = Some(suggestion.word.clone());
                            }
                        }
                    });
                    self.scroll_selection = false;
                }
            });
        egui::CentralPanel::default().frame(egui::Frame::default().inner_margin(24)).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt(("definitions", self.generation)).show(ui, |ui| {
                for error in &self.errors { ui.colored_label(Color32::from_rgb(240, 140, 120), error); }
                if self.books.is_empty() && self.loading.is_none() {
                    ui.heading("Add your dictionaries");
                    ui.label("Build the CloudBytes and Wordset packages with the included setup script, then restart TinyDict.");
                    ui.label(format!("Dictionary folder: {}", self.directory.display()));
                } else if self.query.trim().is_empty() {
                    ui.add_space(40.0);
                    ui.label(RichText::new("One word. Multiple perspectives.").size(32.0));
                    ui.label("Search across your English dictionaries. Each source keeps its own definitions, examples and voice.");
                    ui.add_space(16.0);
                    ui.label("Ctrl+L to search · Arrow keys to browse · Enter to open");
                } else if let Some(result) = &self.result {
                    ui.label(RichText::new(result.query.trim()).size(32.0).strong());
                    ui.add_space(12.0);
                    if result.books.is_empty() { ui.label("Enable a dictionary in the sidebar to search."); }
                    for book in &result.books {
                        let limit = self.visible_senses.entry(book.dictionary.info().id.clone()).or_insert(SENSE_PAGE);
                        render_book(ui, book, &result.query, limit, &mut choose);
                        ui.add_space(16.0);
                    }
                } else { ui.label("Searching..."); }
            });
        });
        if let Some(word) = choose {
            self.query = word;
            self.remember();
            changed = true;
        }
        if changed {
            self.preferences.disabled = self
                .books
                .iter()
                .filter(|book| !book.enabled)
                .map(|book| book.dictionary.info().id.clone())
                .collect();
            self.request_search();
        }
        egui::Window::new("Sources & licenses")
            .open(&mut self.show_licenses)
            .default_size([760.0, 560.0])
            .show(&ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for book in &self.books {
                        let info = book.dictionary.info();
                        ui.heading(&info.name);
                        ui.hyperlink_to("Upstream repository", &info.source_url);
                        ui.label(format!("Revision: {}", info.revision));
                        ui.add(egui::Label::new(&info.license).selectable(true).wrap());
                        ui.separator();
                    }
                });
            });
    }
}

impl eframe::App for TinyDict {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive();
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.content(ui);
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "preferences", &self.preferences);
    }
}

fn render_book(
    ui: &mut egui::Ui,
    book: &crate::worker::BookMatches,
    query: &str,
    limit: &mut usize,
    choose: &mut Option<String>,
) {
    egui::Frame::group(ui.style())
        .inner_margin(18)
        .corner_radius(10)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&book.dictionary.info().name)
                        .size(19.0)
                        .strong()
                        .color(ACCENT),
                );
                if let Some(id) = book.exact {
                    let group = &book.dictionary.groups()[id];
                    let total: usize = group.entries.iter().map(|entry| entry.senses.len()).sum();
                    ui.small(format!("{total} senses"));
                    if ui.small_button("Copy").clicked() {
                        let text = group
                            .entries
                            .iter()
                            .flat_map(|entry| {
                                entry.senses.iter().map(|sense| {
                                    format!(
                                        "{} ({})\n{}",
                                        entry.word,
                                        sense.part_of_speech.as_deref().unwrap_or("—"),
                                        sense.definition
                                    )
                                })
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n");
                        ui.ctx()
                            .copy_text(format!("{}\n\n{text}", book.dictionary.info().name));
                    }
                }
            });
            ui.separator();
            let Some(id) = book.exact else {
                ui.label("No exact match in this dictionary. Select a suggestion on the left.");
                return;
            };
            let group = &book.dictionary.groups()[id];
            let total: usize = group.entries.iter().map(|entry| entry.senses.len()).sum();
            // Exact spelling first, retaining source order inside each spelling class.
            let entries = group
                .entries
                .iter()
                .filter(|entry| entry.word == query.trim())
                .chain(
                    group
                        .entries
                        .iter()
                        .filter(|entry| entry.word != query.trim()),
                );
            let mut shown = 0;
            for entry in entries {
                if shown >= *limit {
                    break;
                }
                if !entry.labels.is_empty() {
                    ui.small(entry.labels.join(" · "));
                }
                for sense in &entry.senses {
                    if shown >= *limit {
                        break;
                    }
                    shown += 1;
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(format!("{shown}. {}", entry.word)).strong());
                        if let Some(pos) = &sense.part_of_speech {
                            ui.label(RichText::new(pos).italics().color(ACCENT));
                        }
                        if let Some(original) = &sense.original_wordtype {
                            ui.small(original);
                        }
                        ui.small(&sense.source);
                        for label in &sense.labels {
                            ui.small(label);
                        }
                    });
                    ui.add(egui::Label::new(&sense.definition).selectable(true).wrap());
                    for example in &sense.examples {
                        ui.label(RichText::new(example).italics().weak());
                    }
                    if !sense.synonyms.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.small("Synonyms:");
                            for synonym in &sense.synonyms {
                                if ui.link(synonym).clicked() {
                                    *choose = Some(synonym.clone());
                                }
                            }
                        });
                    }
                    ui.add_space(8.0);
                }
            }
            if shown < total
                && ui
                    .button(format!("Show more senses ({shown} / {total})"))
                    .clicked()
            {
                *limit += SENSE_PAGE;
            }
        });
}
