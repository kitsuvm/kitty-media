//! This module contains the options for image processing.

use crate::models::image::ImageFormat;

/// Represents the options for image processing.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageOptions {
    /// The desired width of the image.
    pub width: Option<u32>,
    /// The desired height of the image.
    pub height: Option<u32>,
    /// The desired format of the image.
    pub format: Option<ImageFormat>,
    /// Whether the image should be lossless.
    pub lossless: Option<bool>,
    /// The desired quality of the image.
    pub quality: Option<u8>,
    /// The desired alpha quality of the image.
    pub alpha_quality: Option<u8>,
    /// Whether to strip metadata from the image.
    pub strip_metadata: Option<bool>,
    /// The desired frame to extract from the image (for animated formats).
    pub extract_frame: Option<u32>,
}
