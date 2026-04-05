//! A module for handling media URLs extracted from yt-dlp, including parsing and error handling.

use pyo3::{
    CastError, PyErr,
    exceptions::PyValueError,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyListMethods},
};

use crate::yt_dlp::Format;

/// A wrapper around yt-dlp to extract metadata and download media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaUrl {
    /// Separate URLs for video and audio streams, which can be downloaded and merged together.
    Separate {
        /// The URL for the video stream, which may be in a format like MP4 or WebM.
        video_url: String,
        /// The URL for the audio stream, which may be in a format like M4A or Opus.
        audio_url: String,
    },
}

impl MediaUrl {
    /// Parses a string output from yt-dlp into a `MediaUrl` enum variant, handling the expected format for separate video and audio URLs.
    pub fn from_stdout(stdout: &str, _format: Format) -> Result<Self, MediaUrlError> {
        let mut url_lines = stdout.lines();

        let first_url = url_lines.next().ok_or(MediaUrlError::InvalidFormat)?.trim();

        if first_url.is_empty() {
            return Err(MediaUrlError::InvalidFormat);
        }

        if let Some(second_line) = url_lines.next() {
            let second_url = second_line.trim();

            if second_url.is_empty() {
                return Err(MediaUrlError::InvalidFormat);
            }

            if let Some(third_line) = url_lines.next()
                && !third_line.trim().is_empty()
            {
                return Err(MediaUrlError::InvalidFormat);
            }

            Ok(MediaUrl::Separate {
                video_url: first_url.to_string(),
                audio_url: second_url.to_string(),
            })
        } else {
            Err(MediaUrlError::InvalidFormat)
        }
    }

    pub fn from_python_list<'a, T: PyListMethods<'a>>(
        list: &T,
        _format: Format,
    ) -> Result<Self, MediaUrlError> {
        if list.len() != 2 {
            return Err(MediaUrlError::InvalidFormat);
        }

        let video_url_item = list.get_item(0)?;
        let audio_url_item = list.get_item(1)?;

        let video_url = video_url_item
            .cast::<PyDict>()?
            .get_item("url")?
            .ok_or(MediaUrlError::InvalidFormat)?
            .extract::<String>()?;

        let audio_url = audio_url_item
            .cast::<PyDict>()?
            .get_item("url")?
            .ok_or(MediaUrlError::InvalidFormat)?
            .extract::<String>()?;

        if video_url.is_empty() || audio_url.is_empty() {
            return Err(MediaUrlError::InvalidFormat);
        }

        Ok(MediaUrl::Separate {
            video_url,
            audio_url,
        })
    }
}

/// Errors that can occur when processing YouTube content URLs.
#[derive(Debug, derive_more::Error, derive_more::Display, derive_more::From)]
pub enum MediaUrlError {
    /// The provided URL is not in a valid format.
    #[display("The provided string does not match the expected format for content URLs.")]
    InvalidFormat,

    /// An error occurred on the Python side, such as an exception raised by yt-dlp or a failure to extract the expected data.
    #[display("An error occurred on the Python side: {}", _0)]
    Python(PyErr),
}

impl From<CastError<'_, '_>> for MediaUrlError {
    fn from(value: CastError) -> Self {
        Self::Python(value.into())
    }
}

impl From<MediaUrlError> for PyErr {
    fn from(value: MediaUrlError) -> Self {
        match value {
            MediaUrlError::InvalidFormat => PyErr::new::<PyValueError, _>(value.to_string()),
            MediaUrlError::Python(py_err) => py_err,
        }
    }
}
