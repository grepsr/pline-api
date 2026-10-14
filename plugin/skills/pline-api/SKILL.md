---
name: pline-api
description: Use when fetching web pages, extracting HTML, Markdown, screenshots or structured JSON, scraping URL lists, crawling sites, discovering URLs, or searching through the pline.ai API.
---

# pline.ai API

Use the pline.ai MCP tools that ship with this plugin. The user signs in to pline.ai when the server
first connects and chooses the workspace and API key it uses; there is nothing to configure here and
no key to ask for. If the tools are not connected, ask the user to connect the pline.ai server
(for example with `/mcp` in Claude Code) rather than calling the API another way.

The server documents itself: read the `pline://guide` resource for usage guidance and
`pline://reference` for every field, output format, action, and constraint.

## Choose the operation

| Need | MCP tool |
|---|---|
| One page | `scrape` |
| Known URLs | `batch_scrape_start`, then `batch_scrape_status` |
| Discover and scrape pages | `crawl_start`, then `crawl_status` |
| Discover URLs only | `map_site` |
| Search results | `search` |
| Remembered sessions | `sessions_list` |

Prefer a batch for known URLs. Size an unknown site with `map_site` before crawling and establish a
page limit within the user's requested scope before starting a large paid job.

## Fetching and sessions

Prefer Markdown for reading. Leave `js_render` and `proxy_strategy` unset initially; the service
decides. `js_render=true` forces a browser; `js_render=false` forbids it and cannot be combined with
actions or screenshots. JSON output requires `prompt` or `schema`.

The server remembers a session per host and reuses it automatically; pass `new_session=true` only
when a fresh identity is needed. Do not issue parallel calls that share one session.

Scrape and batch accept `actions`, `prompt`, and `schema`. Crawl takes per-page options such as
`actions`, `output`, and `js_render` directly, but has no `prompt` or `schema`.

## Background jobs and results

`batch_scrape_start` and `crawl_start` return an `id`. Check progress with `batch_scrape_status` or
`crawl_status`; on completion the status includes download links (batch: JSONL, crawl: ZIP). For long
jobs, return the ID and current status. Use `batch_scrape_cancel` or `crawl_cancel` when requested.

Check the returned `status` and `data`; empty content can be a soft failure. If rendering was unset,
retry with `js_render=true`; if pinned to the basic proxy tier, remove the pin. Report the API's error
detail and `request_id` when available. Preserve the user's requested outputs.
