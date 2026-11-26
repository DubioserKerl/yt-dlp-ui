use crate::{
    helpers::{
        canonical, cleanup_partial_files, is_youtube_url, parse_command_line, parse_format_list,
        pump_output,
    },
    model::{Config, Entry, Event, FormatState, HistoryFile, Page, SPECS},
    pages,
};

const HISTORY_FILE: &str = "yt-dlp-ui-history.toml";

fn restore_history(config: &Config) -> (Vec<Entry>, u64, String) {
    let mut notice = String::new();
    let mut history = match std::fs::read_to_string(HISTORY_FILE) {
        Ok(contents) => match toml::from_str::<HistoryFile>(&contents) {
            Ok(saved) => saved.entries,
            Err(error) => {
                notice = format!("Could not read saved download history: {error}");
                Vec::new()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            notice = format!("Could not read saved download history: {error}");
            Vec::new()
        }
    };

    let mut next_id = history
        .iter()
        .map(|entry| entry.id)
        .max()
        .unwrap_or_default()
        .saturating_add(1);
    let mut recovered = 0;
    if let Some(archive_path) = config
        .settings
        .get("download_archive")
        .filter(|path| !path.trim().is_empty())
    {
        match std::fs::read_to_string(archive_path) {
            Ok(contents) => {
                for line in contents.lines() {
                    let mut fields = line.split_whitespace();
                    let Some(extractor) = fields.next() else {
                        continue;
                    };
                    let Some(video_id) = fields.next() else {
                        continue;
                    };
                    if !extractor.eq_ignore_ascii_case("youtube")
                        || !video_id.chars().all(|character| {
                            character.is_ascii_alphanumeric() || "_-".contains(character)
                        })
                    {
                        continue;
                    }

                    let url = format!("https://www.youtube.com/watch?v={video_id}");
                    if history
                        .iter()
                        .any(|entry| canonical(&entry.url) == canonical(&url))
                    {
                        continue;
                    }
                    history.push(Entry {
                        id: next_id,
                        url,
                        status: "Finished".into(),
                        progress: "Recovered from yt-dlp archive".into(),
                        error: String::new(),
                        overrides: Default::default(),
                        extra_args: String::new(),
                    });
                    next_id = next_id.saturating_add(1);
                    recovered += 1;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => notice = format!("Could not read yt-dlp archive: {error}"),
        }
    }

    if recovered > 0 {
        if let Err(error) = persist_history(&history) {
            notice =
                format!("Recovered {recovered} downloads, but could not save history: {error}");
        } else {
            notice = format!("Restored {recovered} downloads from the yt-dlp archive.");
        }
    }
    (history, next_id, notice)
}

fn persist_history(history: &[Entry]) -> std::io::Result<()> {
    let file = HistoryFile {
        entries: history.to_vec(),
    };
    let contents =
        toml::to_string_pretty(&file).map_err(|error| std::io::Error::other(error.to_string()))?;
    std::fs::write(HISTORY_FILE, contents)
}
use eframe::egui;
use std::{
    collections::HashMap,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

pub(crate) struct App {
    pub(crate) config: Config,
    pub(crate) staging: Vec<Entry>,
    pub(crate) downloads: Vec<Entry>,
    pub(crate) history: Vec<Entry>,
    pub(crate) format_states: HashMap<u64, FormatState>,
    page: Page,
    next_id: u64,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    running_cancel: Option<(u64, Sender<()>)>,
    clipboard: Option<arboard::Clipboard>,
    last_clipboard: String,
    next_clip_poll: Instant,
    pub(crate) notice: String,
    pub(crate) command_text: String,
    pub(crate) command_error: Option<String>,
}

impl App {
    fn new() -> Self {
        let config = std::fs::read_to_string("yt-dlp-ui.toml")
            .ok()
            .and_then(|contents| toml::from_str(&contents).ok())
            .unwrap_or_default();
        let (history, next_id, notice) = restore_history(&config);
        let (tx, rx) = mpsc::channel();
        Self {
            config,
            staging: Vec::new(),
            downloads: Vec::new(),
            history,
            format_states: HashMap::new(),
            page: Page::Staging,
            next_id,
            tx,
            rx,
            running_cancel: None,
            clipboard: arboard::Clipboard::new().ok(),
            last_clipboard: String::new(),
            next_clip_poll: Instant::now(),
            notice,
            command_text: String::new(),
            command_error: None,
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
            .chain(&self.downloads)
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

    fn save_history(&mut self) {
        if let Err(error) = persist_history(&self.history) {
            self.notice = format!("Download finished, but history could not be saved: {error}");
        }
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
        let mut entry = self.staging.remove(index);
        self.format_states.remove(&entry.id);
        entry.status = "Queued".into();
        entry.progress.clear();
        entry.error.clear();
        self.downloads.push(entry);
        self.start_next_download();
    }

    pub(crate) fn cancel_download(&mut self, id: u64) {
        let Some((running_id, cancel)) = &self.running_cancel else {
            return;
        };
        if *running_id != id {
            return;
        }
        if let Some(entry) = self.downloads.iter_mut().find(|entry| entry.id == id) {
            entry.status = "Cancelling…".into();
        }
        let _ = cancel.send(());
    }

    pub(crate) fn load_formats(&mut self, id: u64) {
        if matches!(self.format_states.get(&id), Some(FormatState::Loading)) {
            return;
        }
        let Some(entry) = self.staging.iter().find(|entry| entry.id == id) else {
            return;
        };
        let url = entry.url.clone();
        let executable = self.config.executable.clone();
        let mut args = vec![
            "--no-warnings".to_string(),
            "--color".to_string(),
            "never".to_string(),
            "-F".to_string(),
        ];
        for key in ["proxy", "cookies"] {
            let value = entry
                .overrides
                .get(key)
                .or_else(|| self.config.settings.get(key))
                .filter(|value| !value.is_empty());
            if let Some(value) = value {
                args.push(
                    if key == "proxy" {
                        "--proxy"
                    } else {
                        "--cookies"
                    }
                    .into(),
                );
                args.push(value.clone());
            }
        }
        args.push(url);
        self.format_states.insert(id, FormatState::Loading);
        let tx = self.tx.clone();

        thread::spawn(move || {
            let mut command = Command::new(&executable);
            command
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            configure_hidden_console(&mut command);
            let result = match command.output() {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let combined_output = format!("{stdout}\n{stderr}");
                    let formats = parse_format_list(&combined_output);
                    if formats.is_empty() {
                        Err(if combined_output.trim().is_empty() {
                            "yt-dlp did not report any available formats.".into()
                        } else {
                            combined_output.trim().to_string()
                        })
                    } else {
                        Ok(formats)
                    }
                }
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let combined_output = format!("{stdout}\n{stderr}");
                    Err(if combined_output.trim().is_empty() {
                        format!("yt-dlp exited with {}", output.status)
                    } else {
                        combined_output.trim().to_string()
                    })
                }
                Err(error) => Err(format!("Could not start {executable}: {error}")),
            };
            let _ = tx.send(Event::Formats(id, result));
        });
    }

    fn start_next_download(&mut self) {
        if self.running_cancel.is_some() {
            return;
        }
        let Some(index) = self
            .downloads
            .iter()
            .position(|entry| entry.status == "Queued")
        else {
            return;
        };

        self.downloads[index].status = "Starting…".into();
        let entry = self.downloads[index].clone();
        let id = entry.id;
        let args = self.command_args(&entry);
        let executable = self.config.executable.clone();
        let no_part = args.iter().any(|argument| argument == "--no-part");
        let tx = self.tx.clone();
        let (cancel_tx, cancel_rx) = mpsc::channel();
        self.running_cancel = Some((id, cancel_tx));

        thread::spawn(move || {
            let mut command = Command::new(&executable);
            command
                .arg("--newline")
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            configure_hidden_console(&mut command);

            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(error) => {
                    let _ = tx.send(Event::Done(
                        id,
                        Err(format!("Could not start {executable}: {error}")),
                    ));
                    return;
                }
            };

            let output_files = Arc::new(Mutex::new(Vec::new()));
            let mut readers = Vec::new();
            if let Some(stdout) = child.stdout.take() {
                readers.push(pump_output(stdout, tx.clone(), id, output_files.clone()));
            }
            if let Some(stderr) = child.stderr.take() {
                readers.push(pump_output(stderr, tx.clone(), id, output_files.clone()));
            }

            let (cancelled, status) = wait_for_process(&mut child, cancel_rx);
            finish_output_readers(readers, Duration::from_millis(500));

            let result = if cancelled {
                let files = output_files
                    .lock()
                    .map(|files| files.clone())
                    .unwrap_or_default();
                let cleanup_errors = cleanup_partial_files(&files, no_part);
                let mut message =
                    "Cancelled by user; partial download files were removed.".to_string();
                if !cleanup_errors.is_empty() {
                    message.push_str(" Cleanup issues: ");
                    message.push_str(&cleanup_errors.join("; "));
                }
                Err(message)
            } else {
                match status {
                    Ok(status) if status.success() => Ok(()),
                    Ok(status) => Err(format!("yt-dlp exited with {status}")),
                    Err(error) => Err(error.to_string()),
                }
            };
            let _ = tx.send(Event::Done(id, result));
        });
    }

    pub(crate) fn import_command(&mut self) {
        self.command_error = None;
        let parts = match parse_command_line(&self.command_text) {
            Ok(parts) => parts,
            Err(error) => {
                self.command_error = Some(error);
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
                    } else if index + 1 < parts.len() && !parts[index + 1].starts_with('-') {
                        index += 1;
                        self.config
                            .settings
                            .insert(spec.key.into(), parts[index].clone());
                    } else {
                        self.command_error =
                            Some(format!("The option {} requires a value.", spec.flag));
                        return;
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
                let spec = SPECS.iter().find(|spec| spec.key == key)?;
                if matches!(spec.kind, crate::model::Kind::Bool) {
                    match (spec.key, value.as_str()) {
                        ("playlist", "false") => Some("--no-playlist".to_string()),
                        ("continue", "false") => Some("--no-continue".to_string()),
                        (_, "true") => Some(spec.flag.to_string()),
                        _ => None,
                    }
                } else if value.is_empty() {
                    None
                } else {
                    Some(format!("{} {}", spec.flag, shell_words::quote(value)))
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
                    if let Some(entry) = self.downloads.iter_mut().find(|entry| entry.id == id) {
                        if entry.status != "Cancelling…" {
                            entry.status = "Downloading".into();
                        }
                        if line.contains('%') || !line.trim().is_empty() {
                            entry.progress = line.clone();
                        }
                        if line.to_lowercase().contains("error:") {
                            entry.error = line;
                        }
                    }
                }
                Event::Done(id, result) => {
                    if let Some(position) = self.downloads.iter().position(|entry| entry.id == id) {
                        let mut entry = self.downloads.remove(position);
                        if self
                            .running_cancel
                            .as_ref()
                            .is_some_and(|(running_id, _)| *running_id == id)
                        {
                            self.running_cancel = None;
                        }
                        match result {
                            Ok(()) => {
                                entry.status = "Finished".into();
                                entry.progress = "Complete".into();
                                entry.error.clear();
                                self.history.push(entry);
                                self.save_history();
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
                        self.start_next_download();
                    }
                }
                Event::Formats(id, result) => {
                    if self.staging.iter().any(|entry| entry.id == id) {
                        self.format_states.insert(
                            id,
                            match result {
                                Ok(formats) => FormatState::Loaded(formats),
                                Err(error) => FormatState::Failed(error),
                            },
                        );
                    }
                }
            }
        }
    }
}

fn wait_for_process(
    child: &mut Child,
    cancel: Receiver<()>,
) -> (bool, std::io::Result<std::process::ExitStatus>) {
    loop {
        if cancel.try_recv().is_ok() {
            terminate_process_tree(child);
            return (true, child.wait());
        }
        match child.try_wait() {
            Ok(Some(status)) => return (false, Ok(status)),
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(error) => return (false, Err(error)),
        }
    }
}

fn terminate_process_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let mut command = Command::new("taskkill");
        command
            .args(["/PID", &pid, "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_hidden_console(&mut command);
        let _ = command.status();
    }

    let _ = child.kill();
}

fn finish_output_readers(readers: Vec<thread::JoinHandle<()>>, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    for reader in readers {
        while !reader.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        if reader.is_finished() {
            let _ = reader.join();
        }
    }
}

fn configure_hidden_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
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
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if !self.notice.is_empty() {
                        ui.label(&self.notice);
                    }
                    let page = self.page.clone();
                    pages::show(self, &page, ui);
                });
        });
        context.request_repaint_after(Duration::from_millis(300));
    }
}

pub(crate) fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 900.0])
            .with_min_inner_size([820.0, 600.0])
            .with_resizable(true)
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native("yt-dlp UI", options, Box::new(|_| Ok(Box::new(App::new()))))
}

fn window_icon() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!("../resources/yt-dlp-ui.png"))
        .expect("the embedded application icon must be a valid PNG")
        .into_rgba8();
    let (width, height) = image.dimensions();
    egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}
