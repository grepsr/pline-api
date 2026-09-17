# pline.ai usage guide

Fetch web content through pline.ai. Give it a URL and the output you want; it returns the page
content. Blocked pages, retries, and access escalation are handled for you. There is nothing to
configure per site.

## Pick the right tool

| Need | Tool | Shape |
|---|---|---|
| One page's content | `scrape` | synchronous, seconds |
| Many known URLs, same options | `batch_scrape_start` + `batch_scrape_status` | async job, artifacts on completion |
| Follow links across a whole site | `crawl_start` + `crawl_status` | async job, artifacts on completion |
| Just the list of a site's URLs | `map_site` | synchronous |
| Search results | `search` | synchronous |

Reach for `scrape` first. Only crawl when the set of URLs is unknown and must be discovered by
following links; if the URLs are already known, a batch is cheaper and more predictable. `map_site`
before a crawl is a good habit: it shows what a crawl would hit, for one cheap call.

## Scraping a page

- `output`: `html` (default), `clean_html`, `links`, `markdown`, `screenshot`,
  `screenshot_full_page`, `json`, in any combination. Prefer `markdown` when the content is for
  reading or for an LLM; `html` only when markup matters.
- `js_render` is a three-way choice. **Leave it unset** and pline.ai decides per page,
  escalating to a browser only when a plain fetch comes back empty or as a JavaScript shell. `true`
  forces a browser from the start; use it when you already know the content is built client-side.
  `false` forbids a browser; use it for plain HTML or API-style targets where you want the cheapest
  path and a fast failure instead of escalation. `false` cannot be combined with `actions` or
  screenshot output.
- `proxy_strategy` is how much access strength to spend: `auto` (default), `basic`, `enhanced`,
  `premium`. Leave it unset and pline.ai escalates through the tiers only as far as a site
  requires. Pin `basic` for sites you know are open to keep cost down; pin `enhanced` or `premium`
  when a site keeps failing under `auto` and the user accepts the higher cost.
- `prompt` / `schema`: with `json` output, extract structured data. One of the two is required. A
  list-shaped schema makes it a listing extraction and returns an array.
- `geolocation`: fetch the page as a visitor from that country.
- `actions`: click/type/scroll before capture. Any action implies a browser and `GET` only.
- `save_dir`: write each output to a file on the machine running the MCP server instead of
  returning it inline. Available only over stdio; HTTP rejects this option. Otherwise outputs are
  clipped at `max_chars`.
- `request_id`: your own correlation id, echoed back as `request_id`.

The response is a flat object: `data` (keyed by output: `html_body`, `clean_html`, `links`,
`markdown`, `json`, `screenshot`, `screenshot_full_page`), `status` (the page's HTTP status),
`final_url`, `session_id`, `request_id`, `effective_geolocation`, and `site_tier`. Quote
`request_id` when reporting a problem to the user or the service owner.

## Sessions: the main cost lever

Every scrape returns a `session_id`. The MCP server remembers it per host and sends it back on the
next `scrape` of the same host automatically, so you rarely need to handle it yourself. A session
carries cookies, request headers, and the settled access choices for that site into the next
request, so a page that was expensive and slow to open the first time is usually cheap and fast
afterwards. Dropping the session means paying to establish all of that again. It also keeps a
logged-in state alive across calls.

- Pass `session_id` only to continue a session created elsewhere; pass `new_session=true` only when
  a fresh identity is actually wanted.
- One registrable domain per session. A different domain on the same id returns `400`.
- No parallelism inside a session. The server serializes same-host calls locally so parallel tool
  calls do not collide; to scrape a site in parallel at scale, use a batch job.
- `sessions_list` shows what the server currently remembers.

## Crawl and batch jobs

Both return `202` with an `id`, then run in the background and survive restarts. Poll the status
tool (optionally with `wait_seconds`), cancel with the cancel tool. When a job completes, the status
carries `s3PresignedUrls`: one signed, expiring download per requested output format; pass
`download_dir` over stdio to fetch them (HTTP rejects this option). Long jobs are fine to leave running: hand the user the id rather than
blocking.

Artifacts differ by job type. A crawl uploads one ZIP per format with a `manifest.json` mapping
each file back to its source URL. A batch uploads one JSONL file per format with one line per URL
(`url`, `success`, `status`, `finalUrl`, `output`, `error`).

Scope a crawl before starting it: `limit` (max pages, ≤10000), `include_paths` / `exclude_paths`
(regex on the path), `max_discovery_depth`, `allow_subdomains`, `sitemap`. An unscoped crawl of a
large site burns credits fast; confirm the scope with the user when the target looks big, and prefer
`map_site` to size it up first. Both job tools accept the same `js_render`, `proxy_strategy`,
`geolocation`, and `actions` as `scrape`, applied to every page.

## Map and search

`map_site` returns a site's URLs, each with a `type` classification plus a title and description
when known. `sitemap="only"` restricts discovery to the site's own sitemaps; `sitemap="skip"`
ignores them and discovers by search instead.

`search` costs 1 credit in `mode="fast"` (organic results only) and 3 in `mode="full"` (adds
knowledge graph, people-also-ask, related searches, sitelinks). Add `output=["clean_html"]` to also
fetch each result page: best-effort, and one extra credit per page attempted.

## What is not exposed

Only these five endpoints are available. How pline.ai obtains a page beyond the documented
fields is not part of this interface. Do not speculate about it or describe it to the user. The
levers you have are `js_render`, `proxy_strategy`, `geolocation`, `actions`, and a reused session.
If a user asks to control request headers, timeouts, wait conditions, or the fetching method, say
those are not exposed here.

## Failures

Tool errors carry the HTTP status and the service's `detail` message. `422` is a malformed request
or an invalid combination (the message names the field), `400` is a session reused across two
domains, `429` is session contention or a rate limit, `502` means every attempt to fetch the page
failed or was blocked, `503` means a required backend is not configured or temporarily unavailable,
`504` means the request exceeded its time budget.

A `200` whose `data` is empty or is an obvious placeholder shell is a soft failure. If `js_render`
was unset, retry with `true`; if `proxy_strategy` was pinned to `basic`, retry without the pin.
