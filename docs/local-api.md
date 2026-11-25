# RustSale local API

Bound to `127.0.0.1:8787` only (`RUSTSALE_LOCAL_API_PORT`). Every request needs `Authorization: Bearer <token>`.
Body limit 64 KB. Errors are always JSON `{"code","message"}`.

## Clients and tokens
- Clients file: `~/.local/share/com.rustsale.crm/local-api-clients.json` (override `RUSTSALE_LOCAL_API_CLIENTS_FILE`), mode 600,
  entries `{name, sha256, scopes, disabled?}`. Only SHA-256 hashes are stored; reloaded automatically when the file changes.
- Scopes: `inbox`, `timeline`, `read`, `send`.
- `RUSTSALE_LOCAL_API_TOKEN` (env, legacy) still works as client `legacy-env` with all scopes; a warning is logged at startup.
- Create / rotate: `scripts/local-api-token.sh create <name> [scopes]` (requires `RUSTSALE_DATA_DIR`, no default; v3 never falls back to the production dir) → token in `$RUSTSALE_DATA_DIR/<name>.token` (600). Re-running `create` rotates.
- Revoke: `scripts/local-api-token.sh revoke <name>` (or set `"disabled": true`). Takes effect on the next request, no restart.
- List: `scripts/local-api-token.sh list` (never shows tokens).

## Limits
Per client, fixed 1-minute window: 60 requests, 10 sends. Over budget → `429` + `Retry-After`. Counted before the handler, including failed requests.

## REST
| Method | Path | Scope | Notes |
|---|---|---|---|
| GET | `/api/inbox[?view=compact&limit=20]` | inbox | compact: unread private chats `{id,name,unread,preview,channelAccountId,at}`; limit ≤ 50 |
| GET | `/api/conversations/{id}/timeline[?view=compact&limit=10&before=]` | timeline | compact: `{items:[{id,dir,text,at,att?,atts?,st?}],next}`, oldest first, limit ≤ 50, `before` = `next`. Full view: limit ≤ 100, `before` = JSON cursor |
| GET | `/api/attachments/{id}` | timeline | raw bytes of one **image** attachment, `Content-Type` = detected image MIME; limit 8 MiB. Errors: see below |
| POST | `/api/conversations/{id}/read` | read | `{rowsMarked}` |
| POST | `/api/conversations/{id}/messages` | send | body `{text, channelAccountId?, replyToMessageId?}`; text 1..2000 chars; account inferred if omitted, mismatch → 400. `201` + message |

`att` (legacy) lists attachment kinds, e.g. `["image"]`. `atts` lists the same attachments as `[{id,kind,mime?}]`; `id` is the `attachment_id` for `read_attachment`. Neither exposes provider URLs.

Without `view=compact` the responses keep the original full shape.

## MCP (`POST /mcp`, Streamable HTTP, JSON only)
Methods: `initialize` (2025-06-18 / 2025-03-26 / 2024-11-05), `ping`, `tools/list`, `tools/call`. Notifications → `202`. GET/DELETE → `405`.
Non-local `Origin` → `403`. JSON-RPC errors: `-32700` parse, `-32600` invalid, `-32601` method, `-32602` unknown tool.
Tool failures come back as `result.isError: true` with text `{"code","message"}`.

| Tool | Args | Scope | Result |
|---|---|---|---|
| `inbox_unread` | `limit?` | inbox | same as compact inbox |
| `read_timeline` | `id, limit?, before?` | timeline | same as compact timeline |
| `mark_read` | `id` | read | `{ok, marked}` |
| `send_message` | `id, text, replyTo?` | send | `{ok, id, status:"queued"}` |
| `read_attachment` | `attachment_id` | timeline | MCP image block `{"type":"image","data":<base64>,"mimeType":"image/jpeg"}`; same limits as REST. Counts against the timeline budget |

### `read_attachment` / `GET /api/attachments/{id}` errors
Bytes come from the stored local file, else the provider URL (http/https, public hosts only, no redirects, 15 s), else an inline thumbnail. URLs and paths are never returned.

| `code` | HTTP | When |
|---|---|---|
| `ATTACHMENT_NOT_FOUND` | 404 | unknown attachment id |
| `UNSUPPORTED_KIND` | 415 | not an image (file, video, voice, sticker…) |
| `LINK_EXPIRED` | 410 | provider answered 401/403/404/410 — ask the customer to resend |
| `DOWNLOAD_FAILED` | 502 | network error, provider 5xx, no usable source, not an image, private host |
| `TOO_LARGE` | 413 | declared or actual size > 8 MiB |

## Zalo webhook payload
Besides the text fields, each POST carries `attachments: [{messageId, attachmentId, kind}]` for every message of the batch (empty when none). Pass `attachmentId` to `read_attachment`. No provider URL is ever included.

## Status codes
| Code | When |
|---|---|
| 400 | bad id, JSON, limit, cursor, text, channelAccountId |
| 401 | missing/invalid token |
| 403 | missing scope, foreign Origin |
| 404 | unknown conversation or route |
| 405 | wrong method |
| 409 | account disconnected / conflicting state |
| 413 | body > 64 KB |
| 415 | wrong content-type |
| 429 | rate limited (`Retry-After`) |
| 500 | internal (details only in the log) |
| 502 / 503 | messaging provider error / provider rate limit |

## Logging
One line per request: client, method, route template, MCP tool, status, ms. Never headers, tokens, query strings or message text.
