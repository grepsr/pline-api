# syntax=docker/dockerfile:1.7
#
# Image for the pline.ai MCP server (Streamable HTTP by default; pass `--stdio` for stdio).
# Self-contained: build with this directory as the context.
#   docker build -t pline-mcp mcp-server/
# Published by .github/workflows/release.yml as ghcr.io/grepsr/pline-mcp:<version>.

FROM rust:1.97.1-bookworm AS builder

WORKDIR /app

# Manifests first so the dependency layer is cached independently of source changes.
COPY Cargo.toml Cargo.lock ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    mkdir -p src docs \
    && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --locked --release \
    && rm -rf src

COPY src src
COPY docs docs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    find src docs -type f -exec touch {} + \
    && cargo build --locked --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get upgrade -y && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/pline-mcp /usr/local/bin/pline-mcp

# The MCP Registry verifies ownership of OCI packages through this label; its value must equal
# the `name` in the repository's server.json.
LABEL io.modelcontextprotocol.server.name="io.github.grepsr/pline-api" \
      org.opencontainers.image.title="pline.ai MCP server" \
      org.opencontainers.image.description="MCP server exposing the pline.ai scrape, crawl, batch, map, and search API." \
      org.opencontainers.image.source="https://github.com/grepsr/pline-api" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.documentation="https://github.com/grepsr/pline-api/blob/main/mcp-server/README.md"

ENV RUST_LOG=info

USER 65534
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s \
    CMD curl -fsS http://127.0.0.1:8080/health || exit 1
# ENTRYPOINT + CMD so `docker run IMAGE` serves HTTP and `docker run -i IMAGE --stdio` serves stdio.
ENTRYPOINT ["/usr/local/bin/pline-mcp"]
CMD ["--http", "0.0.0.0:8080"]
