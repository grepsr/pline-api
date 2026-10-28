# n8n-nodes-pline

An [n8n](https://n8n.io/) community node for the [Pline](https://pline.ai) API. Pline fetches web
content for you and handles blocked pages, retries and access escalation server-side: scrape one
page, batch-scrape a list of URLs, crawl a whole site, map a site's URLs, or get parsed web search
results.

Pline is a paid service with a free tier. You bring your own API key; this node never includes or
falls back to a shared key, and every request counts against your own workspace's credits.

- [Get a Pline API key](#get-a-pline-api-key)
- [Set up the credential in n8n](#set-up-the-credential-in-n8n)
- [Installation](#installation)
- [Operations](#operations)
- [Example workflow](#example-workflow)
- [Errors](#errors)
- [Compatibility](#compatibility)
- [Development](#development)
- [Resources](#resources)

## Get a Pline API key

1. Sign up at <https://app.pline.ai/signup>. The free tier includes credits to try the API.
2. Sign in at <https://app.pline.ai/login> and open **API keys**. A key is created for you when
   you sign up; copy it. Keys can be rotated and revoked there at any time.
3. Keep the key private. n8n stores it encrypted; never paste it into workflow fields or
   expressions.

Every request is billed in credits by what it needed (a plain fetch, a browser, a stronger proxy,
AI extraction). See [Credits and usage](https://apidocs.pline.ai/guides/billing/credit-usage).

## Set up the credential in n8n

1. In n8n open **Credentials**, click **Add credential**, and search for **Pline API**.
2. Paste your API key into **API Key** (the field is masked).
3. Click **Save**. n8n tests the key with a request that spends no credits: a status lookup for a
   job that cannot exist. A valid key answers "not found", which the test reports as success; a
   wrong or revoked key answers 401 or 403 and the test shows which.

Every operation of the Pline node uses this credential and sends the key as the `x-api-key`
header to `https://apix.pline.ai/v1`. Nothing else is configurable.

## Installation

Follow the [community nodes installation guide](https://docs.n8n.io/integrations/community-nodes/installation/)
and install `n8n-nodes-pline`. On n8n Cloud, verified community nodes can be installed from the
nodes panel.

## Operations

All optional fields are sent only when you set them, so the API's own defaults apply otherwise.

### Scrape: Scrape a Page

`POST /scrape`. Fetches one URL synchronously and returns a flat object: `data` (one key per
requested format: `html_body`, `clean_html`, `links`, `markdown`, `clean_markdown`, `json`,
`screenshot`, `screenshot_full_page`), the page's `status`, `final_url`, `session_id`,
`request_id`, `effective_geolocation` and `site_tier`.

| Field | Notes |
|---|---|
| URL | required |
| Output Formats | default Markdown. JSON needs a Prompt and/or Schema. Screenshot and Screenshot (Full Page) are exclusive. |
| JavaScript Rendering | Let Pline Decide (default), Require a Browser, Forbid a Browser |
| Proxy Strategy | Auto (default), Basic, Premium, in ascending cost |
| Only Main Content, Geolocation, Tags, Timeout, Wait for Selector, Wait | per-page fetch controls |
| Actions | JSON array of browser steps (click, type, scroll, wait, ...). Implies a browser and GET. |
| Prompt, Schema | drive JSON extraction. A list-shaped schema returns an array. |
| Method, Body | for API-style targets (POST/PUT/DELETE with a body); not with a browser |
| Session ID | reuse the `session_id` from an earlier scrape of the same site to keep cookies and settled access choices, which is cheaper |

Screenshot outputs are temporary signed URLs; download them promptly. A `200` whose `data` is
empty is a soft failure: retry with Require a Browser, or without a Basic proxy pin.

### Batch Scrape

`POST /batch/scrape` and friends. The same per-page options applied to a list of URLs, as a
background job that survives restarts.

| Operation | Endpoint | Returns |
|---|---|---|
| Start | `POST /batch/scrape` | the `202` body: `id`, `url`, `workflowId`, `runId` |
| Get Status | `GET /batch/scrape/{id}` | the job: `status` (`scraping`, `completed`, `failed`, `cancelled`), counts, and on completion `s3PresignedUrls` (one signed JSONL download per format) |
| Get Results | `GET /batch/scrape/{id}` + downloads | one item per URL: `index`, `url`, `success`, `status`, `finalUrl`, `error`, plus one key per format (`markdown`, `html`, ...) holding that URL's output |
| Cancel | `DELETE /batch/scrape/{id}` | `{"status": "cancelled"}`, or 409 when the job already finished |

Start takes URLs one per line or comma-separated (an expression may return an array), the output
formats, the scrape options above, Maximum URLs, and delivery options: Email, Webhook URL,
Webhook Events (`started`, `completed`, `failed`), Webhook Headers and Webhook Metadata.

### Crawl

`POST /crawl` and friends. Discovers pages by following links from a seed URL and scrapes them,
as a background job.

| Operation | Endpoint | Returns |
|---|---|---|
| Start | `POST /crawl` | the `202` body with the job `id` |
| Get Status | `GET /crawl/{id}` | the job, and on completion `s3PresignedUrls`: one signed ZIP per format, each with a `manifest.json` mapping files to source URLs |
| Cancel | `DELETE /crawl/{id}` | `{"status": "cancelled"}` |

Start takes the URL, Maximum Pages (default 100 in the node; the API allows up to 10000), Output
Formats, Crawl Options (Include Paths and Exclude Paths as regexes, Maximum Discovery Depth,
Sitemap, Ignore Query Parameters, Ignore Robots.txt, Crawl Entire Domain, Allow Subdomains,
Email, Webhook fields, timeouts) and Page Options, the per-page fetch controls applied to every
page. Each crawled page is billed at the scrape cost for the options you choose, so size a site
with Map first and keep Maximum Pages tight.

Crawl results are ZIP files; use the HTTP Request node to download the URLs from `s3PresignedUrls`
and the Compression node to unpack them.

### Waiting for a job

Get Status and Get Results have a **Wait for Completion** toggle. When on, the node polls every 5
seconds until the job is completed, failed or cancelled, or until **Maximum Wait** (default 120 s,
at most 600 s) passes, then returns the latest job object. A job that is still running is not an
error: check `status`. For long jobs, prefer the Email or Webhook delivery options and an n8n
Webhook trigger over polling.

### Map: Map a Site

`POST /map`. Synchronous URL discovery from the site's sitemaps, with search as a fallback. Returns
one item per link (`url`, `type`, `title`, `description`) or, with Split Into Items off, the whole
response. Options: Sitemap, Include Subdomains, Ignore Query Parameters, Ignore Cache, Maximum
Links, Timeout.

### Search: Search

`GET /serp`. Parsed web search results. Returns one item per organic result (`position`,
`title`, `link`, `snippet`, `domain`, `page`), or with Split Into Items off the whole response,
which in Full mode also carries `knowledgeGraph`, `peopleAlsoAsk`, `relatedSearches` and
`totalOrganicResults`. Options: Country, Language, Number of Results, Time Range, Device, Mode
(Fast is cheaper than Full), Page Content (also fetch each result page; one extra credit per page)
and Only Main Content.

## Example workflow

Scrape a pricing page as Markdown and extract the plans as JSON:

1. **Manual Trigger**.
2. **Pline** node: Resource *Scrape*, Operation *Scrape a Page*, URL `https://example.com/pricing`,
   Output Formats *Markdown* and *JSON*, Options → Prompt `List every plan with its name, monthly
   price and included credits`.
3. The output item has `data.markdown` and `data.json`. Connect a **Google Sheets** or **Slack**
   node and reference `{{ $json.data.json }}`.

Batch example: a **Pline** *Map a Site* node feeds its items into a **Pline** *Batch Scrape →
Start* node with URLs set to `{{ $input.all().map(i => i.json.url) }}`, then a **Pline** *Batch
Scrape → Get Results* node with Job ID `{{ $json.id }}` and Wait for Completion on returns one
item per page.

Importable JSON for the first example:

```json
{
  "nodes": [
    { "parameters": {}, "name": "Manual Trigger", "type": "n8n-nodes-base.manualTrigger", "typeVersion": 1, "position": [0, 0] },
    {
      "parameters": {
        "resource": "scrape",
        "operation": "scrape",
        "url": "https://example.com/pricing",
        "output": ["markdown", "json"],
        "options": { "prompt": "List every plan with its name, monthly price and included credits" }
      },
      "name": "Pline",
      "type": "n8n-nodes-pline.pline",
      "typeVersion": 1,
      "position": [220, 0],
      "credentials": { "plineApi": { "name": "Pline API" } }
    }
  ],
  "connections": { "Manual Trigger": { "main": [[{ "node": "Pline", "type": "main", "index": 0 }]] } }
}
```

## Errors

The node shows the API's own message and code, plus what to do:

| HTTP | Meaning |
|---|---|
| 401 | No API key was sent, or the key was revoked. Check the credential. |
| 403 | `API_KEY_NOT_AUTHORIZED`: the key is wrong or the workspace is inactive. |
| 429 | Rate limit reached, session busy, or the workspace is out of credits; the message says which. Retry after a short delay or top up. Use the node's *Retry On Fail* setting for transient 429s. |
| 400 / 422 | Invalid request or option combination; the message names the field. The node also checks the documented combinations before sending. |
| 404 / 409 | Unknown job ID / job already finished and cannot be cancelled. |
| 502 / 503 / 504 | Every fetch attempt failed, a Pline backend is unavailable, or the request exceeded its time budget. |

With *Continue On Fail* enabled, failed items are returned with an `error` field instead of
stopping the workflow.

## Compatibility

Built and tested against n8n 1.x with `n8n-workflow` as a peer dependency. Requires Node.js 20.15
or newer to build. The package has no runtime dependencies.

## Development

```sh
cd n8n-pline
npm install
npm run lint      # n8n community-node lint rules
npm run build     # compiles to dist/
npm test          # smoke test: asserts the exact requests each operation sends
npm run dev       # starts a local n8n at http://localhost:5678 with this node linked, hot reload
```

In the local n8n, add a **Pline API** credential with your key, add a **Pline** node, and run it.
The n8n CLI needs Node 22.12 or newer; on Node 22.11 prefix the commands with
`NODE_OPTIONS=--experimental-require-module`.

## Resources

- [Pline API documentation](https://apidocs.pline.ai/)
- [Pline API reference](https://apidocs.pline.ai/api-reference)
- [n8n community nodes documentation](https://docs.n8n.io/integrations/#community-nodes)
- [Issues](https://github.com/grepsr/pline-api/issues)
