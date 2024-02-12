FROM rust:latest

RUN --mount=type=cache,target=/var/cache/apt apt-get install -y protobuf-compiler

WORKDIR /usr/src/rust-libp2p-webrtc-peer
COPY . .

RUN --mount=type=cache,target=./target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release  --jobs 1

ENV RUST_BACKTRACE=1
EXPOSE 9090

CMD ["./target/release/rust-libp2p-webrtc-peer"]

