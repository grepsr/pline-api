# pline.ai API reference (exposed surface)

Base URL: `$PLINE_BASE_URL`. Auth header on every request: `x-api-key: $PLINE_API_KEY`.
Optional `x-request-id` header: echoed back as `request_id` on `/scrape`; generated when absent.
Errors are always `{"detail": "<message>"}` with a matching HTTP status.

Casing follows the API request models, and unknown or mis-cased keys are rejected with `422`
rather than silently ignored:

| Endpoint | Body casing |
|---|---|
| `POST /scrape` | snake_case |
| `POST /map` | camelCase |
| `POST /crawl` | camelCase fields; `output` is a list of format names and nested `scrape` fields are snake_case |
| `POST /batch/scrape` | batch-only fields are camelCase; shared scrape options are snake_case (legacy camelCase aliases are accepted) |
| `GET /serp` | snake_case query params |

Only the fields listed in this document are part of the interface. Request headers and service-level
retry settings are not caller-controlled.

---

## POST /scrape

Synchronous. Returns `200` with the page, or an error status. Server-side budget is ~300s.

### Request

| Field | Type | Default | Notes |
|---|---|---|---|
| `url` | string | — | required, absolute `http`/`https`, ≤2048 chars |
| `method` | string | `"GET"` | `GET`, `POST`, `PUT`, `DELETE`. Browser-rendered requests are `GET`-only |
| `body` | object \| string | — | not allowed with `GET`, not allowed with browser rendering |
| `js_render` | bool \| omitted | omitted | tri-state, see below |
| `proxy_strategy` | string | `"auto"` | `auto`, `basic`, `enhanced`, `premium`, see below |
| `output` | string[] | `["html"]` | see output table; must be a list, not a bare string |
| `only_main_content` | bool | `true` | strip nav/footer/ads from cleaned outputs |
| `geolocation` | string | — | fetch as a visitor from this 2-letter ISO country, e.g. `"US"` |
| `session_id` | string | generated | alias `session`; see Sessions |
| `tag` | string \| string[] | `[]` | ≤25 tags, 1–25 chars each, `[A-Za-z0-9._-]` |
| `actions` | object[] | `[]` | see Actions; any action implies a browser |
| `timeout` | int | — | milliseconds, 1000–60000 |
| `wait_selector` | string | — | CSS selector to wait for in browser mode |
| `wait_ms` | int | — | extra browser settle wait, 0–60000 ms |
| `prompt` | string | — | ≤16384 chars, for `output: ["json"]` |
| `schema` | object | — | ≤65536 bytes, lenient shape (not strict JSON Schema) |

**`js_render`** constrains which fetch modes are eligible; it is not a mode selection.

- omitted: pline.ai decides. A plain fetch is tried first and a browser is used only when the
  page comes back empty, as a JavaScript shell, or blocked.
- `true`: a browser is required from the first attempt. Use when the content is known to be built
  client-side.
- `false`: a browser is forbidden. Cheapest and fastest, but fails instead of escalating. Rejected
  with `422` when combined with `actions` or screenshot output.

**`proxy_strategy`** selects the access-strength tier, in ascending cost.

- `auto` (default): start cheap and escalate only as far as the site requires.
- `basic`: the cheapest tier only. Good for open sites; fails rather than escalating.
- `enhanced`: a stronger tier from the first attempt.
- `premium`: the strongest and most expensive tier from the first attempt.

### Output formats

| `output` value | `data` key | Content |
|---|---|---|
| `html` | `html_body` | raw HTML of the final page |
| `clean_html` | `clean_html` | scripts/styles/clutter removed |
| `links` | `links` | absolute, deduplicated links from cleaned HTML |
| `markdown` | `markdown` | Markdown conversion, good LLM input |
| `screenshot` | `screenshot` | temporary signed image URL, viewport only |
| `screenshot_full_page` | `screenshot_full_page` | temporary signed image URL, full page |
| `json` | `json` | AI extraction driven by `prompt` and/or `schema` |

Rules: `json` requires `prompt` or `schema`. `screenshot` and `screenshot_full_page` are mutually
exclusive. Screenshot output requires a browser, so it cannot be combined with `js_render: false`.
A list-shaped `schema` switches JSON extraction into listing mode: the page is chunked and one
concatenated array is returned.

### Actions

Executed in order after load. `selector_type` is one of `css` (default), `xpath`, `role`, `text`,
`regex`, `test_id`, `label`, `placeholder`.

| `action` | Requires | Notes |
|---|---|---|
| `click` | `selector` | |
| `click_and_hold` | `selector` | `duration_ms` 0–30000, default 1000 |
| `type` | `selector`, `value` | sequential keypress timing |
| `hover` | `selector` | |
| `select_option` | `selector`, `value` | |
| `press` | `selector`, `value` | key name, e.g. `Enter` |
| `wait_for_selector` | `selector` | |
| `scroll_into_view` | `selector` | |
| `drag_to` | `selector`, `value` | `value` is the drop-target selector |
| `scroll` | — | `value` = pixels, −100000…100000 |
| `wait` | — | `value` = milliseconds, 0…30000 |
| `reload` | — | |

Extras: `frame_selector` scopes the selector to an iframe; `if_present: true` skips the action when
the selector is not visible.

### Response

A flat object. There is no nested wrapper and no diagnostics block.

```json
{
  "request_id": "req_01HZ...",
  "session_id": "my-session-abc123",
  "effective_geolocation": {"country": "US", "city": "New York"},
  "final_url": "https://example.com/pricing",
  "site_tier": "standard",
  "status": 200,
  "data": {"markdown": "# Pricing\n\n## Starter\n$9/mo..."}
}
```

| Field | Meaning |
|---|---|
| `request_id` | the `x-request-id` you sent, or a generated id. Quote it when reporting a problem |
| `session_id` | the session used; send it back to reuse state (see Sessions) |
| `effective_geolocation` | country and, when known, city the request appeared to come from; `null` when none was selected |
| `final_url` | URL after redirects or browser navigation |
| `site_tier` | difficulty classification of the target, e.g. `standard`, `premium` |
| `status` | HTTP status of the returned page |
| `data` | one key per requested output, named per the output table |

Screenshot outputs put a signed, expiring image URL in `data`; download it promptly.

### curl

```bash
curl -sS -X POST "$PLINE_BASE_URL/scrape" \
  -H "x-api-key: $PLINE_API_KEY" -H 'content-type: application/json' \
  -d '{"url":"https://example.com/pricing","output":["markdown"],"only_main_content":true}'
```

---

## Sessions

`session_id` is returned on every scrape; one is generated when you do not send one. Sending it
back on the next request reuses the stored cookies, request headers, and settled access choices
for that site, which avoids paying to establish them again.

- A session is bound to one registrable domain; a different domain returns `400`.
- Requests sharing a session id are serialized; a second in-flight one returns `429`.
- Sessions expire; treat a session id as a cache, not as durable state, and be ready for a request
  under an old id to cost what a first request costs.
- `/batch/scrape` accepts `session_id` too, applying it to every URL in the job.

---

## POST /crawl

Link-following crawl. Runs in the background, survives restarts, returns `202` immediately.

### Request (camelCase, unknown fields rejected)

| Field | Type | Default | Notes |
|---|---|---|---|
| `url` | string | — | required, absolute `http`/`https` |
| `limit` | int | `10000` | 1–10000 max pages |
| `includePaths` | string[] | `[]` | regexes; only matching paths are crawled |
| `excludePaths` | string[] | `[]` | regexes; matching paths are skipped |
| `maxDiscoveryDepth` | int | — | link depth from the seed |
| `sitemap` | enum | `include` | `skip` \| `include` \| `only` |
| `ignoreQueryParameters` | bool | `false` | drop URLs with query strings |
| `crawlEntireDomain` | bool | `false` | leave the seed path |
| `allowSubdomains` | bool | `false` | include subdomains |
| `output` | string[] | `["markdown"]` | `markdown`, `html`, `clean_html`, `links`, `screenshot`, `screenshot_full_page`; at least one format is required |
| `email` | string | — | notification address, validated |
| `webhook` | object | — | `{url, headers, metadata, events}`; `events` ⊆ `started`,`page`,`completed`,`failed` (empty = all) |
| `scrape` | object | `{}` | per-page fetch options, snake_case, see below (alias `scrapeOptions`) |
| `scheduleToCloseTimeoutSecs` | int | `86400` | |
| `heartbeatTimeoutSecs` | int | `30` | |

`scrape` is forwarded to each page fetch and accepts the shared `POST /scrape` options with the same
names and meaning, including `output`, `js_render`, `proxy_strategy`, `geolocation`,
`only_main_content`, `actions`, `tag`, `timeout`, `wait_selector`, `wait_ms`, `prompt`, and `schema`. The
crawler sets `url`, `method`, and `output` itself (`body` is dropped). `scrape.output` may be used
instead of the top-level `output` with the same format names as `/scrape`; when both are present,
top-level `output` wins.

`202` body: `{"success": true, "id": "<uuid>", "url": "/crawl/<uuid>", "workflowId": "...", "runId": "..."}`.

### GET /crawl/{id}

```json
{
  "id": "<uuid>", "status": "scraping",
  "limit": 200, "total": 200, "completed": 143, "failed": 2, "pending": 55, "inFlight": 4,
  "createdAt": "...", "completedAt": null, "duration": 91.4,
  "s3PresignedUrls": null, "error": null
}
```

`status` is `scraping` | `completed` | `failed` | `cancelled`. On completion `s3PresignedUrls` maps
each requested format (`markdown`, `html`, `clean_html`, `links`, `screenshot`) to a signed,
expiring URL for that format's ZIP. Inside each ZIP, files are named `page_<n>/<seq>-<hash>.<ext>`
and `manifest.json` maps each file to its source URL and page metadata. `404` for an unknown or
non-UUID id.

### DELETE /crawl/{id}

Requests cancellation. `200` `{"status": "cancelled"}`; `409` if the crawl is no longer
cancellable; `404` if unknown.

---

## POST /batch/scrape

The `/scrape` options applied to a list of URLs. Runs in the background, survives restarts,
returns `202` immediately.

| Field | Type | Default | Notes |
|---|---|---|---|
| `urls` | string[] | — | required, non-empty, length ≤ `limit` |
| `limit` | int | `10000` | 1–10000 |
| `method` | string | `"GET"` | |
| `body` | object \| string | — | same rules as `/scrape` |
| `js_render` | bool \| omitted | omitted | same tri-state and name as `/scrape` |
| `proxy_strategy` | string | `"auto"` | same values and name as `/scrape` |
| `output` | string[] | `["html"]` | same values and rules as `/scrape` |
| `only_main_content` | bool | `true` | same name as `/scrape` |
| `geolocation` | string | — | |
| `session_id` | string | — | snake_case; applies to every URL |
| `tag` | string \| string[] | `[]` | same shape as `/scrape` |
| `actions` | object[] | `[]` | |
| `prompt` | string | — | required with `json` output unless `schema` is set |
| `schema` | object | — | |
| `timeout` | int | — | milliseconds, 1000–60000 |
| `wait_selector` | string | — | CSS selector to wait for |
| `wait_ms` | int | — | extra browser settle wait in milliseconds |
| `scheduleToCloseTimeoutSecs` | int | `86400` | batch job setting |
| `heartbeatTimeoutSecs` | int | `30` | batch job setting |
| `email` | string | — | completion notification |
| `webhook` | object | — | notification configuration |

Validation mirrors `/scrape`, so a bad combination fails the whole job with `422` at submission
rather than failing every item. `js_render: false` with `actions` or screenshot output is rejected.

`202` body mirrors crawl's: `{"success", "id", "url": "/batch/scrape/<id>", "workflowId", "runId"}`.
`GET /batch/scrape/{id}` and `DELETE /batch/scrape/{id}` behave exactly like their crawl
counterparts. On completion `s3PresignedUrls` maps each requested format to a signed URL for a
**JSONL** file (not a ZIP). Each line is one URL:

```json
{"index": 0, "url": "https://example.com/a", "success": true, "status": 200,
 "finalUrl": "https://example.com/a", "output": "# A\n...", "error": null}
```

`output` holds that file's format (Markdown text, HTML, a links array, or the extracted JSON).
Failed items have `success: false` and a message in `error`.

---

## POST /map

Synchronous URL discovery. Prefers the site's own sitemaps and falls back to search-based
discovery when none are published.

| Field | Type | Default | Notes |
|---|---|---|---|
| `url` | string | — | required base URL |
| `sitemap` | enum | `include` | `include` (search fallback only when no sitemap exists), `only` (sitemaps only), `skip` (search only) |
| `includeSubdomains` | bool | `true` | |
| `ignoreQueryParameters` | bool | `true` | |
| `ignoreCache` | bool | `false` | bypass discovery caches |
| `limit` | int | `1000` | 1–1000000 |
| `timeout` | int | — | whole-request budget in ms; `504` names it when exceeded |

```json
{"success": true,
 "links": [{"url": "https://example.com/deals", "type": "product-list",
            "title": "Deals", "description": "Today's featured deals."}]}
```

---

## GET /serp

Query parameters (snake_case, unknown params rejected):

| Param | Default | Notes |
|---|---|---|
| `query` | — | required, ≤512 chars |
| `country` | `US` | 2-letter code |
| `language` | `en` | 2-letter code |
| `num_results` | `10` | 1–100 |
| `tbs` | — | `qdr:d`, `qdr:w`, `qdr:m` |
| `device` | `desktop` | `desktop`, `mobile`, `tablet` |
| `mode` | `full` | `fast` = 1 credit, organic results + metadata; `full` = 3 credits, adds search features |
| `output` | — | comma-separated `html`, `clean_html`, `links`; enriches each result page, best-effort, +1 credit per page attempted |
| `only_main_content` | `true` | applies to enriched page content |

`fast`: `{query, pageNumber, dateDownloaded, creditsUsed, organic[]}`.
`full` additionally: `totalOrganicResults`, `knowledgeGraph`, `peopleAlsoAsk`, `relatedSearches`,
plus `sitelinks` and `attributes` on organic results.

Organic result: `position`, `title`, `link`, `snippet`, `domain`, `page`, and, when `output` was
requested, `html_body`, `clean_html`, `links`.

```bash
curl -sS -G "$PLINE_BASE_URL/serp" -H "x-api-key: $PLINE_API_KEY" \
  --data-urlencode 'query=best pizza in SF' -d 'mode=fast' -d 'num_results=10'
```

---

## Status codes

| Code | Meaning |
|---|---|
| `200` | success; check `data` is non-empty before trusting it |
| `202` | crawl / batch job accepted |
| `400` | malformed request, unsupported output, or a session used across two domains |
| `404` | unknown job id |
| `409` | job no longer cancellable |
| `422` | validation failure or invalid option combination; the `detail` names the offending field |
| `429` | session contention, or a rate limit; retry after a short delay |
| `500` | unexpected internal error; quote `request_id` |
| `502` | every attempt to fetch the target failed, timed out, or was blocked |
| `503` | a required backend is not configured or temporarily unavailable |
| `504` | request exceeded its timeout budget |
