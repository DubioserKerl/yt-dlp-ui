use crate::{app::App, helpers::render_specs};
use eframe::egui;

pub(super) fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Options");
    ui.horizontal(|ui| {
        ui.label("yt-dlp executable");
        ui.text_edit_singleline(&mut app.config.executable);
        if ui.button("Browse…").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                app.config.executable = path.display().to_string();
            }
        }
    });
    ui.label("Baseline arguments");
    render_specs(ui, &mut app.config.settings);
    ui.horizontal(|ui| {
        ui.label("Additional arguments");
        ui.text_edit_singleline(&mut app.config.extra_args);
    });

    ui.separator();
    if ui.button("Save settings").clicked() {
        app.save_config();
    }
    ui.horizontal(|ui| {
        if ui.button("Import config").clicked() {
            app.import_config();
        }
        if ui.button("Export config").clicked() {
            app.export_config();
        }
    });
    ui.separator();
    ui.heading("Command line import / export");
    ui.text_edit_multiline(&mut app.command_text);
    ui.horizontal(|ui| {
        if ui.button("Import command line").clicked() {
            app.import_command();
        }
        if ui.button("Export command line").clicked() {
            app.export_command();
        }
    });
}
