# Set platform explicitly for builder
FROM --platform=linux/amd64 rust:1.88 AS builder

WORKDIR /app

# Copy Cargo files first to leverage Docker caching
COPY Cargo.toml Cargo.lock ./

# Create dummy files to cache dependencies for all binaries
RUN mkdir src api \
    && echo 'fn main() {}' > src/main.rs \
    && echo 'fn main() {}' > api/eligibility.rs \
    && echo 'fn main() {}' > api/validity.rs \
    && echo 'fn main() {}' > api/health.rs \
    && echo 'fn main() {}' > api/create.rs \
    && echo 'fn main() {}' > api/create_solana.rs \
    && echo 'fn main() {}' > api/eligibility_solana.rs \

RUN cargo build --release && rm -rf src api

# Copy actual source code
COPY ./src ./src
COPY ./api ./api

# Build the actual application binary (main.rs)
RUN cargo build --release --bin sablier_merkle_api

# Runtime stage (also set platform)
FROM --platform=linux/amd64 debian:bookworm-slim

WORKDIR /app

# ✅ Install CA certificates in runtime image
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy built binary from builder stage
COPY --from=builder /app/target/release/sablier_merkle_api ./sablier_merkle_api

EXPOSE 3030

CMD ["./sablier_merkle_api"]
