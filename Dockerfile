FROM rust:latest as builder

ARG BUILD_OPTS=""

ENV HOME=/home/root

RUN apt-get update && \
    apt-get install -y pkg-config build-essential libudev-dev

WORKDIR $HOME/app

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/home/root/target \
    --mount=type=cache,uid=1500,target=/usr/local/cargo/git \
    cargo build --release --bin lookup-registry-server

# ----------------------------------------------------

FROM ubuntu:focal

COPY --from=builder /home/root/app/target/release/lookup-registry-server /usr/bin/lookup-registry-server

RUN apt-get update && \
    apt-get install -y libssl-dev

CMD ["lookup-registry-server"]
