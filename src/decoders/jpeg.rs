use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use zune_jpeg::JpegDecoder as ZuneJpegDecoder;

use super::{DecodedImage, ImageDecoder};

pub struct JpegDecoder;

impl ImageDecoder for JpegDecoder {
    fn item_matches(path: &Path) -> bool
    where
        Self: Sized,
    {
        let Some(ext) = path.extension() else { return false; };
        let lower_ext = ext.to_ascii_lowercase();

        lower_ext == "jpg" || lower_ext == "jpeg"
    }

    fn decode(bytes: &[u8]) -> Result<DecodedImage>
    where
        Self: Sized,
    {
        let mut decoder = ZuneJpegDecoder::new(bytes);

        let pixel_bytes = decoder
            .decode()
            .map_err(|err| anyhow!("Failed to decode JPEG buffer: {err:?}"))?;

        decoder
            .decode_headers()
            .map_err(|err| anyhow!("Failed to decode JPEG headers: {err:?}"))?;

        let infos = decoder.info().context("Missing info headers from JPEG")?;

        let width = usize::from(infos.width);
        let height = usize::from(infos.height);

        let rgb8_pixels = if pixel_bytes.len() == width * height * 3 {
            pixel_bytes
        } else if pixel_bytes.len() == width * height {
            pixel_bytes
                .into_iter()
                .flat_map(|pixel| [pixel, pixel, pixel])
                .collect()
        } else {
            bail!(
                "Got invalid number of bytes from JPEG decoding: expected a multiple of {}, got {}",
                width * height,
                pixel_bytes.len(),
            );
        };

        Ok(DecodedImage {
            rgb8_pixels,
            width,
            height,
        })
    }
}
