# pline.ai MCP server

Run this MCP server on your own machine to scrape pages, crawl sites, discover URLs, and search
through the hosted pline.ai API. Each user supplies their own API key.

```text
Your MCP client → local MCP process (stdio) → hosted pline.ai API (HTTPS)
```

The scraping backend stays on the service operator's infrastructure. You do not need to run the
orchestrator, browser fetcher, Redis, or Temporal locally.

## Run with npx

The server is published to npm as [`pline-api`](https://www.npmjs.com/package/pline-api): a small
launcher that selects the prebuilt binary for your platform (macOS arm64/x64, Linux x64/arm64,
Windows x64) from an optional dependency, falling back to a checksum-verified download from the
GitHub Release. Node.js 18+ is the only requirement:

```json
{
  "mcpServers": {
    "pline.ai": {
      "command": "npx",
      "args": ["-y", "pline-api"],
      "env": {
        "PLINE_BASE_URL": "https://api.example.com/v1",
        "PLINE_API_KEY": "your-own-api-key"
      }
    }
  }
}
```

Arguments pass straight through to the binary (`npx -y pline-api --http 127.0.0.1:8080`), and
`PLINE_MCP_BINARY=/path/to/pline-mcp` makes the launcher run a build of your own. The launcher
source is in [`../npm`](../npm); per-platform packages are named `pline-api-<os>-<cpu>`.

## Install a prebuilt binary

Use the platform downloads and checksum instructions in the
[repository setup guide](../README.md#download-the-mcp-binary). Extract `pline-mcp` (or
`pline-mcp.exe` on Windows) and configure your MCP client with its absolute path below.
You do not need Rust, Cargo, or a repository checkout for a prebuilt installation.

## Run the Docker image

Each release is also published as a multi-platform image at
`ghcr.io/grepsr/pline-mcp` (linux/amd64 and linux/arm64), which is the package the
[MCP Registry](https://registry.modelcontextprotocol.io) entry `io.github.grepsr/pline-api` points at.
With Docker installed, an MCP client can run the server over stdio without downloading a binary:

```json
{
  "mcpServers": {
    "pline.ai": {
      "command": "docker",
      "args": ["run", "-i", "--rm", "-e", "PLINE_BASE_URL", "-e", "PLINE_API_KEY",
               "ghcr.io/grepsr/pline-mcp:0.3.1", "--stdio"],
      "env": {
        "PLINE_BASE_URL": "https://api.example.com/v1",
        "PLINE_API_KEY": "your-own-api-key"
      }
    }
  }
}
```

`-e NAME` without a value forwards the variable from the client-provided environment into the
container, so the key never appears on the command line. The image runs as an unprivileged user and
has no access to your filesystem, so `save_dir` and `download_dir` are unavailable unless you mount a
volume. Pin the tag to a release version; `latest` moves. The same image serves Streamable HTTP when
started without `--stdio`:

```sh
docker run --rm -p 8080:8080 -e PLINE_BASE_URL=https://api.example.com/v1 ghcr.io/grepsr/pline-mcp:0.3.1
```

## Build from Git

You need Git, a current stable Rust toolchain with Cargo, an API key, and the public HTTPS API base
URL supplied by the service operator. `https://api.example.com/v1` below is a placeholder.

From the repository root:

```sh
git clone https://github.com/grepsr/pline-api.git
cd pline-api
cargo build --manifest-path mcp-server/Cargo.toml --release --locked
cp .env.example .env
```

Edit `.env` with your API URL and your own key, then load it before starting your MCP client:

```sh
set -a
. ./.env
set +a
```

These environment-loading commands are for POSIX shells. On other platforms, set the same two
environment variables in your shell or MCP client configuration. The server does not load `.env`
automatically; the file is ignored by Git.

Use the public HTTPS API address supplied by the service operator. Internal deployment addresses
are not reachable from a user's machine.

## Connect an MCP client

For a client with a `mcpServers` JSON configuration, add this entry. Replace the binary path, API
URL, and key with your own values. On Windows the binary has an `.exe` suffix.

```json
{
  "mcpServers": {
    "pline.ai": {
      "command": "/absolute/path/to/pline-mcp",
      "args": ["--stdio"],
      "env": {
        "PLINE_BASE_URL": "https://api.example.com/v1",
        "PLINE_API_KEY": "your-own-api-key"
      }
    }
  }
}
```

Use an absolute binary path so the client can launch it from any working directory.
For source builds, the executable is `mcp-server/target/release/pline-mcp` inside the checkout. Store real
credentials in your local client settings, not in a committed configuration file.

For Claude Code launched from the repository root, the root `.mcp.json` runs the server through
Cargo and expands the two environment variables from your shell. The first release build can
take a while; building it first avoids waiting during the client's connection attempt.

The MCP client launches and manages the stdio process. You do not need to start an HTTP listener
or expose a port. Logs go to stderr (`RUST_LOG=debug` for more detail).

## Update

For prebuilt installs, download the new archive, verify its checksum, and replace the executable.
For source builds:

```sh
git pull --ff-only
cargo build --manifest-path mcp-server/Cargo.toml --release --locked
```

Restart or reconnect the MCP client to load the rebuilt binary.

## Tools

| Tool | Endpoint |
|---|---|
| `scrape` | `POST /scrape` |
| `batch_scrape_start` / `batch_scrape_status` / `batch_scrape_cancel` | `/batch/scrape` |
| `crawl_start` / `crawl_status` / `crawl_cancel` | `/crawl` |
| `map_site` | `POST /map` |
| `search` | `GET /serp` |
| `sessions_list` | local: remembered host-to-session map for this caller |

Tool results contain structured JSON. API errors return the HTTP status and the service's
`detail` message. The client sends your key to the configured API as `x-api-key`.

`scrape` remembers sessions per caller and host for the process lifetime. A second scrape of the
same host reuses the session unless you pass `session_id` or set `new_session=true`. Same-host
scrapes are serialized locally. A 4xx response on a remembered session drops it for the next call.

Over stdio, `save_dir` writes scrape outputs to your local filesystem and returns their paths. Without it,
outputs are clipped at `max_chars` (default 60000); `truncated_outputs` lists clipped fields.
Over stdio, the status tools can download finished job artifacts to a local `download_dir`.
HTTP requests with `save_dir` or `download_dir` are rejected before contacting the API.

Read `pline://guide` for usage guidance and `pline://reference` for the API fields and constraints.
The documents are also available in [docs/guide.md](docs/guide.md) and
[docs/reference.md](docs/reference.md).

## Hosted OAuth connection

The remote MCP endpoint can use Pline sign-in instead of asking every agent user to paste an API
key. After sign-in, each user selects a workspace and an active API key. Follow
[`docs/oauth-hosting.md`](docs/oauth-hosting.md) to configure the Pline web app and host.

## Configuration

The executable is `pline-mcp`; its public MCP identity is `pline.ai`.
Set `PLINE_BASE_URL` and `PLINE_API_KEY` for local stdio use.
For HTTP mode, `PLINE_MCP_HTTP_ADDR` sets the listener address and
`PLINE_MCP_ALLOWED_HOSTS` accepts a comma-separated list of allowed hosts. OAuth mode also uses
`PLINE_PLATFORM_BASE_URL` (Pline web app origin), `PLINE_SUPABASE_URL` (Supabase project origin),
and `PLINE_MCP_PUBLIC_URL` (public MCP origin).

## Where this server is listed

The server is published to the official MCP Registry as `io.github.grepsr/pline-api` with the npm
package and the GHCR image above as its packages, and the repository root is installable as a Claude
Code plugin (`pline-ai`) that runs the npm package. The root [`.cursor/mcp.json`](../.cursor/mcp.json)
and [`.vscode/mcp.json`](../.vscode/mcp.json) configure `npx -y pline-api` for Cursor and VS Code
workspaces. [PUBLISHING.md](../PUBLISHING.md) describes how each listing is produced and kept current.

## HTTP mode

The binary also supports Streamable HTTP, for example for local transport development:

```sh
PLINE_BASE_URL=https://api.example.com/v1 \
  ./mcp-server/target/release/pline-mcp --http 127.0.0.1:8080
```

`/health` reports liveness. `/mcp` accepts the caller's API key through `Authorization: Bearer` or
`x-api-key`; the legacy `/<key>/mcp` route also exists, but puts credentials in URLs and access logs.
The server forwards each caller's key rather than using the stdio environment key. A Supabase OAuth
access token is also accepted when the platform and OAuth environment variables are configured. The
user selects an active workspace API key in Pline's OAuth consent screen, and the server resolves it
for each request through the authenticated Pline platform. The MCP process does not keep a shared API
key.

Configure Supabase OAuth 2.1 with dynamic client registration enabled and `/oauth/consent` as its
authorization UI path. Apply API service migrations with `uv run alembic upgrade head` from the
`pline-hades-api-service` repository before enabling the hosted endpoint.

HTTP transport is stateless: no `Mcp-Session-Id` is issued, and standalone GET streams and DELETE
session requests return 405. Each POST uses its own API key; remembered scrape sessions remain
scoped to that key. Session replay and MCP cancellation notifications are not supported in HTTP
mode. The Git installation above uses stdio.

## Develop

Run from `mcp-server/`:

```sh
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --check
```

The guide and reference are embedded with `include_str!`; rebuild after editing them.
