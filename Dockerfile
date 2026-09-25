# syntax=docker/dockerfile:1
FROM rust:1.98.1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml build.rs ./
COPY src ./src
RUN cargo build --locked --release --bin rust-starterkit

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 app && useradd --uid 10001 --gid app --no-create-home app
COPY --from=build /app/target/release/rust-starterkit /usr/local/bin/rust-starterkit
USER 10001:10001
ENV BIND_ADDR=0.0.0.0:3000 ENABLE_EXAMPLE=false
EXPOSE 3000
STOPSIGNAL SIGTERM
ENTRYPOINT ["rust-starterkit"]
CMD ["serve"]
