mod png;

use std::path::Path;

use anyhow::{bail, Result};

use self::png::PngDecoder;

pub trait ImageDecoder {
    /// Check if a path can be handled by the source
    /// e.g. is it a file with a specific extension, etc.
    fn item_matches(path: &Path) -> bool
    where
        Self: Sized;

    /// Decode an image
    fn decode(bytes: &[u8]) -> Result<DecodedImage>
    where
        Self: Sized;
}

pub struct DecodedImage {
    pub rgb8_pixels: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

pub fn is_image_supported(filename: &Path) -> bool {
    PngDecoder::item_matches(filename)
}

pub fn decode_image(filename: &Path, raw: &[u8]) -> Result<DecodedImage> {
    if PngDecoder::item_matches(filename) {
        PngDecoder::decode(raw)
    } else {
        bail!("Unsupported image type provided");
    }
}
