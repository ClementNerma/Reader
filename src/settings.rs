use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Settings {
    pub right_to_left: bool,
    pub double_page: bool,
    pub display_pages_number: bool,
}
