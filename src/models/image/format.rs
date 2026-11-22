//! Image format detection and conversion options.

use std::{fmt, str::FromStr};

/// Portable Network Graphics (PNG) magic number.
const PNG_MAGIC_NUMBER: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
/// Joint Photographic Experts Group (JPEG) magic number (first variant).
const JPEG_0_MAGIC_NUMBER: [u8; 4] = [0xFF, 0xD8, 0xFF, 0xDB];
/// Joint Photographic Experts Group (JPEG) magic number (second variant).
const JPEG_1_MAGIC_NUMBER: [u8; 12] = [
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
];
/// Joint Photographic Experts Group (JPEG) magic number (third variant, first part).
const JPEG_2P0_MAGIC_NUMBER: [u8; 4] = [0xFF, 0xD8, 0xFF, 0xE1];
/// Joint Photographic Experts Group (JPEG) magic number (third variant, gap between first and second part).
const JPEG_2P0P1_MAGIC_NUMBER: usize = 2;
/// Joint Photographic Experts Group (JPEG) magic number (third variant, second part).
const JPEG_2P1_MAGIC_NUMBER: [u8; 6] = [0x45, 0x78, 0x69, 0x66, 0x00, 0x00];
/// WebP magic number (first part).
const WEBP_P0_MAGIC_NUMBER: [u8; 4] = [0x52, 0x49, 0x46, 0x46];
/// WebP magic number (gap between first and second part).
const WEBP_P0P1_MAGIC_NUMBER: usize = 4;
/// WebP magic number (second part).
const WEBP_P1_MAGIC_NUMBER: [u8; 4] = [0x57, 0x45, 0x42, 0x50];
/// AVIF magic number (gap until the first part).
const AVIF_PP0_MAGIC_NUMBER: usize = 4;
/// AVIF magic number (first part).
const AVIF_P0_MAGIC_NUMBER: [u8; 7] = [0x66, 0x74, 0x79, 0x70, 0x61, 0x76, 0x69];
/// AVIF magic number (second part, first variant).
const AVIF_P1_MAGIC_NUMBER: u8 = 0x66;
/// AVIF magic number (second part, second variant).
const AVIF_ANIMATED_P1_MAGIC_NUMBER: u8 = 0x73;
/// Graphics Interchange Format (GIF) magic number (first variant).
const GIF_0_MAGIC_NUMBER: [u8; 6] = [0x47, 0x49, 0x46, 0x38, 0x37, 0x61];
/// Graphics Interchange Format (GIF) magic number (second variant).
const GIF_1_MAGIC_NUMBER: [u8; 6] = [0x47, 0x49, 0x46, 0x38, 0x39, 0x61];
/// JPEG XL (JXL) magic number (first variant).
const JXL_0_MAGIC_NUMBER: [u8; 12] = [
    0x00, 0x00, 0x00, 0x0C, 0x4A, 0x58, 0x4C, 0x20, 0x0D, 0x0A, 0x87, 0x0A,
];
/// JPEG XL (JXL) magic number (second variant).
const JXL_1_MAGIC_NUMBER: [u8; 2] = [0xFF, 0x0A];

/// Represents the supported image formats for conversion and processing.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ImageFormat {
    /// The default image format, which is PNG.
    #[default]
    Png,
    /// Joint Photographic Experts Group (JPEG) format.
    Jpeg,
    /// WebP format.
    Webp,
    /// AVIF format.
    Avif,
    /// Graphics Interchange Format (GIF).
    Gif,
    /// JPEG XL (JXL) format.
    Jxl,
}

impl ImageFormat {
    /// Determines the image format based on the provided magic number.
    pub fn from_magic_number(magic: [u8; 12]) -> Option<Self> {
        if magic.starts_with(&PNG_MAGIC_NUMBER) {
            Some(ImageFormat::Png)
        } else if magic.starts_with(&JPEG_0_MAGIC_NUMBER)
            || magic.starts_with(&JPEG_1_MAGIC_NUMBER)
            || (magic.starts_with(&JPEG_2P0_MAGIC_NUMBER)
                && magic[(JPEG_2P0_MAGIC_NUMBER.len() + JPEG_2P0P1_MAGIC_NUMBER)..]
                    .starts_with(&JPEG_2P1_MAGIC_NUMBER))
        {
            Some(ImageFormat::Jpeg)
        } else if magic.starts_with(&WEBP_P0_MAGIC_NUMBER)
            && magic[(WEBP_P0_MAGIC_NUMBER.len() + WEBP_P0P1_MAGIC_NUMBER)..]
                .starts_with(&WEBP_P1_MAGIC_NUMBER)
        {
            Some(ImageFormat::Webp)
        } else if magic[AVIF_PP0_MAGIC_NUMBER..].starts_with(&AVIF_P0_MAGIC_NUMBER)
            && (magic[AVIF_PP0_MAGIC_NUMBER + AVIF_P0_MAGIC_NUMBER.len()] == AVIF_P1_MAGIC_NUMBER
                || magic[AVIF_PP0_MAGIC_NUMBER + AVIF_P0_MAGIC_NUMBER.len()]
                    == AVIF_ANIMATED_P1_MAGIC_NUMBER)
        {
            Some(ImageFormat::Avif)
        } else if magic.starts_with(&GIF_0_MAGIC_NUMBER) || magic.starts_with(&GIF_1_MAGIC_NUMBER) {
            Some(ImageFormat::Gif)
        } else if magic.starts_with(&JXL_0_MAGIC_NUMBER) || magic.starts_with(&JXL_1_MAGIC_NUMBER) {
            Some(ImageFormat::Jxl)
        } else {
            None
        }
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let format_str = match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Webp => "webp",
            ImageFormat::Avif => "avif",
            ImageFormat::Gif => "gif",
            ImageFormat::Jxl => "jxl",
        };

        write!(f, "{}", format_str)
    }
}

impl FromStr for ImageFormat {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "png" => Ok(ImageFormat::Png),
            "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
            "webp" => Ok(ImageFormat::Webp),
            "avif" => Ok(ImageFormat::Avif),
            "gif" => Ok(ImageFormat::Gif),
            "jxl" => Ok(ImageFormat::Jxl),
            _ => Err(()),
        }
    }
}
