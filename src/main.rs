mod cmd;
mod gap_vec;
mod img_sources;
mod settings;
mod ui;

use clap::Parser;
use eframe::NativeOptions;

use self::{
    cmd::Args,
    ui::{app::ReaderApp, show_err_dialog},
};

fn main() -> eframe::Result<()> {
    let Args {
        path,
        // double_page,
        // right_to_left,
    } = Args::parse();

    eframe::run_native(
        "reader",
        // There are problems with fullscreen, the settings below allow to reproduce
        // a borderless fullscreen window without any of the other problems
        NativeOptions {
            decorated: false,
            maximized: true,
            ..Default::default()
        },
        Box::new(|cc| match ReaderApp::new(cc, path) {
            Ok(app) => Box::new(app),
            Err(err) => {
                show_err_dialog(err);
                std::process::exit(1);
            }
        }),
    )
}
