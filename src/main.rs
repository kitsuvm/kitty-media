//! # Kitsu/VM // Kitty Media
//!
//! A tool created to transcode and cache YouTube videos.
//!
//! ## License
//!
//! Kitty Media is a project from [KitsuVM](https://github.com/kitsuvm) for Sinabar Works, licensed under the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.html). See the [LICENSE.txt](LICENSE.txt) file for more details.

use std::{
    env,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    process::{Stdio, exit},
    str::FromStr,
};

use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use tokio::process::Command;
use tokio_util::io::ReaderStream;
use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Default port if no addresses are specified
const DEFAULT_PORT: u16 = 5000;

/// Application state shared across handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Precompiled regex for validating YouTube video IDs
    pub youtube_id_regex: regex::Regex,
}

/// Handler for the `/yt/{id}` endpoint. It validates the YouTube ID, uses `yt-dlp` to extract direct video and audio URLs, and then uses `ffmpeg` to stream the combined output back to the client as an MP4 file.
#[get("/yt/{id}")]
async fn youtube(id: web::Path<String>, app_state: web::Data<AppState>) -> impl Responder {
    if !app_state.youtube_id_regex.is_match(&id) {
        return HttpResponse::BadRequest().body("Invalid YouTube ID");
    }

    let Ok(yt_output) = Command::new("yt-dlp")
        .arg("-f")
        .arg("bestvideo[ext=mp4][vcodec^=avc],bestaudio[ext=m4a]")
        .arg("-g")
        .arg(format!("https://www.youtube.com/watch?v={id}"))
        .output()
        .await
        .inspect_err(|e| error!("Failed to execute yt-dlp: {e}"))
    else {
        return HttpResponse::InternalServerError().body("Failed to execute yt-dlp");
    };

    if !yt_output.status.success() {
        warn!(
            "yt-dlp exited with non-zero status: {}. Stderr: {}",
            yt_output.status,
            String::from_utf8_lossy(&yt_output.stderr)
        );
        return HttpResponse::InternalServerError().body("yt-dlp failed to extract URLs");
    }

    // Parse the output. yt-dlp -g with two formats prints two lines: Video URL, then Audio URL.
    let urls_str = String::from_utf8_lossy(&yt_output.stdout);
    let mut lines = urls_str.lines();

    let video_url = lines.next().unwrap_or("");
    let audio_url = lines.next().unwrap_or("");

    if video_url.is_empty() || audio_url.is_empty() {
        warn!(
            "yt-dlp did not return valid URLs. Output: {}",
            String::from_utf8_lossy(&yt_output.stdout)
        );
        return HttpResponse::InternalServerError().body("Failed to parse direct media URLs");
    }

    let Ok(mut ffmpeg) = Command::new("ffmpeg")
        .stderr(Stdio::null())
        .arg("-i")
        .arg(video_url)
        .arg("-i")
        .arg(audio_url)
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
        .inspect_err(|e| error!("Failed to spawn ffmpeg: {e}"))
    else {
        return HttpResponse::InternalServerError().body("Failed to spawn ffmpeg");
    };

    let Some(stdout) = ffmpeg.stdout.take() else {
        error!("Failed to capture ffmpeg stdout");
        return HttpResponse::InternalServerError().finish();
    };

    let stream = ReaderStream::new(stdout);

    HttpResponse::Ok()
        .content_type("video/mp4")
        .streaming(stream)
}

#[actix_web::main]
/// The main function is the entry point of the application. It initializes logging, reads configuration from environment variables, sets up the HTTP server with optional TLS and H2C support, and starts listening for incoming requests.
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("KITTY_MEDIA_LOG"))
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

    let youtube_id_regex = regex::Regex::new(r"^[a-zA-Z0-9_-]{11}$").unwrap_or_else(|e| {
        error!("Failed to compile YouTube ID regex: {e}");
        exit(1)
    });

    let app_state = web::Data::new(AppState { youtube_id_regex });

    let mut http_server =
        HttpServer::new(move || App::new().service(youtube).app_data(app_state.clone()));

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
