#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![warn(unused_crate_dependencies)]
// Don't display terminal when launching the program on Windows
#![windows_subsystem = "windows"]

use std::path::PathBuf;

use anyhow::anyhow;
// Required for image decoding support
use image as _;

mod gap_vec;
mod img_sources;
mod settings;
mod ui;

use eframe::NativeOptions;

use self::ui::{app::ReaderApp, show_err_dialog};

fn main() -> eframe::Result<()> {
    let Some(path) = std::env::args().nth(1) else {
        show_err_dialog(anyhow!("Please open a comic with this program"));
        std::process::exit(1);
    };

    let path = PathBuf::from(path);

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
