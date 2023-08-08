use rfd::{MessageDialog, MessageLevel};

pub mod app;

pub fn show_err_dialog(err: anyhow::Error) {
    MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title("Error")
        .set_description(&format!("{err:?}"))
        .show();
}
