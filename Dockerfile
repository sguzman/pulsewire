FROM rust:1.78-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY res ./res
COPY tests ./tests
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/feedrv3 /usr/local/bin/feedrv3
COPY res /app/res
RUN useradd -r -u 10001 -g nogroup feedrv3 \
    && mkdir -p /app/res/logs \
    && chown -R feedrv3:nogroup /app/res
ENV CONFIG_PATH=/app/res/config.toml
ENV FEEDS_DIR=/app/res/feeds
VOLUME ["/app/res/feeds"]
USER feedrv3
ENTRYPOINT ["feedrv3"]
