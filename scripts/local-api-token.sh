#!/usr/bin/env bash
# Manage RustSale local API clients. Never prints a token.
#   local-api-token.sh create <name> [scopes]   # scopes: comma list, default inbox,timeline,read,send
#   local-api-token.sh revoke <name>
#   local-api-token.sh list
# Requires RUSTSALE_DATA_DIR (no default). Token plaintext goes to $DATA_DIR/<name>.token (mode 600); the clients file keeps only its sha256.
# The running app reloads the clients file on change; no restart needed.
set -euo pipefail
umask 077
# No default on purpose: v3 must never fall back to the production data dir
# (~/.local/share/com.rustsale.crm). The caller names the directory explicitly.
if [ -z "${RUSTSALE_DATA_DIR:-}" ]; then
  echo "error: RUSTSALE_DATA_DIR is required (no default; v3 never falls back to the production data dir)" >&2
  exit 2
fi
DATA_DIR="$RUSTSALE_DATA_DIR"
[ -d "$DATA_DIR" ] || { echo "error: RUSTSALE_DATA_DIR '$DATA_DIR' is not an existing directory" >&2; exit 2; }
FILE="${RUSTSALE_LOCAL_API_CLIENTS_FILE:-$DATA_DIR/local-api-clients.json}"
command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }
[ -f "$FILE" ] || echo '[]' > "$FILE"
chmod 600 "$FILE"

write() { local tmp; tmp="$(mktemp "$FILE.XXXX")"; jq "$@" "$FILE" > "$tmp"; chmod 600 "$tmp"; mv "$tmp" "$FILE"; }

case "${1:-}" in
  create)
    name="${2:?name required}"; scopes="${3:-inbox,timeline,read,send}"
    [[ "$name" =~ ^[a-z0-9_-]+$ ]] || { echo "name must match [a-z0-9_-]+" >&2; exit 1; }
    token="$(head -c 32 /dev/urandom | base64 | tr '+/' '-_' | tr -d '=\n')"
    hash="$(printf '%s' "$token" | sha256sum | cut -d' ' -f1)"
    printf '%s' "$token" > "$DATA_DIR/$name.token"; chmod 600 "$DATA_DIR/$name.token"
    unset token
    write --arg n "$name" --arg h "$hash" --arg s "$scopes" \
      '[.[] | select(.name != $n)] + [{name: $n, sha256: $h, scopes: ($s | split(","))}]'
    echo "client '$name' created; token saved to $DATA_DIR/$name.token" ;;
  revoke)
    name="${2:?name required}"
    write --arg n "$name" '[.[] | select(.name != $n)]'
    rm -f "$DATA_DIR/$name.token"
    echo "client '$name' revoked" ;;
  list)
    jq -r '.[] | "\(.name)\t\(.scopes | join(","))\t\(if .disabled then "disabled" else "active" end)"' "$FILE" ;;
  *) sed -n '2,7p' "$0"; exit 1 ;;
esac
