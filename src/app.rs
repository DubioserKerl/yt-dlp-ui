use crate::{
    helpers::{
        canonical, cleanup_partial_files, is_youtube_url, normalize_staging_url,
        parse_command_line, parse_format_list, parse_playlist_position, parse_progress_fraction,
        pump_output,
    },
    model::{Config, Entry, Event, FormatState, HistoryFile, Page, SPECS},
    pages,
};

const HISTORY_FILE: &str = "yt-dlp-ui-history.toml";
const STAGING_FILE: &str = "yt-dlp-ui-staging.toml";

fn restore_staging() -> (Vec<Entry>, String) {
    match std::fs::read_to_string(STAGING_FILE) {
        Ok(contents) => match toml::from_str::<HistoryFile>(&contents) {
            Ok(saved) => (saved.entries, String::new()),
            Err(error) => (
                Vec::new(),
                format!("Could not read saved download staging: {error}"),
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Vec::new(), String::new()),
        Err(error) => (
            Vec::new(),
            format!("Could not read saved download staging: {error}"),
        ),
    }
}

fn persist_staging(staging: &[Entry]) -> std::io::Result<()> {
    let file = HistoryFile {
        entries: staging.to_vec(),
    };
    let contents =
        toml::to_string_pretty(&file).map_err(|error| std::io::Error::other(error.to_string()))?;
    std::fs::write(STAGING_FILE, contents)
}

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
    let archive_path = if std::path::Path::new(HISTORY_FILE).exists() {
        None
    } else {
        config
            .settings
            .get("download_archive")
            .filter(|path| !path.trim().is_empty())
    };
    if let Some(archive_path) = archive_path {
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
                        title: String::new(),
                        already_in_history: false,
                        completed_at: 0,
                        download_progress: 0.0,
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

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
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

#[derive(Default)]
struct DownloadProgress {
    fraction: f32,
    playlist_position: Option<(u32, u32)>,
}

pub(crate) struct App {
    pub(crate) config: Config,
    pub(crate) staging: Vec<Entry>,
    staging_snapshot: Vec<Entry>,
    pub(crate) downloads: Vec<Entry>,
    pub(crate) history: Vec<Entry>,
    pub(crate) format_states: HashMap<u64, FormatState>,
    download_progress: HashMap<u64, DownloadProgress>,
    notification_processes: Vec<Child>,
    page: Page,
    next_id: u64,
    next_notification_at: Instant,
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
        let (mut staging, staging_notice) = restore_staging();
        let staging_snapshot = staging.clone();
        let mut normalized_staging = Vec::<Entry>::new();
        for mut entry in staging.drain(..) {
            entry.url = normalize_staging_url(&entry.url);
            if let Some(existing) = normalized_staging
                .iter_mut()
                .find(|existing| canonical(&existing.url) == canonical(&entry.url))
            {
                existing.overrides.extend(entry.overrides);
                if existing.extra_args.is_empty() {
                    existing.extra_args = entry.extra_args;
                }
            } else {
                normalized_staging.push(entry);
            }
        }
        let staging = normalized_staging;
        let next_id = staging
            .iter()
            .map(|entry| entry.id)
            .max()
            .unwrap_or_default()
            .saturating_add(1)
            .max(next_id);
        let notice = if staging_notice.is_empty() {
            notice
        } else {
            staging_notice
        };
        let title_lookups: Vec<_> = staging
            .iter()
            .chain(&history)
            .filter(|entry| entry.title.trim().is_empty())
            .map(|entry| (entry.id, entry.url.clone()))
            .collect();
        let (tx, rx) = mpsc::channel();
        let app = Self {
            config,
            staging_snapshot,
            staging,
            downloads: Vec::new(),
            history,
            format_states: HashMap::new(),
            download_progress: HashMap::new(),
            notification_processes: Vec::new(),
            page: Page::Staging,
            next_id,
            next_notification_at: Instant::now(),
            tx,
            rx,
            running_cancel: None,
            clipboard: arboard::Clipboard::new().ok(),
            last_clipboard: String::new(),
            next_clip_poll: Instant::now(),
            notice,
            command_text: String::new(),
            command_error: None,
        };
        for (id, url) in title_lookups {
            app.lookup_title(id, url, false);
        }
        app
    }

    pub(crate) fn add_url(&mut self, url: &str) {
        if !is_youtube_url(url) {
            return;
        }
        let clean = normalize_staging_url(url);
        let duplicate_in_progress = self
            .staging
            .iter()
            .chain(&self.downloads)
            .any(|entry| canonical(&entry.url) == canonical(&clean));
        if duplicate_in_progress {
            self.notice = format!("Already in staging or downloads: {clean}");
            return;
        }

        let already_in_history = self
            .history
            .iter()
            .any(|entry| canonical(&entry.url) == canonical(&clean));

        self.staging.push(Entry {
            id: self.next_id,
            title: String::new(),
            already_in_history,
            completed_at: 0,
            download_progress: 0.0,
            url: clean.clone(),
            status: "Ready".into(),
            progress: String::new(),
            error: String::new(),
            overrides: Default::default(),
            extra_args: String::new(),
        });
        self.save_staging_if_changed();
        self.lookup_title(self.next_id, clean.clone(), true);
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
            self.notice = format!("Could not save download history: {error}");
        }
    }

    pub(crate) fn remove_history_entry(&mut self, id: u64) {
        self.history.retain(|entry| entry.id != id);
        self.save_history();
    }

    pub(crate) fn clear_history(&mut self) {
        self.history.clear();
        self.save_history();
    }

    fn save_staging_if_changed(&mut self) {
        if self.staging == self.staging_snapshot {
            return;
        }
        match persist_staging(&self.staging) {
            Ok(()) => self.staging_snapshot.clone_from(&self.staging),
            Err(error) => self.notice = format!("Could not save download staging: {error}"),
        }
    }

    pub(crate) fn remove_staging(&mut self, id: u64) {
        self.staging.retain(|entry| entry.id != id);
        self.format_states.remove(&id);
        self.save_staging_if_changed();
    }

    fn lookup_title(&self, id: u64, url: String, notify_when_ready: bool) {
        let executable = self.config.executable.clone();
        let settings = self.config.settings.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let mut command = Command::new(&executable);
            command
                .args(["--no-warnings", "--color", "never", "--get-title"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            for (key, flag) in [
                ("proxy", "--proxy"),
                ("cookies", "--cookies"),
                ("cookies_from_browser", "--cookies-from-browser"),
                ("socket_timeout", "--socket-timeout"),
            ] {
                if let Some(value) = settings.get(key).filter(|value| !value.is_empty()) {
                    command.args([flag, value]);
                }
            }
            command.arg(&url);
            configure_hidden_console(&mut command);
            let title = match command.output() {
                Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .map(str::trim)
                    .map(str::to_string)
                    .ok_or_else(|| "yt-dlp returned no title".to_string()),
                Ok(output) => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
                Err(error) => Err(format!("Could not start {executable}: {error}")),
            };
            let _ = tx.send(Event::Title(id, title, notify_when_ready));
        });
    }

    fn notify(&mut self, heading: &str, title: &str, url: &str) {
        if !self.config.notifications_enabled {
            return;
        }
        let Ok(executable) = std::env::current_exe() else {
            return;
        };
        let now = Instant::now();
        let start_at = self.next_notification_at.max(now);
        let delay = start_at.saturating_duration_since(now);
        let delay_ms = delay.as_millis().to_string();
        let display_seconds = self.config.notification_display_seconds.clamp(1, 60);
        let display_seconds_arg = display_seconds.to_string();
        let color_scheme = self.config.notification_color_scheme.as_str();
        let mut command = Command::new(executable);
        command
            .args([
                "--notification",
                &delay_ms,
                &display_seconds_arg,
                color_scheme,
                heading,
                title,
                url,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_hidden_console(&mut command);
        if let Ok(process) = command.spawn() {
            self.notification_processes.push(process);
            self.next_notification_at = start_at + Duration::from_secs(display_seconds as u64);
        }
    }

    fn reap_notification_processes(&mut self, close_all: bool) {
        self.notification_processes.retain_mut(|process| {
            if close_all {
                let _ = process.kill();
                let _ = process.wait();
                return false;
            }
            !matches!(process.try_wait(), Ok(Some(_)) | Err(_))
        });
        if self.notification_processes.is_empty() {
            self.next_notification_at = Instant::now();
        }
    }

    pub(crate) fn dismiss_notifications(&mut self) {
        self.reap_notification_processes(true);
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
        self.queue_download(index, true);
    }

    pub(crate) fn start_all_downloads(&mut self) {
        let count = self.staging.len();
        if count == 0 {
            return;
        }

        while !self.staging.is_empty() {
            self.queue_download(0, false);
        }
        self.page = Page::Downloads;
        self.notify(
            &format!("Added {count} downloads to the download queue"),
            "",
            "",
        );
    }

    fn queue_download(&mut self, index: usize, show_notification: bool) {
        let mut entry = self.staging.remove(index);
        self.format_states.remove(&entry.id);
        if show_notification {
            let title = if entry.title.is_empty() {
                entry.url.clone()
            } else {
                entry.title.clone()
            };
            self.notify("Added to downloads", &title, &entry.url);
        }
        entry.status = "Queued".into();
        entry.progress.clear();
        entry.download_progress = 0.0;
        entry.error.clear();
        self.download_progress
            .insert(entry.id, DownloadProgress::default());
        self.downloads.push(entry);
        self.save_staging_if_changed();
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
        for (key, flag) in [
            ("proxy", "--proxy"),
            ("cookies", "--cookies"),
            ("cookies_from_browser", "--cookies-from-browser"),
            ("socket_timeout", "--socket-timeout"),
        ] {
            let value = entry
                .overrides
                .get(key)
                .or_else(|| self.config.settings.get(key))
                .filter(|value| !value.is_empty());
            if let Some(value) = value {
                args.push(flag.into());
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
                        || (spec.key == "write_thumbnail" && part == "--no-write-thumbnail")
                        || (spec.key == "write_info_json" && part == "--no-write-info-json")
                        || (spec.key == "write_description" && part == "--no-write-description")
                });
                if let Some(spec) = spec {
                    if matches!(spec.kind, crate::model::Kind::Bool) {
                        let enabled = !matches!(
                            part.as_str(),
                            "--no-playlist"
                                | "--no-continue"
                                | "--no-write-thumbnail"
                                | "--no-write-info-json"
                                | "--no-write-description"
                        );
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
                    let tracker = self.download_progress.entry(id).or_default();
                    if let Some(position) = parse_playlist_position(&line) {
                        tracker.playlist_position = Some(position);
                    }
                    if let Some(item_fraction) = parse_progress_fraction(&line) {
                        let overall_fraction =
                            if let Some((position, total)) = tracker.playlist_position {
                                let position = position.saturating_sub(1) as f32;
                                ((position + item_fraction) / total.max(1) as f32).clamp(0.0, 1.0)
                            } else {
                                item_fraction
                            };
                        tracker.fraction = tracker.fraction.max(overall_fraction.min(0.99));
                    }
                    if let Some(entry) = self.downloads.iter_mut().find(|entry| entry.id == id) {
                        if entry.status != "Cancelling…" {
                            entry.status = "Downloading".into();
                        }
                        if line.contains('%') || !line.trim().is_empty() {
                            entry.progress = line.clone();
                        }
                        entry.download_progress = tracker.fraction;
                        if line.to_lowercase().contains("error:") {
                            entry.error = line;
                        }
                    }
                }
                Event::Done(id, result) => {
                    self.download_progress.remove(&id);
                    if let Some(position) = self.downloads.iter().position(|entry| entry.id == id) {
                        let mut entry = self.downloads.remove(position);
                        if self
                            .running_cancel
                            .as_ref()
                            .is_some_and(|(running_id, _)| *running_id == id)
                        {
                            self.running_cancel = None;
                        }
                        let (heading, title, url) = match &result {
                            Ok(()) => ("Download finished", entry.title.clone(), entry.url.clone()),
                            Err(_) => ("Download failed", entry.title.clone(), entry.url.clone()),
                        };
                        self.notify(heading, &title, &url);
                        match result {
                            Ok(()) => {
                                entry.status = "Finished".into();
                                entry.progress = "Complete".into();
                                entry.download_progress = 1.0;
                                entry.error.clear();
                                entry.completed_at = unix_timestamp();
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
                                self.save_staging_if_changed();
                            }
                        }
                        self.start_next_download();
                        if self.downloads.is_empty() {
                            self.notify("All current downloads are complete", "", "");
                        }
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
                Event::Title(id, result, notify_when_ready) => {
                    let title = match result {
                        Ok(title) if !title.trim().is_empty() => title.trim().to_string(),
                        _ => {
                            if notify_when_ready {
                                if let Some(url) = self
                                    .staging
                                    .iter()
                                    .find(|entry| entry.id == id)
                                    .map(|entry| entry.url.clone())
                                {
                                    self.notify("Download staged", &url, &url);
                                }
                            }
                            continue;
                        }
                    };
                    let in_history = self.history.iter().any(|entry| entry.id == id);
                    let is_staged = self.staging.iter().any(|entry| entry.id == id);
                    let mut staged_url = None;
                    for entry in self
                        .staging
                        .iter_mut()
                        .chain(&mut self.downloads)
                        .chain(&mut self.history)
                    {
                        if entry.id == id {
                            entry.title.clone_from(&title);
                            if is_staged {
                                staged_url = Some(entry.url.clone());
                            }
                        }
                    }
                    if let Some(url) = staged_url {
                        if notify_when_ready {
                            self.notify("Download staged", &title, &url);
                        }
                        self.save_staging_if_changed();
                    }
                    if in_history {
                        self.save_history();
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
        let closing = context.input(|input| input.viewport().close_requested());
        self.reap_notification_processes(closing);
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
        self.save_staging_if_changed();
        context.request_repaint_after(Duration::from_millis(300));
    }
}

pub(crate) fn run() -> eframe::Result<()> {
    let mut arguments = std::env::args().skip(1);
    if arguments.next().as_deref() == Some("--notification") {
        let delay_ms = arguments
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_default();
        let display_seconds = arguments
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(6)
            .clamp(1, 60);
        let color_scheme = arguments.next().unwrap_or_else(|| "Blue".into());
        let heading = arguments.next().unwrap_or_else(|| "yt-dlp UI".into());
        let title = arguments.next().unwrap_or_default();
        let url = arguments.next().unwrap_or_default();
        thread::sleep(Duration::from_millis(delay_ms));
        return run_notification(heading, title, url, display_seconds, color_scheme);
    }

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

struct NotificationPopup {
    heading: String,
    title: String,
    url: String,
    display_time: Duration,
    color_scheme: String,
    app_icon: egui::TextureHandle,
    started_at: Instant,
}

impl eframe::App for NotificationPopup {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        let elapsed = self.started_at.elapsed();
        if elapsed >= self.display_time || context.input(|input| input.viewport().close_requested())
        {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let monitor_size = context.input(|input| {
            input
                .viewport()
                .monitor_size
                .unwrap_or(egui::vec2(800.0, 600.0))
        });
        let window_size = egui::vec2(460.0, 170.0);
        let target_x = (monitor_size.x - window_size.x - 24.0).max(12.0);
        let target_y = (monitor_size.y - window_size.y - 24.0).max(12.0);
        let slide_duration = Duration::from_millis(450);
        let entering = (elapsed.as_secs_f32() / slide_duration.as_secs_f32()).clamp(0.0, 1.0);
        let remaining = self.display_time.saturating_sub(elapsed);
        let leaving = (remaining.as_secs_f32() / slide_duration.as_secs_f32()).clamp(0.0, 1.0);
        let progress = entering.min(leaving);
        let eased_progress = progress * progress * (3.0 - 2.0 * progress);
        let x = target_x + (monitor_size.x + 12.0 - target_x) * (1.0 - eased_progress);
        context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            x, target_y,
        )));

        let (background, border, accent) = notification_colors(&self.color_scheme);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(background)
                    .stroke(egui::Stroke::new(2.0_f32, border))
                    .corner_radius(12.0_f32)
                    .inner_margin(16.0),
            )
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.image((self.app_icon.id(), egui::vec2(30.0, 30.0)));
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("yt-dlp UI").size(16.0).strong());
                        ui.label(
                            egui::RichText::new(&self.heading)
                                .size(14.0)
                                .color(accent)
                                .strong(),
                        );
                    });
                });
                if !self.title.is_empty() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(&self.title).size(18.0).strong());
                }
                if !self.url.is_empty() {
                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&self.url)
                                .size(13.0)
                                .color(egui::Color32::from_gray(190)),
                        )
                        .wrap(),
                    );
                }
            });

        let animating = elapsed < slide_duration || remaining < slide_duration;
        context.request_repaint_after(if animating {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(100)
        });
    }
}

fn run_notification(
    heading: String,
    title: String,
    url: String,
    display_seconds: u64,
    color_scheme: String,
) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("yt-dlp UI notification")
            .with_inner_size([460.0, 170.0])
            .with_position([0.0, 0.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_always_on_top()
            .with_active(true)
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "yt-dlp UI notification",
        options,
        Box::new(move |creation_context| {
            let image = image::load_from_memory(include_bytes!("../resources/yt-dlp-ui.png"))
                .expect("the embedded application icon must be a valid PNG")
                .into_rgba8();
            let [width, height] = [image.width() as usize, image.height() as usize];
            let app_icon = creation_context.egui_ctx.load_texture(
                "yt-dlp-ui-notification-icon",
                egui::ColorImage::from_rgba_unmultiplied([width, height], image.as_raw()),
                egui::TextureOptions::LINEAR,
            );
            Ok(Box::new(NotificationPopup {
                heading,
                title,
                url,
                display_time: Duration::from_secs(display_seconds),
                color_scheme,
                app_icon,
                started_at: Instant::now(),
            }))
        }),
    )
}

fn notification_colors(scheme: &str) -> (egui::Color32, egui::Color32, egui::Color32) {
    match scheme {
        "Green" => (
            egui::Color32::from_rgb(20, 36, 31),
            egui::Color32::from_rgb(64, 190, 130),
            egui::Color32::from_rgb(128, 230, 170),
        ),
        "Purple" => (
            egui::Color32::from_rgb(32, 26, 42),
            egui::Color32::from_rgb(166, 120, 235),
            egui::Color32::from_rgb(205, 170, 255),
        ),
        "Amber" => (
            egui::Color32::from_rgb(42, 34, 22),
            egui::Color32::from_rgb(235, 170, 60),
            egui::Color32::from_rgb(255, 205, 110),
        ),
        _ => (
            egui::Color32::from_rgb(22, 29, 42),
            egui::Color32::from_rgb(72, 160, 240),
            egui::Color32::from_rgb(116, 190, 255),
        ),
    }
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
