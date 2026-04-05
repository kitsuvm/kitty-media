//! A wrapper around yt-dlp.

use std::{io, path::PathBuf, process::Output};

use pyo3::{
    CastError,
    prelude::*,
    types::{PyDict, PyList},
};
use tokio::task;

mod format;
mod media_url;
mod remote_components;

pub use self::{format::*, media_url::*, remote_components::*};

/// A wrapper around yt-dlp to make an easier and "Rusty" interface for interacting with yt-dlp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YtDlp {
    /// An optional path to a cookies file that can be used with yt-dlp to access content that requires authentication or specific cookies.
    cookies: Option<PathBuf>,
    /// An optional remote component that can be used with yt-dlp to set ejs remote components for fetching metadata or downloading media from specific platforms.
    remote_components: RemoteComponents,
}

impl YtDlp {
    /// Creates a new instance of `YtDlp` with the specified command, cookies file, and remote components.
    pub fn new(
        remote_components: RemoteComponents,
        cookies: Option<PathBuf>,
    ) -> Result<Self, Error> {
        Python::initialize();

        Python::try_attach(|py| {
            // Necessary to import these modules to avoid "NameError: name 'base_events' is not defined" when using yt-dlp.
            let _ = py.import("asyncio").map_err(Error::Import)?;
            let _ = py.import("asyncio.base_events").map_err(Error::Import)?;

            Ok(())
        })
        .ok_or(Error::Python)
        .flatten()?;

        Ok(Self {
            cookies,
            remote_components,
        })
    }

    pub async fn get_media_url(&self, url: &str, format: Format) -> Result<MediaUrl, Error> {
        let self_clone = self.clone();
        let url = url.to_string();

        task::spawn_blocking(move || self_clone.get_media_url_blocking(&url, format)).await?
    }

    /// Executes the yt-dlp command to get the media URL for the specified URL, returning a [`MediaUrl`].
    pub fn get_media_url_blocking(&self, url: &str, format: Format) -> Result<MediaUrl, Error> {
        Python::try_attach(|py| {
            let ydl_module = py.import("yt_dlp").map_err(Error::Import)?;

            let ydl_opts = PyDict::new(py);

            ydl_opts
                .set_item("format", format.to_string())
                .map_err(Error::SetDictItem)?;

            ydl_opts
                .set_item("quiet", true)
                .map_err(Error::SetDictItem)?;

            if let Some(cookies_path) = &self.cookies {
                ydl_opts
                    .set_item("cookiefile", cookies_path.to_str().unwrap())
                    .map_err(Error::SetDictItem)?;
            }

            if let Some(remote_components_arg) = self.remote_components.as_arg() {
                ydl_opts
                    .set_item("remote_components", remote_components_arg)
                    .map_err(Error::SetDictItem)?;
            }

            let ydl_class = ydl_module.getattr("YoutubeDL").map_err(Error::GetClass)?;
            let ydl_instance = ydl_class.call1((ydl_opts,)).map_err(Error::Instance)?;

            let kwargs = PyDict::new(py);
            kwargs
                .set_item("download", false)
                .map_err(Error::SetDictItem)?;

            let info_obj = ydl_instance
                .call_method("extract_info", (url,), Some(&kwargs))
                .map_err(Error::ExtractInfo)?;

            let info_dict = info_obj
                .cast::<PyDict>()
                .map_err(|e| Error::Cast(e.into()))?;

            if let Some(requested_formats) = info_dict
                .get_item("requested_formats")
                .map_err(Error::GetDictItem)?
            {
                let formats_list = requested_formats
                    .cast::<PyList>()
                    .map_err(|e| Error::Cast(e.into()))?;

                return MediaUrl::from_python_list(formats_list, format).map_err(From::from);
            }

            Err(Error::MediaUrlParse(MediaUrlError::InvalidFormat))
        })
        .ok_or(Error::Python)
        .flatten()
    }
}

/// Errors that can occur when using the `YtDlp` wrapper.
#[derive(Debug, derive_more::Error, derive_more::From, derive_more::Display)]
pub enum Error {
    #[display("An error occurred while trying to attach to the Python interpreter.")]
    Python,

    /// An error occurred while importing the yt-dlp Python module.
    #[from(ignore)]
    #[display("An error occurred while importing the yt-dlp Python module: {}", _0)]
    Import(PyErr),

    /// An error occurred while getting the `YoutubeDL` class from the yt-dlp Python module.
    #[from(ignore)]
    #[display(
        "An error occurred while getting YoutubeDL class from yt-dlp Python module: {}",
        _0
    )]
    GetClass(PyErr),

    /// An error occurred while creating a `YoutubeDL` instance.
    #[from(ignore)]
    #[display("An error occurred while creating a YoutubeDL instance: {}", _0)]
    Instance(PyErr),

    /// An error occurred while setting options inside the Python dictionary.
    #[from(ignore)]
    #[display(
        "An error occurred while setting options inside Python dictionary: {}",
        _0
    )]
    SetDictItem(PyErr),

    /// An error occurred while getting an item from the Python dictionary.
    #[from(ignore)]
    #[display(
        "An error occurred while getting an item from the Python dictionary: {}",
        _0
    )]
    GetDictItem(PyErr),

    #[from(ignore)]
    #[display("An error occurred while casting a Python object: {}", _0)]
    Cast(PyErr),

    /// An error occurred while extracting media information with yt-dlp.
    #[from(ignore)]
    #[display(
        "An error occurred while extracting media information with yt-dlp: {}",
        _0
    )]
    ExtractInfo(PyErr),

    /// An error occurred while executing the yt-dlp command, such as the command not being found or failing to execute properly.
    #[display("An error occurred while executing the yt-dlp command: {}", _0)]
    Output(io::Error),

    /// The yt-dlp command executed successfully but returned a non-zero exit code, indicating that the command did not complete successfully.
    #[display(
        "The yt-dlp command executed successfully but returned a non-zero exit code: {}\nStandard Output: {}\nStandard Error: {}",
        _0.status,
        String::from_utf8_lossy(&_0.stdout),
        String::from_utf8_lossy(&_0.stderr)
    )]
    #[error(ignore)]
    NonSuccessfulExit(Output),

    /// The output from the yt-dlp was not in the expected format and could not be parsed into a [`MediaUrl`].
    #[display("Could not parse the output from yt-dlp into a MediaUrl: {}", _0)]
    MediaUrlParse(MediaUrlError),

    /// An error occurred while joining the asynchronous task that executes the yt-dlp command.
    #[display(
        "An error occurred while joining the asynchronous task that executes the yt-dlp command: {}",
        _0
    )]
    Join(task::JoinError),
}

impl From<CastError<'_, '_>> for Error {
    fn from(value: CastError) -> Self {
        Self::Cast(value.into())
    }
}
