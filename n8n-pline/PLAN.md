# n8n-nodes-pline: build plan

Community node for the pline.ai API, built for n8n verification. Sources of truth for every
endpoint, field, default and status code: the public docs at https://apidocs.pline.ai (guides,
API reference, billing), `mcp-server/docs/reference.md`, `mcp-server/src/client.rs` and
`mcp-server/src/main.rs` (wire casing and validation), and `mcp-server/docs/guide.md` (use cases).
Nothing below is invented; where every source is silent it is listed under Open questions.

Where the sources disagree, the node sends only the fields the user sets, so API defaults apply
server-side and the UI never states a default the sources disagree on:

- crawl `crawlEntireDomain`: docs say default true, the MCP reference says false.
- search cost: docs billing page says 2 credits per 10 results, the MCP guide says fast = 1 and
  full = 3 credits. The UI says "costs credits; full mode costs more" without a number.

Note: `skills/pline-api/references/endpoints.md` lags the MCP reference (no `clean_markdown`,
`timeout`, `wait_selector`, `wait_ms`, `ignoreRobotsTxt`; crawl `output` as an object; batch in
camelCase). The MCP server is what talks to the API today, so the node follows the MCP reference.

## Package

| Item | Value |
|---|---|
| Directory | `n8n-pline/` in this repo (scaffolded with `npm create @n8n/node@latest`, then moved in) |
| npm name | `n8n-nodes-pline` |
| License | MIT (same as the repo, copyright Grepsr) |
| Runtime dependencies | none; HTTP goes through `this.helpers.httpRequestWithAuthentication` |
| Node style | programmatic (one `Pline` node, `execute()`), needed for job polling and error mapping |
| Resources | Scrape, Batch Scrape, Crawl, Map, Search |
| Keywords | `n8n-community-node-package`, scraping, crawling, web-search, data-extraction, pline |

## Operations to endpoints

MCP-only tools and parameters are skipped: `sessions_list`, `save_dir`, `download_dir`,
`max_chars`, `new_session`, `request_id` (correlation id), `wait_seconds` (replaced by the node's
own wait option), and the server-side per-host session memory.

| Resource | Operation | Request | Response handling |
|---|---|---|---|
| Scrape | Scrape a Page | `POST /scrape` (snake_case body) | returns the flat response object as one item |
| Batch Scrape | Start | `POST /batch/scrape` | returns the `202` body (`id`, `url`, `workflowId`, `runId`) |
| Batch Scrape | Get Status | `GET /batch/scrape/{id}` | returns the job object; optional wait, see below |
| Batch Scrape | Cancel | `DELETE /batch/scrape/{id}` | returns `{status: "cancelled"}` |
| Crawl | Start | `POST /crawl` (camelCase body, nested snake_case `scrape`) | returns the `202` body |
| Crawl | Get Status | `GET /crawl/{id}` | returns the job object; optional wait |
| Crawl | Cancel | `DELETE /crawl/{id}` | returns `{status: "cancelled"}` |
| Map | Map a Site | `POST /map` (camelCase body) | one item per link (`url`, `type`, `title`, `description`) by default, with a "Return as single item" toggle |
| Search | Search | `GET /serp` (snake_case query params) | one item per organic result by default, with a toggle to return the whole response (needed for `full` mode extras: `knowledgeGraph`, `peopleAlsoAsk`, `relatedSearches`) |

### Scrape fields

Required: URL. Options collection (sent only when set, so API defaults apply):

`output` (multi-select: html, clean_html, links, markdown, clean_markdown, screenshot,
screenshot_full_page, json; default markdown in the UI), `js_render` (three-way: service decides
/ require browser / forbid browser), `proxy_strategy` (auto, basic, premium),
`only_main_content` (default true), `geolocation` (2-letter), `session_id`, `tag` (comma list),
`actions` (JSON array), `prompt`, `schema` (JSON), `timeout` (1000-60000 ms), `wait_selector`,
`wait_ms` (0-60000), `method` (GET/POST/PUT/DELETE), `body` (JSON or string).

The node repeats the server's local validation so bad combinations fail with a clear message
before spending a request: `json` output needs `prompt` or `schema`; `screenshot` and
`screenshot_full_page` are exclusive; `js_render: false` cannot be combined with actions, wait
options or screenshot output; actions and screenshots are GET only; `body` is not allowed with GET.

### Batch Scrape: Start fields

Required: URLs (one per line or comma separated, or an expression yielding an array). The same
option set as Scrape (same snake_case names per the MCP reference) plus `limit` (1-10000),
`session_id`, `webhook` (url, headers, metadata, events from started/completed/failed), `email`,
`scheduleToCloseTimeoutSecs`, `heartbeatTimeoutSecs`.

### Crawl: Start fields

Required: URL. Options: `limit` (1-10000, API default 10000; the UI description warns that an
unscoped crawl of a large site burns credits and suggests Map first), `output` (multi-select:
markdown, html, clean_html, links, screenshot, screenshot_full_page), `includePaths`,
`excludePaths`, `maxDiscoveryDepth`, `sitemap` (include/only/skip), `ignoreQueryParameters`,
`ignoreRobotsTxt`, `crawlEntireDomain`, `allowSubdomains`, `email`, `webhook` (events may also
include `page`), `scheduleToCloseTimeoutSecs`, `heartbeatTimeoutSecs`, and the per-page fetch
options sent in the nested `scrape` object: `js_render`, `proxy_strategy`, `geolocation`,
`only_main_content`, `actions`, `tag`, `timeout`, `wait_selector`, `wait_ms`.

### Map fields

Required: URL. Options: `sitemap`, `includeSubdomains` (default true), `ignoreQueryParameters`
(default true), `ignoreCache`, `limit` (1-1000000, default 1000), `timeout` (ms).

### Search fields

Required: query (max 512 chars). Options: `country` (default all), `language` (default en),
`num_results` (1-100, default 10), `tbs` (qdr:d / qdr:w / qdr:m), `device`, `mode` (fast = 1
credit, full = 3 credits; descriptions state the cost), `output` (html, clean_html, links; +1
credit per page, stated in the description), `only_main_content`.

## Async crawl and batch jobs

Both start endpoints return `202` with an `id`; the job runs server-side and survives restarts.
The node exposes three operations per job type and no hidden polling on Start, so a workflow can
hand the id to a later node, a webhook, or a schedule.

Get Status takes a "Wait for Completion" option. When on, the node polls `GET .../{id}` every
5 seconds (same interval as the MCP server) until `status` is `completed`, `failed` or `cancelled`,
or until "Maximum Wait (seconds)" passes (default 120, capped at 600 like the MCP server), then
returns the last job object either way. The caller checks `status`; a still-running job is not an
error. The job's `error` field is passed through.

On completion the job object carries `s3PresignedUrls`, one signed and expiring URL per format.
The node returns those URLs as-is. Downloading is left to the HTTP Request node: a batch result is
a JSONL file per format and a crawl result is a ZIP per format, and unpacking a ZIP needs a
dependency, which verification forbids. The README shows the HTTP Request follow-up for the batch
JSONL case. Open question 5 asks whether a built-in "Get Results" for batch JSONL is wanted.

Webhook and email notifications are passed through on Start so users can also go event-driven
with an n8n Webhook trigger instead of polling.

## Credential design

`PlineApi` credential, display name "Pline API", one field:

- API Key: `password: true` (masked). Description links to sign-up and the API key page (URLs are
  open question 1).

Auth is generic header auth: `x-api-key: {{$credentials.apiKey}}`, exactly what the MCP server and
the Python CLI send. Base URL is a constant `https://apix.pline.ai/v1` (the documented public API).
There is no shared, bundled or fallback key anywhere; the credential field is required.

Credential test: `GET /crawl/00000000-0000-0000-0000-000000000000`. The billing page prices every
endpoint (map is 1 credit, so it is not free) but lists no cost for job status polls, and a lookup of
an unknown job does no work. Verified today against the live API: a bad key returns
`403 API_KEY_NOT_AUTHORIZED` on this route, no key returns `401 UNAUTHORIZED`, and a valid key
returns `404` (unknown job). The test treats `404` as success and `401`/`403` as failure. This needs
a programmatic test (`methods.credentialTest`) because the declarative test only accepts 2xx.
Open question 4 asks you to confirm this or to name a dedicated free endpoint.

Credential description text: "Sign in at https://app.pline.ai/login (a key is created for you at
sign-up), then copy it from the API keys page." with the API keys page linked once its URL is known.

## Error mapping

The API returns `{"code", "message"}` bodies (some proxies `{"detail"}`); the node shows the
message plus the code. Verified live today:

| HTTP | Code | Node message |
|---|---|---|
| 401 | `UNAUTHORIZED` | "No API key was sent, or the key was revoked. Check the Pline credential." (docs: a revoked key returns 401) |
| 403 | `API_KEY_NOT_AUTHORIZED` | "The API key is not authorized. Check the key and your workspace status at <API key page>." |
| 429 | none documented | "Rate limit reached or the session is busy. Retry after a short delay." (docs: per-plan requests per second; scrape 429 = session busy, serp 429 = search rate limit; 429s cost no credits; n8n's Retry On Fail covers this) |
| 422 | validation | the API's `detail`/`message`, which names the field |
| 400 | malformed, or session reused across domains | the API message |
| 404 | unknown job id | "No job with this ID." |
| 409 | job no longer cancellable | "This job has already finished and cannot be cancelled." |
| 502 / 503 / 504 | fetch failed / backend unavailable / timeout | the API message, with `request_id` when present |

Out of credits: neither the repo nor the public docs (billing, credit usage, every endpoint's
response-code table) document a status or error code for an exhausted balance (open question 2).
The node will map whatever you confirm; until then any other 4xx shows the API's own message. `continueOnFail` is honored: errors become items with a `json.error` field.

## Verification and UX checklist the build follows

- Zero `dependencies`; only `devDependencies` from the scaffold (n8n-workflow, eslint plugin, typescript).
- MIT license, English only, `n8n-community-node-package` keyword, `n8n` block with one credential and one node.
- Node naming per guidelines: resource and operation structure, sentence-case display names,
  descriptions on every field, options collection for optional fields, `noDataExpression` on selects.
- `usableAsTool: true` so the node works with AI agents (allowed for verified nodes).
- Icon: `plugin/assets/icon.png` converted to SVG, or an SVG you provide (open question 6).
- Lint with `@n8n/scan-community-package` and the scaffold's eslint config; build with `tsc`.

## Decisions (approved 2026-10-27)

1. Sign-up: https://app.pline.ai/signup. Login: https://app.pline.ai/login. API keys page: "API keys" in the app after sign-in.
2. Out of credits returns `429`, same as rate limiting; the API's `message` distinguishes them, so the node shows the API message for every 429 plus a hint covering both.
3. Code lives in `n8n-pline/` in this repo (`grepsr/pline-api`). npm org `pline`, npm user `pline.ai`.
4. Credential test: `GET /crawl/<nil uuid>`, 404 = valid key, 401/403 = invalid. Implemented as a declarative `test` with `ignoreHttpStatusErrors` and `responseSuccessBody` rules on the error `code`, because `ICredentialTestFunctions` only offers the deprecated request helper.
5. Batch Scrape gets a "Get Results" operation: downloads the completed JSONL files and returns one item per line.
6. Icon converted from `plugin/assets/icon.png` to SVG. Author Grepsr, dev@grepsr.com.
7. Base URL fixed on the public API; no base URL field.
