use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

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

                if path.is_file() && !is_image_supported(&path) {
                    return None;
                }

                Some(path)
            })
            .collect::<Vec<_>>();

        image_files.sort();

        Ok(Self { image_files })
    }

    fn total_pages(&self) -> usize {
        self.image_files.len()
    }

    fn load_page(&mut self, page: usize) -> Result<(PathBuf, Vec<u8>)> {
        let page_path = self.image_files.get(page).context("Page not found")?;
        fs::read(page_path)
            .map(|page| (page_path.to_owned(), page))
            .map_err(Into::into)
    }

    fn quick_clone(&self) -> Box<dyn ImageSource>
    where
        Self: Sized,
    {
        Box::new(self.clone())
    }
}
