use std::{
    fs::File,
    io::{self, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use zip_next::ZipArchive;

use crate::decoders::is_image_supported;

use super::ImageSource;

/// ZIP archive handler
pub struct ZipFile {
    path: PathBuf,
    archive: ZipArchive<BufReader<File>>,
    page_file_indexes: Vec<usize>,
}

impl ImageSource for ZipFile {
    fn item_matches(path: &Path) -> bool
    where
        Self: Sized,
    {
        if !path.is_file() {
            return false;
        }

        let Some(ext) = path.extension() else {
            return false;
        };

        let lower_ext = ext.to_ascii_lowercase();

        lower_ext == "zip" || lower_ext == "cbz"
    }

    fn load(path: &Path) -> Result<Self>
    where
        Self: Sized,
    {
        assert!(Self::item_matches(path));

        let file = File::open(path).context("Failed to open archive file")?;
        let buf = BufReader::new(file);

        let mut archive = ZipArchive::new(buf).context("Failed to open archive content")?;

        let mut page_files = vec![];

        for i in 0..archive.len() {
            let item = archive
                .by_index_raw(i)
                .context("Failed to read file in archive")?;

            if !item.is_file() {
                continue;
            }

            let Some(item_path) = item.enclosed_name() else {
                continue;
            };

            if is_image_supported(item_path) {
                page_files.push((i, item_path.to_path_buf()));
            }
        }

        page_files.sort_by(|(_, a), (_, b)| a.cmp(b));

        Ok(Self {
            path: path.to_owned(),
            archive,
            page_file_indexes: page_files.into_iter().map(|(i, _)| i).collect(),
        })
    }

    fn total_pages(&self) -> usize {
        self.page_file_indexes.len()
    }

    fn load_page(&mut self, page: usize) -> Result<(PathBuf, Vec<u8>), String> {
        let mut file = self
            .archive
            .by_index(self.page_file_indexes[page])
            .map_err(|err| format!("Failed to read file in archive for page {page}: {err}"))?;

        let mut out = vec![];

        io::copy(&mut file, &mut out).map_err(|err| {
            format!("Failed to read page file's content from archive for page {page}: {err}")
        })?;

        Ok((file.mangled_name(), out))
    }

    fn quick_clone(&self) -> Result<Box<dyn ImageSource>>
    where
        Self: Sized,
    {
        let clone = Self {
            path: self.path.clone(),
            archive: ZipArchive::new(BufReader::new(File::open(&self.path)?))?,
            page_file_indexes: self.page_file_indexes.clone(),
        };

        Ok(Box::new(clone))
    }
}
