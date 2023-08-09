use std::path::Path;

use anyhow::{bail, Result};

use super::ImageSource;

/// An empty set of images
/// Useful when no real source is opened
pub struct EmptySource;

impl EmptySource {
    pub fn new() -> Self {
        Self
    }
}

impl ImageSource for EmptySource {
    fn item_matches(_: &Path) -> bool
    where
        Self: Sized,
    {
        false
    }

    fn load(_: &Path) -> Result<Self>
    where
        Self: Sized,
    {
        bail!("Empty sources cannot be loaded from a path");
    }

    fn total_pages(&self) -> usize {
        0
    }

    fn load_page(&mut self, _: usize) -> Result<Vec<u8>> {
        bail!("Cannot load any page from an empty source")
    }

    fn quick_clone(&self) -> Box<dyn ImageSource>
    where
        Self: Sized,
    {
        Box::new(Self)
    }
}
