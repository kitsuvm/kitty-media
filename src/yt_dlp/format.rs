//! Module containing the structs and functions related to formats.

use std::fmt;

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
            Format::Separate { video, audio } => write!(f, "{}+{}", video, audio),
        }
    }
}

/// A format for specifying video properties when downloading media with yt-dlp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFormat {
    /// The container format for the video stream, which can be used to specify the desired video format when downloading media with yt-dlp.
    container: Option<Container>,
    /// The video codec used for encoding the video stream, which can be used to specify the desired video codec when downloading media with yt-dlp.
    codec: Option<VideoCodec>,
    /// The quality of the video stream, which can be used to specify the desired video quality level when downloading media with yt-dlp.
    quality: Quality,
}

impl Default for VideoFormat {
    fn default() -> Self {
        VideoFormat {
            container: Some(Container::Mp4),
            codec: Some(VideoCodec::Avc),
            quality: Quality::Best,
        }
    }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFormat {
    /// The container format for the audio stream, which can be used to specify the desired audio format when downloading media with yt-dlp.
    codec: Option<AudioCodec>,
    /// The quality of the audio stream, which can be used to specify the desired audio quality level when downloading media with yt-dlp.
    quality: Quality,
}

impl Default for AudioFormat {
    fn default() -> Self {
        AudioFormat {
            codec: Some(AudioCodec::Aac),
            quality: Quality::Best,
        }
    }
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
