# ---- Build Stage ----
FROM rust:latest as builder

WORKDIR /usr/src/probe

# Copy manifest file first for caching
COPY Cargo.toml ./
# Copy source code
COPY src ./src
# Copy benches for benchmarks referenced in Cargo.toml
COPY benches ./benches

# Build the project in release mode (this will generate Cargo.lock if missing)
RUN cargo build --release

# ---- Runtime Stage ----
FROM debian:bookworm-slim

# Install CA certificates (for HTTPS support)
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder
COPY --from=builder /usr/src/probe/target/release/probe /usr/local/bin/probe

# Set the default command
ENTRYPOINT ["/usr/local/bin/probe"]