FROM rust:1.94.1-alpine3.23 AS builder
WORKDIR /app
RUN apk -U upgrade --no-cache && apk add --no-cache musl-dev
COPY . .
RUN cargo build --release

FROM alpine:3.23
WORKDIR /app
RUN apk -U upgrade --no-cache && apk add --no-cache ffmpeg yt-dlp deno
ENV KITTY_MEDIA_CACHE_DIR=/cache
ENV KITTY_MEDIA_DELETE_OLD_THAN=1
ENV KITTY_MEDIA_REMOTE_COMPONENTS=ejs:github
COPY --from=builder /app/target/release/kitty-media .
VOLUME [ "/cache" ]
EXPOSE 5000
CMD [ "/app/kitty-media" ]
