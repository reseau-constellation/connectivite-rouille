FROM rust:latest

RUN --mount=type=cache,target=/var/cache/apt apt-get update && apt-get install -y protobuf-compiler

WORKDIR /usr/src/my_rust_app
COPY . .

RUN --mount=type=cache,target=./target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

ENV RUST_BACKTRACE=1
EXPOSE 9090

CMD ["./target/release/my_rust_app"]

