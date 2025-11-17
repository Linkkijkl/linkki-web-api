FROM rust:alpine AS builder
RUN apk add --no-cache build-base openssl-dev
WORKDIR /app

# Backend
#RUN rustup default nightly
ENV RUSTFLAGS="-C target-feature=-crt-static"
COPY Cargo.lock Cargo.toml .
COPY src src
RUN cargo build --release --bin linkki-web-backend 

# Final container
FROM alpine AS runtime
RUN apk add --no-cache libgcc tzdata
ENV TZ="Europe/Helsinki"
WORKDIR /app
COPY --from=builder /app/target/release/linkki-web-backend .
USER 1000
CMD [ "./linkki-web-backend" ]
