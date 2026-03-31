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
    process::exit,
    str::FromStr,
};

use actix_web::{App, HttpServer, Responder, get, web};
use rustls::ServerConfig;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Default port if no addresses are specified
const DEFAULT_PORT: u16 = 5000;

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Hello, {}!", name)
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

    let mut http_server = HttpServer::new(|| App::new().service(greet));

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
