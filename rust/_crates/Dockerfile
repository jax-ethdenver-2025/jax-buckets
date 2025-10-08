# Get started with a build env with Rust nightly
FROM rustlang/rust:nightly-bullseye AS builder

# Install essential dev packages
RUN apt-get update && \
    apt-get install -y \
    build-essential \
    cmake \
    libclang-dev \
    libssl-dev \
    pkg-config \
    libpq-dev \
    && apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Set the LIBCLANG_PATH environment variable
ENV LIBCLANG_PATH=/usr/lib/llvm-11/lib

# Set the working directory
WORKDIR /app

# Copy the entire workspace
COPY . .

# Build the app
RUN cargo build --release -p leaky-server -vv

FROM debian:bullseye-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
  && apt-get install -y --no-install-recommends openssl ca-certificates \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*

# Copy the server binary to the /app directory
COPY --from=builder /app/target/release/leaky-server /app/

# Copy Cargo.toml files if they're needed at runtime
COPY --from=builder /app/Cargo.toml /app/
COPY --from=builder /app/crates/leaky-server/Cargo.toml /app/crates/leaky-server/

# Set any required env variables and expose the port
EXPOSE 3000

# Run the server
CMD ["/app/leaky-server"]
