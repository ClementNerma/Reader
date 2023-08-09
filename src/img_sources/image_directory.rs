use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use super::{ImageSource, IMG_EXTENSIONS};

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

                if !path.is_file() {
                    return None;
                }

                let ext = path.extension()?.to_str()?;

                if !IMG_EXTENSIONS
                    .iter()
                    .any(|c| ext.to_ascii_lowercase() == c.to_ascii_lowercase())
                {
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

    fn load_page(&mut self, page: usize) -> Result<Vec<u8>> {
        let page_path = self.image_files.get(page).context("Page not found")?;
        Ok(fs::read(page_path)?)
    }

    fn quick_clone(&self) -> Box<dyn ImageSource>
    where
        Self: Sized,
    {
        Box::new(self.clone())
    }
}
