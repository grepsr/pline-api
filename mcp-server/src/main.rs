//! pline.ai MCP server: scrape, batch scrape, crawl, map, and search tools over stdio or
//! Streamable HTTP.

mod client;
mod http_server;

use std::collections::HashMap;
use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, ErrorData, Extensions, Implementation, ListResourcesResult,
    PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult,
    Resource, ResourceContents, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{tool, tool_handler, tool_router, RoleServer, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use client::{PlineClient, PlineError, ScrapeOptions, SessionMemory};
pub use http_server::ApiKey;

const GUIDE: &str = include_str!("../docs/guide.md");
const REFERENCE: &str = include_str!("../docs/reference.md");
const GUIDE_URI: &str = "pline://guide";
const REFERENCE_URI: &str = "pline://reference";

const INSTRUCTIONS: &str = "\
pline.ai fetches web content: one page (scrape), many known URLs (batch_scrape_*), a whole site by \
following links (crawl_*), a site's URL list (map_site), or search results (search). Blocked pages, \
retries, and access escalation are handled server-side; there is nothing to configure per site.

Rules of thumb:
- Reach for `scrape` first. Use `batch_scrape_start` for a known URL list, `crawl_start` only when URLs \
must be discovered, and `map_site` before a crawl to size it.
- Leave `js_render` unset so the service decides; set true only when content is known to be \
client-rendered, false for plain HTML/API targets where cheap-and-fast beats escalation.
- Leave `proxy_strategy` unset (auto). Pin `basic` for open sites to save cost, `enhanced` or `premium` \
only when a site keeps failing and the caller accepts higher cost.
- Sessions are the main cost lever: the server remembers the session per host and reuses it. Do not pass \
`new_session=true` unless a fresh identity is actually wanted.
- Prefer `markdown` for reading or LLM input; `html` only when markup matters. Pass `save_dir` for \
anything but small pages when the server runs locally.
- Quote `request_id` when reporting a problem. Read `pline://guide` for the full guidance and \
`pline://reference` for every field and constraint.

How pline.ai obtains a page beyond the documented fields is not part of this interface: request \
headers, timeouts, wait conditions, and the fetching method are not caller-controlled. Say so if asked, \
rather than speculating.";

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum ProxyStrategy {
    Auto,
    Basic,
    Enhanced,
    Premium,
}

impl ProxyStrategy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Basic => "basic",
            Self::Enhanced => "enhanced",
            Self::Premium => "premium",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl HttpMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
enum SitemapMode {
    Skip,
    #[default]
    Include,
    Only,
}

impl SitemapMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Include => "include",
            Self::Only => "only",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
enum SerpMode {
    Fast,
    #[default]
    Full,
}

impl SerpMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Full => "full",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
enum SerpDevice {
    #[default]
    Desktop,
    Mobile,
    Tablet,
}

impl SerpDevice {
    fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
            Self::Tablet => "tablet",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
enum TimeFilter {
    #[serde(rename = "qdr:d")]
    Day,
    #[serde(rename = "qdr:w")]
    Week,
    #[serde(rename = "qdr:m")]
    Month,
}

impl TimeFilter {
    fn as_str(self) -> &'static str {
        match self {
            Self::Day => "qdr:d",
            Self::Week => "qdr:w",
            Self::Month => "qdr:m",
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_max_chars() -> usize {
    60_000
}

fn default_batch_limit() -> u32 {
    10_000
}

fn default_crawl_limit() -> u32 {
    200
}

fn default_map_limit() -> u32 {
    1_000
}

fn default_country() -> String {
    "US".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_num_results() -> u32 {
    10
}

/// Options shared by `scrape`, `batch_scrape_start`, and (as a subset) `crawl_start`.
#[derive(Debug, Deserialize, JsonSchema)]
struct FetchOptions {
    /// Formats to return, any combination of html, clean_html, links, markdown, screenshot,
    /// screenshot_full_page, json. Default ["html"]. Returned under `data` as html_body, clean_html,
    /// links, markdown, screenshot, screenshot_full_page, json.
    #[serde(default)]
    output: Option<Vec<String>>,
    /// JavaScript rendering, three-way. Unset (default): the service decides, escalating to a browser
    /// only when a plain fetch comes back empty or as a JS shell. true: require a browser from the
    /// first attempt. false: forbid a browser; cheapest and fastest, fails instead of escalating.
    /// false cannot be combined with actions or screenshot output.
    #[serde(default)]
    js_render: Option<bool>,
    /// Access-strength tier in ascending cost. Unset/auto: start cheap and escalate only as far as
    /// the site requires. basic: cheapest tier only, fails rather than escalating. enhanced /
    /// premium: start at a stronger, costlier tier.
    #[serde(default)]
    proxy_strategy: Option<ProxyStrategy>,
    /// Strip navigation, footer, and ads from clean_html, links, markdown, and json input.
    #[serde(default = "default_true")]
    only_main_content: bool,
    /// Fetch as a visitor from this 2-letter ISO country, e.g. "US".
    #[serde(default)]
    geolocation: Option<String>,
    /// Up to 25 labels, 1-25 chars each of [A-Za-z0-9._-], echoed in usage logs.
    #[serde(default)]
    tag: Option<Vec<String>>,
    /// Browser steps run in order after load, e.g. {"action":"click","selector":"#more"},
    /// {"action":"type","selector":"input","value":"q"}, {"action":"scroll","value":"800"},
    /// {"action":"wait","value":"1500"}, {"action":"wait_for_selector","selector":".grid"}. Any
    /// action implies a browser and method GET. See pline://reference for the full table.
    #[serde(default)]
    actions: Option<Vec<Value>>,
    /// Extraction instructions for output json (max 16384 chars).
    #[serde(default)]
    prompt: Option<String>,
    /// Desired JSON shape for output json; lenient, not strict JSON Schema, e.g.
    /// {"name":"string","price":"number"}. A list-shaped schema extracts a listing page and returns
    /// an array.
    #[serde(default)]
    schema: Option<Value>,
    /// HTTP method for API-style targets; browser paths are GET only. Default GET.
    #[serde(default)]
    method: Option<HttpMethod>,
    /// Request body (JSON object or string) for POST/PUT/DELETE; not allowed with GET or a browser.
    #[serde(default)]
    body: Option<Value>,
}

impl FetchOptions {
    fn as_scrape_options(&self) -> ScrapeOptions<'_> {
        ScrapeOptions {
            output: self.output.as_deref(),
            js_render: self.js_render,
            proxy_strategy: self.proxy_strategy.map(ProxyStrategy::as_str),
            only_main_content: self.only_main_content,
            geolocation: self.geolocation.as_deref(),
            tag: self.tag.as_deref(),
            actions: self.actions.as_deref(),
            prompt: self.prompt.as_deref(),
            schema: self.schema.as_ref(),
            method: self.method.map(HttpMethod::as_str).unwrap_or("GET"),
            body: self.body.as_ref(),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ScrapeParams {
    /// Absolute http/https URL to fetch (max 2048 chars).
    url: String,
    #[serde(flatten)]
    options: FetchOptions,
    /// Explicit session id to reuse. Unset: the server reuses the session it remembers for this host.
    #[serde(default)]
    session_id: Option<String>,
    /// Start a fresh session for this host instead of reusing the remembered one.
    #[serde(default)]
    new_session: bool,
    /// Your correlation id; echoed back as request_id.
    #[serde(default)]
    request_id: Option<String>,
    /// Write each output to a file in this directory on the server's filesystem and return paths
    /// instead of content. Available only over stdio; HTTP rejects this option.
    #[serde(default)]
    save_dir: Option<String>,
    /// Clip each returned output to this many characters (0 = no clipping). Default 60000.
    #[serde(default = "default_max_chars")]
    max_chars: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct BatchScrapeStartParams {
    /// URLs to scrape with identical options (max 10000).
    urls: Vec<String>,
    #[serde(flatten)]
    options: FetchOptions,
    /// Session id applied to every URL; all URLs must then share one registrable domain.
    #[serde(default)]
    session_id: Option<String>,
    /// Maximum URLs accepted for this job (1-10000). Default 10000.
    #[serde(default = "default_batch_limit")]
    limit: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct JobStatusParams {
    /// Job UUID returned by the start tool.
    id: String,
    /// Poll until the job finishes or this many seconds pass (0 = return current status now; max 600).
    #[serde(default)]
    wait_seconds: f64,
    /// When the job is completed, download every artifact into this directory on the server's
    /// filesystem. Available only over stdio; HTTP rejects this option.
    #[serde(default)]
    download_dir: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct JobIdParams {
    /// Job UUID returned by the start tool.
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CrawlStartParams {
    /// Seed URL; the crawl follows links from here.
    url: String,
    /// Maximum pages (1-10000). Default 200. Confirm scope with the user for big sites.
    #[serde(default = "default_crawl_limit")]
    limit: u32,
    /// Formats to store, any of markdown, html, clean_html, links, screenshot. Default ["markdown"].
    #[serde(default)]
    output: Option<Vec<String>>,
    /// Regexes; only matching paths are crawled.
    #[serde(default)]
    include_paths: Option<Vec<String>>,
    /// Regexes; matching paths are skipped.
    #[serde(default)]
    exclude_paths: Option<Vec<String>>,
    /// Link depth from the seed.
    #[serde(default)]
    max_discovery_depth: Option<u32>,
    /// include: use sitemaps plus links; only: sitemaps alone; skip: ignore sitemaps. Default include.
    #[serde(default)]
    sitemap: SitemapMode,
    /// Drop URLs carrying query strings.
    #[serde(default)]
    ignore_query_parameters: bool,
    /// Allow leaving the seed's path prefix.
    #[serde(default)]
    crawl_entire_domain: bool,
    /// Include subdomains of the seed's domain.
    #[serde(default)]
    allow_subdomains: bool,
    /// Same meaning as on scrape, applied to every page fetch.
    #[serde(default)]
    js_render: Option<bool>,
    /// Same meaning as on scrape, applied to every page fetch.
    #[serde(default)]
    proxy_strategy: Option<ProxyStrategy>,
    /// Fetch every page as a visitor from this 2-letter ISO country.
    #[serde(default)]
    geolocation: Option<String>,
    /// Strip navigation, footer, and ads from cleaned outputs.
    #[serde(default = "default_true")]
    only_main_content: bool,
    /// Browser steps run on every page after load; implies a browser.
    #[serde(default)]
    actions: Option<Vec<Value>>,
    /// Up to 25 labels echoed in usage logs.
    #[serde(default)]
    tag: Option<Vec<String>>,
    /// Address notified when the crawl completes.
    #[serde(default)]
    email: Option<String>,
    /// Public HTTPS URL receiving started/page/completed/failed events.
    #[serde(default)]
    webhook_url: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct MapSiteParams {
    /// Base website URL to discover URLs for.
    url: String,
    /// include: sitemaps, search fallback only when none exist; only: sitemaps alone; skip: search only.
    #[serde(default)]
    sitemap: SitemapMode,
    /// Include subdomains of the registrable domain. Default true.
    #[serde(default = "default_true")]
    include_subdomains: bool,
    /// Exclude URLs carrying query strings. Default true.
    #[serde(default = "default_true")]
    ignore_query_parameters: bool,
    /// Bypass discovery caches and refresh.
    #[serde(default)]
    ignore_cache: bool,
    /// Maximum links to return (1-1000000). Default 1000.
    #[serde(default = "default_map_limit")]
    limit: u32,
    /// Whole-request budget in ms; the service answers 504 when exceeded.
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchParams {
    /// Search keywords (max 512 chars).
    query: String,
    /// 2-letter country code for localization. Default US.
    #[serde(default = "default_country")]
    country: String,
    /// 2-letter result language code. Default en.
    #[serde(default = "default_language")]
    language: String,
    /// Organic results to return (1-100). Default 10.
    #[serde(default = "default_num_results")]
    num_results: u32,
    /// Time filter: qdr:d (past day), qdr:w (past week), qdr:m (past month).
    #[serde(default)]
    tbs: Option<TimeFilter>,
    /// Device profile. Default desktop.
    #[serde(default)]
    device: SerpDevice,
    /// fast = 1 credit, organic results only; full = 3 credits, adds knowledge graph, people-also-ask,
    /// related searches, sitelinks. Default full.
    #[serde(default)]
    mode: SerpMode,
    /// Also fetch each result page: any of html, clean_html, links. Best-effort, +1 credit per page.
    #[serde(default)]
    output: Option<Vec<String>>,
    /// Strip navigation and layout from enriched page content. Default true.
    #[serde(default = "default_true")]
    only_main_content: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NoParams {}

#[derive(Clone)]
pub struct PlineServer {
    api: Arc<PlineClient>,
    sessions: Arc<SessionMemory>,
    allow_local_files: bool,
}

/// The pline.ai API key for one tool call.
///
/// Over HTTP the key arrives with every request (see `http_server`) and is read from the injected
/// request parts; over stdio the server is a single-user local process and the key comes from
/// `PLINE_API_KEY`.
fn api_key_for(extensions: &Extensions) -> client::Result<String> {
    if let Some(parts) = extensions.get::<http::request::Parts>() {
        return parts
            .extensions
            .get::<ApiKey>()
            .map(|key| key.0.clone())
            .ok_or_else(|| PlineError::Transport("request carried no pline.ai API key".into()));
    }
    client::api_key_from_env()
}

fn tool_error(error: impl std::fmt::Display) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error.to_string())])
}

fn ok(value: Value) -> CallToolResult {
    CallToolResult::structured(value)
}

fn to_result(outcome: client::Result<Value>) -> CallToolResult {
    match outcome {
        Ok(value) => ok(value),
        Err(error) => tool_error(error),
    }
}

macro_rules! api_key_or_return {
    ($extensions:expr) => {
        match api_key_for(&$extensions) {
            Ok(key) => key,
            Err(error) => return tool_error(error),
        }
    };
}

#[tool_router]
impl PlineServer {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            api: Arc::new(PlineClient::new()?),
            sessions: Arc::new(SessionMemory::default()),
            allow_local_files: true,
        })
    }

    /// Fetch one page synchronously and return its content in the requested formats (POST /scrape).
    ///
    /// Returns the flat pline.ai response: data (keyed by output), status (the page's HTTP status),
    /// final_url, session_id, request_id, effective_geolocation, site_tier. A 200 with empty data is a
    /// soft failure: retry with js_render=true if it was unset, or drop a proxy_strategy pin. The
    /// session returned is remembered per host and reused on the next scrape of that host.
    #[tool(
        name = "scrape",
        annotations(
            title = "Scrape one page",
            read_only_hint = false,
            open_world_hint = true
        )
    )]
    async fn scrape(
        &self,
        extensions: Extensions,
        Parameters(params): Parameters<ScrapeParams>,
    ) -> CallToolResult {
        let key = api_key_or_return!(extensions);
        to_result(self.scrape_impl(&key, params).await)
    }

    /// Scrape a known list of URLs as one durable background job (POST /batch/scrape).
    ///
    /// Returns 202 metadata with the job id. Poll with batch_scrape_status. Completed jobs expose one
    /// JSONL artifact per output format, one line per URL: index, url, success, status, finalUrl,
    /// output, error. Cheaper and more predictable than a crawl when the URLs are already known.
    #[tool(
        name = "batch_scrape_start",
        annotations(title = "Start a batch scrape job", open_world_hint = true)
    )]
    async fn batch_scrape_start(
        &self,
        extensions: Extensions,
        Parameters(params): Parameters<BatchScrapeStartParams>,
    ) -> CallToolResult {
        let key = api_key_or_return!(extensions);
        to_result(self.batch_scrape_start_impl(&key, params).await)
    }

    /// Read a batch scrape job's progress (GET /batch/scrape/{id}).
    ///
    /// status is scraping | completed | failed | cancelled with completed/failed/pending/inFlight
    /// counts. On completion s3PresignedUrls maps each format to an expiring JSONL download. Long
    /// jobs are fine to leave running: report the id instead of blocking.
    #[tool(
        name = "batch_scrape_status",
        annotations(title = "Check a batch scrape job", read_only_hint = false)
    )]
    async fn batch_scrape_status(
        &self,
        extensions: Extensions,
        Parameters(params): Parameters<JobStatusParams>,
    ) -> CallToolResult {
        let key = api_key_or_return!(extensions);
        to_result(
            self.job_status(&key, "/batch/scrape", "jsonl", params)
                .await,
        )
    }

    /// Request cancellation of a batch scrape job (DELETE /batch/scrape/{id}). 409 if already finished.
    #[tool(
        name = "batch_scrape_cancel",
        annotations(title = "Cancel a batch scrape job")
    )]
    async fn batch_scrape_cancel(
        &self,
        extensions: Extensions,
        Parameters(params): Parameters<JobIdParams>,
    ) -> CallToolResult {
        let key = api_key_or_return!(extensions);
        to_result(self.cancel_job(&key, "/batch/scrape", &params.id).await)
    }

    /// Discover and scrape a whole site by following links, as a durable background job (POST /crawl).
    ///
    /// Returns 202 metadata with the job id. Poll with crawl_status. Completed jobs expose one ZIP per
    /// format with a manifest.json mapping files to source URLs. Scope it first: an unscoped crawl of
    /// a large site burns credits fast, and map_site shows what a crawl would hit for one cheap call.
    #[tool(
        name = "crawl_start",
        annotations(title = "Start a site crawl job", open_world_hint = true)
    )]
    async fn crawl_start(
        &self,
        extensions: Extensions,
        Parameters(params): Parameters<CrawlStartParams>,
    ) -> CallToolResult {
        let key = api_key_or_return!(extensions);
        to_result(self.crawl_start_impl(&key, params).await)
    }

    /// Read a crawl job's progress (GET /crawl/{id}).
    ///
    /// status is scraping | completed | failed | cancelled with completed/failed/pending/inFlight
    /// counts. On completion s3PresignedUrls maps each format to an expiring ZIP download. Long jobs
    /// are fine to leave running: report the id instead of blocking.
    #[tool(
        name = "crawl_status",
        annotations(title = "Check a crawl job", read_only_hint = false)
    )]
    async fn crawl_status(
        &self,
        extensions: Extensions,
        Parameters(params): Parameters<JobStatusParams>,
    ) -> CallToolResult {
        let key = api_key_or_return!(extensions);
        to_result(self.job_status(&key, "/crawl", "zip", params).await)
    }

    /// Request cancellation of a crawl job (DELETE /crawl/{id}). 409 if already finished.
    #[tool(name = "crawl_cancel", annotations(title = "Cancel a crawl job"))]
    async fn crawl_cancel(
        &self,
        extensions: Extensions,
        Parameters(params): Parameters<JobIdParams>,
    ) -> CallToolResult {
        let key = api_key_or_return!(extensions);
        to_result(self.cancel_job(&key, "/crawl", &params.id).await)
    }

    /// List a site's URLs synchronously (POST /map), each with a type classification plus title and
    /// description when known.
    ///
    /// Cheap. Use it to size up a site before crawl_start, or when only the URL list is needed.
    #[tool(
        name = "map_site",
        annotations(
            title = "Discover a site's URLs",
            read_only_hint = true,
            open_world_hint = true
        )
    )]
    async fn map_site(
        &self,
        extensions: Extensions,
        Parameters(params): Parameters<MapSiteParams>,
    ) -> CallToolResult {
        let key = api_key_or_return!(extensions);
        to_result(self.map_site_impl(&key, params).await)
    }

    /// Search the web (GET /serp) and return organic results, plus search features in full mode.
    ///
    /// Organic results carry position, title, link, snippet, domain, page, and
    /// html_body/clean_html/links when output was requested.
    #[tool(
        name = "search",
        annotations(
            title = "Get search results",
            read_only_hint = true,
            open_world_hint = true
        )
    )]
    async fn search(
        &self,
        extensions: Extensions,
        Parameters(params): Parameters<SearchParams>,
    ) -> CallToolResult {
        let key = api_key_or_return!(extensions);
        to_result(self.search_impl(&key, params).await)
    }

    /// Return the host -> session_id map this server is reusing for you. Empty until the first scrape.
    #[tool(
        name = "sessions_list",
        annotations(title = "List remembered sessions", read_only_hint = true)
    )]
    async fn sessions_list(
        &self,
        extensions: Extensions,
        Parameters(_params): Parameters<NoParams>,
    ) -> CallToolResult {
        let key = api_key_or_return!(extensions);
        let sessions: HashMap<String, String> = self.sessions.snapshot(&key).await;
        ok(json!(sessions))
    }
}

impl PlineServer {
    fn check_local_output(&self, directory: Option<&str>) -> client::Result<()> {
        if !self.allow_local_files && directory.is_some_and(|dir| !dir.trim().is_empty()) {
            return Err(PlineError::Invalid(
                "save_dir and download_dir are only available over stdio".into(),
            ));
        }
        Ok(())
    }

    async fn scrape_impl(&self, key: &str, params: ScrapeParams) -> client::Result<Value> {
        self.check_local_output(params.save_dir.as_deref())?;
        let mut body = client::build_scrape_options(params.options.as_scrape_options())?;
        let host = client::host_of(&params.url)?;
        body.insert("url".into(), json!(params.url.trim()));
        let headers: Vec<(&str, String)> = params
            .request_id
            .as_deref()
            .map(|id| vec![("x-request-id", id.to_string())])
            .unwrap_or_default();

        let host_lock = self.sessions.lock_for(key, &host).await;
        let _serialized = host_lock.lock().await;
        let explicit = params.session_id.filter(|id| !id.trim().is_empty());
        let chosen = match &explicit {
            Some(id) => Some(id.clone()),
            None if params.new_session => None,
            None => self.sessions.get(key, &host).await,
        };
        if let Some(session_id) = &chosen {
            body.insert("session_id".into(), json!(session_id));
        }
        let response = match self
            .api
            .request(
                key,
                "POST",
                "/scrape",
                Some(&Value::Object(body)),
                &[],
                &headers,
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                let remembered_session_rejected = explicit.is_none()
                    && chosen.is_some()
                    && matches!(&error, PlineError::Api { status, .. } if (400..500).contains(status));
                if remembered_session_rejected {
                    self.sessions.forget(key, &host).await;
                }
                return Err(error);
            }
        };
        let Value::Object(mut response) = response else {
            return Err(PlineError::Transport(format!(
                "unexpected /scrape payload: {response}"
            )));
        };
        self.sessions
            .remember(
                key,
                &host,
                response.get("session_id").and_then(Value::as_str),
            )
            .await;

        let data = response
            .get("data")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if client::data_is_empty(&data) {
            response.insert(
                "warning".into(),
                json!("data is empty; if js_render was unset retry with js_render=true"),
            );
        }
        if let Some(save_dir) = params
            .save_dir
            .as_deref()
            .filter(|dir| !dir.trim().is_empty())
        {
            let saved = client::save_scrape_data(&data, save_dir).await?;
            let mut replaced = Map::new();
            for (key, value) in &data {
                replaced.insert(
                    key.clone(),
                    saved.get(key).cloned().unwrap_or_else(|| value.clone()),
                );
            }
            response.insert("data".into(), Value::Object(replaced));
            response.insert("saved_files".into(), Value::Object(saved));
        } else {
            let (clipped, truncated) = client::truncate_data(&data, params.max_chars);
            response.insert("data".into(), Value::Object(clipped));
            if !truncated.is_empty() {
                response.insert("truncated_outputs".into(), json!(truncated));
            }
        }
        Ok(Value::Object(response))
    }

    async fn batch_scrape_start_impl(
        &self,
        key: &str,
        params: BatchScrapeStartParams,
    ) -> client::Result<Value> {
        let options = client::build_scrape_options(params.options.as_scrape_options())?;
        let urls: Vec<String> = params
            .urls
            .iter()
            .map(|url| url.trim().to_string())
            .collect();
        if urls.is_empty() {
            return Err(PlineError::Invalid("urls must not be empty".into()));
        }
        for url in &urls {
            client::host_of(url)?;
        }
        if !(1..=10_000).contains(&params.limit) {
            return Err(PlineError::Invalid(
                "limit must be between 1 and 10000".into(),
            ));
        }
        if urls.len() > params.limit as usize {
            return Err(PlineError::Invalid(format!(
                "{} urls exceed limit {}",
                urls.len(),
                params.limit
            )));
        }
        let mut body = client::to_batch_casing(options);
        body.insert("urls".into(), json!(urls));
        body.insert("limit".into(), json!(params.limit));
        if let Some(session_id) = params.session_id.filter(|id| !id.trim().is_empty()) {
            body.insert("session_id".into(), json!(session_id));
        }
        self.api
            .request(
                key,
                "POST",
                "/batch/scrape",
                Some(&Value::Object(body)),
                &[],
                &[],
            )
            .await
    }

    async fn crawl_start_impl(&self, key: &str, params: CrawlStartParams) -> client::Result<Value> {
        client::host_of(&params.url)?;
        if !(1..=10_000).contains(&params.limit) {
            return Err(PlineError::Invalid(
                "limit must be between 1 and 10000".into(),
            ));
        }
        let requested = client::csv_or_list(params.output.as_deref());
        let outputs = client::validate_outputs(
            if requested.is_empty() {
                vec!["markdown".to_string()]
            } else {
                requested
            },
            client::CRAWL_OUTPUTS,
            "output",
        )?;
        let mut body = Map::new();
        body.insert("url".into(), json!(params.url.trim()));
        body.insert("limit".into(), json!(params.limit));
        body.insert("sitemap".into(), json!(params.sitemap.as_str()));
        body.insert(
            "output".into(),
            Value::Object(
                outputs
                    .iter()
                    .map(|format| (format.clone(), json!(true)))
                    .collect(),
            ),
        );
        if let Some(paths) = params.include_paths.filter(|paths| !paths.is_empty()) {
            body.insert("includePaths".into(), json!(paths));
        }
        if let Some(paths) = params.exclude_paths.filter(|paths| !paths.is_empty()) {
            body.insert("excludePaths".into(), json!(paths));
        }
        if let Some(depth) = params.max_discovery_depth {
            body.insert("maxDiscoveryDepth".into(), json!(depth));
        }
        if params.ignore_query_parameters {
            body.insert("ignoreQueryParameters".into(), json!(true));
        }
        if params.crawl_entire_domain {
            body.insert("crawlEntireDomain".into(), json!(true));
        }
        if params.allow_subdomains {
            body.insert("allowSubdomains".into(), json!(true));
        }
        if let Some(email) = params.email.filter(|email| !email.trim().is_empty()) {
            body.insert("email".into(), json!(email.trim()));
        }
        if let Some(url) = params.webhook_url.filter(|url| !url.trim().is_empty()) {
            body.insert("webhook".into(), json!({"url": url.trim()}));
        }

        let mut per_page = Map::new();
        if let Some(js_render) = params.js_render {
            per_page.insert("js_render".into(), json!(js_render));
        }
        if let Some(strategy) = params.proxy_strategy {
            per_page.insert("proxy_strategy".into(), json!(strategy.as_str()));
        }
        if let Some(country) =
            client::validate_country(params.geolocation.as_deref(), "geolocation")?
        {
            per_page.insert("geolocation".into(), json!(country));
        }
        if !params.only_main_content {
            per_page.insert("only_main_content".into(), json!(false));
        }
        let actions = params.actions.filter(|actions| !actions.is_empty());
        let has_actions = actions.is_some();
        if let Some(actions) = actions {
            if !actions.iter().all(Value::is_object) {
                return Err(PlineError::Invalid(
                    "actions must be a list of action objects".into(),
                ));
            }
            per_page.insert("actions".into(), json!(actions));
        }
        let tags = client::validate_tags(client::csv_or_list(params.tag.as_deref()))?;
        if !tags.is_empty() {
            per_page.insert("tag".into(), json!(tags));
        }
        if params.js_render == Some(false)
            && (has_actions || outputs.iter().any(|o| o == "screenshot"))
        {
            return Err(PlineError::Invalid(
                "js_render=false cannot be combined with actions or screenshot output; they need a browser".into(),
            ));
        }
        if !per_page.is_empty() {
            body.insert("scrape".into(), Value::Object(per_page));
        }
        self.api
            .request(key, "POST", "/crawl", Some(&Value::Object(body)), &[], &[])
            .await
    }

    async fn job_status(
        &self,
        key: &str,
        job_path: &str,
        extension: &str,
        params: JobStatusParams,
    ) -> client::Result<Value> {
        self.check_local_output(params.download_dir.as_deref())?;
        let path = client::job_path(job_path, &params.id)?;
        let mut job = self.api.poll_job(key, &path, params.wait_seconds).await?;
        if let Some(dir) = params
            .download_dir
            .as_deref()
            .filter(|dir| !dir.trim().is_empty())
        {
            if job
                .get("s3PresignedUrls")
                .is_some_and(|urls| !urls.is_null())
            {
                let saved = self.api.download_artifacts(&job, dir, extension).await?;
                if let Value::Object(map) = &mut job {
                    map.insert("downloaded_files".into(), Value::Object(saved));
                }
            }
        }
        Ok(job)
    }

    async fn cancel_job(&self, key: &str, job_path: &str, id: &str) -> client::Result<Value> {
        let path = client::job_path(job_path, id)?;
        self.api.request(key, "DELETE", &path, None, &[], &[]).await
    }

    async fn map_site_impl(&self, key: &str, params: MapSiteParams) -> client::Result<Value> {
        client::host_of(&params.url)?;
        if !(1..=1_000_000).contains(&params.limit) {
            return Err(PlineError::Invalid(
                "limit must be between 1 and 1000000".into(),
            ));
        }
        let mut body = Map::new();
        body.insert("url".into(), json!(params.url.trim()));
        body.insert("sitemap".into(), json!(params.sitemap.as_str()));
        body.insert("includeSubdomains".into(), json!(params.include_subdomains));
        body.insert(
            "ignoreQueryParameters".into(),
            json!(params.ignore_query_parameters),
        );
        body.insert("limit".into(), json!(params.limit));
        if params.ignore_cache {
            body.insert("ignoreCache".into(), json!(true));
        }
        if let Some(timeout) = params.timeout_ms {
            body.insert("timeout".into(), json!(timeout));
        }
        self.api
            .request(key, "POST", "/map", Some(&Value::Object(body)), &[], &[])
            .await
    }

    async fn search_impl(&self, key: &str, params: SearchParams) -> client::Result<Value> {
        let query = params.query.trim();
        if query.is_empty() || query.chars().count() > 512 {
            return Err(PlineError::Invalid("query must be 1-512 characters".into()));
        }
        if !(1..=100).contains(&params.num_results) {
            return Err(PlineError::Invalid(
                "num_results must be between 1 and 100".into(),
            ));
        }
        let country =
            client::validate_country(Some(&params.country), "country")?.unwrap_or_default();
        let language = params.language.trim().to_ascii_lowercase();
        if language.len() != 2 || !language.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(PlineError::Invalid(
                "language must be a 2-letter code".into(),
            ));
        }
        let mut query_params: Vec<(&str, String)> = vec![
            ("query", query.to_string()),
            ("country", country),
            ("language", language),
            ("num_results", params.num_results.to_string()),
            ("device", params.device.as_str().to_string()),
            ("mode", params.mode.as_str().to_string()),
        ];
        if let Some(tbs) = params.tbs {
            query_params.push(("tbs", tbs.as_str().to_string()));
        }
        let outputs = client::validate_outputs(
            client::csv_or_list(params.output.as_deref()),
            client::SERP_OUTPUTS,
            "output",
        )?;
        if !outputs.is_empty() {
            query_params.push(("output", outputs.join(",")));
        }
        if !params.only_main_content {
            query_params.push(("only_main_content", "false".to_string()));
        }
        self.api
            .request(key, "GET", "/serp", None, &query_params, &[])
            .await
    }
}

#[tool_handler]
impl ServerHandler for PlineServer {
    fn get_info(&self) -> ServerInfo {
        let mut implementation = Implementation::from_build_env();
        implementation.name = "pline.ai".to_string();
        implementation.title = Some("pline.ai".to_string());
        implementation.version = env!("CARGO_PKG_VERSION").to_string();
        implementation.description = Some(
            "Scrape, crawl, batch-scrape, map, and search the web through pline.ai.".to_string(),
        );
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(implementation)
        .with_instructions(INSTRUCTIONS)
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new(GUIDE_URI, "guide")
                .with_title("pline.ai usage guide")
                .with_description(
                    "When to use which tool, how sessions and cost work, and how to read failures.",
                )
                .with_mime_type("text/markdown"),
            Resource::new(REFERENCE_URI, "reference")
                .with_title("pline.ai API reference")
                .with_description(
                    "Every exposed endpoint, field, constraint, response shape, and status code.",
                )
                .with_mime_type("text/markdown"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let text = match request.uri.as_str() {
            GUIDE_URI => GUIDE,
            REFERENCE_URI => REFERENCE,
            other => {
                return Err(ErrorData::resource_not_found(
                    format!("unknown resource {other}"),
                    None,
                ))
            }
        };
        let contents = match ResourceContents::text(text, request.uri.clone()) {
            ResourceContents::TextResourceContents {
                uri, text, meta, ..
            } => ResourceContents::TextResourceContents {
                uri,
                mime_type: Some("text/markdown".to_string()),
                text,
                meta,
            },
            other => other,
        };
        Ok(ReadResourceResult::new(vec![contents]).into())
    }
}

const USAGE: &str = "\
usage: pline-mcp [--stdio | --http [ADDR]]

  (no flags)      serve MCP over stdio for a local client; the API key comes from PLINE_API_KEY
  --stdio        serve MCP over stdio, overriding PLINE_MCP_HTTP_ADDR
  --http [ADDR]   serve MCP over Streamable HTTP at ADDR (default 0.0.0.0:8080, or PLINE_MCP_HTTP_ADDR);
                  every request must carry the caller's pline.ai API key

PLINE_BASE_URL must point at the pline.ai API in both modes.";

enum Mode {
    Stdio,
    Http(String),
}

fn parse_mode() -> anyhow::Result<Mode> {
    let mut args = std::env::args().skip(1);
    let mut mode = match std::env::var("PLINE_MCP_HTTP_ADDR") {
        Ok(addr) if !addr.trim().is_empty() => Mode::Http(addr.trim().to_string()),
        _ => Mode::Stdio,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--http" => {
                let addr = args
                    .next()
                    .filter(|next| !next.starts_with("--"))
                    .unwrap_or_else(|| "0.0.0.0:8080".to_string());
                mode = Mode::Http(addr);
            }
            "--stdio" => mode = Mode::Stdio,
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument {other:?}\n{USAGE}"),
        }
    }
    Ok(mode)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    let server = PlineServer::new()?;
    match parse_mode()? {
        Mode::Stdio => {
            let service = server.serve(rmcp::transport::stdio()).await?;
            service.waiting().await?;
        }
        Mode::Http(addr) => http_server::serve(server, &addr).await?,
    }
    Ok(())
}
