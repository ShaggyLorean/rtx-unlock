#![windows_subsystem = "windows"]

mod app;
mod custom;
mod dlss5;
mod download;
mod fg;
mod game;
mod gpu;
mod nr;
mod sources;
mod steam;
mod update;
mod winutil;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([980.0, 680.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("rtx-unlock"),
        ..Default::default()
    };
    eframe::run_native(
        "rtx-unlock",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
