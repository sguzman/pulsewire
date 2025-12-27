FROM rust:1.78-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY res/sql ./res/sql
RUN cargo build --release
RUN strip /app/target/release/feedrv3

FROM gcr.io/distroless/cc-debian12
WORKDIR /app
COPY --from=builder /app/target/release/feedrv3 /usr/local/bin/feedrv3
COPY res/config.toml res/docker.toml res/domains.toml res/categories.toml /app/res/
COPY res/feeds /app/res/feeds
ENV CONFIG_PATH=/app/res/docker.toml
ENV FEEDS_DIR=/app/res/feeds
VOLUME ["/app/res/feeds"]
USER 65532:65532
ENTRYPOINT ["feedrv3"]
