FROM rust:1.91-alpine AS builder
ARG TARGET=x86_64-unknown-linux-musl
WORKDIR /app
RUN apk add --no-cache musl-dev ca-certificates \
    && update-ca-certificates
RUN rustup target add ${TARGET}
COPY Cargo.toml Cargo.lock ./
COPY fetcher/Cargo.toml fetcher/Cargo.toml
COPY fetcher/src fetcher/src
COPY fetcher/res/sql fetcher/res/sql
RUN cargo build -p fetcher --release --target ${TARGET}
RUN strip /app/target/${TARGET}/release/feedrv3

FROM scratch
ARG TARGET=x86_64-unknown-linux-musl
WORKDIR /app
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/target/${TARGET}/release/feedrv3 /usr/local/bin/feedrv3
COPY fetcher/res/config.toml fetcher/res/docker.toml fetcher/res/domains.toml fetcher/res/categories.toml /app/fetcher/res/
COPY fetcher/res/schemas /app/fetcher/res/schemas
COPY fetcher/res/feeds /app/fetcher/res/feeds
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
ENV CONFIG_PATH=/app/fetcher/res/docker.toml
ENV FEEDS_DIR=/app/fetcher/res/feeds
VOLUME ["/app/fetcher/res/feeds"]
USER 65532:65532
ENTRYPOINT ["feedrv3"]
