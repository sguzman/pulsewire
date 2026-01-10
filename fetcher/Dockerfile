FROM rust:1.91-alpine AS builder
ARG TARGET=x86_64-unknown-linux-musl
WORKDIR /app
RUN apk add --no-cache musl-dev ca-certificates \
    && update-ca-certificates
RUN rustup target add ${TARGET}
COPY Cargo.toml Cargo.lock ./
COPY core/Cargo.toml core/Cargo.toml
COPY core/src core/src
COPY core/res/sql core/res/sql
COPY fetcher/Cargo.toml fetcher/Cargo.toml
COPY fetcher/src fetcher/src
RUN cargo build -p fetcher --release --target ${TARGET}
RUN strip /app/target/${TARGET}/release/feedrv3

FROM scratch
ARG TARGET=x86_64-unknown-linux-musl
WORKDIR /app
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/target/${TARGET}/release/feedrv3 /usr/local/bin/feedrv3
COPY res/config.toml res/docker.toml res/domains.toml res/categories.toml /app/res/
COPY res/schemas /app/res/schemas
COPY res/feeds /app/res/feeds
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENV CONFIG_PATH=/app/res/docker.toml
ENV FEEDS_DIR=/app/res/feeds
VOLUME ["/app/res/feeds"]
USER 65532:65532
ENTRYPOINT ["feedrv3"]
