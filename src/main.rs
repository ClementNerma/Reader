#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]
// Don't display terminal when launching the program on Windows
#![windows_subsystem = "windows"]

use std::path::PathBuf;

mod decoders;
mod gap_vec;
mod settings;
mod sources;
mod ui;

use eframe::NativeOptions;
use once_cell::sync::Lazy;

use self::ui::{app::ReaderApp, show_err_dialog};

static LOGICAL_CORES: Lazy<usize> = Lazy::new(num_cpus::get_physical);

fn main() -> eframe::Result<()> {
    let path = std::env::args().nth(1).map(PathBuf::from);

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
