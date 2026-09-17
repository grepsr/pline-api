---
name: pline-api
description: Use when fetching web pages, extracting HTML, Markdown, screenshots or structured JSON, scraping URL lists, crawling sites, discovering URLs, or searching through the pline.ai API.
---

# pline.ai API

Use the connected pline.ai MCP tools when available. Otherwise use the bundled Python client;
it needs Python 3.10+ and only the standard library.

## Configuration and paths

The client requires `PLINE_BASE_URL` (the operator's public HTTPS API URL) and
`PLINE_API_KEY` in its environment.
If missing, ask the user to configure them locally; do not ask them to paste the key into chat.
Never put the key in a URL, output, or committed file. Do not guess the API host.

Resolve `SKILL_DIR` to the **absolute directory containing this loaded SKILL.md**, wherever the
skill is installed. Set that variable in each shell invocation below. Do not assume the current
working directory contains this repository. Keep the caller's working directory for relative
input files, session files, and output paths.

```sh
python3 "$SKILL_DIR/scripts/pline.py" --help
python3 "$SKILL_DIR/scripts/pline.py" scrape --url https://example.com/pricing \
  --output markdown --session-file .pline-session-example --save ./page-output
```

## Choose the operation

| Need | MCP tool | CLI command |
|---|---|---|
| One page | `scrape` | `scrape --url URL` |
| Known URLs | `batch_scrape_start` | `batch start --urls-file urls.txt` |
| Discover and scrape pages | `crawl_start` | `crawl start --url URL --limit N` |
| Discover URLs only | `map_site` | `map --url URL` |
| Search results | `search` | `serp --query TEXT` |

Prefer a batch for known URLs. Size an unknown site with map before crawling and establish a
page limit within the user's requested scope before starting a large paid job.

## Fetching and sessions

Prefer Markdown for reading. Leave JavaScript rendering and proxy strategy unset initially;
the service decides. `--js-render` forces a browser; `--no-js-render` forbids it and cannot be
combined with actions or screenshots. `--output json` requires `--prompt` or `--schema`.
JSON-valued options accept inline JSON or `@file`.

Reuse `--session-file` for repeated requests to one site; use one file per site and account.
Do not issue parallel calls with the same session. MCP remembers sessions automatically;
use `new_session=true` only when a fresh identity is needed.

Scrape and batch accept `--actions`, `--prompt`, and `--schema`. Crawl accepts its narrower
per-page options through `--scrape-options`, for example
`'{"actions":[{"action":"click","selector":"#more"}]}'`; it has no `--actions`, `--prompt`, or
`--schema` flags. Read [references/endpoints.md](references/endpoints.md) for supported fields,
wire casing, actions, output formats, and constraints; MCP arguments follow the tool schema.

## Background jobs and results

Batch and crawl return an `id`. Use `batch status ID` or `crawl status ID`; `--wait` polls and
`--download DIR` saves completed artifacts locally. Batch produces JSONL; crawl produces ZIP.
For long jobs, return the ID and current status. Use the matching cancel command when requested.

Check returned `status` and `data`; empty content can be a soft failure. If rendering was unset,
retry with JavaScript rendering; if pinned to the basic proxy tier, remove the pin. Report the
API's error detail and `request_id` when available. Preserve the user's requested outputs.
