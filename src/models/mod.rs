//! This module contains the different media processing options available in the application.

pub use crate::models::image::{ImageFormat, ImageOptions};

mod image;

/// Represents the different types of options available for processing media.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Options {
    /// Options for image processing.
    Image(ImageOptions),
}

/// Represents a task for processing media with specific options, input, and output.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// The options for processing the media.
    pub options: Options,
    /// The input file path for the media to be processed.
    pub input: String,
    /// The output file path for the processed media.
    pub output: String,
}
