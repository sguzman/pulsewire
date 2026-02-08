FROM rust:1.91-alpine AS builder
ARG TARGET=x86_64-unknown-linux-musl
WORKDIR /app
RUN apk add --no-cache musl-dev ca-certificates \
    && update-ca-certificates
RUN rustup target add ${TARGET}
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/fetcher/Cargo.toml crates/fetcher/Cargo.toml
COPY crates/cli/Cargo.toml crates/cli/Cargo.toml
COPY crates/server/Cargo.toml crates/server/Cargo.toml
COPY crates/tui/Cargo.toml crates/tui/Cargo.toml
COPY crates/core/src crates/core/src
COPY crates/core/res/sql crates/core/res/sql
COPY crates/fetcher/src crates/fetcher/src
COPY crates/cli/src crates/cli/src
COPY crates/server/src crates/server/src
COPY crates/tui/src crates/tui/src
RUN cargo build -p pulsewire-fetcher --release --target ${TARGET}
RUN strip /app/target/${TARGET}/release/pulsewire

FROM scratch
ARG TARGET=x86_64-unknown-linux-musl
WORKDIR /app
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/target/${TARGET}/release/pulsewire /usr/local/bin/pulsewire
COPY crates/fetcher/res/config.toml crates/fetcher/res/docker.toml crates/fetcher/res/domains.toml crates/fetcher/res/categories.toml /app/res/
COPY crates/fetcher/res/schemas /app/res/schemas
COPY crates/fetcher/res/feeds /app/res/feeds
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENV CONFIG_PATH=/app/res/docker.toml
ENV FEEDS_DIR=/app/res/feeds
VOLUME ["/app/res/feeds"]
USER 65532:65532
ENTRYPOINT ["pulsewire"]
