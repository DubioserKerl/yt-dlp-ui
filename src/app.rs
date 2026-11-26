use crate::{
    helpers::{canonical, is_youtube_url, pump},
    model::{Config, Entry, Event, Page, SPECS},
    pages,
};
use eframe::egui;
use std::{
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

pub(crate) struct App {
    pub(crate) config: Config,
    pub(crate) staging: Vec<Entry>,
    pub(crate) active: Vec<Entry>,
    pub(crate) history: Vec<Entry>,
    page: Page,
    next_id: u64,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    clipboard: Option<arboard::Clipboard>,
    last_clipboard: String,
    next_clip_poll: Instant,
    pub(crate) notice: String,
    pub(crate) command_text: String,
}

impl App {
    fn new() -> Self {
        let config = std::fs::read_to_string("yt-dlp-ui.toml")
            .ok()
            .and_then(|contents| toml::from_str(&contents).ok())
            .unwrap_or_default();
        let (tx, rx) = mpsc::channel();
        Self {
            config,
            staging: Vec::new(),
            active: Vec::new(),
            history: Vec::new(),
            page: Page::Staging,
            next_id: 1,
            tx,
            rx,
            clipboard: arboard::Clipboard::new().ok(),
            last_clipboard: String::new(),
            next_clip_poll: Instant::now(),
            notice: String::new(),
            command_text: String::new(),
        }
    }

    pub(crate) fn add_url(&mut self, url: &str) {
        if !is_youtube_url(url) {
            return;
        }
        let clean = url.trim().trim_end_matches('/').to_string();
        let duplicate = self
            .staging
            .iter()
            .chain(&self.active)
            .chain(&self.history)
            .any(|entry| canonical(&entry.url) == canonical(&clean));
        if duplicate {
            self.notice = format!("Already added or downloaded: {clean}");
            return;
        }

        self.staging.push(Entry {
            id: self.next_id,
            url: clean,
            status: "Ready".into(),
            progress: String::new(),
            error: String::new(),
            overrides: Default::default(),
            extra_args: String::new(),
        });
        self.next_id += 1;
    }

    pub(crate) fn save_config(&mut self) {
        self.notice = match toml::to_string_pretty(&self.config) {
            Ok(contents) => match std::fs::write("yt-dlp-ui.toml", contents) {
                Ok(()) => "Settings saved to yt-dlp-ui.toml".into(),
                Err(error) => format!("Could not save settings: {error}"),
            },
            Err(error) => format!("Could not serialize settings: {error}"),
        };
    }

    fn command_args(&self, entry: &Entry) -> Vec<String> {
        let mut args = Vec::new();
        for spec in SPECS {
            let value = entry
                .overrides
                .get(spec.key)
                .cloned()
                .or_else(|| self.config.settings.get(spec.key).cloned())
                .unwrap_or_default();
            if value.is_empty() {
                continue;
            }
            if spec.key == "playlist" {
                if value == "false" {
                    args.push("--no-playlist".into());
                }
                continue;
            }
            if spec.key == "continue" {
                if value == "false" {
                    args.push("--no-continue".into());
                }
                continue;
            }
            if matches!(spec.kind, crate::model::Kind::Bool) {
                if value == "true" {
                    args.push(spec.flag.into());
                }
                continue;
            }
            args.push(spec.flag.into());
            args.push(value);
        }
        if let Ok(extra) = shell_words::split(&self.config.extra_args) {
            args.extend(extra);
        }
        if let Ok(extra) = shell_words::split(&entry.extra_args) {
            args.extend(extra);
        }
        args.push(entry.url.clone());
        args
    }

    pub(crate) fn start_download(&mut self, index: usize) {
        let entry = self.staging.remove(index);
        let args = self.command_args(&entry);
        let executable = self.config.executable.clone();
        let tx = self.tx.clone();
        let id = entry.id;
        self.active.push(Entry {
            status: "Starting…".into(),
            ..entry
        });

        thread::spawn(move || {
            let mut child = match Command::new(&executable)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(error) => {
                    let _ = tx.send(Event::Done(
                        id,
                        Err(format!("Could not start {executable}: {error}")),
                    ));
                    return;
                }
            };

            if let Some(stdout) = child.stdout.take() {
                pump(stdout, tx.clone(), id);
            }
            if let Some(stderr) = child.stderr.take() {
                pump(stderr, tx.clone(), id);
            }
            let result = match child.wait() {
                Ok(status) if status.success() => Ok(()),
                Ok(status) => Err(format!("yt-dlp exited with {status}")),
                Err(error) => Err(error.to_string()),
            };
            let _ = tx.send(Event::Done(id, result));
        });
    }

    pub(crate) fn import_command(&mut self) {
        let parts = match shell_words::split(&self.command_text) {
            Ok(parts) => parts,
            Err(error) => {
                self.notice = format!("Could not parse command: {error}");
                return;
            }
        };
        let mut index = 0;
        let mut urls = Vec::new();
        while index < parts.len() {
            let part = &parts[index];
            if part == "yt-dlp" || part.to_lowercase().ends_with("yt-dlp.exe") {
                self.config.executable = part.clone();
            } else if is_youtube_url(part) {
                urls.push(part.clone());
            } else if part.starts_with('-') {
                let spec = SPECS.iter().find(|spec| {
                    spec.flag == part
                        || (spec.key == "playlist" && part == "--no-playlist")
                        || (spec.key == "continue" && part == "--no-continue")
                });
                if let Some(spec) = spec {
                    if matches!(spec.kind, crate::model::Kind::Bool) {
                        let enabled = part != "--no-playlist" && part != "--no-continue";
                        self.config
                            .settings
                            .insert(spec.key.into(), enabled.to_string());
                    } else if index + 1 < parts.len() {
                        index += 1;
                        self.config
                            .settings
                            .insert(spec.key.into(), parts[index].clone());
                    }
                } else {
                    let mut extra = part.clone();
                    if index + 1 < parts.len()
                        && !parts[index + 1].starts_with('-')
                        && !is_youtube_url(&parts[index + 1])
                    {
                        index += 1;
                        extra.push(' ');
                        extra.push_str(&shell_words::quote(&parts[index]));
                    }
                    if !self.config.extra_args.is_empty() {
                        self.config.extra_args.push(' ');
                    }
                    self.config.extra_args.push_str(&extra);
                }
            }
            index += 1;
        }
        for url in urls {
            self.add_url(&url);
        }
        self.notice =
            "Imported recognized settings and preserved other flags as extra arguments".into();
    }

    pub(crate) fn import_config(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("TOML config", &["toml"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(path)
            .ok()
            .and_then(|contents| toml::from_str::<Config>(&contents).ok())
        {
            Some(config) => {
                self.config = config;
                self.notice = "Configuration imported".into();
            }
            None => self.notice = "Could not read that TOML configuration".into(),
        }
    }

    pub(crate) fn export_config(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("yt-dlp-ui.toml")
            .add_filter("TOML config", &["toml"])
            .save_file()
        else {
            return;
        };
        self.notice = match toml::to_string_pretty(&self.config)
            .map_err(|error| error.to_string())
            .and_then(|contents| std::fs::write(path, contents).map_err(|error| error.to_string()))
        {
            Ok(()) => "Configuration exported".into(),
            Err(error) => error,
        };
    }

    pub(crate) fn export_command(&mut self) {
        let args = self
            .config
            .settings
            .iter()
            .filter_map(|(key, value)| {
                SPECS
                    .iter()
                    .find(|spec| spec.key == key)
                    .map(|spec| (spec, value))
            })
            .filter(|(spec, value)| {
                !value.is_empty()
                    && (!matches!(spec.kind, crate::model::Kind::Bool) || *value == "true")
            })
            .map(|(spec, value)| {
                if matches!(spec.kind, crate::model::Kind::Bool) {
                    spec.flag.to_string()
                } else {
                    format!("{} {}", spec.flag, shell_words::quote(value))
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let extra = [args, self.config.extra_args.clone()]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        self.command_text = format!("{} {extra}", shell_words::quote(&self.config.executable));
        self.notice = self.command_text.clone();
    }

    fn poll_clipboard(&mut self) {
        if Instant::now() < self.next_clip_poll {
            return;
        }
        self.next_clip_poll = Instant::now() + Duration::from_millis(900);
        if let Some(clipboard) = &mut self.clipboard {
            if let Ok(contents) = clipboard.get_text() {
                if contents != self.last_clipboard {
                    self.last_clipboard = contents.clone();
                    for token in contents.split_whitespace() {
                        let url = token.trim_matches(|c: char| {
                            matches!(c, '<' | '>' | '"' | '\'' | ',' | ')' | ']')
                        });
                        if is_youtube_url(url) {
                            self.add_url(url);
                            self.page = Page::Staging;
                        }
                    }
                }
            }
        }
    }

    fn receive_download_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Line(id, line) => {
                    if let Some(entry) = self.active.iter_mut().find(|entry| entry.id == id) {
                        entry.status = "Downloading".into();
                        if line.contains('%') || !line.trim().is_empty() {
                            entry.progress = line.clone();
                        }
                        if line.to_lowercase().contains("error:") {
                            entry.error = line;
                        }
                    }
                }
                Event::Done(id, result) => {
                    if let Some(position) = self.active.iter().position(|entry| entry.id == id) {
                        let mut entry = self.active.remove(position);
                        match result {
                            Ok(()) => {
                                entry.status = "Finished".into();
                                entry.progress = "Complete".into();
                                entry.error.clear();
                                self.history.push(entry);
                            }
                            Err(error) => {
                                entry.status = "Failed".into();
                                if entry.error.is_empty() {
                                    entry.error = error;
                                } else {
                                    entry.error.push('\n');
                                    entry.error.push_str(&error);
                                }
                                self.staging.push(entry);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_clipboard();
        self.receive_download_events();

        egui::TopBottomPanel::top("header").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.heading("yt-dlp UI");
                ui.separator();
                for (label, page) in [
                    ("Staging", Page::Staging),
                    ("Downloads", Page::Downloads),
                    ("History", Page::History),
                    ("Options", Page::Options),
                ] {
                    if ui.selectable_label(self.page == page, label).clicked() {
                        self.page = page;
                    }
                }
            });
        });

        egui::CentralPanel::default().show(context, |ui| {
            if !self.notice.is_empty() {
                ui.label(&self.notice);
            }
            let page = self.page.clone();
            pages::show(self, &page, ui);
        });
        context.request_repaint_after(Duration::from_millis(300));
    }
}

pub(crate) fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([760.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native("yt-dlp UI", options, Box::new(|_| Ok(Box::new(App::new()))))
}
