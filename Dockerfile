FROM rust:1.65.0
RUN apt-get update && apt-get install protobuf-compiler -y
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path .
CMD ["discordshim"]