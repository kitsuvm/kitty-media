# Kitsu/VM // Kitty Media

A tool created to retrieve and cache YouTube videos for streaming, using FFmpeg and yt-dlp.

## Using

You will need to have [FFmpeg](https://ffmpeg.org/) and [yt-dlp](https://github.com/yt-dlp/yt-dlp) installed, and set the following environment variables to configure the server (every variable is optional, with defaults provided where applicable):

- `KITTY_MEDIA_LOG`: The log level for tracing. (e.g., `kitty_media=trace`, `kitty_media=info`)
- `KITTY_MEDIA_ADDRESSES`: The addresses to bind the server to, separated by commas. (e.g., `0.0.0.0:5000,[::]:5000`)
- `KITTY_MEDIA_ENABLE_H2C`: Whether to enable HTTP/2 over clear text (H2C).
- `KITTY_MEDIA_CERT_PATH`: The path to the certificate file for TLS.
- `KITTY_MEDIA_KEY_PATH`: The path to the private key file for TLS.
- `KITTY_MEDIA_CACHE_DIR`: The directory to cache downloaded videos. (default disables caching)
- `KITTY_MEDIA_COOKIES_PATH`: The path to the cookies file for authentication.
- `KITTY_MEDIA_REMOTE_COMPONENTS`: The paths to remote components, separated by commas. (e.g., `ejs:github`, `ejs:npm`)
- `KITTY_MEDIA_MAX_CONCURRENT_DOWNLOADS`: The maximum number of concurrent downloads. (default: `128`)
- `KITTY_MEDIA_BUFFER_SIZE`: The size of the buffer for downloading videos in bytes. (default: `32768`)
- `KITTY_MEDIA_PACKETS_ON_FLY`: The number of packets to keep in flight during download. (default: `128`)
- `KITTY_MEDIA_FFMPEG_PATH`: The path to the FFmpeg executable. (default: `ffmpeg`)
- `KITTY_MEDIA_YTDLP_PATH`: The path to the yt-dlp executable. (default: `yt-dlp`)

Then, you can run the server executable, and it will start listening for requests in the path `/yt/{video_id}` where `{video_id}` is the ID of the YouTube video you want to retrieve and cache.

## License

Kitty Media is a project from [KitsuVM](https://github.com/kitsuvm) for Sinabar Works, licensed under the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.html). See the [LICENSE.txt](LICENSE.txt) file for more details.

```

```
