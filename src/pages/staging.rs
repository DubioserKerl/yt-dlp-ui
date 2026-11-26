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
    let format_states = &app.format_states;
    let mut remove_id = None;
    let mut start_ids = Vec::new();
    let mut format_load_ids = Vec::new();
    for entry in app.staging.iter_mut() {
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
                if ui.button("Download").clicked() {
                    start_ids.push(entry.id);
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
            let override_section = ui.collapsing("Override arguments for this download", |ui| {
                let scope = egui::Id::new(("download-overrides", entry.id));
                render_overrides(
                    ui,
                    &mut entry.overrides,
                    &baseline,
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
        start_ids = app.staging.iter().map(|entry| entry.id).collect();
    }
    for id in start_ids {
        if let Some(index) = app.staging.iter().position(|entry| entry.id == id) {
            app.start_download(index);
        }
    }
    for id in format_load_ids {
        app.load_formats(id);
    }
}
