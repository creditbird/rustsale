//! A QR code as a PNG `data:` URL, for `<Image src>` in TSX: the WhatsApp login sends the code's
//! content, the screen draws it here instead of shipping a QR encoder to the UI.

use crate::error::CoreApiError;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use image::ImageEncoder as _;
use qrcode::{EcLevel, QrCode};

/// How wide one QR module is drawn, in pixels; the quiet zone is the renderer's default 4 modules.
const MODULE_PIXELS: u32 = 6;

/// `text` as a QR code, encoded to PNG and returned as `data:image/png;base64,...`.
pub(crate) fn png_data_url(text: &str) -> Result<String, CoreApiError> {
    if text.trim().is_empty() {
        return Err(CoreApiError::invalid("the QR content is empty"));
    }
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|error| CoreApiError::invalid(format!("cannot encode the QR content: {error}")))?;
    let image = code
        .render::<image::Luma<u8>>()
        .quiet_zone(true)
        .module_dimensions(MODULE_PIXELS, MODULE_PIXELS)
        .build();
    let (width, height) = image.dimensions();
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(image.as_raw(), width, height, image::ExtendedColorType::L8)
        .map_err(|error| CoreApiError::internal(format!("cannot write the QR image: {error}")))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(data_url: &str) -> Result<Vec<u8>, CoreApiError> {
        let payload = data_url
            .strip_prefix("data:image/png;base64,")
            .ok_or_else(|| CoreApiError::invalid(format!("not a PNG data URL: {data_url}")))?;
        STANDARD
            .decode(payload)
            .map_err(|error| CoreApiError::internal(format!("invalid base64: {error}")))
    }

    #[test]
    fn the_content_becomes_a_png_data_url() -> Result<(), CoreApiError> {
        let url = png_data_url("2@AbCdEfGhIjKlMnOpQrStUvWxYz+/=,1234567890")?;
        assert!(url.starts_with("data:image/png;base64,"));
        let png = decode(&url)?;
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "the bytes are a PNG");
        Ok(())
    }

    #[test]
    fn the_same_content_gives_the_same_image() -> Result<(), CoreApiError> {
        let first = png_data_url("same-content")?;
        let second = png_data_url("same-content")?;
        assert_eq!(first, second);
        Ok(())
    }

    #[test]
    fn empty_content_is_refused_before_encoding() {
        for text in ["", "   ", "\n\t"] {
            let code = png_data_url(text).err().map(|error| error.code);
            assert_eq!(
                code.as_deref(),
                Some("invalid_request"),
                "empty content must be refused: {text:?}"
            );
        }
    }

    #[test]
    fn content_too_long_for_a_qr_code_is_refused() {
        let long = "x".repeat(5_000);
        let code = png_data_url(&long).err().map(|error| error.code);
        assert_eq!(
            code.as_deref(),
            Some("invalid_request"),
            "a QR code cannot hold this"
        );
    }
}
