mod empty;
mod image_directory;
mod zip_file;

pub use empty::EmptySource;

use std::path::Path;

use anyhow::{bail, Result};

use self::{image_directory::ImageDirectory, zip_file::ZipFile};

/// Source providing a set of images
pub trait ImageSource: Send + Sync {
    /// Check if a path can be handled by the source
    /// e.g. is it a file with a specific extension, etc.
    fn item_matches(path: &Path) -> bool
    where
        Self: Sized;

    /// Load an image set from a path
    /// Should come after a check from [`ImageSource::item_matches`]
    fn load(path: &Path) -> Result<Self>
    where
        Self: Sized;

    /// Get the total number of pages (= number of images) in the set
    fn total_pages(&self) -> usize;

    /// Load a page (= an image) as a vector of bytes
    fn load_page(&mut self, page: usize) -> Result<Vec<u8>>;

    /// Quick clone
    fn quick_clone(&self) -> Box<dyn ImageSource>;
}

/// List of supported image extensions (used for filtering)
static IMG_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];

/// Try to load a path as an image source
pub fn load_image_source(path: &Path) -> Result<Box<dyn ImageSource>> {
    macro_rules! identify_source {
        ($($source: ident),+) => {{
            $( if $source::item_matches(path) {
                return Ok(Box::new($source::load(path)?))
            } )+
        }}
    }

    identify_source!(ImageDirectory, ZipFile);
    bail!("Provided item is not supported");
}
