use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::decoders::is_image_supported;

use super::ImageSource;

/// Handler for directory of images
#[derive(Clone)]
pub struct ImageDirectory {
    image_files: Vec<PathBuf>,
}

impl ImageSource for ImageDirectory {
    fn item_matches(path: &Path) -> bool
    where
        Self: Sized,
    {
        path.is_dir()
    }

    fn load(path: &Path) -> Result<Self>
    where
        Self: Sized,
    {
        assert!(Self::item_matches(path));

        let items = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;

        let mut image_files = items
            .into_iter()
            .filter_map(|item| {
                let path = item.path();

                if path.is_file() && is_image_supported(&path) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        image_files.sort();

        Ok(Self { image_files })
    }

    fn total_pages(&self) -> usize {
        self.image_files.len()
    }

    fn load_page(&mut self, page: usize) -> Result<(PathBuf, Vec<u8>), String> {
        let page_path = self
            .image_files
            .get(page)
            .ok_or_else(|| format!("Page {page} was not found"))?;

        fs::read(page_path)
            .map(|page| (page_path.to_owned(), page))
            .map_err(|err| format!("Failed to load file for page {page}: {err}"))
    }

    fn quick_clone(&self) -> Result<Box<dyn ImageSource>>
    where
        Self: Sized,
    {
        Ok(Box::new(self.clone()))
    }
}
