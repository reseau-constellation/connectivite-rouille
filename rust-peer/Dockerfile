FROM rust:latest

RUN --mount=type=cache,target=/var/cache/apt apt-get update && apt-get install -y protobuf-compiler

WORKDIR /usr/src/my_rust_app
COPY . .

RUN cargo build --release

CMD ["./target/release/my_rust_app"]
ENV RUST_BACKTRACE=1

EXPOSE 9090
CMD ["rust-libp2p-webrtc-peer"]
