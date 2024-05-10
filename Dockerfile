# Leveraging the pre-built Docker images with
# cargo-chef and the Rust toolchain
FROM lukemathwalker/cargo-chef:latest AS chef
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
RUN cargo install --locked tokio-console
# Build application
COPY . .
RUN cargo build --release --bin discordshim


# We do not need the Rust toolchain to run the binary!
FROM ubuntu:latest AS runtime
WORKDIR app
COPY --from=builder /root/.cargo/bin/tokio-console /usr/bin
COPY --from=builder /app/target/release/discordshim /usr/bin
ENTRYPOINT ["/usr/bin/discordshim", "serve"]
HEALTHCHECK CMD netstat -an | grep 23416 > /dev/null; if [ 0 != $? ]; then exit 1; fi;
