use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Args {
    #[clap(help = "Path to open")]
    pub path: PathBuf,
    // #[clap(long, help = "Enable/disable double-page mode")]
    // pub double_page: Option<bool>,

    // #[clap(long, help = "Enable/disable right-to-left mode")]
    // pub right_to_left: Option<bool>,
}
