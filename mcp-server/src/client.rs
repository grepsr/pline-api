//! HTTP client for the exposed pline.ai API surface, plus request-shape validation that
//! mirrors the service so bad combinations fail locally with a clear message.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use url::Url;

pub const SCRAPE_OUTPUTS: &[&str] = &[
    "html",
    "clean_html",
    "links",
    "markdown",
    "screenshot",
    "screenshot_full_page",
    "json",
];
pub const CRAWL_OUTPUTS: &[&str] = &[
    "markdown",
    "html",
    "clean_html",
    "links",
    "screenshot",
    "screenshot_full_page",
];
pub const SERP_OUTPUTS: &[&str] = &["html", "clean_html", "links"];
pub const PROXY_STRATEGIES: &[&str] = &["auto", "basic", "enhanced", "premium"];
pub const TERMINAL_JOB_STATUSES: &[&str] = &["completed", "failed", "cancelled"];
pub const MAX_WAIT_SECS: f64 = 600.0;
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(360);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, thiserror::Error)]
pub enum PlineError {
    /// The caller's arguments cannot form a valid request.
    #[error("{0}")]
    Invalid(String),
    /// The service rejected the request; carries its HTTP status and `detail`.
    #[error("{method} {path} -> HTTP {status}: {detail}")]
    Api {
        method: &'static str,
        path: String,
        status: u16,
        detail: String,
    },
    /// Transport-level failure, missing configuration, or a local file problem.
    #[error("{0}")]
    Transport(String),
}

pub type Result<T> = std::result::Result<T, PlineError>;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum TagInput {
    One(String),
    Many(Vec<String>),
}

impl TagInput {
    pub fn values(&self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value.clone()],
            Self::Many(values) => values.clone(),
        }
    }
}

fn invalid(message: impl Into<String>) -> PlineError {
    PlineError::Invalid(message.into())
}

/// The API key for local stdio use, read from `PLINE_API_KEY`.
pub fn api_key_from_env() -> Result<String> {
    env_var("PLINE_API_KEY")
}

fn env_var(name: &str) -> Result<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            PlineError::Transport(format!(
                "{name} is not set; export it in the environment that launches the MCP server"
            ))
        })
}

/// Flattens repeated and comma-separated values, trimming blanks.
pub fn csv_or_list(values: Option<&[String]>) -> Vec<String> {
    values
        .unwrap_or_default()
        .iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn validate_outputs(
    outputs: Vec<String>,
    allowed: &[&str],
    where_: &str,
) -> Result<Vec<String>> {
    let mut normalized: Vec<String> = Vec::new();
    for output in outputs {
        let output = output.trim().to_ascii_lowercase();
        if output.is_empty() {
            continue;
        }
        if !allowed.contains(&output.as_str()) {
            return Err(invalid(format!(
                "unsupported {where_} {output:?}; choose from {}",
                allowed.join(", ")
            )));
        }
        if !normalized.contains(&output) {
            normalized.push(output);
        }
    }
    if normalized.iter().any(|o| o == "screenshot")
        && normalized.iter().any(|o| o == "screenshot_full_page")
    {
        return Err(invalid(format!(
            "{where_} cannot include both screenshot and screenshot_full_page"
        )));
    }
    Ok(normalized)
}

pub fn validate_country(value: Option<&str>, field: &str) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().to_ascii_uppercase();
    if value.len() == 2 && value.chars().all(|c| c.is_ascii_alphabetic()) {
        Ok(Some(value))
    } else {
        Err(invalid(format!(
            "{field} must be a 2-letter ISO country code"
        )))
    }
}

pub fn validate_proxy_strategy(value: Option<&str>) -> Result<Option<String>> {
    match value {
        None => Ok(None),
        Some(value) if PROXY_STRATEGIES.contains(&value) => Ok(Some(value.to_string())),
        Some(_) => Err(invalid(format!(
            "proxy_strategy must be one of {}",
            PROXY_STRATEGIES.join(", ")
        ))),
    }
}

pub fn validate_tags(tags: Vec<String>) -> Result<Vec<String>> {
    if tags.len() > 25 {
        return Err(invalid("at most 25 tags are allowed"));
    }
    for tag in &tags {
        let ok = !tag.is_empty()
            && tag.len() <= 25
            && tag
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
        if !ok {
            return Err(invalid(format!(
                "tag {tag:?} must be 1-25 chars of ASCII letters, digits, period, hyphen, underscore"
            )));
        }
    }
    Ok(tags)
}

pub fn host_of(url: &str) -> Result<String> {
    Url::parse(url.trim())
        .ok()
        .filter(|parsed| matches!(parsed.scheme(), "http" | "https"))
        .and_then(|parsed| parsed.host_str().map(|host| host.to_ascii_lowercase()))
        .ok_or_else(|| {
            invalid(format!(
                "url {url:?} must be an absolute http/https URL with a hostname"
            ))
        })
}

/// The `/scrape` options shared with `/batch/scrape` and (as a subset) with `/crawl`.
pub struct ScrapeOptions<'a> {
    pub output: Option<&'a [String]>,
    pub js_render: Option<bool>,
    pub proxy_strategy: Option<&'a str>,
    pub only_main_content: bool,
    pub geolocation: Option<&'a str>,
    pub tag: Option<&'a TagInput>,
    pub actions: Option<&'a [Value]>,
    pub prompt: Option<&'a str>,
    pub schema: Option<&'a Value>,
    pub timeout: Option<u64>,
    pub wait_selector: Option<&'a str>,
    pub wait_ms: Option<u64>,
    pub method: &'a str,
    pub body: Option<&'a Value>,
}

/// Builds the shared snake_case scrape options used by `/scrape` and `/batch/scrape`.
pub fn build_scrape_options(options: ScrapeOptions<'_>) -> Result<Map<String, Value>> {
    let requested = csv_or_list(options.output);
    let outputs = validate_outputs(
        if requested.is_empty() {
            vec!["html".to_string()]
        } else {
            requested
        },
        SCRAPE_OUTPUTS,
        "output",
    )?;
    let mut body = Map::new();
    body.insert("output".into(), json!(outputs));
    let method = options.method.trim().to_ascii_uppercase();
    if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "DELETE") {
        return Err(invalid("method must be GET, POST, PUT, or DELETE"));
    }
    if method != "GET" {
        body.insert("method".into(), json!(method));
    }
    if let Some(payload) = options.body {
        if method == "GET" {
            return Err(invalid("body is not allowed with method GET"));
        }
        if !(payload.is_object() || payload.is_string()) {
            return Err(invalid("body must be a JSON object or a string"));
        }
        body.insert("body".into(), payload.clone());
    }
    if let Some(js_render) = options.js_render {
        body.insert("js_render".into(), json!(js_render));
    }
    if let Some(timeout) = options.timeout {
        body.insert("timeout".into(), json!(timeout));
    }
    if let Some(wait_selector) = options.wait_selector {
        body.insert("wait_selector".into(), json!(wait_selector));
    }
    if let Some(wait_ms) = options.wait_ms {
        body.insert("wait_ms".into(), json!(wait_ms));
    }
    if let Some(strategy) = validate_proxy_strategy(options.proxy_strategy)? {
        body.insert("proxy_strategy".into(), json!(strategy));
    }
    if !options.only_main_content {
        body.insert("only_main_content".into(), json!(false));
    }
    if let Some(country) = validate_country(options.geolocation, "geolocation")? {
        body.insert("geolocation".into(), json!(country));
    }
    let tag_values = options.tag.map(TagInput::values).unwrap_or_default();
    let tags = validate_tags(csv_or_list(Some(&tag_values)))?;
    if !tags.is_empty() {
        body.insert("tag".into(), json!(tags));
    }
    let actions = options.actions.filter(|actions| !actions.is_empty());
    if let Some(actions) = actions {
        if !actions.iter().all(Value::is_object) {
            return Err(invalid("actions must be a list of action objects"));
        }
        body.insert("actions".into(), json!(actions));
    }
    if let Some(prompt) = options.prompt.map(str::trim).filter(|p| !p.is_empty()) {
        if prompt.len() > 16_384 {
            return Err(invalid("prompt is too long (max 16384 characters)"));
        }
        body.insert("prompt".into(), json!(prompt));
    }
    if let Some(schema) = options.schema {
        body.insert("schema".into(), schema.clone());
    }
    if outputs.iter().any(|o| o == "json")
        && !body.contains_key("prompt")
        && !body.contains_key("schema")
    {
        return Err(invalid("output json requires prompt and/or schema"));
    }
    let browser_needed = actions.is_some() || outputs.iter().any(|o| o.starts_with("screenshot"));
    if options.js_render == Some(false) && browser_needed {
        return Err(invalid(
            "js_render=false cannot be combined with actions or screenshot output; they need a browser",
        ));
    }
    if browser_needed && method != "GET" {
        return Err(invalid("actions and screenshots only support method GET"));
    }
    if browser_needed && options.body.is_some() {
        return Err(invalid(
            "body is not allowed with actions or screenshot output",
        ));
    }
    Ok(body)
}

/// Remembers the session id pline.ai returned per (tenant, host) and serializes requests per
/// (tenant, host).
///
/// A session is bound to one registrable domain and rejects concurrent use, so keying by host is
/// safe (one host always lies within one registrable domain) and a per-host lock keeps parallel
/// tool calls from tripping the service's 429. The tenant component is a hash of the API key, so
/// callers sharing one server process never see each other's sessions and no key is stored.
#[derive(Default)]
pub struct SessionMemory {
    sessions: Mutex<HashMap<String, String>>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

fn tenant_of(api_key: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    api_key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn memory_key(api_key: &str, host: &str) -> String {
    format!("{}:{host}", tenant_of(api_key))
}

impl SessionMemory {
    pub async fn get(&self, api_key: &str, host: &str) -> Option<String> {
        self.sessions
            .lock()
            .await
            .get(&memory_key(api_key, host))
            .cloned()
    }

    pub async fn remember(&self, api_key: &str, host: &str, session_id: Option<&str>) {
        if let Some(session_id) = session_id.filter(|id| !id.is_empty()) {
            self.sessions
                .lock()
                .await
                .insert(memory_key(api_key, host), session_id.to_string());
        }
    }

    pub async fn forget(&self, api_key: &str, host: &str) {
        self.sessions
            .lock()
            .await
            .remove(&memory_key(api_key, host));
    }

    pub async fn lock_for(&self, api_key: &str, host: &str) -> Arc<Mutex<()>> {
        self.locks
            .lock()
            .await
            .entry(memory_key(api_key, host))
            .or_default()
            .clone()
    }

    /// The host -> session_id map for one tenant.
    pub async fn snapshot(&self, api_key: &str) -> HashMap<String, String> {
        let prefix = format!("{}:", tenant_of(api_key));
        self.sessions
            .lock()
            .await
            .iter()
            .filter_map(|(key, session_id)| {
                key.strip_prefix(&prefix)
                    .map(|host| (host.to_string(), session_id.clone()))
            })
            .collect()
    }
}

pub fn job_path(prefix: &str, id: &str) -> Result<String> {
    let id = uuid::Uuid::parse_str(id.trim())
        .map_err(|_| PlineError::Invalid("job id must be a UUID".into()))?;
    Ok(format!("{prefix}/{id}"))
}

pub struct PlineClient {
    http: reqwest::Client,
}

impl PlineClient {
    pub fn new() -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { http })
    }

    pub async fn request(
        &self,
        api_key: &str,
        method: &'static str,
        path: &str,
        json_body: Option<&Value>,
        query: &[(&str, String)],
        headers: &[(&str, String)],
    ) -> Result<Value> {
        let base = env_var("PLINE_BASE_URL")?;
        let mut url =
            Url::parse(&format!("{}{}", base.trim_end_matches('/'), path)).map_err(|error| {
                PlineError::Transport(format!("PLINE_BASE_URL is invalid: {error}"))
            })?;
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (name, value) in query {
                pairs.append_pair(name, value);
            }
        }
        let http_method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|error| PlineError::Transport(error.to_string()))?;
        let mut request = self
            .http
            .request(http_method, url)
            .header("x-api-key", api_key)
            .header("accept", "application/json");
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        if let Some(body) = json_body {
            request = request.json(body);
        }
        let response = request
            .send()
            .await
            .map_err(|error| PlineError::Transport(format!("{method} {path} failed: {error}")))?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|error| {
            PlineError::Transport(format!("{method} {path} read failed: {error}"))
        })?;
        let payload: Value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        if status >= 400 {
            let detail = payload
                .get("detail")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| payload.to_string());
            return Err(PlineError::Api {
                method,
                path: path.to_string(),
                status,
                detail,
            });
        }
        Ok(payload)
    }

    /// Returns the job status, polling until it is terminal or `wait_secs` elapse.
    pub async fn poll_job(&self, api_key: &str, path: &str, wait_secs: f64) -> Result<Value> {
        let wait = Duration::from_secs_f64(wait_secs.clamp(0.0, MAX_WAIT_SECS));
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            let job = self.request(api_key, "GET", path, None, &[], &[]).await?;
            let status = job
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let now = tokio::time::Instant::now();
            if TERMINAL_JOB_STATUSES.contains(&status) || now >= deadline {
                return Ok(job);
            }
            tokio::time::sleep(POLL_INTERVAL.min(deadline - now)).await;
        }
    }

    /// Downloads every presigned artifact of a finished job into `directory`; returns format -> path.
    pub async fn download_artifacts(
        &self,
        job: &Value,
        directory: &str,
        extension: &str,
    ) -> Result<Map<String, Value>> {
        let target = PathBuf::from(directory);
        tokio::fs::create_dir_all(&target)
            .await
            .map_err(|error| PlineError::Transport(format!("create {directory}: {error}")))?;
        let mut saved = Map::new();
        let Some(urls) = job.get("s3PresignedUrls").and_then(Value::as_object) else {
            return Ok(saved);
        };
        for (format, url) in urls {
            let Some(url) = url.as_str() else { continue };
            let name = Url::parse(url)
                .ok()
                .and_then(|parsed| {
                    parsed
                        .path_segments()
                        .and_then(|mut segments| segments.next_back().map(ToString::to_string))
                })
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("{format}.{extension}"));
            let path = target.join(name);
            self.download(url, &path).await?;
            saved.insert(format.clone(), json!(path.to_string_lossy()));
        }
        Ok(saved)
    }

    async fn download(&self, url: &str, path: &Path) -> Result<()> {
        let response = self
            .http
            .get(url)
            .timeout(DOWNLOAD_TIMEOUT)
            .send()
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|error| PlineError::Transport(format!("download {url}: {error}")))?;
        let mut file = tokio::fs::File::create(path).await.map_err(|error| {
            PlineError::Transport(format!("create {}: {error}", path.display()))
        })?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|error| PlineError::Transport(format!("download {url}: {error}")))?;
            file.write_all(&chunk).await.map_err(|error| {
                PlineError::Transport(format!("write {}: {error}", path.display()))
            })?;
        }
        file.flush()
            .await
            .map_err(|error| PlineError::Transport(format!("write {}: {error}", path.display())))?;
        Ok(())
    }
}

fn extension_for(data_key: &str) -> &'static str {
    match data_key {
        "html_body" | "clean_html" => "html",
        "markdown" => "md",
        "json" => "json",
        _ => "txt",
    }
}

/// Writes each non-screenshot output to `directory`; returns data key -> path.
pub async fn save_scrape_data(
    data: &Map<String, Value>,
    directory: &str,
) -> Result<Map<String, Value>> {
    let target = PathBuf::from(directory);
    tokio::fs::create_dir_all(&target)
        .await
        .map_err(|error| PlineError::Transport(format!("create {directory}: {error}")))?;
    let mut written = Map::new();
    for (key, value) in data {
        if key.starts_with("screenshot") {
            continue;
        }
        let path = target.join(format!("{key}.{}", extension_for(key)));
        let text = match value {
            Value::String(text) => text.clone(),
            Value::Array(items) if key == "links" => {
                let mut lines: Vec<String> = items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| item.to_string())
                    })
                    .collect();
                lines.push(String::new());
                lines.join("\n")
            }
            other => serde_json::to_string_pretty(other).unwrap_or_default(),
        };
        tokio::fs::write(&path, text)
            .await
            .map_err(|error| PlineError::Transport(format!("write {}: {error}", path.display())))?;
        written.insert(key.clone(), json!(path.to_string_lossy()));
    }
    Ok(written)
}

/// Clips oversized outputs so one page cannot flood the conversation; returns the clipped keys.
pub fn truncate_data(
    data: &Map<String, Value>,
    max_chars: usize,
) -> (Map<String, Value>, Vec<String>) {
    if max_chars == 0 {
        return (data.clone(), Vec::new());
    }
    let mut clipped = Map::new();
    let mut truncated = Vec::new();
    for (key, value) in data {
        let text = match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        let char_count = text.chars().count();
        if char_count > max_chars {
            let head: String = text.chars().take(max_chars).collect();
            clipped.insert(
                key.clone(),
                json!(format!(
                    "{head}\n... [truncated {} chars; pass save_dir to get it all]",
                    char_count - max_chars
                )),
            );
            truncated.push(key.clone());
        } else {
            clipped.insert(key.clone(), value.clone());
        }
    }
    (clipped, truncated)
}

pub fn data_is_empty(data: &Map<String, Value>) -> bool {
    data.values().all(|value| match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_options<'a>() -> ScrapeOptions<'a> {
        ScrapeOptions {
            output: None,
            js_render: None,
            proxy_strategy: None,
            only_main_content: true,
            geolocation: None,
            tag: None,
            actions: None,
            prompt: None,
            schema: None,
            timeout: None,
            wait_selector: None,
            wait_ms: None,
            method: "GET",
            body: None,
        }
    }

    #[test]
    fn defaults_to_html_and_omits_unset_fields() {
        let body = build_scrape_options(base_options()).unwrap();
        let expected: Map<String, Value> =
            serde_json::from_value(json!({"output": ["html"]})).unwrap();
        assert_eq!(body, expected);
    }

    #[test]
    fn json_output_requires_prompt_or_schema() {
        let outputs = vec!["json".to_string()];
        let error = build_scrape_options(ScrapeOptions {
            output: Some(&outputs),
            ..base_options()
        })
        .unwrap_err();
        assert!(error.to_string().contains("prompt and/or schema"));
    }

    #[test]
    fn js_render_false_rejects_browser_only_options() {
        let outputs = vec!["screenshot".to_string()];
        let error = build_scrape_options(ScrapeOptions {
            output: Some(&outputs),
            js_render: Some(false),
            ..base_options()
        })
        .unwrap_err();
        assert!(error.to_string().contains("need a browser"));
    }

    #[test]
    fn batch_scrape_keeps_shared_scrape_option_names() {
        let outputs = vec!["markdown".to_string()];
        let body = build_scrape_options(ScrapeOptions {
            output: Some(&outputs),
            js_render: Some(true),
            proxy_strategy: Some("basic"),
            geolocation: Some("us"),
            ..base_options()
        })
        .unwrap();
        assert_eq!(body["js_render"], json!(true));
        assert_eq!(body["proxy_strategy"], json!("basic"));
        assert_eq!(body["geolocation"], json!("US"));
    }

    #[test]
    fn truncation_reports_clipped_keys() {
        let data: Map<String, Value> =
            serde_json::from_value(json!({"markdown": "abcdef", "links": []})).unwrap();
        let (clipped, truncated) = truncate_data(&data, 3);
        assert_eq!(truncated, vec!["markdown".to_string()]);
        assert!(clipped["markdown"]
            .as_str()
            .unwrap()
            .starts_with("abc\n... [truncated 3 chars"));
        let empty: Map<String, Value> =
            serde_json::from_value(json!({"markdown": "", "links": []})).unwrap();
        assert!(data_is_empty(&empty));
    }

    #[tokio::test]
    async fn sessions_are_scoped_per_tenant() {
        let memory = SessionMemory::default();
        memory
            .remember("key-a", "example.com", Some("sess-a"))
            .await;
        memory
            .remember("key-b", "example.com", Some("sess-b"))
            .await;
        assert_eq!(
            memory.get("key-a", "example.com").await.as_deref(),
            Some("sess-a")
        );
        assert_eq!(
            memory.get("key-b", "example.com").await.as_deref(),
            Some("sess-b")
        );
        assert_eq!(memory.snapshot("key-a").await.len(), 1);
        memory.forget("key-a", "example.com").await;
        assert!(memory.get("key-a", "example.com").await.is_none());
        assert!(memory.get("key-b", "example.com").await.is_some());
    }
}
