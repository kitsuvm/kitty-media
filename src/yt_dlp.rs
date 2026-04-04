//! A wrapper around yt-dlp to extract metadata and download media.

use std::{
    fmt, io,
    path::PathBuf,
    process::{Command, Output},
    str::FromStr,
};

use url::{ParseError, Url};

/// A wrapper around yt-dlp to extract metadata and download media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YtDlp {
    /// The command to execute yt-dlp, which can be a path to the executable or a command available in the system's PATH.
    yt_dlp_command: String,
    /// An optional path to a cookies file that can be used with yt-dlp to access content that requires authentication or specific cookies.
    cookies: Option<PathBuf>,
    /// An optional remote component that can be used with yt-dlp to set ejs remote components for fetching metadata or downloading media from specific platforms.
    remote_components: RemoteComponents,
}

impl YtDlp {
    /// Creates a new instance of `YtDlp` with the specified command, cookies file, and remote components.
    pub fn new(
        yt_dlp_command: String,
        remote_components: RemoteComponents,
        cookies: Option<PathBuf>,
    ) -> Self {
        Self {
            yt_dlp_command,
            cookies,
            remote_components,
        }
    }

    /// Creates a `Command` to execute yt-dlp with the appropriate arguments based on the configuration of the `YtDlp` instance.
    fn create_command(&self) -> Command {
        let mut yt_dlp_command = Command::new(&self.yt_dlp_command);

        if let Some(cookies) = &self.cookies {
            yt_dlp_command.arg("--cookies").arg(cookies);
        }

        if let Some(remote_components) = self.remote_components.as_arg() {
            yt_dlp_command
                .arg("--remote-components")
                .arg(remote_components);
        }

        yt_dlp_command
    }

    /// Executes the yt-dlp command to get the content URL for the specified media URL, returning a [`ContentUrl`].
    pub fn get_content_url(&self, url: &str, format: Format) -> Result<ContentUrl, Error> {
        let mut command = self.create_command();

        command
            .arg("--format")
            .arg(format.to_string())
            .arg("--get-url")
            .arg(url);

        let output = command.output()?;

        if !output.status.success() {
            return Err(output.into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        stdout.parse::<ContentUrl>().map_err(From::from)
    }
}

/// Errors that can occur when using the `YtDlp` wrapper.
#[derive(Debug, derive_more::Error, derive_more::From, derive_more::Display)]
pub enum Error {
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

    /// The output from the yt-dlp command was not in the expected format and could not be parsed into a `ContentUrl`.
    #[display(
        "The output from the yt-dlp command could not be parsed into a ContentUrl: {}",
        _0
    )]
    ContentUrlParse(ContentUrlError),
}

/// The container format of the video.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, Copy, Default)]
pub enum Container {
    /// The MP4 container format.
    #[display("mp4")]
    #[default]
    Mp4,
}

/// The video codec used for encoding the video stream.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, Copy, Default)]
pub enum VideoCodec {
    /// The AVC (Advanced Video Coding) codec, also known as H.264, which is commonly used for video compression and is widely supported across various platforms and devices.
    #[display("avc")]
    #[default]
    Avc,
}

/// The audio codec used for encoding the audio stream.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, Copy, Default)]
pub enum AudioCodec {
    /// The AAC (Advanced Audio Coding) or M4A (MPEG-4 Audio) codec.
    #[display("m4a")]
    #[default]
    Aac,
}

/// The quality of the video or audio stream, which can be used to specify the desired quality level when downloading media with yt-dlp.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, Copy, Default)]
pub enum Quality {
    /// The best available quality.
    #[display("best")]
    #[default]
    Best,
}

/// A format for specifying video properties when downloading media with yt-dlp.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VideoFormat {
    /// The container format for the video stream, which can be used to specify the desired video format when downloading media with yt-dlp.
    container: Option<Container>,
    /// The video codec used for encoding the video stream, which can be used to specify the desired video codec when downloading media with yt-dlp.
    codec: Option<VideoCodec>,
    /// The quality of the video stream, which can be used to specify the desired video quality level when downloading media with yt-dlp.
    quality: Quality,
}

impl fmt::Display for VideoFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut format_string = format!("{}video", self.quality);

        if let Some(container) = self.container {
            format_string.push_str(&format!("[ext={}]", container));
        }

        if let Some(codec) = self.codec {
            format_string.push_str(&format!("[vcodec^={}]", codec));
        }

        write!(f, "{}", format_string)
    }
}

/// A format for specifying audio properties when downloading media with yt-dlp.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudioFormat {
    /// The container format for the audio stream, which can be used to specify the desired audio format when downloading media with yt-dlp.
    codec: Option<AudioCodec>,
    /// The quality of the audio stream, which can be used to specify the desired audio quality level when downloading media with yt-dlp.
    quality: Quality,
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut format_string = format!("{}audio", self.quality);

        if let Some(codec) = self.codec {
            format_string.push_str(&format!("[ext={}]", codec));
        }

        write!(f, "{}", format_string)
    }
}

/// A format that specifies whether to download only the video stream, only the audio stream, or both streams together when using yt-dlp to download media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    /// A format that specifies both video and audio, which can be used to download both the video and audio streams of a media item with yt-dlp and merge them together.
    Separate {
        /// The video format, which can be used to specify the video stream when downloading media with yt-dlp.
        video: VideoFormat,
        /// The audio format, which can be used to specify the audio stream when downloading media with yt-dlp.
        audio: AudioFormat,
    },
}

impl Default for Format {
    fn default() -> Self {
        Format::Separate {
            video: VideoFormat::default(),
            audio: AudioFormat::default(),
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Format::Separate { video, audio } => write!(f, "{},{}", video, audio),
        }
    }
}

/// Remote components that can be used with yt-dlp.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display, Default)]
pub enum RemoteComponents {
    #[display("ejs:github")]
    GitHub,
    #[display("ejs:npm")]
    Npm,
    #[display("none")]
    #[default]
    None,
}

impl RemoteComponents {
    /// Converts the `RemoteComponents` enum variant into an optional string argument that can be passed to yt-dlp.
    pub fn as_arg(&self) -> Option<String> {
        match self {
            RemoteComponents::None => None,
            v => Some(v.to_string()),
        }
    }
}

impl FromStr for RemoteComponents {
    type Err = RemoteComponentsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ejs:github" => Ok(RemoteComponents::GitHub),
            "ejs:npm" => Ok(RemoteComponents::Npm),
            "none" => Ok(RemoteComponents::None),
            _ => Err(RemoteComponentsError::InvalidOption),
        }
    }
}

/// Errors that can occur when processing remote components for yt-dlp.
#[derive(Debug, derive_more::Error, derive_more::Display, Clone, PartialEq, Eq)]
pub enum RemoteComponentsError {
    /// The provided string does not match any valid remote component options.
    #[display("The provided string does not match any valid remote component options.")]
    InvalidOption,
}

/// A wrapper around yt-dlp to extract metadata and download media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentUrl {
    /// Separate URLs for video and audio streams, which can be downloaded and merged together.
    Separate {
        /// The URL for the video stream, which may be in a format like MP4 or WebM.
        video_url: Url,
        /// The URL for the audio stream, which may be in a format like M4A or Opus.
        audio_url: Url,
    },

    /// A single URL that can contain both video and audio streams, or only one of the streams, depending on the format of the media item and the options used with yt-dlp.
    Single(Url),
}

impl FromStr for ContentUrl {
    type Err = ContentUrlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut url_lines = s.lines();

        let first_url = url_lines
            .next()
            .ok_or(ContentUrlError::InvalidFormat)?
            .trim()
            .parse::<Url>()?;

        if let Some(second_line) = url_lines.next() {
            let second_url = second_line.trim().parse::<Url>()?;

            if let Some(third_line) = url_lines.next()
                && !third_line.trim().is_empty()
            {
                return Err(ContentUrlError::InvalidFormat);
            }

            Ok(ContentUrl::Separate {
                video_url: first_url,
                audio_url: second_url,
            })
        } else {
            Ok(ContentUrl::Single(first_url))
        }
    }
}

/// Errors that can occur when processing YouTube content URLs.
#[derive(
    Debug, derive_more::Error, derive_more::Display, Clone, PartialEq, Eq, derive_more::From,
)]
pub enum ContentUrlError {
    /// The provided URL is not in a valid format.
    #[display("The provided URL is not in a valid format.")]
    InvalidFormat,
    /// The string provided is not a valid URL.
    #[display("Failed to parse the URL: {}", _0)]
    UrlParse(ParseError),
}
