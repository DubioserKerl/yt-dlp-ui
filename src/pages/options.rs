use crate::{
    app::App,
    helpers::{render_extra_args, render_specs},
};
use eframe::egui;

pub(super) fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Options");
    ui.group(|ui| {
        ui.heading("Notifications");
        ui.horizontal(|ui| {
            ui.label("Display time (seconds)");
            ui.add(
                egui::DragValue::new(&mut app.config.notification_display_seconds).range(1..=60),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Color scheme");
            egui::ComboBox::from_id_salt("notification-color-scheme")
                .selected_text(&app.config.notification_color_scheme)
                .show_ui(ui, |ui| {
                    for scheme in ["Blue", "Green", "Purple", "Amber"] {
                        ui.selectable_value(
                            &mut app.config.notification_color_scheme,
                            scheme.to_string(),
                            scheme,
                        );
                    }
                });
        });
    });

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
    render_specs(
        ui,
        &mut app.config.settings,
        egui::Id::new("baseline-options"),
    );
    render_extra_args(
        ui,
        &mut app.config.extra_args,
        egui::Id::new("baseline-extra-arguments"),
        true,
    );

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
    egui::CollapsingHeader::new("Command line import / export")
        .id_salt("command-line-tools")
        .default_open(true)
        .show(ui, |ui| {
            ui.text_edit_multiline(&mut app.command_text);
            ui.horizontal(|ui| {
                if ui.button("Import command line").clicked() {
                    app.import_command();
                }
                if ui.button("Export command line").clicked() {
                    app.export_command();
                }
            });
        });

    if let Some(error) = app.command_error.clone() {
        let mut dismiss = false;
        egui::Modal::new(egui::Id::new("command_line_import_error")).show(ui.ctx(), |ui| {
            ui.set_min_width(360.0);
            ui.heading("Could not import command line");
            ui.label(error);
            ui.add_space(8.0);
            if ui.button("OK").clicked() {
                dismiss = true;
            }
        });
        if dismiss {
            app.command_error = None;
        }
    }
}
