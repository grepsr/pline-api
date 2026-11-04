//! The public OpenAPI 3.1 description served at `/openapi.json`.
//!
//! Built from the live tool router, so every tool's input schema is exactly what `tools/list`
//! returns. MCP is JSON-RPC over one URL, so the document describes `POST /mcp` with one request
//! body variant per tool (`tools/call`) and per protocol method, plus the public helper routes.
//! Each tool schema's `$defs` are hoisted into `components/schemas`, because a `#/$defs/...`
//! reference inside an OpenAPI document resolves against the document root, not the schema.

use serde_json::{json, Map, Value};

use crate::PlineServer;

/// Rewrites `#/$defs/<name>` references to `#/components/schemas/<name>` everywhere in `value`.
fn rewrite_refs(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/$defs/") {
                    *reference = format!("#/components/schemas/{name}");
                }
            }
            map.values_mut().for_each(rewrite_refs);
        }
        Value::Array(items) => items.iter_mut().for_each(rewrite_refs),
        _ => {}
    }
}

/// Moves a tool schema's `$defs` into `schemas`, drops the per-schema `$schema` dialect marker,
/// and returns the schema with its references pointed at `components/schemas`.
fn hoist(schema: &Map<String, Value>, schemas: &mut Map<String, Value>) -> Value {
    let mut schema = schema.clone();
    schema.remove("$schema");
    if let Some(Value::Object(defs)) = schema.remove("$defs") {
        for (name, mut def) in defs {
            rewrite_refs(&mut def);
            schemas.entry(name).or_insert(def);
        }
    }
    let mut schema = Value::Object(schema);
    rewrite_refs(&mut schema);
    schema
}

fn component_name(tool: &str) -> String {
    let pascal: String = tool
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect();
    format!("{pascal}Arguments")
}

fn jsonrpc_request(method: &str, params: Value) -> Value {
    json!({
        "type": "object",
        "required": ["jsonrpc", "id", "method"],
        "properties": {
            "jsonrpc": {"const": "2.0"},
            "id": {"type": ["string", "integer"]},
            "method": {"const": method},
            "params": params
        }
    })
}

/// The OpenAPI document; `public_url` becomes the single server entry.
pub fn document(public_url: &str) -> Value {
    let mut schemas = Map::new();
    let mut variants = Vec::new();
    let mut tool_list = Vec::new();

    let mut tools = PlineServer::tool_router().list_all();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    for tool in tools {
        let name = component_name(&tool.name);
        let mut arguments = hoist(&tool.input_schema, &mut schemas);
        if let (Value::Object(arguments), Some(description)) = (&mut arguments, &tool.description) {
            arguments
                .entry("description")
                .or_insert_with(|| Value::String(description.to_string()));
        }
        schemas.insert(name.clone(), arguments);
        let mut call = jsonrpc_request(
            "tools/call",
            json!({
                "type": "object",
                "required": ["name", "arguments"],
                "properties": {
                    "name": {"const": tool.name},
                    "arguments": {"$ref": format!("#/components/schemas/{name}")}
                }
            }),
        );
        call["title"] = json!(format!("tools/call {}", tool.name));
        if let Some(description) = &tool.description {
            call["description"] = json!(description);
        }
        variants.push(call);
        tool_list.push(json!({
            "name": tool.name,
            "title": tool.annotations.as_ref().and_then(|a| a.title.clone()).or(tool.title),
            "description": tool.description,
            "arguments": format!("#/components/schemas/{name}"),
            "annotations": tool.annotations,
        }));
    }

    for (method, params) in [
        (
            "initialize",
            json!({
                "type": "object",
                "required": ["protocolVersion", "capabilities", "clientInfo"],
                "properties": {
                    "protocolVersion": {"type": "string"},
                    "capabilities": {"type": "object"},
                    "clientInfo": {
                        "type": "object",
                        "required": ["name", "version"],
                        "properties": {"name": {"type": "string"}, "version": {"type": "string"}}
                    }
                }
            }),
        ),
        ("tools/list", json!({"type": "object"})),
        ("resources/list", json!({"type": "object"})),
        (
            "resources/read",
            json!({
                "type": "object",
                "required": ["uri"],
                "properties": {"uri": {"enum": [crate::GUIDE_URI, crate::REFERENCE_URI]}}
            }),
        ),
    ] {
        let mut request = jsonrpc_request(method, params);
        request["title"] = json!(method);
        variants.push(request);
    }

    // No discriminator: every tool shares `method: tools/call`, so `method` alone cannot pick one.
    schemas.insert("JsonRpcRequest".into(), json!({"oneOf": variants}));
    schemas.insert(
        "JsonRpcResponse".into(),
        json!({
            "type": "object",
            "required": ["jsonrpc", "id"],
            "properties": {
                "jsonrpc": {"const": "2.0"},
                "id": {"type": ["string", "integer", "null"]},
                "result": {
                    "description": "For tools/call: a CallToolResult whose structuredContent is the pline.ai API response, and isError=true with a text message on failure."
                },
                "error": {
                    "type": "object",
                    "required": ["code", "message"],
                    "properties": {
                        "code": {"type": "integer"},
                        "message": {"type": "string"},
                        "data": {}
                    }
                }
            }
        }),
    );
    schemas.insert(
        "Error".into(),
        json!({"type": "object", "properties": {"detail": {"type": "string"}}}),
    );

    let public_url = public_url.trim_end_matches('/');
    let mcp_operation = json!({
        "operationId": "mcp",
        "summary": "MCP Streamable HTTP endpoint (JSON-RPC 2.0)",
        "description": "Send one JSON-RPC request per POST. Call a tool with method `tools/call`, `params.name` set to the tool, and `params.arguments` matching that tool's schema. Responses are JSON or a single-event SSE stream, depending on `Accept`.",
        "tags": ["mcp"],
        "security": [{"bearer": []}, {"apiKey": []}],
        "parameters": [{
            "name": "Accept",
            "in": "header",
            "required": true,
            "schema": {"type": "string", "default": "application/json, text/event-stream"}
        }],
        "requestBody": {
            "required": true,
            "content": {"application/json": {"schema": {"$ref": "#/components/schemas/JsonRpcRequest"}}}
        },
        "responses": {
            "200": {
                "description": "JSON-RPC response",
                "content": {
                    "application/json": {"schema": {"$ref": "#/components/schemas/JsonRpcResponse"}},
                    "text/event-stream": {"schema": {"type": "string"}}
                }
            },
            "202": {"description": "Notification accepted"},
            "401": {
                "description": "Missing or invalid API key or OAuth token",
                "headers": {"WWW-Authenticate": {"schema": {"type": "string"}}},
                "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Error"}}}
            }
        }
    });

    json!({
        "openapi": "3.1.0",
        "jsonSchemaDialect": "https://json-schema.org/draft/2020-12/schema",
        "info": {
            "title": "pline.ai MCP server",
            "version": env!("CARGO_PKG_VERSION"),
            "summary": "Scrape, crawl, batch-scrape, map, and search the web through pline.ai over MCP.",
            "description": "The hosted pline.ai Model Context Protocol server. Every tool is called through `POST /mcp` as a JSON-RPC `tools/call` request; `x-mcp-tools` lists them with their argument schemas.",
            "license": {"name": "MIT", "identifier": "MIT"},
            "contact": {"url": "https://pline.ai"}
        },
        "externalDocs": {"url": "https://github.com/grepsr/pline-api#readme"},
        "servers": [{"url": public_url}],
        "tags": [
            {"name": "mcp", "description": "Model Context Protocol"},
            {"name": "public", "description": "Unauthenticated helper routes"}
        ],
        "paths": {
            "/mcp": {"post": mcp_operation},
            "/health": {"get": {
                "operationId": "health",
                "tags": ["public"],
                "security": [],
                "responses": {"200": {"description": "Liveness", "content": {"application/json": {"schema": {
                    "type": "object",
                    "properties": {"status": {"const": "ok"}, "service": {"type": "string"}}
                }}}}}
            }},
            "/.well-known/oauth-protected-resource": {"get": {
                "operationId": "oauthProtectedResource",
                "summary": "OAuth protected resource metadata (RFC 9728)",
                "tags": ["public"],
                "security": [],
                "responses": {
                    "200": {"description": "Resource metadata", "content": {"application/json": {"schema": {"type": "object"}}}},
                    "404": {"description": "OAuth is not configured on this deployment"}
                }
            }},
            "/openapi.json": {"get": {
                "operationId": "openapi",
                "summary": "This document",
                "tags": ["public"],
                "security": [],
                "responses": {"200": {"description": "OpenAPI 3.1 document", "content": {"application/json": {}}}}
            }}
        },
        "components": {
            "schemas": schemas,
            "securitySchemes": {
                "bearer": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "A pline.ai API key, or an OAuth access token from the pline.ai sign-in flow."
                },
                "apiKey": {"type": "apiKey", "in": "header", "name": "x-api-key"}
            }
        },
        "x-mcp-tools": tool_list
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(reference)) = map.get("$ref") {
                    out.push(reference.clone());
                }
                map.values().for_each(|v| refs(v, out));
            }
            Value::Array(items) => items.iter().for_each(|v| refs(v, out)),
            _ => {}
        }
    }

    #[test]
    fn describes_every_tool_with_resolvable_refs() {
        let doc = document("https://mcp.example.test/");
        assert_eq!(doc["servers"][0]["url"], "https://mcp.example.test");
        let tools = PlineServer::tool_router().list_all();
        assert_eq!(doc["x-mcp-tools"].as_array().unwrap().len(), tools.len());
        for tool in &tools {
            assert!(doc["components"]["schemas"][component_name(&tool.name)].is_object());
        }
        let mut all = Vec::new();
        refs(&doc, &mut all);
        assert!(!all.is_empty());
        for reference in all {
            let name = reference
                .strip_prefix("#/components/schemas/")
                .unwrap_or_else(|| panic!("unexpected ref {reference}"));
            assert!(
                doc["components"]["schemas"].get(name).is_some(),
                "dangling ref {reference}"
            );
        }
    }

    #[test]
    fn names_components_in_pascal_case() {
        assert_eq!(
            component_name("batch_scrape_start"),
            "BatchScrapeStartArguments"
        );
    }
}
