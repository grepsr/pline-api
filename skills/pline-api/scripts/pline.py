#!/usr/bin/env python3
"""Client for the exposed pline.ai API surface.

Exposed: POST /scrape, POST|GET|DELETE /crawl, POST|GET|DELETE /batch/scrape,
POST /map, GET /serp.

Configuration comes from the environment:
    PLINE_BASE_URL   e.g. https://apix.pline.ai/v1
    PLINE_API_KEY    sent as the x-api-key header
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import http.client
import urllib.request
import uuid
from typing import NoReturn

SCRAPE_OUTPUTS = (
    "html",
    "clean_html",
    "links",
    "markdown",
    "screenshot",
    "screenshot_full_page",
    "json",
)
CRAWL_OUTPUTS = ("markdown", "html", "clean_html", "links", "screenshot")
PROXY_STRATEGIES = ("auto", "basic", "enhanced", "premium")
DATA_KEY_EXTENSION = {
    "html_body": "html",
    "clean_html": "html",
    "markdown": "md",
    "links": "txt",
    "json": "json",
}
SCRAPE_FIELDS = frozenset(
    {
        "url",
        "method",
        "body",
        "js_render",
        "proxy_strategy",
        "output",
        "only_main_content",
        "geolocation",
        "session_id",
        "session",
        "tag",
        "actions",
        "prompt",
        "schema",
    }
)
CRAWL_FIELDS = frozenset(
    {
        "url",
        "limit",
        "includePaths",
        "excludePaths",
        "maxDiscoveryDepth",
        "sitemap",
        "ignoreQueryParameters",
        "crawlEntireDomain",
        "allowSubdomains",
        "output",
        "email",
        "webhook",
        "scrape",
        "scrapeOptions",
        "scheduleToCloseTimeoutSecs",
        "heartbeatTimeoutSecs",
    }
)
CRAWL_SCRAPE_FIELDS = frozenset(
    {
        "js_render",
        "proxy_strategy",
        "geolocation",
        "only_main_content",
        "actions",
        "tag",
        "output",
    }
)
TERMINAL_JOB_STATUSES = ("completed", "failed", "cancelled")
ARTIFACT_EXTENSION = {"/crawl": "zip", "/batch/scrape": "jsonl"}


def die(message: str, code: int = 1) -> NoReturn:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(code)


def base_url() -> str:
    url = os.environ.get("PLINE_BASE_URL", "").strip().rstrip("/")
    if not url:
        die("PLINE_BASE_URL is not set (e.g. https://apix.pline.ai/v1)")
    return url


def _lowercase_header_names(http_class: type) -> type:
    class Connection(http_class):
        def putheader(self, header, *values):
            super().putheader(header.lower(), *values)

    return Connection


class _LowercaseHTTPHandler(urllib.request.HTTPHandler):
    def do_open(self, http_class, request, **kwargs):
        return super().do_open(_lowercase_header_names(http_class), request, **kwargs)


class _LowercaseHTTPSHandler(urllib.request.HTTPSHandler):
    def do_open(self, http_class, request, **kwargs):
        return super().do_open(_lowercase_header_names(http_class), request, **kwargs)


# urllib title-cases header names (x-api-key becomes X-Api-Key). The API matches x-api-key
# case-sensitively over HTTP/1.1, so send every header name in lowercase, as HTTP/2 does.
API_OPENER = urllib.request.build_opener(_LowercaseHTTPHandler, _LowercaseHTTPSHandler)


def api_key() -> str:
    key = os.environ.get("PLINE_API_KEY", "").strip()
    if not key:
        die("PLINE_API_KEY is not set")
    return key


def require_known_fields(payload: dict, allowed: frozenset[str], where: str) -> None:
    """Refuse payload keys outside the documented interface before they reach the API."""
    for key in payload:
        if key not in allowed:
            die(f"{where}.{key} is not a supported field; see references/endpoints.md")


def request_api(
    method: str,
    path: str,
    body: dict | None = None,
    query: dict | None = None,
    headers: dict | None = None,
    timeout: float = 360.0,
) -> tuple[int, object]:
    url = base_url() + path
    if query:
        pairs = [(k, v) for k, v in query.items() if v is not None]
        url += "?" + urllib.parse.urlencode(pairs)
    data = None
    request_headers = {"x-api-key": api_key(), "accept": "application/json"}
    request_headers.update(headers or {})
    if body is not None:
        data = json.dumps(body).encode()
        request_headers["content-type"] = "application/json"
    request = urllib.request.Request(
        url, data=data, headers=request_headers, method=method
    )
    try:
        with API_OPENER.open(request, timeout=timeout) as response:
            raw = response.read().decode("utf-8", "replace")
            status = response.status
    except urllib.error.HTTPError as error:
        raw = error.read().decode("utf-8", "replace")
        status = error.code
    except urllib.error.URLError as error:
        die(f"{method} {path} failed to connect: {error.reason}")
    try:
        return status, json.loads(raw) if raw else None
    except json.JSONDecodeError:
        return status, raw


def call(
    method: str,
    path: str,
    body: dict | None = None,
    query: dict | None = None,
    headers: dict | None = None,
    timeout: float = 360.0,
) -> object:
    status, payload = request_api(
        method, path, body=body, query=query, headers=headers, timeout=timeout
    )
    if status >= 400:
        detail = payload
        if isinstance(payload, dict):
            # The API reports errors as {"code", "message"}; some proxies use "detail".
            if payload.get("message"):
                detail = f"{payload.get('code') or 'ERROR'}: {payload['message']}"
            elif payload.get("detail"):
                detail = payload["detail"]
        die(f"{method} {path} -> HTTP {status}: {detail}")
    return payload


def json_arg(value: str | None) -> object | None:
    """Parse an inline JSON argument, or read it from @path."""
    if value is None:
        return None
    if value.startswith("@"):
        with open(value[1:], encoding="utf-8") as handle:
            value = handle.read()
    try:
        return json.loads(value)
    except json.JSONDecodeError as error:
        die(f"invalid JSON: {error}")


def csv_list(values: list[str] | None) -> list[str]:
    """Flatten repeated and comma-separated option values."""
    items: list[str] = []
    for value in values or []:
        items.extend(part.strip() for part in value.split(",") if part.strip())
    return items


def validate_outputs(outputs: list[str], allowed: tuple[str, ...]) -> list[str]:
    for output in outputs:
        if output not in allowed:
            die(f"unsupported output {output!r}; choose from {', '.join(allowed)}")
    if "screenshot" in outputs and "screenshot_full_page" in outputs:
        die("output cannot include both screenshot and screenshot_full_page")
    return outputs


def emit(payload: object, args: argparse.Namespace) -> None:
    text = json.dumps(payload, indent=2, ensure_ascii=False)
    limit = getattr(args, "max_chars", 0) or 0
    if not getattr(args, "raw", False) and limit and len(text) > limit:
        print(text[:limit])
        print(
            f"... [truncated {len(text) - limit} chars; use --raw or --save to get it all]"
        )
    else:
        print(text)


def read_session_file(path: str | None) -> str | None:
    if path and os.path.exists(path):
        with open(path, encoding="utf-8") as handle:
            value = handle.read().strip()
        return value or None
    return None


def write_session_file(path: str | None, session_id: str | None) -> None:
    if path and session_id:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(session_id + "\n")


def js_render_choice(args: argparse.Namespace) -> bool | None:
    """Tri-state: --js-render -> True, --no-js-render -> False, neither -> None (server decides)."""
    if args.js_render and args.no_js_render:
        die("--js-render and --no-js-render are mutually exclusive")
    if args.js_render:
        return True
    if args.no_js_render:
        return False
    return None


def scrape_options(args: argparse.Namespace) -> dict:
    """Shared /scrape and /batch/scrape option builder (snake_case keys)."""
    outputs = validate_outputs(csv_list(args.output) or ["html"], SCRAPE_OUTPUTS)
    body: dict = {"output": outputs}
    if args.method and args.method != "GET":
        body["method"] = args.method
    payload_body = json_arg(args.body)
    if payload_body is not None:
        body["body"] = payload_body
    js_render = js_render_choice(args)
    if js_render is not None:
        body["js_render"] = js_render
    if args.proxy_strategy:
        body["proxy_strategy"] = args.proxy_strategy
    if args.no_main_content:
        body["only_main_content"] = False
    if args.geolocation:
        body["geolocation"] = args.geolocation.upper()
    tags = csv_list(args.tag)
    if tags:
        body["tag"] = tags
    actions = json_arg(args.actions)
    if actions is not None:
        if not isinstance(actions, list):
            die("--actions must be a JSON array")
        body["actions"] = actions
    if args.prompt:
        body["prompt"] = args.prompt
    schema = json_arg(args.schema)
    if schema is not None:
        body["schema"] = schema
    if "json" in outputs and not (args.prompt or schema is not None):
        die("--output json requires --prompt and/or --schema")
    if js_render is False and (
        actions or any(o.startswith("screenshot") for o in outputs)
    ):
        die(
            "--no-js-render cannot be combined with --actions or screenshot output (they need a browser)"
        )
    return body


def save_scrape_data(data: dict, directory: str) -> list[str]:
    os.makedirs(directory, exist_ok=True)
    written = []
    for key, value in data.items():
        if key.startswith("screenshot"):
            continue
        extension = DATA_KEY_EXTENSION.get(key, "txt")
        path = os.path.join(directory, f"{key}.{extension}")
        if isinstance(value, str):
            text = value
        elif key == "links" and isinstance(value, list):
            text = "\n".join(str(item) for item in value) + "\n"
        else:
            text = json.dumps(value, indent=2, ensure_ascii=False)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text)
        written.append(path)
    return written


def request_headers(args: argparse.Namespace) -> dict:
    request_id = getattr(args, "request_id", None)
    return {"x-request-id": request_id} if request_id else {}


def cmd_scrape(args: argparse.Namespace) -> None:
    body = scrape_options(args)
    body["url"] = args.url
    session_id = args.session or read_session_file(args.session_file)
    if session_id:
        body["session_id"] = session_id
    payload = call(
        "POST",
        "/scrape",
        body=body,
        headers=request_headers(args),
        timeout=args.timeout,
    )
    if not isinstance(payload, dict):
        emit(payload, args)
        return
    write_session_file(args.session_file, payload.get("session_id"))
    if args.raw:
        emit(payload, args)
        return
    geolocation = payload.get("effective_geolocation") or {}
    print(f"status       {payload.get('status')}")
    print(f"final_url    {payload.get('final_url')}")
    print(f"session_id   {payload.get('session_id')}")
    print(f"request_id   {payload.get('request_id')}")
    print(f"site_tier    {payload.get('site_tier')}")
    if geolocation:
        location = ", ".join(
            str(v) for v in (geolocation.get("city"), geolocation.get("country")) if v
        )
        print(f"geolocation  {location}")
    data = payload.get("data") or {}
    if not data or all(not value for value in data.values()):
        print(
            "warning      data is empty; if you omitted --js-render, retry with it",
            file=sys.stderr,
        )
    for key, value in data.items():
        if key.startswith("screenshot"):
            print(f"{key:12} {value}")
    if args.save:
        for path in save_scrape_data(data, args.save):
            print(f"saved        {path}")
    else:
        emit(data, args)


def cmd_map(args: argparse.Namespace) -> None:
    body: dict = {"url": args.url, "limit": args.limit, "sitemap": args.sitemap}
    if args.no_subdomains:
        body["includeSubdomains"] = False
    if args.keep_query_parameters:
        body["ignoreQueryParameters"] = False
    if args.ignore_cache:
        body["ignoreCache"] = True
    if args.request_timeout_ms:
        body["timeout"] = args.request_timeout_ms
    payload = call("POST", "/map", body=body, timeout=args.timeout)
    links = payload.get("links") if isinstance(payload, dict) else None
    if args.raw or links is None:
        emit(payload, args)
        return
    print(f"{len(links)} links")
    if args.save:
        os.makedirs(os.path.dirname(args.save) or ".", exist_ok=True)
        with open(args.save, "w", encoding="utf-8") as handle:
            json.dump(links, handle, indent=2, ensure_ascii=False)
        print(f"saved {args.save}")
    else:
        for link in links:
            print(f"{link.get('type', '-'):16} {link.get('url')}")


def cmd_serp(args: argparse.Namespace) -> None:
    query = {
        "query": args.query,
        "country": args.country,
        "language": args.language,
        "num_results": args.num_results,
        "device": args.device,
        "mode": args.mode,
    }
    if args.tbs:
        query["tbs"] = args.tbs
    outputs = csv_list(args.output)
    if outputs:
        for output in outputs:
            if output not in ("html", "clean_html", "links"):
                die(
                    f"serp --output must be html, clean_html, or links (got {output!r})"
                )
        query["output"] = ",".join(outputs)
    if args.no_main_content:
        query["only_main_content"] = "false"
    payload = call("GET", "/serp", query=query, timeout=args.timeout)
    if args.raw or not isinstance(payload, dict):
        emit(payload, args)
        return
    print(f"query        {payload.get('query')}")
    print(f"creditsUsed  {payload.get('creditsUsed')}")
    if payload.get("totalOrganicResults") is not None:
        print(f"totalResults {payload['totalOrganicResults']}")
    for result in payload.get("organic") or []:
        print(f"{result.get('position'):>3}. {result.get('title')}")
        print(f"     {result.get('link')}")
        if result.get("snippet"):
            print(f"     {result['snippet']}")
    if args.save:
        with open(args.save, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=2, ensure_ascii=False)
        print(f"saved {args.save}")


def download(url: str, directory: str, fallback_name: str) -> str:
    os.makedirs(directory, exist_ok=True)
    name = os.path.basename(urllib.parse.urlparse(url).path) or fallback_name
    path = os.path.join(directory, name)
    with (
        urllib.request.urlopen(url, timeout=600) as response,
        open(path, "wb") as handle,
    ):
        while chunk := response.read(1 << 20):
            handle.write(chunk)
    return path


def print_job(job: dict) -> None:
    fields = (
        "id",
        "status",
        "total",
        "completed",
        "failed",
        "pending",
        "inFlight",
        "duration",
        "error",
    )
    for field in fields:
        if job.get(field) is not None:
            print(f"{field:12} {job[field]}")


def job_path(prefix: str, job_id: str) -> str:
    try:
        parsed = uuid.UUID(job_id.strip())
    except (ValueError, AttributeError):
        die("job id must be a UUID")
    return f"{prefix}/{parsed}"


def poll_job(path: str, args: argparse.Namespace) -> dict:
    started = time.monotonic()
    deadline = started + args.wait_timeout
    while True:
        job = call("GET", path, timeout=args.timeout)
        if not isinstance(job, dict):
            die(f"unexpected status payload for {path}")
        status = job.get("status")
        print(
            f"[{int(time.monotonic() - started):>6}s] {status} "
            f"completed={job.get('completed')} failed={job.get('failed')} "
            f"pending={job.get('pending')}",
            file=sys.stderr,
        )
        if status in TERMINAL_JOB_STATUSES:
            return job
        if time.monotonic() >= deadline:
            print(
                f"still running after {args.wait_timeout}s; poll it later with "
                f"`status {job.get('id')}`",
                file=sys.stderr,
            )
            return job
        time.sleep(args.interval)


def finish_job(job: dict, args: argparse.Namespace) -> None:
    print_job(job)
    urls = job.get("s3PresignedUrls") or {}
    extension = ARTIFACT_EXTENSION.get(args.job_path, "bin")
    for fmt, url in urls.items():
        if args.download:
            print(
                f"downloaded   {fmt}: {download(url, args.download, f'{fmt}.{extension}')}"
            )
        else:
            print(f"{fmt:12} {url}")
    if job.get("status") == "failed":
        raise SystemExit(1)


def crawl_scrape_options(args: argparse.Namespace) -> dict:
    """Per-page fetch options forwarded by the crawler (snake_case, subset of /scrape)."""
    scrape = json_arg(args.scrape_options) or {}
    if not isinstance(scrape, dict):
        die("--scrape-options must be a JSON object")
    js_render = js_render_choice(args)
    if js_render is not None:
        scrape["js_render"] = js_render
    if args.proxy_strategy:
        scrape["proxy_strategy"] = args.proxy_strategy
    if args.geolocation:
        scrape["geolocation"] = args.geolocation.upper()
    if args.no_main_content:
        scrape["only_main_content"] = False
    require_known_fields(scrape, CRAWL_SCRAPE_FIELDS, "scrape")
    return scrape


def cmd_crawl_start(args: argparse.Namespace) -> None:
    body = json_arg(args.json) if args.json else {}
    if not isinstance(body, dict):
        die("--json must be a JSON object")
    require_known_fields(body, CRAWL_FIELDS, "$")
    body.setdefault("url", args.url)
    if not body.get("url"):
        die("--url (or a url in --json) is required")
    body.setdefault("limit", args.limit)
    body.setdefault("sitemap", args.sitemap)
    outputs = validate_outputs(csv_list(args.output) or ["markdown"], CRAWL_OUTPUTS)
    body.setdefault("output", {output: True for output in outputs})
    if args.include_path:
        body["includePaths"] = args.include_path
    if args.exclude_path:
        body["excludePaths"] = args.exclude_path
    if args.max_depth is not None:
        body["maxDiscoveryDepth"] = args.max_depth
    if args.allow_subdomains:
        body["allowSubdomains"] = True
    if args.crawl_entire_domain:
        body["crawlEntireDomain"] = True
    if args.ignore_query_parameters:
        body["ignoreQueryParameters"] = True
    if args.email:
        body["email"] = args.email
    if args.webhook_url:
        body["webhook"] = {"url": args.webhook_url}
    scrape = crawl_scrape_options(args)
    if scrape:
        existing = body.pop("scrapeOptions", None) or body.get("scrape") or {}
        body["scrape"] = {**existing, **scrape}
    started = call("POST", "/crawl", body=body, timeout=args.timeout)
    emit(started, args)
    if args.wait and isinstance(started, dict) and started.get("id"):
        finish_job(poll_job(job_path("/crawl", started["id"]), args), args)


def cmd_batch_start(args: argparse.Namespace) -> None:
    body = scrape_options(args)
    urls = csv_list(args.url)
    if args.urls_file:
        with open(args.urls_file, encoding="utf-8") as handle:
            urls.extend(
                line.strip()
                for line in handle
                if line.strip() and not line.startswith("#")
            )
    if not urls:
        die("provide at least one --url or a --urls-file")
    body["urls"] = urls
    body["limit"] = args.limit
    # /batch/scrape spells these two in camelCase; everything else matches /scrape.
    if "js_render" in body:
        body["jsRender"] = body.pop("js_render")
    if "proxy_strategy" in body:
        body["proxyStrategy"] = body.pop("proxy_strategy")
    session_id = args.session or read_session_file(args.session_file)
    if session_id:
        body["session_id"] = session_id
    started = call("POST", "/batch/scrape", body=body, timeout=args.timeout)
    emit(started, args)
    if args.wait and isinstance(started, dict) and started.get("id"):
        finish_job(poll_job(job_path("/batch/scrape", started["id"]), args), args)


def cmd_job_status(args: argparse.Namespace) -> None:
    path = job_path(args.job_path, args.id)
    job = poll_job(path, args) if args.wait else call("GET", path, timeout=args.timeout)
    if not isinstance(job, dict):
        die(f"unexpected status payload for {path}")
    if args.raw:
        emit(job, args)
    else:
        finish_job(job, args)


def cmd_job_cancel(args: argparse.Namespace) -> None:
    emit(call("DELETE", job_path(args.job_path, args.id), timeout=args.timeout), args)


def add_common(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--timeout",
        type=float,
        default=360.0,
        help="per-HTTP-request timeout in seconds",
    )
    parser.add_argument(
        "--raw", action="store_true", help="print the full JSON response"
    )
    parser.add_argument(
        "--max-chars", type=int, default=4000, help="truncate printed JSON (0 disables)"
    )


def add_fetch_options(parser: argparse.ArgumentParser) -> None:
    """Options shared by scrape, batch, and crawl page fetches."""
    render = parser.add_mutually_exclusive_group()
    render.add_argument(
        "--js-render",
        action="store_true",
        help="require JavaScript rendering (browser) from the first attempt",
    )
    render.add_argument(
        "--no-js-render",
        action="store_true",
        help="forbid JavaScript rendering; cheapest, fails instead of escalating",
    )
    parser.add_argument(
        "--proxy-strategy",
        choices=PROXY_STRATEGIES,
        metavar="TIER",
        help=f"access-strength tier: {', '.join(PROXY_STRATEGIES)} (default auto)",
    )
    parser.add_argument("--geolocation", metavar="CC", help="2-letter ISO country code")
    parser.add_argument(
        "--no-main-content",
        action="store_true",
        help="keep nav/footer/layout in cleaned outputs",
    )


def add_scrape_options(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--output",
        action="append",
        metavar="FORMAT",
        help=f"repeatable/comma-separated: {', '.join(SCRAPE_OUTPUTS)} (default html)",
    )
    add_fetch_options(parser)
    parser.add_argument(
        "--method", default="GET", choices=("GET", "POST", "PUT", "DELETE")
    )
    parser.add_argument(
        "--body", help="request body as JSON or @file (not valid with GET)"
    )
    parser.add_argument(
        "--tag", action="append", help="repeatable/comma-separated labels"
    )
    parser.add_argument("--actions", help="JSON array of page actions, or @file")
    parser.add_argument("--prompt", help="extraction prompt for --output json")
    parser.add_argument("--schema", help="extraction schema as JSON or @file")


def add_wait_options(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--wait",
        action="store_true",
        help="poll until the job reaches a terminal state",
    )
    parser.add_argument(
        "--interval", type=float, default=10.0, help="poll interval in seconds"
    )
    parser.add_argument(
        "--wait-timeout",
        type=float,
        default=3600.0,
        help="give up polling after N seconds",
    )
    parser.add_argument(
        "--download", metavar="DIR", help="download the completed artifacts into DIR"
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    scrape = subparsers.add_parser("scrape", help="scrape one URL (POST /scrape)")
    scrape.add_argument("--url", required=True)
    scrape.add_argument("--session", help="explicit session id to reuse")
    scrape.add_argument(
        "--session-file",
        metavar="PATH",
        help="read the session id from PATH and write the returned one back",
    )
    scrape.add_argument(
        "--request-id", help="correlation id sent as x-request-id and echoed back"
    )
    scrape.add_argument(
        "--save", metavar="DIR", help="write each output to a file in DIR"
    )
    add_scrape_options(scrape)
    add_common(scrape)
    scrape.set_defaults(func=cmd_scrape)

    mapper = subparsers.add_parser("map", help="discover a site's URLs (POST /map)")
    mapper.add_argument("--url", required=True)
    mapper.add_argument("--limit", type=int, default=1000)
    mapper.add_argument(
        "--sitemap", default="include", choices=("skip", "include", "only")
    )
    mapper.add_argument("--no-subdomains", action="store_true")
    mapper.add_argument("--keep-query-parameters", action="store_true")
    mapper.add_argument("--ignore-cache", action="store_true")
    mapper.add_argument(
        "--request-timeout-ms", type=int, help="server-side discovery budget in ms"
    )
    mapper.add_argument(
        "--save", metavar="FILE", help="write the links array to FILE as JSON"
    )
    add_common(mapper)
    mapper.set_defaults(func=cmd_map)

    serp = subparsers.add_parser("serp", help="get search results (GET /serp)")
    serp.add_argument("--query", required=True)
    serp.add_argument("--country", default="US")
    serp.add_argument("--language", default="en")
    serp.add_argument("--num-results", type=int, default=10)
    serp.add_argument("--tbs", choices=("qdr:d", "qdr:w", "qdr:m"), help="time filter")
    serp.add_argument(
        "--device", default="desktop", choices=("desktop", "mobile", "tablet")
    )
    serp.add_argument("--mode", default="full", choices=("full", "fast"))
    serp.add_argument(
        "--output", action="append", help="html, clean_html, links (enriches results)"
    )
    serp.add_argument("--no-main-content", action="store_true")
    serp.add_argument(
        "--save", metavar="FILE", help="write the full response to FILE as JSON"
    )
    add_common(serp)
    serp.set_defaults(func=cmd_serp)

    crawl = subparsers.add_parser("crawl", help="durable site crawl (/crawl)")
    crawl_sub = crawl.add_subparsers(dest="crawl_command", required=True)

    crawl_start = crawl_sub.add_parser("start", help="POST /crawl")
    crawl_start.add_argument("--url")
    crawl_start.add_argument(
        "--limit", type=int, default=200, help="max pages, 1-10000"
    )
    crawl_start.add_argument(
        "--output",
        action="append",
        help=f"repeatable/comma-separated: {', '.join(CRAWL_OUTPUTS)} (default markdown)",
    )
    crawl_start.add_argument(
        "--sitemap", default="include", choices=("skip", "include", "only")
    )
    crawl_start.add_argument("--include-path", action="append", metavar="REGEX")
    crawl_start.add_argument("--exclude-path", action="append", metavar="REGEX")
    crawl_start.add_argument("--max-depth", type=int)
    crawl_start.add_argument("--allow-subdomains", action="store_true")
    crawl_start.add_argument("--crawl-entire-domain", action="store_true")
    crawl_start.add_argument("--ignore-query-parameters", action="store_true")
    crawl_start.add_argument("--email")
    crawl_start.add_argument("--webhook-url")
    add_fetch_options(crawl_start)
    crawl_start.add_argument(
        "--scrape-options",
        metavar="JSON",
        help="extra per-page fetch options as JSON or @file (snake_case)",
    )
    crawl_start.add_argument(
        "--json", metavar="JSON", help="full crawl request body as JSON or @file"
    )
    add_wait_options(crawl_start)
    add_common(crawl_start)
    crawl_start.set_defaults(func=cmd_crawl_start, job_path="/crawl")

    crawl_status = crawl_sub.add_parser("status", help="GET /crawl/{id}")
    crawl_status.add_argument("id")
    add_wait_options(crawl_status)
    add_common(crawl_status)
    crawl_status.set_defaults(func=cmd_job_status, job_path="/crawl")

    crawl_cancel = crawl_sub.add_parser("cancel", help="DELETE /crawl/{id}")
    crawl_cancel.add_argument("id")
    add_common(crawl_cancel)
    crawl_cancel.set_defaults(func=cmd_job_cancel, job_path="/crawl")

    batch = subparsers.add_parser(
        "batch", help="durable multi-URL scrape (/batch/scrape)"
    )
    batch_sub = batch.add_subparsers(dest="batch_command", required=True)

    batch_start = batch_sub.add_parser("start", help="POST /batch/scrape")
    batch_start.add_argument(
        "--url", action="append", help="repeatable/comma-separated URLs"
    )
    batch_start.add_argument(
        "--urls-file", metavar="FILE", help="one URL per line ('#' comments ignored)"
    )
    batch_start.add_argument(
        "--limit", type=int, default=10000, help="max URLs, 1-10000"
    )
    batch_start.add_argument("--session", help="session id applied to every URL")
    batch_start.add_argument(
        "--session-file", metavar="PATH", help="read the session id from PATH"
    )
    add_scrape_options(batch_start)
    add_wait_options(batch_start)
    add_common(batch_start)
    batch_start.set_defaults(func=cmd_batch_start, job_path="/batch/scrape")

    batch_status = batch_sub.add_parser("status", help="GET /batch/scrape/{id}")
    batch_status.add_argument("id")
    add_wait_options(batch_status)
    add_common(batch_status)
    batch_status.set_defaults(func=cmd_job_status, job_path="/batch/scrape")

    batch_cancel = batch_sub.add_parser("cancel", help="DELETE /batch/scrape/{id}")
    batch_cancel.add_argument("id")
    add_common(batch_cancel)
    batch_cancel.set_defaults(func=cmd_job_cancel, job_path="/batch/scrape")

    return parser


def main() -> None:
    args = build_parser().parse_args()
    if not hasattr(args, "download"):
        args.download = None
    if not hasattr(args, "wait"):
        args.wait = False
    args.func(args)


if __name__ == "__main__":
    main()
