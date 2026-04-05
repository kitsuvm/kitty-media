//! Remote component parsing and handling for yt-dlp.

use std::str::FromStr;

/// Remote components that can be used with yt-dlp.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display, Default)]
pub enum RemoteComponents {
    #[display("ejs:github")]
    EjsGitHub,
    #[display("ejs:npm")]
    EjsNpm,
    #[display("none")]
    #[default]
    None,
}

impl RemoteComponents {
    /// Converts the [`RemoteComponents`] enum variant into an optional string argument that can be passed to yt-dlp.
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
            "ejs:github" => Ok(RemoteComponents::EjsGitHub),
            "ejs:npm" => Ok(RemoteComponents::EjsNpm),
            "none" => Ok(RemoteComponents::None),
            _ => Err(RemoteComponentsError::Invalid),
        }
    }
}

/// Errors that can occur when processing remote components for yt-dlp.
#[derive(Debug, derive_more::Error, derive_more::Display, Clone, PartialEq, Eq)]
pub enum RemoteComponentsError {
    /// The provided string does not match any valid remote components options.
    #[display("The provided string does not match any valid remote components options.")]
    Invalid,
}
