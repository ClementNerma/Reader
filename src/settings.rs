use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub right_to_left: bool,
    pub double_page: bool,
    pub display_pages_number: bool,
    pub display_first_page_in_single_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            right_to_left: false,
            double_page: false,
            display_pages_number: true,
            display_first_page_in_single_mode: true,
        }
    }
}
