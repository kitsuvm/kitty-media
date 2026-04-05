//! # Kitsu/VM // Kitty Media
//!
//! A tool created to retrieve and cache YouTube videos for streaming, using FFmpeg and yt-dlp.
//!
//! ## Using
//!
//! You will need to have [FFmpeg](https://ffmpeg.org/) and [yt-dlp](https://github.com/yt-dlp/yt-dlp) installed, and set the following environment variables:
//!
//! - `KITTY_MEDIA_LOG`: The log level for tracing. (e.g., `kitty_media=trace`, `kitty_media=info`)
//! - `KITTY_MEDIA_ADDRESSES`: The addresses to bind the server to, separated by commas. (e.g., `0.0.0.0:5000,[::]:5000`)
//! - `KITTY_MEDIA_ENABLE_H2C`: Whether to enable HTTP/2 over cleartext (H2C).
//! - `KITTY_MEDIA_CERT_PATH`: The path to the certificate file for TLS.
//! - `KITTY_MEDIA_KEY_PATH`: The path to the private key file for TLS.
//! - `KITTY_MEDIA_CACHE_DIR`: The directory to cache downloaded videos. (default disables caching)
//! - `KITTY_MEDIA_COOKIES_PATH`: The path to the cookies file for authentication.
//! - `KITTY_MEDIA_REMOTE_COMPONENTS`: The paths to remote components, separated by commas. (e.g., `ejs:github`, `ejs:npm`)
//! - `KITTY_MEDIA_MAX_CONCURRENT_DOWNLOADS`: The maximum number of concurrent downloads. (default: `128`)
//! - `KITTY_MEDIA_BUFFER_SIZE`: The size of the buffer for downloading videos. (default: `32768`)
//! - `KITTY_MEDIA_PACKETS_ON_FLY`: The number of packets to keep in flight during download. (default: `128`)
//! - `KITTY_MEDIA_FFMPEG_PATH`: The path to the FFmpeg executable. (default: `ffmpeg`)
//! - `KITTY_MEDIA_YT_DLP_PATH`: The path to the yt-dlp executable. (default: `yt-dlp`)
//!
//! Then, you can run the server executable, and it will start listening for requests in the path `/yt/{video_id}` where `{video_id}` is the ID of the YouTube video you want to retrieve and cache.
//!
//! ## License
//!
//! Kitty Media is a project from [KitsuVM](https://github.com/kitsuvm) for Sinabar Works, licensed under the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.html). See the [LICENSE.txt](LICENSE.txt) file for more details.

use std::{
    env,
    fs::{self, File},
    io::{BufWriter, Read, Write},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    path::PathBuf,
    process::{Command, Stdio, exit},
    str::FromStr,
    sync::Arc,
    thread,
    time::Duration,
};

use actix_files::NamedFile;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder, ResponseError, get, head,
    http::{
        StatusCode,
        header::{
            self, CacheControl, CacheDirective, ContentLength, ContentType, ETag, EntityTag,
            HeaderName,
        },
    },
    mime::Mime,
    web::{self, Bytes},
};
use dashmap::DashSet;
use derive_more::{Display, Error};
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use serde::Deserialize;
use tokio::{sync::mpsc, task};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::yt_dlp::{RemoteComponents, YtDlp};

mod yt_dlp;

/// Default port if no addresses are specified
const DEFAULT_PORT: u16 = 5000;

/// Number of packets to buffer in memory while streaming.
const DEFAULT_PACKETS_ON_FLY: usize = 128;

/// Size of the buffer used for reading ffmpeg output. This should be large enough to hold a few packets, but not too large to cause excessive memory usage.
const DEFAULT_BUFFER_SIZE: usize = 32 * 1024; // 32 KB

/// Maximum number of concurrent downloads to prevent resource exhaustion.
const DEFAULT_MAX_CONCURRENT_DOWNLOADS: usize = 128;

/// Background downloader state to track in-progress downloads and prevent duplicate processing of the same video ID, as well as configuration for caching and yt-dlp options.
#[derive(Debug, Clone)]
pub struct BackgroundDownloader {
    /// Set of video IDs that are currently being processed to prevent duplicate downloads.
    pub in_progress: DashSet<String>,
    /// Directory path for caching downloaded videos.
    pub cache_dir: Option<PathBuf>,
    /// How many downloads can be processed concurrently.
    pub max_concurrent_downloads: usize,
    /// Size of the buffer used for reading ffmpeg output.
    pub buffer_size: usize,
    /// Number of packets to buffer in memory while streaming.
    pub packets_on_fly: usize,
    /// Path to the ffmpeg executable, allowing for custom paths or versions.
    pub ffmpeg_path: String,
    /// Wrapper around yt-dlp to extract video and audio URLs.
    pub yt_dlp: YtDlp,
}

impl BackgroundDownloader {
    /// Checks if there is an available slot for processing a new download based on the current number of in-progress downloads and the configured maximum.
    pub fn available_slot(&self) -> bool {
        self.in_progress.len() < self.max_concurrent_downloads
    }
}

/// Struct to hold precompiled regular expressions for validating YouTube video IDs and URLs.
#[derive(Debug, Clone)]
pub struct YouTubeIdExtractor {
    /// Precompiled regex for validating YouTube video IDs.
    youtube_id_regex: regex::Regex,
    /// Precompiled regex for validating full YouTube URLs.
    youtube_url_regex: regex::Regex,
    /// Precompiled regex for validating short YouTube URLs.
    short_youtube_url_regex: regex::Regex,
}

impl YouTubeIdExtractor {
    /// Creates a new instance of `YouTubeIdExtractor` with precompiled regular expressions for validating YouTube video IDs and URLs.
    pub fn new() -> Result<Self, regex::Error> {
        Ok(Self {
            youtube_id_regex: regex::Regex::new(r"^[a-zA-Z0-9_-]{11}$")?,
            youtube_url_regex: regex::Regex::new(
                r"^(?:https?://)?(?:music\.|www\.)?youtube\.com/watch$",
            )?,
            short_youtube_url_regex: regex::Regex::new(
                r"^(?:https?://)?(?:www\.)?(?:youtu\.be/|youtube\.com/shorts/)([a-zA-Z0-9_-]{11})$",
            )?,
        })
    }

    /// Extracts the YouTube video ID from the given path and query parameters. It supports full YouTube URLs, short URLs, and direct video IDs, validating them against the precompiled regular expressions.
    pub fn extract_id(&self, path: &str, query: &YoutubeQuery) -> Option<String> {
        if self.youtube_url_regex.is_match(path) {
            debug!("Received full YouTube URL: {path}");
            query.v.clone()
        } else if let Some(captures) = self.short_youtube_url_regex.captures(path) {
            debug!("Received short YouTube URL: {path}");
            captures.get(1).map(|m| m.as_str().to_string())
        } else if self.youtube_id_regex.is_match(path) {
            debug!("Received YouTube ID: {path}");
            Some(path.to_string())
        } else {
            debug!("Invalid YouTube ID or URL: {path}");
            None
        }
    }
}

/// Application state shared across handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Precompiled regular expressions for validating YouTube video IDs and URLs.
    pub youtube_id_extractor: YouTubeIdExtractor,
    /// Background downloader state to track in-progress downloads and prevent duplicate processing of the same video ID.
    pub downloader: Arc<BackgroundDownloader>,
}

/// Custom error type for the streaming process can respond the HTTP client with appropriate error messages and status codes.
#[derive(Debug, Display, Error, Clone, Copy, PartialEq, Eq, Hash)]
enum StreamError {
    /// ffmpeg can't be executed, likely because it's not installed or not in the PATH.
    #[display("Failed to execute ffmpeg")]
    FfmpegExecute,

    /// ffmpeg executed but its stdout could not be captured, which is necessary for streaming the output to the client.
    #[display("Failed to capture ffmpeg stdout")]
    FfmpegCaptureStdout,
}

impl ResponseError for StreamError {}

/// Query parameters used by YouTube.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct YoutubeQuery {
    /// The YouTube video ID extracted from the URL query parameters.
    v: Option<String>,
}

#[head("/yt/{path:.*}")]
/// Handler for the HEAD request to the `/yt/{url}?v={video_id}` endpoint. It checks if the video is cached and responds with appropriate headers for caching and content type, without sending the actual video data.
async fn youtube_head(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<YoutubeQuery>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let id = match app_state.youtube_id_extractor.extract_id(&path, &query) {
        Some(video_id) => video_id,
        None => {
            return HttpResponse::BadRequest().body("Invalid YouTube ID or URL");
        }
    };

    let maybe_cached_path = app_state
        .downloader
        .cache_dir
        .as_ref()
        .map(|dir| dir.join(format!("{}.mp4", id)));

    if let Some(cached_path) = maybe_cached_path
        && cached_path.exists()
    {
        let file_size = match fs::metadata(&cached_path) {
            Ok(metadata) => metadata.len(),
            Err(e) => {
                error!("Failed to get metadata for cached file ({id}): {e}");
                return HttpResponse::InternalServerError().body("Failed to access cached file");
            }
        };

        let mut response = HttpResponse::Ok();

        response
            .insert_header(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(31536000), // 1 year in seconds
                CacheDirective::Extension("immutable".into(), None),
            ]))
            .insert_header(ETag(EntityTag::new_strong(id.to_string())))
            .insert_header(ContentLength(file_size as usize))
            .insert_header(ContentType("video/mp4".parse::<Mime>().unwrap()))
            .insert_header(("x-cache", "HIT"));

        if req
            .headers()
            .get(header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            == Some(&format!("\"{}\"", id))
        {
            trace!("Cache hit with matching ETag for video ID: {id}, returning 304 Not Modified");
            return response.status(StatusCode::NOT_MODIFIED).finish();
        }

        response.finish()
    } else {
        HttpResponse::Ok()
            .insert_header(CacheControl(vec![
                CacheDirective::NoCache,
                CacheDirective::NoStore,
                CacheDirective::MustRevalidate,
            ]))
            .insert_header(ContentType("video/mp4".parse::<Mime>().unwrap()))
            .insert_header(("x-cache", "MISS"))
            .finish()
    }
}

/// Handler for the `/yt/{url}?v={video_id}` endpoint. It validates the YouTube ID, uses `yt-dlp` to extract direct video and audio URLs, and then uses `ffmpeg` to stream the combined output back to the client as an MP4 file while optionally caching the result for future requests.
#[get("/yt/{path:.*}")]
async fn youtube(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<YoutubeQuery>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let id = match app_state.youtube_id_extractor.extract_id(&path, &query) {
        Some(video_id) => video_id,
        None => {
            return HttpResponse::BadRequest().body("Invalid YouTube ID or URL");
        }
    };

    let maybe_cached_path = app_state
        .downloader
        .cache_dir
        .as_ref()
        .map(|dir| dir.join(format!("{}.mp4", id)));

    if let Some(cached_path) = &maybe_cached_path
        && cached_path.exists()
    {
        if req
            .headers()
            .get(header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            == Some(&format!("\"{}\"", id))
        {
            trace!("Cache hit with matching ETag for video ID: {id}, returning 304 Not Modified");

            let file_size = match fs::metadata(cached_path) {
                Ok(metadata) => metadata.len(),
                Err(e) => {
                    error!("Failed to get metadata for cached file ({id}): {e}");
                    return HttpResponse::InternalServerError()
                        .body("Failed to access cached file");
                }
            };

            return HttpResponse::NotModified()
                .content_type("video/mp4")
                .insert_header(CacheControl(vec![
                    CacheDirective::Public,
                    CacheDirective::MaxAge(31536000), // 1 year in seconds
                    CacheDirective::Extension("immutable".into(), None),
                ]))
                .insert_header(ETag(EntityTag::new_strong(id.to_string())))
                .insert_header(ContentLength(file_size as usize))
                .insert_header(("x-cache", "HIT"))
                .finish();
        }

        info!("Cache HIT for video ID: {id}, serving cached file...");
        return match NamedFile::open_async(cached_path).await {
            Ok(file) => {
                let mut response = file.into_response(&req);

                let headers = response.headers_mut();

                headers.insert(
                    header::CACHE_CONTROL,
                    "public, max-age=31536000, immutable".parse().unwrap(),
                );
                headers.insert(header::ETAG, format!("\"{}\"", id).parse().unwrap());
                headers.insert(HeaderName::from_static("x-cache"), "HIT".parse().unwrap());

                response
            }
            Err(e) => {
                error!("Failed to open cached file ({id}): {e}");
                HttpResponse::InternalServerError().body("Failed to open cached file")
            }
        };
    }

    if !app_state.downloader.available_slot() {
        warn!("Maximum concurrent downloads reached, rejecting request for video ID: {id}");
        return HttpResponse::ServiceUnavailable()
            .body("Server is busy processing other requests. Please try again later.");
    }

    let background_downloader = app_state.downloader.clone();

    let (tx, rx) = mpsc::channel::<Result<Bytes, StreamError>>(app_state.downloader.packets_on_fly);

    let yt_dlp_output = match background_downloader
        .yt_dlp
        .get_media_url(
            &format!("https://youtube.com/watch?v={id}"),
            Default::default(),
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            error!("yt-dlp error for video ID: {id}: {e}");
            return HttpResponse::InternalServerError().body("Failed to retrieve media URL");
        }
    };

    task::spawn_blocking(move || {
        info!("Cache MISS, processing video ID: {id}");

        let (video_url, audio_url) = match yt_dlp_output {
            yt_dlp::MediaUrl::Separate {
                video_url,
                audio_url,
            } => (video_url, audio_url),
        };

        let Ok(mut ffmpeg) = Command::new(&app_state.downloader.ffmpeg_path)
            .stderr(Stdio::null())
            .arg("-i")
            .arg(video_url.as_str())
            .arg("-i")
            .arg(audio_url.as_str())
            .arg("-c:v")
            .arg("copy")
            .arg("-c:a")
            .arg("copy")
            .arg("-movflags")
            .arg("frag_keyframe+empty_moov")
            .arg("-f")
            .arg("mp4")
            .arg("pipe:1")
            .stdout(Stdio::piped())
            .spawn()
            .inspect_err(|e| error!("Failed to spawn ffmpeg ({id}): {e}"))
        else {
            tx.try_send(Err(StreamError::FfmpegExecute))
                .unwrap_or_else(|e| error!("Failed to send error response ({id}): {e}"));
            return;
        };

        let maybe_temp_cache_path = app_state
            .downloader
            .cache_dir
            .as_ref()
            .map(|dir| dir.join(format!("{}.frag.mp4", id)));

        let Some(mut stdout) = ffmpeg.stdout.take() else {
            error!("Failed to capture ffmpeg stdout for video ID: {id}");
            tx.try_send(Err(StreamError::FfmpegCaptureStdout))
                .unwrap_or_else(|e| error!("Failed to send error response ({id}): {e}"));
            return;
        };

        let cache_available = background_downloader.in_progress.insert(id.to_string());

        let mut cache = if cache_available && let Some(temp_cache_path) = maybe_temp_cache_path {
            File::create(&temp_cache_path)
                .map(BufWriter::new)
                .inspect_err(|e| warn!("Failed to create cache file ({id}): {e}"))
                .ok()
                .zip(maybe_cached_path)
                .map(|(file_writer, cached_path)| (file_writer, temp_cache_path, cached_path))
        } else {
            debug!(
                "Video ID: {id} is already being processed by another request, skipping cache..."
            );
            None
        };

        let mut buffer = vec![0u8; app_state.downloader.buffer_size];

        let mut streaming_error = false;
        let mut cache_error = false;

        loop {
            if streaming_error && cache.is_none() {
                info!(
                    "Streaming has failed and caching is disabled, stopping processing video ID: {id}"
                );
                break;
            }

            if streaming_error && (cache.is_some() && cache_error) {
                warn!("Both streaming and caching have failed, stopping processing video ID: {id}");
                break;
            }

            match stdout.read(&mut buffer) {
                Ok(0) => {
                    match ffmpeg.try_wait() {
                        Ok(Some(_)) => {}
                        Ok(None) => {
                            warn!("FFmpeg is too slow for video ID: {id}");
                            thread::sleep(Duration::from_secs(1));
                            continue;
                        }
                        Err(e) => {
                            error!("Error waiting for ffmpeg process to exit ({id}): {e}");
                            break;
                        }
                    }

                    info!("Completed streaming for video ID: {id}");

                    if let Some((mut file_writer, temp_cache_path, cached_path)) = cache.take() {
                        if let Err(e) = file_writer.flush() {
                            warn!("Failed to flush cache file ({id}): {e}");
                        }

                        drop(file_writer);

                        match Command::new(&app_state.downloader.ffmpeg_path)
                            .arg("-i")
                            .arg(&temp_cache_path)
                            .arg("-c:v")
                            .arg("copy")
                            .arg("-c:a")
                            .arg("copy")
                            .arg("-movflags")
                            .arg("faststart")
                            .arg(cached_path)
                            .output()
                        {
                            Ok(output) => {
                                if !output.status.success() {
                                    warn!(
                                        "ffmpeg exited with non-zero status while finalizing cache ({id}): {}. Stderr: {}",
                                        output.status,
                                        String::from_utf8_lossy(&output.stderr)
                                    );
                                } else {
                                    info!("Cache file finalized for video ID: {id}");
                                }

                                if let Err(e) = fs::remove_file(&temp_cache_path) {
                                    warn!("Failed to remove temp cache file ({id}): {e}");
                                }
                            }

                            Err(e) => {
                                warn!("Failed to finalize cache file ({id}): {e}");
                            }
                        }

                        background_downloader.in_progress.remove(&id.to_string());
                    }

                    break;
                }
                Ok(n) => {
                    let chunk = web::Bytes::copy_from_slice(&buffer[..n]);

                    if !streaming_error && let Err(e) = tx.blocking_send(Ok(chunk)) {
                        warn!("Failed to send chunk ({id}): {e}");
                        streaming_error = true;
                    }

                    if let Some((file_writer, _, _)) = cache.as_mut()
                        && let Err(e) = file_writer.write_all(&buffer[..n])
                    {
                        warn!("Failed to write to cache file ({id}): {e}");

                        cache_error = true;
                        cache = None;
                    }
                }
                Err(e) => {
                    error!("Error reading from ffmpeg stdout: {e}");
                    break;
                }
            }
        }

        if let Some((file_writer, temp_cache_path, _)) = cache.take() {
            warn!(
                "Cache file for video ID: {id} may be incomplete due to streaming error, removing temp cache file"
            );

            drop(file_writer);

            if let Err(e) = fs::remove_file(&temp_cache_path) {
                warn!("Failed to remove temp cache file ({id}): {e}");
            }

            background_downloader.in_progress.remove(&id.to_string());
        }
    });

    let stream = ReceiverStream::new(rx);

    HttpResponse::Ok()
        .content_type("video/mp4")
        .insert_header(CacheControl(vec![
            CacheDirective::NoCache,
            CacheDirective::NoStore,
            CacheDirective::MustRevalidate,
        ]))
        .insert_header(("x-cache", "MISS"))
        .streaming(stream)
}

#[actix_web::main]
/// The main function is the entry point of the application. It initializes logging, reads configuration from environment variables, sets up the HTTP server with optional TLS and H2C support, and starts listening for incoming requests.
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::try_from_env("KITTY_MEDIA_LOG")
                .unwrap_or_else(|_| EnvFilter::new("kitty_media=info")),
        )
        .init();

    let addresses = env::var("KITTY_MEDIA_ADDRESSES")
        .map(|v| {
            v.split(',')
                .map(|s| {
                    SocketAddr::from_str(s.trim()).unwrap_or_else(|e| {
                        error!("Failed to parse socket address: {e}");
                        exit(1)
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|_| {
            vec![
                SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DEFAULT_PORT)),
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, DEFAULT_PORT, 0, 0)),
            ]
        });

    let enable_h2c = env::var("KITTY_MEDIA_ENABLE_H2C")
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "y" | "on"))
        .unwrap_or_default();

    let tls_cert = env::var("KITTY_MEDIA_CERT_PATH")
        .map(|s| {
            CertificateDer::pem_file_iter(s)
                .unwrap_or_else(|e| {
                    error!("Failed to load certificate: {e}");
                    exit(1)
                })
                .map(|v| {
                    v.unwrap_or_else(|e| {
                        error!("Failed to parse certificate: {e}");
                        exit(1)
                    })
                })
                .collect::<Vec<_>>()
        })
        .ok();

    let tls_key = env::var("KITTY_MEDIA_KEY_PATH")
        .map(|s| {
            PrivateKeyDer::from_pem_file(s).unwrap_or_else(|e| {
                error!("Failed to load private key: {e}");
                exit(1)
            })
        })
        .ok();

    let tls_config = tls_cert.zip(tls_key).map(|(cert, key)| {
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert, key)
            .unwrap_or_else(|e| {
                error!("Failed to create TLS config: {e}");
                exit(1)
            })
    });

    let cache_dir = env::var("KITTY_MEDIA_CACHE_DIR").ok().map(PathBuf::from);

    if let Some(cache_dir) = &cache_dir
        && let Err(e) = fs::create_dir_all(cache_dir)
    {
        error!("Failed to create cache directory: {e}");
        exit(1);
    }

    let cookies_path = env::var("KITTY_MEDIA_COOKIES_PATH").ok().map(PathBuf::from);

    let remote_components = env::var("KITTY_MEDIA_REMOTE_COMPONENTS")
        .ok()
        .map(|s| s.parse::<RemoteComponents>())
        .transpose()
        .unwrap_or_else(|e| {
            error!("Failed to parse remote components: {e}");
            exit(1)
        })
        .unwrap_or_default();

    let max_concurrent_downloads = env::var("KITTY_MEDIA_MAX_CONCURRENT_DOWNLOADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_CONCURRENT_DOWNLOADS);

    let buffer_size = env::var("KITTY_MEDIA_BUFFER_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_BUFFER_SIZE);

    let packets_on_fly = env::var("KITTY_MEDIA_PACKETS_ON_FLY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PACKETS_ON_FLY);

    let ffmpeg_path = env::var("KITTY_MEDIA_FFMPEG_PATH").unwrap_or_else(|_| "ffmpeg".to_string());

    if let Some(old_than_days) = env::var("KITTY_MEDIA_DELETE_OLD_THAN")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        && let Some(cache_dir) = cache_dir.clone()
    {
        thread::Builder::new()
            .name("cache_cleanup".to_string())
            .spawn(move || {
            info!(
                "Started background cache cleanup thread, checking for files older than {} days in {}",
                old_than_days,
                cache_dir.display()
            );
            loop {
                debug!("Starting cache cleanup cycle...");
                let dir = match fs::read_dir(&cache_dir) {
                    Ok(dir) => dir,
                    Err(e) => {
                        warn!(
                            "Failed to read cache directory, sleeping 1 minute before retrying: {e}"
                        );
                        thread::sleep(Duration::from_mins(1));
                        continue;
                    }
                };

                for entry in dir {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if let Ok(metadata) = entry.metadata() {
                                let modified_time = metadata.modified()
                                    .unwrap_or_else(|e| {
                                        warn!("Failed to get modified time for file ({path:?}): {e}");
                                        std::time::UNIX_EPOCH
                                    })
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();

                                let accessed_time = metadata.accessed()
                                    .unwrap_or_else(|e| {
                                        warn!("Failed to get accessed time for file ({path:?}): {e}");
                                        std::time::UNIX_EPOCH
                                    })
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();

                                let file_time = modified_time.max(accessed_time);

                                if file_time == 0 {
                                    warn!("File has invalid modified and accessed times, skipping: {path:?}");
                                    continue;
                                }

                                let current_time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();

                                if current_time - file_time > old_than_days * 24 * 3600 {
                                    if let Err(e) = fs::remove_file(&path) {
                                        warn!("Failed to delete old cache file ({path:?}): {e}");
                                    } else {
                                        info!("Deleted old cache file: {path:?}");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to read cache directory entry, skipping: {e}");
                        }
                    }
                }
                info!("Finished cache cleanup, sleeping for 12 hours before next cleanup...");
                thread::sleep(Duration::from_hours(12));
            }
        }).unwrap();
    } else {
        info!("No cache cleanup configured, old cache files will not be automatically deleted");
    }

    let ram_usage_per_download = buffer_size * packets_on_fly;
    let max_ram_usage = ram_usage_per_download * max_concurrent_downloads;

    if let Some(cache_dir) = &cache_dir {
        info!("Configured cache directory: {}", cache_dir.display());
    } else {
        info!("No cache directory configured, caching is disabled");
    }

    info!(
        "Configured maximum concurrent downloads: {}",
        max_concurrent_downloads
    );
    info!(
        "Buffer size: {} KB, Packets on fly: {}",
        buffer_size / 1024,
        packets_on_fly
    );
    info!(
        "Expected maximum RAM usage: {} MB / {} MB",
        ram_usage_per_download / (1024 * 1024),
        max_ram_usage / (1024 * 1024)
    );

    let youtube_id_extractor = YouTubeIdExtractor::new().unwrap_or_else(|e| {
        error!("Failed to compile YouTube ID regex: {e}");
        exit(1)
    });

    let yt_dlp = YtDlp::new(remote_components, cookies_path).unwrap_or_else(|e| {
        error!("Failed to initialize yt-dlp: {e}");
        exit(1)
    });

    let app_state = web::Data::new(AppState {
        youtube_id_extractor,
        downloader: Arc::new(BackgroundDownloader {
            in_progress: DashSet::new(),
            cache_dir,
            max_concurrent_downloads,
            buffer_size,
            packets_on_fly,
            ffmpeg_path,
            yt_dlp,
        }),
    });

    let mut http_server = HttpServer::new(move || {
        App::new()
            .service(youtube)
            .service(youtube_head)
            .app_data(app_state.clone())
    });

    if let Some(config) = tls_config {
        info!("Using HTTP/2 with TLS");
        http_server = http_server
            .bind_rustls_0_23(addresses.as_slice(), config)
            .unwrap_or_else(|e| {
                error!("Failed to bind to addresses with TLS: {e}");
                exit(1)
            });
    } else if enable_h2c {
        info!("Using HTTP/2 without TLS (H2C)");
        http_server = http_server
            .bind_auto_h2c(addresses.as_slice())
            .unwrap_or_else(|e| {
                error!("Failed to bind to addresses with H2C: {e}");
                exit(1)
            });
    } else {
        info!("Using HTTP/1.1");
        http_server = http_server.bind(addresses.as_slice()).unwrap_or_else(|e| {
            error!("Failed to bind to addresses: {e}");
            exit(1)
        });
    }

    for addr in &addresses {
        info!("Listening on {addr}");
    }

    http_server.run().await.unwrap_or_else(|e| {
        error!("Can't run server: {e}");
        exit(1)
    });
}
