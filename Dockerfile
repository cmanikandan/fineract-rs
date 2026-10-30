# Stage 1: Build native Rust release binary for fineract-server-rs
FROM rust:1.85-slim-bookworm AS builder
WORKDIR /usr/src/fineract-rs
COPY Cargo.toml ./
COPY anchor ./anchor
COPY crates ./crates
RUN cargo build --release -p fineract-server-rs

# Stage 2: Minimal distroless runtime container (< 25 MB image, non-root user)
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder /usr/src/fineract-rs/target/release/fineract-server /app/fineract-server
ENV PORT=8443
EXPOSE 8443
USER nonroot:nonroot
ENTRYPOINT ["/app/fineract-server"]
