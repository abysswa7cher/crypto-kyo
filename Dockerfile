FROM rust:1.75 as builder

WORKDIR /app
COPY . .

# Build with offline mode disabled - queries will be runtime checked
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/crypto-kyo ./
COPY --from=builder /app/migrations ./migrations

ENV RUST_LOG=info
EXPOSE 3000

CMD ["./crypto-kyo"]