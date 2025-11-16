FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev openssl-dev
WORKDIR /app

# Backend
#RUN rustup default nightly
ENV RUSTFLAGS="-C target-feature=-crt-static"
COPY Cargo.lock Cargo.toml .
COPY src src
RUN cargo build --release --bin linkki-web-backend 

# Final container
FROM alpine AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/linkki-web-backend .
USER 1000
CMD [ "./linkki-web-backend" ]

