# pline-mcp

MCP server for [pline.ai](https://github.com/grepsr/pline-api): scrape pages, crawl sites,
batch-scrape URL lists, map a site's URLs, and search the web, all through your own pline.ai API key.

No install step. Point any MCP client at `npx`:

```json
{
  "mcpServers": {
    "pline.ai": {
      "command": "npx",
      "args": ["-y", "pline-mcp"],
      "env": {
        "PLINE_BASE_URL": "https://apix.pline.ai/v1",
        "PLINE_API_KEY": "YOUR-API-KEY"
      }
    }
  }
}
```

`PLINE_API_KEY` is your key, sent to the API as `x-api-key`. `PLINE_BASE_URL` is the public HTTPS
address of the pline.ai API supplied by the service operator.

The package is a small launcher. The server is a native binary selected for your platform
(macOS arm64 and x64, Linux x64 and arm64, Windows x64) through an optional dependency, with a
checksum-verified download from the GitHub Release as a fallback. Set `PLINE_MCP_BINARY` to run a
build of your own. Arguments pass through: `npx -y pline-mcp --http 127.0.0.1:8080` serves
Streamable HTTP instead of stdio.

Tools: `scrape`, `batch_scrape_start`, `batch_scrape_status`, `batch_scrape_cancel`, `crawl_start`,
`crawl_status`, `crawl_cancel`, `map_site`, `search`, `sessions_list`. Resources `pline://guide` and
`pline://reference` document usage and every API field.

Client-specific setup (Cursor, VS Code, Claude Desktop, Claude Code, Windsurf, Docker, prebuilt
binaries) and the agent skill are in the
[repository README](https://github.com/grepsr/pline-api#readme). MCP Registry name:
`io.github.grepsr/pline-mcp`.
