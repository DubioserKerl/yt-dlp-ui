use crate::model::{Config, Kind, OptionCategory, SPECS, Spec, VideoFormat};
use eframe::egui;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc::Sender},
    thread,
};

pub(crate) fn render_specs(
    ui: &mut egui::Ui,
    values: &mut BTreeMap<String, String>,
    scope: egui::Id,
) {
    render_option_sections(ui, scope, true, |ui, spec| {
        let value = values.entry(spec.key.into()).or_insert_with(|| {
            Config::default()
                .settings
                .get(spec.key)
                .cloned()
                .unwrap_or_default()
        });
        ui.horizontal(|ui| {
            ui.add_sized([190.0, 22.0], egui::Label::new(spec.label));
            if spec.key == "format" {
                render_format_selector(ui, value, &common_format_presets(), scope);
            } else {
                setting_editor(ui, value, spec);
            }
            ui.label(spec.help);
        });
    });
}

pub(crate) fn render_overrides(
    ui: &mut egui::Ui,
    overrides: &mut BTreeMap<String, String>,
    baseline: &BTreeMap<String, String>,
    scope: egui::Id,
    available_formats: &[VideoFormat],
) {
    render_option_sections(ui, scope, false, |ui, spec| {
        let inherited = baseline.get(spec.key).cloned().unwrap_or_default();
        let mut enabled = overrides.contains_key(spec.key);
        let mut value = overrides
            .get(spec.key)
            .cloned()
            .unwrap_or_else(|| inherited.clone());
        ui.horizontal(|ui| {
            ui.add_sized([190.0, 22.0], egui::Label::new(spec.label));
            if ui.checkbox(&mut enabled, "Override").changed() && enabled {
                value = inherited.clone();
            }
            ui.add_enabled_ui(enabled, |ui| {
                if spec.key == "format" {
                    render_format_selector(ui, &mut value, available_formats, scope);
                } else {
                    setting_editor(ui, &mut value, spec);
                }
            });
            if !enabled {
                let inherited_label = if inherited.is_empty() {
                    "(empty)"
                } else {
                    &inherited
                };
                ui.weak(format!("Inherited: {inherited_label}"));
            }
            ui.label(spec.help);
        });
        if enabled {
            overrides.insert(spec.key.into(), value);
        } else {
            overrides.remove(spec.key);
        }
    });
}

fn render_format_selector(
    ui: &mut egui::Ui,
    value: &mut String,
    choices: &[VideoFormat],
    scope: egui::Id,
) {
    let selected = choices
        .iter()
        .find(|choice| choice.id == *value)
        .map(|choice| choice.label.as_str())
        .unwrap_or_else(|| {
            if value.is_empty() {
                "Select a format"
            } else {
                value
            }
        });
    egui::ComboBox::from_id_salt((scope, "format-selector"))
        .selected_text(selected)
        .width(300.0)
        .show_ui(ui, |ui| {
            if choices.is_empty() {
                ui.weak("Format list loads when this override is opened");
            }
            for choice in choices {
                ui.selectable_value(value, choice.id.clone(), &choice.label);
            }
        });
    ui.add(egui::TextEdit::singleline(value).desired_width(240.0));
}

fn common_format_presets() -> Vec<VideoFormat> {
    [
        (
            "bestvideo+bestaudio/best",
            "Best video + best audio (fallback to best)",
        ),
        ("best", "Best single-file format"),
        ("bv*+ba/b", "Best video and audio (fallback to best)"),
        ("b", "Best combined format"),
        ("worst", "Worst available format"),
    ]
    .into_iter()
    .map(|(id, label)| VideoFormat {
        id: id.into(),
        label: label.into(),
    })
    .collect()
}

pub(crate) fn parse_format_list(output: &str) -> Vec<VideoFormat> {
    let mut in_table = false;
    let mut formats = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        let columns: Vec<_> = trimmed.split_whitespace().collect();
        let heading = columns
            .iter()
            .map(|column| column.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let has_header = (heading.len() >= 2 && heading[0] == "id" && heading[1] == "ext")
            || (heading.len() >= 2 && heading[0] == "format" && heading[1] == "extension")
            || (heading.len() >= 3
                && heading[0] == "format"
                && heading[1] == "code"
                && heading[2] == "extension");
        if has_header {
            in_table = true;
            continue;
        }
        if !in_table || columns.len() < 2 {
            continue;
        }
        let format_id = columns[0];
        if format_id.chars().all(|character| character == '-') {
            continue;
        }
        if !format_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
        {
            continue;
        }
        if formats
            .iter()
            .any(|format: &VideoFormat| format.id == format_id)
        {
            continue;
        }
        formats.push(VideoFormat {
            id: format_id.into(),
            label: trimmed.into(),
        });
    }
    formats
}

fn render_option_sections(
    ui: &mut egui::Ui,
    scope: egui::Id,
    default_open: bool,
    mut render_option: impl FnMut(&mut egui::Ui, &Spec),
) {
    for category in OptionCategory::HELP_ORDER {
        if !SPECS.iter().any(|spec| spec.category() == category) {
            continue;
        }
        egui::CollapsingHeader::new(category.title())
            .id_salt((scope, category.id()))
            .default_open(default_open)
            .show(ui, |ui| {
                for spec in SPECS.iter().filter(|spec| spec.category() == category) {
                    render_option(ui, spec);
                }
            });
    }
}

pub(crate) fn render_extra_args(
    ui: &mut egui::Ui,
    extra_args: &mut String,
    scope: egui::Id,
    default_open: bool,
) {
    egui::CollapsingHeader::new("Additional arguments")
        .id_salt((scope, "additional-arguments"))
        .default_open(default_open)
        .show(ui, |ui| {
            ui.text_edit_singleline(extra_args);
        });
}

fn setting_editor(ui: &mut egui::Ui, value: &mut String, spec: &Spec) {
    match &spec.kind {
        Kind::Bool => {
            let mut checked = value == "true";
            if ui.checkbox(&mut checked, "").changed() {
                *value = checked.to_string();
            }
        }
        Kind::Choice(items) => {
            egui::ComboBox::from_id_salt(spec.key)
                .selected_text(if value.is_empty() {
                    "Keep source container"
                } else {
                    value.as_str()
                })
                .show_ui(ui, |ui| {
                    for item in *items {
                        ui.selectable_value(
                            value,
                            item.to_string(),
                            if item.is_empty() {
                                "Keep source container"
                            } else {
                                item
                            },
                        );
                    }
                });
        }
        Kind::Path => {
            ui.text_edit_singleline(value);
            if ui.button("…").clicked() {
                let selected = if spec.key == "paths" {
                    rfd::FileDialog::new().pick_folder()
                } else {
                    rfd::FileDialog::new().pick_file()
                };
                if let Some(path) = selected {
                    *value = path.display().to_string();
                }
            }
        }
        Kind::Number => {
            let mut number = value.parse::<u32>().unwrap_or(0);
            if ui
                .add(egui::DragValue::new(&mut number).range(0..=100_000))
                .changed()
            {
                *value = number.to_string();
            }
        }
        Kind::Text => {
            ui.add(egui::TextEdit::singleline(value).desired_width(300.0));
        }
    }
}

#[derive(Clone)]
pub(crate) struct OutputFile {
    path: PathBuf,
    existed_before_download: bool,
}

pub(crate) fn pump_output<R: Read + Send + 'static>(
    stream: R,
    tx: Sender<crate::model::Event>,
    id: u64,
    output_files: Arc<Mutex<Vec<OutputFile>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().flatten() {
            if let Some(path) = destination_from_line(&line) {
                let existed_before_download = path.exists();
                if let Ok(mut files) = output_files.lock() {
                    if !files.iter().any(|file| file.path == path) {
                        files.push(OutputFile {
                            path,
                            existed_before_download,
                        });
                    }
                }
            }
            let _ = tx.send(crate::model::Event::Line(id, line));
        }
    })
}

fn destination_from_line(line: &str) -> Option<PathBuf> {
    let (_, destination) = line.split_once("Destination:")?;
    let destination = destination.trim();
    (!destination.is_empty()).then(|| PathBuf::from(destination))
}

pub(crate) fn cleanup_partial_files(output_files: &[OutputFile], no_part: bool) -> Vec<String> {
    let mut failures = Vec::new();
    for output in output_files {
        let mut partial = output.path.as_os_str().to_os_string();
        partial.push(".part");
        remove_file_if_present(Path::new(&partial), &mut failures);

        let mut state_file = output.path.as_os_str().to_os_string();
        state_file.push(".ytdl");
        remove_file_if_present(Path::new(&state_file), &mut failures);

        if let Some(filename) = output.path.file_name() {
            let parent = output
                .path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let fragment_prefix = format!("{}.part-Frag", filename.to_string_lossy());
            if let Ok(entries) = std::fs::read_dir(parent) {
                for entry in entries.flatten() {
                    if entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(&fragment_prefix)
                    {
                        remove_file_if_present(&entry.path(), &mut failures);
                    }
                }
            }
        }

        if no_part && !output.existed_before_download {
            remove_file_if_present(&output.path, &mut failures);
        }
    }
    failures
}

fn remove_file_if_present(path: &Path, failures: &mut Vec<String>) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => failures.push(format!("{}: {error}", path.display())),
    }
}

pub(crate) fn is_youtube_url(value: &str) -> bool {
    let lower = value
        .trim()
        .trim_matches(|c: char| matches!(c, '<' | '>' | '"' | '\'' | ',' | ')' | ']'))
        .to_lowercase();
    let valid_scheme = lower.starts_with("https://") || lower.starts_with("http://");
    let valid_host = [
        "youtube.com/",
        "www.youtube.com/",
        "m.youtube.com/",
        "youtu.be/",
        "music.youtube.com/",
        "youtube-nocookie.com/",
    ]
    .iter()
    .any(|host| {
        lower
            .split_once("://")
            .is_some_and(|(_, rest)| rest.starts_with(host))
    });
    valid_scheme
        && valid_host
        && (lower.contains("watch?v=")
            || lower.contains("youtu.be/")
            || lower.contains("/playlist?")
            || lower.contains("/shorts/")
            || lower.contains("/live/"))
}

pub(crate) fn canonical(url: &str) -> String {
    url.trim_end_matches('/').to_lowercase()
}

pub(crate) fn parse_command_line(command: &str) -> Result<Vec<String>, String> {
    let command = strip_outer_quotes(command.trim());
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut token_started = false;

    for character in command.chars() {
        match (quote, character) {
            (Some(active), character) if active == character => quote = None,
            (Some('\''), _) | (Some('"'), _) => current.push(character),
            (Some(_), character) => current.push(character),
            (None, '\'' | '"') => {
                quote = Some(character);
                token_started = true;
            }
            (None, character) if character.is_whitespace() => {
                if token_started {
                    arguments.push(std::mem::take(&mut current));
                    token_started = false;
                }
            }
            (None, character) => {
                current.push(character);
                token_started = true;
            }
        }
    }

    if let Some(active_quote) = quote {
        let kind = if active_quote == '\'' {
            "single"
        } else {
            "double"
        };
        return Err(format!("Unterminated {kind} quote in command line."));
    }
    if token_started {
        arguments.push(current);
    }
    if arguments.is_empty() {
        return Err("Enter a yt-dlp command line to import.".into());
    }
    Ok(arguments)
}

fn strip_outer_quotes(command: &str) -> &str {
    let Some(outer_quote) = command.chars().next() else {
        return command;
    };
    if !matches!(outer_quote, '\'' | '"') || command.chars().last() != Some(outer_quote) {
        return command;
    }
    if command
        .chars()
        .skip(1)
        .take_while(|character| *character != outer_quote)
        .count()
        == command.chars().count().saturating_sub(2)
    {
        &command[outer_quote.len_utf8()..command.len() - outer_quote.len_utf8()]
    } else {
        command
    }
}

pub(crate) fn progress_fraction(progress: &str) -> f32 {
    if let Some(percent_index) = progress.find('%') {
        let start = progress[..percent_index]
            .rfind(|c: char| !c.is_ascii_digit() && c != '.')
            .map(|index| index + 1)
            .unwrap_or(0);
        return progress[start..percent_index]
            .parse::<f32>()
            .unwrap_or(0.0)
            .clamp(0.0, 100.0)
            / 100.0;
    }
    0.0
}
