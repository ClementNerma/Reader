use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use zune_jpeg::zune_core::result::DecodingResult;
use zune_png::PngDecoder as ZunePngDecoder;

use super::{DecodedImage, ImageDecoder};

pub struct PngDecoder;

impl ImageDecoder for PngDecoder {
    fn item_matches(path: &Path) -> bool
    where
        Self: Sized,
    {
        let Some(ext) = path.extension() else { return false; };
        ext.to_ascii_lowercase() == "png"
    }

    fn decode(bytes: &[u8]) -> Result<DecodedImage>
    where
        Self: Sized,
    {
        let mut decoder = ZunePngDecoder::new(bytes);

        let pixels = decoder
            .decode()
            .map_err(|err| anyhow!("Failed to decode PNG buffer: {err:?}"))?;

        let pixel_bytes = match pixels {
            DecodingResult::U8(pixel_bytes) => pixel_bytes,
            DecodingResult::U16(_) => bail!("16-bit depth PNG images are not supported"),
            DecodingResult::F32(_) => bail!("Unsupported PNG bit depth"),
            _ => todo!(),
        };

        decoder
            .decode_headers()
            .map_err(|err| anyhow!("Failed to decode PNG headers: {err:?}"))?;

        let infos = decoder
            .get_info()
            .context("Missing info headers from PNG")?;

        let rgb8_pixels = if pixel_bytes.len() == infos.width * infos.height * 3 {
            pixel_bytes
        } else if pixel_bytes.len() == infos.width * infos.height {
            pixel_bytes
                .into_iter()
                .flat_map(|pixel| [pixel, pixel, pixel])
                .collect()
        } else {
            bail!(
                "Got invalid number of bytes from PNG decoding: expected a multiple of {}, got {}",
                infos.width * infos.height,
                pixel_bytes.len(),
            );
        };

        Ok(DecodedImage {
            rgb8_pixels,
            width: infos.width,
            height: infos.height,
        })
    }
}
