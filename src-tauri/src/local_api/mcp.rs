//! Minimal MCP (Streamable HTTP, JSON responses only, no SSE) at POST /mcp.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::{json, Value};

use super::clients::{Client, SCOPE_INBOX, SCOPE_READ, SCOPE_SEND, SCOPE_TIMELINE};
use super::error::{ApiError, ApiResult};
use super::handlers::ApiState;
use super::intake;

const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// Tool name attached to the response so the request log can include it.
#[derive(Clone, Copy)]
pub struct McpTool(pub &'static str);

pub const TOOL_SEND: &str = "send_message";

fn tool_scope(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "inbox_unread" => Some(("inbox_unread", SCOPE_INBOX)),
        "read_timeline" => Some(("read_timeline", SCOPE_TIMELINE)),
        "mark_read" => Some(("mark_read", SCOPE_READ)),
        TOOL_SEND => Some((TOOL_SEND, SCOPE_SEND)),
        _ => None,
    }
}

fn tools() -> Value {
    let id = json!({"type": "integer", "description": "conversation id"});
    json!([
        {"name": "inbox_unread", "description": "List private chats with unread messages, newest first.",
         "inputSchema": {"type": "object", "properties": {"limit": {"type": "integer", "minimum": 1, "maximum": intake::INBOX_MAX}}}},
        {"name": "read_timeline", "description": "Read messages of a chat, oldest first; pass next as before for older ones.",
         "inputSchema": {"type": "object", "properties": {"id": id, "limit": {"type": "integer", "minimum": 1, "maximum": intake::TIMELINE_MAX}, "before": {"type": "string"}}, "required": ["id"]}},
        {"name": "mark_read", "description": "Mark a chat as read.",
         "inputSchema": {"type": "object", "properties": {"id": id}, "required": ["id"]}},
        {"name": TOOL_SEND, "description": "Queue a text reply to a chat.",
         "inputSchema": {"type": "object", "properties": {"id": id, "text": {"type": "string", "minLength": 1, "maxLength": intake::TEXT_MAX_CHARS}, "replyTo": {"type": "integer"}}, "required": ["id", "text"]}}
    ])
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn origin_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let host = origin
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(origin);
    let host = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// Cheap peek used by the middleware to apply the send limit before the handler runs.
pub fn is_send_call(body: &[u8]) -> bool {
    serde_json::from_slice::<Value>(body)
        .map(|v| v["method"] == "tools/call" && v["params"]["name"] == TOOL_SEND)
        .unwrap_or(false)
}

fn arg_i64(args: &Value, key: &str) -> ApiResult<i64> {
    args.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::bad_request(format!("'{key}' must be an integer")))
}

fn arg_opt_u32(args: &Value, key: &str) -> ApiResult<Option<u32>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| ApiError::bad_request(format!("'{key}' must be a positive integer"))),
    }
}

fn run_tool(state: &ApiState, name: &str, args: &Value) -> ApiResult<Value> {
    let conv = &state.conversation;
    let to_value = |v: Result<Value, serde_json::Error>| v.map_err(ApiError::internal);
    match name {
        "inbox_unread" => to_value(serde_json::to_value(intake::inbox_unread(
            conv,
            arg_opt_u32(args, "limit")?,
        )?)),
        "read_timeline" => to_value(serde_json::to_value(intake::read_timeline(
            conv,
            arg_i64(args, "id")?,
            arg_opt_u32(args, "limit")?,
            args.get("before").and_then(Value::as_str),
        )?)),
        "mark_read" => {
            let marked = intake::mark_read(conv, arg_i64(args, "id")?)?;
            to_value(serde_json::to_value(intake::MarkReadView {
                ok: true,
                marked,
            }))
        }
        _ => {
            let text = args
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiError::bad_request("'text' must be a string"))?;
            let reply_to = match args.get("replyTo") {
                None | Some(Value::Null) => None,
                Some(_) => Some(arg_i64(args, "replyTo")?),
            };
            let message =
                intake::send_message(conv, arg_i64(args, "id")?, text.to_string(), None, reply_to)?;
            to_value(serde_json::to_value(intake::SendView {
                ok: true,
                id: message.id,
                status: "queued",
            }))
        }
    }
}

fn tool_text(value: &Value, is_error: bool) -> Value {
    let text = serde_json::to_string(value).unwrap_or_default();
    let mut result = json!({"content": [{"type": "text", "text": text}]});
    if is_error {
        result["isError"] = Value::Bool(true);
    }
    result
}

pub async fn handle(
    State(state): State<ApiState>,
    Extension(client): Extension<Client>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !origin_allowed(&headers) {
        return ApiError::forbidden("origin not allowed").into_response();
    }
    let Ok(request) = serde_json::from_slice::<Value>(&body) else {
        return Json(rpc_error(Value::Null, -32700, "parse error")).into_response();
    };
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Json(rpc_error(Value::Null, -32600, "invalid request")).into_response();
    };
    let Some(id) = request.get("id").cloned() else {
        // Notification (e.g. notifications/initialized): accepted, no body.
        return StatusCode::ACCEPTED.into_response();
    };
    let params = request.get("params").cloned().unwrap_or(Value::Null);

    let mut tool = None;
    let reply = match method {
        "initialize" => {
            let asked = params["protocolVersion"].as_str().unwrap_or_default();
            let version = PROTOCOL_VERSIONS
                .iter()
                .find(|v| **v == asked)
                .unwrap_or(&PROTOCOL_VERSIONS[0]);
            rpc_result(
                id,
                json!({"protocolVersion": version, "capabilities": {"tools": {}},
                       "serverInfo": {"name": "rustsale", "version": env!("CARGO_PKG_VERSION")}}),
            )
        }
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, json!({"tools": tools()})),
        "tools/call" => match params["name"].as_str().and_then(tool_scope) {
            None => rpc_error(id, -32602, "unknown tool"),
            Some((name, scope)) => {
                tool = Some(McpTool(name));
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let outcome = if client.has_scope(scope) {
                    run_tool(&state, name, &args)
                } else {
                    Err(ApiError::forbidden(format!("missing scope '{scope}'")))
                };
                match outcome {
                    Ok(value) => rpc_result(id, tool_text(&value, false)),
                    Err(error) => rpc_result(id, tool_text(&json!(error.body()), true)),
                }
            }
        },
        _ => rpc_error(id, -32601, "method not found"),
    };
    let mut response = Json(reply).into_response();
    if let Some(tool) = tool {
        response.extensions_mut().insert(tool);
    }
    response
}
