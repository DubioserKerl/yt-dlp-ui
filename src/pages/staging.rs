use crate::{
    app::App,
    helpers::{render_extra_args, render_overrides},
};
use eframe::egui;

pub(super) fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Download staging");
    ui.label("YouTube links copied to the clipboard appear here. You can also paste a link below.");
    let mut url = String::new();
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut url);
        if ui.button("Add URL").clicked() {
            app.add_url(&url);
        }
    });
    let mut download_all = false;
    ui.add_enabled_ui(!app.staging.is_empty(), |ui| {
        if ui.button("Download all").clicked() {
            download_all = true;
        }
    });
    ui.separator();

    let baseline = app.config.settings.clone();
    let preset_names: Vec<_> = app.config.presets.keys().cloned().collect();
    let format_states = &app.format_states;
    let mut remove_id = None;
    let mut start_ids = Vec::new();
    let mut retry_ids = Vec::new();
    let mut format_load_ids = Vec::new();
    let mut playlist_load_ids = Vec::new();
    let mut playlist_stage_ids = Vec::new();
    for entry in app.staging.iter_mut() {
        let mut override_baseline = baseline.clone();
        if let Some(preset) = app.config.presets.get(&entry.preset) {
            override_baseline.extend(preset.settings.clone());
        }
        let format_state = format_states.get(&entry.id);
        let available_formats = match format_state {
            Some(crate::model::FormatState::Loaded(formats)) => formats.as_slice(),
            _ => &[],
        };
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(if entry.error.is_empty() {
                    "◷"
                } else {
                    "⚠ Failed"
                });
                ui.vertical(|ui| {
                    if !entry.title.is_empty() {
                        ui.strong(&entry.title);
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.label(&entry.url);
                        if entry.already_in_history {
                            ui.colored_label(
                                egui::Color32::YELLOW,
                                "⚠ Already in download history",
                            );
                        }
                    });
                });
                if entry.error.is_empty() && ui.button("Download").clicked() {
                    start_ids.push(entry.id);
                }
                if !entry.error.is_empty() && ui.button("Retry").clicked() {
                    retry_ids.push(entry.id);
                }
                if ui.button("Remove").clicked() {
                    remove_id = Some(entry.id);
                }
                match format_state {
                    Some(crate::model::FormatState::Loading) => {
                        ui.spinner();
                        ui.label("Loading formats…");
                    }
                    Some(crate::model::FormatState::Loaded(formats)) => {
                        ui.label(format!("{} formats loaded", formats.len()));
                        if ui.button("Reload formats").clicked() {
                            format_load_ids.push(entry.id);
                        }
                    }
                    Some(crate::model::FormatState::Failed(_)) | None => {
                        if ui.button("Load formats").clicked() {
                            format_load_ids.push(entry.id);
                        }
                    }
                }
            });
            if let Some(crate::model::FormatState::Failed(error)) = format_state {
                ui.colored_label(egui::Color32::RED, error);
            }
            if !entry.error.is_empty() {
                ui.colored_label(egui::Color32::RED, &entry.error);
                if ui.button("Show error").clicked() {
                    app.notice = entry.error.clone();
                }
            }
            ui.horizontal(|ui| {
                ui.label("Output folder override");
                ui.text_edit_singleline(&mut entry.output_directory);
                if ui.button("Browse…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        entry.output_directory = path.display().to_string();
                    }
                }
            });
            if app.playlist_loading.contains(&entry.id) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Loading playlist preview…");
                });
            } else if !app.playlist_previews.contains_key(&entry.id) {
                if ui.button("Preview playlist").clicked() {
                    playlist_load_ids.push(entry.id);
                }
            }
            if let Some(items) = app.playlist_previews.get(&entry.id) {
                ui.label(format!("Playlist contains {} videos:", items.len()));
                let selected = app
                    .selected_playlist_items
                    .entry(entry.id)
                    .or_insert_with(|| vec![false; items.len()]);
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for (index, item) in items.iter().enumerate() {
                            if let Some(include) = selected.get_mut(index) {
                                ui.checkbox(include, format!("{} — {}", item.title, item.url));
                            }
                        }
                    });
                if ui.button("Add selected videos to staging").clicked() {
                    playlist_stage_ids.push(entry.id);
                }
            }
            let override_section = ui.collapsing("Override arguments for this download", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Preset override");
                    egui::ComboBox::from_id_salt(("download-preset", entry.id))
                        .selected_text(if entry.preset.is_empty() {
                            "Use baseline"
                        } else {
                            &entry.preset
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut entry.preset, String::new(), "Use baseline");
                            for name in &preset_names {
                                ui.selectable_value(&mut entry.preset, name.clone(), name);
                            }
                        });
                });
                let scope = egui::Id::new(("download-overrides", entry.id));
                render_overrides(
                    ui,
                    &mut entry.overrides,
                    &override_baseline,
                    scope,
                    available_formats,
                );
                render_extra_args(ui, &mut entry.extra_args, scope, false);
            });
            if override_section.body_returned.is_some() && format_state.is_none() {
                format_load_ids.push(entry.id);
            }
        });
    }
    if let Some(id) = remove_id {
        app.remove_staging(id);
    }
    if download_all {
        app.start_all_downloads();
    } else {
        for id in start_ids {
            if let Some(index) = app.staging.iter().position(|entry| entry.id == id) {
                app.start_download(index);
            }
        }
    }
    for id in format_load_ids {
        app.load_formats(id);
    }
    for id in retry_ids {
        app.retry_download(id);
    }
    for id in playlist_load_ids {
        app.load_playlist_preview(id);
    }
    for id in playlist_stage_ids {
        app.stage_selected_playlist_items(id);
    }
}
