use crate::model::{Config, Kind, SPECS, Spec};
use eframe::egui;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read},
    sync::mpsc::Sender,
    thread,
};

pub(crate) fn render_specs(ui: &mut egui::Ui, values: &mut BTreeMap<String, String>) {
    for spec in SPECS {
        let value = values.entry(spec.key.into()).or_insert_with(|| {
            Config::default()
                .settings
                .get(spec.key)
                .cloned()
                .unwrap_or_default()
        });
        ui.horizontal(|ui| {
            ui.add_sized([190.0, 22.0], egui::Label::new(spec.label));
            setting_editor(ui, value, spec);
            ui.label(spec.help);
        });
    }
}

pub(crate) fn render_overrides(
    ui: &mut egui::Ui,
    overrides: &mut BTreeMap<String, String>,
    baseline: &BTreeMap<String, String>,
) {
    for spec in SPECS {
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
            ui.add_enabled_ui(enabled, |ui| setting_editor(ui, &mut value, spec));
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
    }
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
                .selected_text(value.as_str())
                .show_ui(ui, |ui| {
                    for item in *items {
                        ui.selectable_value(value, item.to_string(), *item);
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

pub(crate) fn pump<R: Read + Send + 'static>(stream: R, tx: Sender<crate::model::Event>, id: u64) {
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().flatten() {
            let _ = tx.send(crate::model::Event::Line(id, line));
        }
    });
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
