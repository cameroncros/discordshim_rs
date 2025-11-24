# Leveraging the pre-built Docker images with
# cargo-chef and the Rust toolchain
FROM rust:1-trixie AS builder
# Build dependencies - this is the caching Docker layer!
RUN apt update && apt install cmake protobuf-compiler -y
RUN cargo install --locked tokio-console
# Build application
WORKDIR /build
COPY . /build
RUN cargo build --release --bin discordshim
RUN cargo build --release --bin healthcheck


# We do not need the Rust toolchain to run the binary!
FROM ubuntu:latest AS runtime
COPY --from=builder /usr/local/cargo/bin/tokio-console /usr/bin
COPY --from=builder /build/target/release/discordshim /usr/bin
COPY --from=builder /build/target/release/healthcheck /usr/bin
ENTRYPOINT ["/usr/bin/discordshim"]
HEALTHCHECK CMD netstat -an | grep 23416 > /dev/null; if [ 0 != $? ]; then exit 1; fi;
