FROM mcr.microsoft.com/devcontainers/rust:2-1-bullseye
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install pkg-config
RUN rustup toolchain install nightly \
    --component rust-src \
    --component rust-docs \
    --component clippy \
    --component rustfmt
