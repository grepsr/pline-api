# pline.ai plugin

Scrape, crawl, batch-scrape, map, and search the web with [pline.ai](https://pline.ai), from Claude.

The plugin bundles two things:

- **The pline.ai MCP server**, hosted at `https://mcp.pline.ai/mcp`. It works wherever Claude
  supports remote MCP servers, including claude.ai, Cowork, and Claude Code.
- **The `pline-api` agent skill**, which teaches Claude when to use each tool, how to scope crawls,
  reuse scrape sessions, extract structured JSON, and follow long-running jobs to completion.

## Getting started

1. Install the plugin from the Claude directory, or in Claude Code (2.1.275 or later):

   ```text
   /plugin install pline-ai --marketplace grepsr/pline-api
   ```
2. When Claude first connects to the server, sign in with your pline.ai account.
3. On the consent page, choose the workspace and API key the plugin should use, and approve.

No API key is pasted into Claude: the server resolves the key you selected for each request, and you
can change or disconnect it at any time at <https://app.pline.ai/mcp/connections>. Usage counts against
the selected workspace's pline.ai plan.

## Tools

| Tool | What it does |
|---|---|
| `scrape` | Fetch one page as Markdown, HTML, a screenshot, or JSON extracted to a prompt or schema |
| `batch_scrape_start`, `batch_scrape_status`, `batch_scrape_cancel` | Scrape a list of URLs as a background job |
| `crawl_start`, `crawl_status`, `crawl_cancel` | Crawl a site as a background job |
| `map_site` | Discover a site's URLs |
| `search` | Get web search results |
| `sessions_list` | List remembered scrape sessions |

The server also provides the `pline://guide` and `pline://reference` resources.

## Example prompts

- "Scrape https://example.com as Markdown."
- "Extract the product name and price from this page as JSON."
- "Map the URLs on https://example.com, then scrape the pricing page."
- "Crawl the docs section of https://example.com, at most 50 pages, and summarize it."

## Privacy policy

pline.ai's [privacy policy](https://www.pline.ai/privacy-policy) applies to all data the plugin sends.

- **What is sent:** the URLs, search queries, and options for each tool call go to the pline.ai API
  (`https://apix.pline.ai`) through the hosted MCP server, authenticated as the signed-in pline.ai user.
- **Storage:** the plugin stores nothing on your machine. The hosted server does not persist request
  data; it looks up your selected API key for each request. pline.ai's handling of job results and
  account data is described in its privacy policy.
- **Third parties:** pages you ask to scrape or crawl are fetched from their own websites. Other
  sharing is covered by the privacy policy.
- **Retention:** as set out in the privacy policy.
- **Contact:** see the privacy policy, or open an issue at <https://github.com/grepsr/pline-api/issues>.

Terms of service: <https://www.pline.ai/terms-of-service>.

## Support

Report problems at <https://github.com/grepsr/pline-api/issues>. Source, a local (stdio) version of the
server, and setup for other MCP clients are in the [pline-api repository](https://github.com/grepsr/pline-api).

## License

MIT
