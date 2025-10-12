# Multi-stage build for smaller final image
FROM rust:1.85-slim as builder

# Install jax-bucket from crates.io with pinned version
ARG JAX_VERSION=0.1.0
RUN cargo install jax-bucket --version ${JAX_VERSION}

# Runtime stage with minimal dependencies
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /usr/local/cargo/bin/jax /usr/local/bin/jax

# Set up working directory
WORKDIR /data

# Configuration arguments
ARG CONFIG_PATH=/data/node
ARG API_PORT=3000
ARG HTML_PORT=8080
ARG PEER_PORT=9000

# Expose ports
EXPOSE ${API_PORT}
EXPOSE ${HTML_PORT}
EXPOSE ${PEER_PORT}

# Environment variables for runtime configuration
ENV CONFIG_PATH=${CONFIG_PATH}
ENV API_ADDR=0.0.0.0:${API_PORT}
ENV HTML_ADDR=0.0.0.0:${HTML_PORT}
ENV PEER_PORT=${PEER_PORT}

# Entrypoint script to initialize if needed and run service
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["service"]
