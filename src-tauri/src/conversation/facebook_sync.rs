//! Forward-only monotonic Messenger synchronization (revised plan Task 3).
//!
//! Invariants enforced at this layer:
//! - Events are applied strictly in ascending provider order by the caller;
//!   helpers below additionally guard every transition so replays and
//!   out-of-order deliveries can never regress message status.
//! - Delivery receipts move `sent|queued|pending -> delivered` only.
//! - Read receipts apply to OUTBOUND messages with
//!   `provider_timestamp <= watermark`, moving them to `read` from any
//!   non-terminal pre-read state; inbound rows are never touched.
//! - Reactions carrying out-of-range/invalid timestamps are quarantined into
//!   `channel_sync_dead_letters` with `item_index = idx` instead of being
//!   dropped or applied.
use rusqlite::{params, Connection};

/// Marks matching messages delivered. Forward-only: rows already `delivered`
/// or `read` are never regressed. Returns the number of rows advanced.
pub fn mark_messages_delivered(
    conn: &Connection,
    conversation_id: i64,
    provider_message_id: &str,
) -> anyhow::Result<usize> {
    let updated = conn.execute(
        "UPDATE messages SET status = 'delivered'
          WHERE conversation_id = ?1 AND provider_message_id = ?2
            AND status IN ('sent', 'queued', 'pending')",
        params![conversation_id, provider_message_id],
    )?;
    Ok(updated)
}

/// Applies a read receipt watermark to outbound messages. Forward-only:
/// only pre-`read` states transition, and only messages whose
/// `provider_timestamp` is at or before the watermark. Returns rows advanced.
pub fn apply_read_receipt(
    conn: &Connection,
    conversation_id: i64,
    watermark: &str,
) -> anyhow::Result<usize> {
    // Watermark is an epoch-milliseconds string from Meta; lexical comparison
    // against the stored ISO-8601 provider_timestamp is wrong, so compare on
    // parsed millisecond values instead.
    let watermark_ms: i64 = watermark.parse()?;
    let mut stmt = conn.prepare(
        "SELECT id, provider_timestamp FROM messages
          WHERE conversation_id = ?1 AND direction = 'outbound'
            AND status IN ('sent', 'queued', 'pending', 'delivered')
            AND provider_timestamp IS NOT NULL",
    )?;
    let candidates: Vec<(i64, String)> = stmt
        .query_map(params![conversation_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    let mut updated = 0usize;
    for (id, ts) in candidates {
        if timestamp_to_epoch_millis(&ts)
            .map(|ms| ms <= watermark_ms)
            .unwrap_or(false)
        {
            updated += conn.execute(
                "UPDATE messages SET status = 'read' WHERE id = ?1 AND status <> 'read'",
                params![id],
            )?;
        }
    }
    Ok(updated)
}

fn timestamp_to_epoch_millis(iso: &str) -> Option<i64> {
    // Stored timestamps are RFC-3339 with millisecond precision (utc_now()).
    chrono::DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

/// Quarantines a reaction event whose timestamp cannot be trusted. Mirrors the
/// array-indexed dead-letter contract: `item_index = idx as i64`.
#[allow(clippy::too_many_arguments)]
pub fn quarantine_reaction(
    conn: &Connection,
    channel_account_id: i64,
    scope: &str,
    event_id: i64,
    item_index: usize,
    event_key: &str,
    payload_json: &str,
    error_reason: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO channel_sync_dead_letters
            (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, attempts)
         VALUES (?1, ?2, ?3, ?4, 'reaction', ?5, ?6, ?7, 'pending', 0)
         ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
        params![
            channel_account_id,
            scope,
            event_id,
            item_index as i64,
            event_key,
            payload_json,
            error_reason
        ],
    )?;
    Ok(())
}

/// Decides whether a reaction webhook timestamp is within the acceptable
/// range. Meta reaction timestamps are epoch milliseconds; values that parse
/// outside [0, now + 5 min skew] are treated as corrupt.
pub fn is_reaction_timestamp_valid(timestamp_ms: f64, now_ms: i64) -> bool {
    if !timestamp_ms.is_finite() || timestamp_ms < 0.0 {
        return false;
    }
    timestamp_ms <= (now_ms + 5 * 60 * 1000) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id INTEGER NOT NULL,
                direction TEXT NOT NULL CHECK (direction IN ('inbound','outbound','system')),
                provider_message_id TEXT,
                provider_timestamp TEXT,
                content_type TEXT NOT NULL DEFAULT 'text',
                status TEXT NOT NULL DEFAULT 'received'
             );
             CREATE TABLE channel_sync_dead_letters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL,
                scope TEXT NOT NULL,
                event_id INTEGER NOT NULL,
                item_index INTEGER NOT NULL DEFAULT 0,
                event_type TEXT NOT NULL,
                event_key TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                error_reason TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                attempts INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(channel_account_id, scope, event_id, item_index)
             );",
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO messages (id, conversation_id, direction, provider_message_id, provider_timestamp, status) VALUES
             (1, 500, 'outbound', 'mid.out.sent',      '2026-08-21T00:00:00.000Z', 'sent'),
             (2, 500, 'outbound', 'mid.out.delivered', '2026-08-21T00:00:01.000Z', 'delivered'),
             (3, 500, 'outbound', 'mid.out.read',      '2026-08-21T00:00:02.000Z', 'read'),
             (4, 500, 'outbound', 'mid.out.late',      '2026-08-21T05:00:00.000Z', 'sent'),
             (5, 500, 'inbound',  'mid.in.msg',        '2026-08-21T00:00:03.000Z', 'received');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn delivery_is_forward_only_and_scoped() {
        let conn = fixture_conn();

        // sent -> delivered advances.
        assert_eq!(mark_messages_delivered(&conn, 500, "mid.out.sent").unwrap(), 1);
        // Already-delivered/read rows do not regress.
        assert_eq!(mark_messages_delivered(&conn, 500, "mid.out.delivered").unwrap(), 0);
        assert_eq!(mark_messages_delivered(&conn, 500, "mid.out.read").unwrap(), 0);
        // Unknown ids touch nothing.
        assert_eq!(mark_messages_delivered(&conn, 500, "mid.missing").unwrap(), 0);
        // Other conversations are isolated by scoping.
        assert_eq!(mark_messages_delivered(&conn, 999, "mid.out.sent").unwrap(), 0);

        let status: String = conn
            .query_row("SELECT status FROM messages WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "delivered");
    }

    #[test]
    fn read_receipt_applies_only_to_outbound_at_or_before_watermark() {
        let conn = fixture_conn();

        // Watermark covers mid.out.sent (t=0s) and mid.out.delivered (t=1s);
        // mid.out.late (t=5h) must stay unread; inbound untouched; already-read stays read.
        let wm = timestamp_to_epoch_millis("2026-08-21T00:00:01.500Z").unwrap().to_string();
        let advanced = apply_read_receipt(&conn, 500, &wm).unwrap();
        assert_eq!(advanced, 2, "sent + delivered advance to read");

        let status = |id: i64| -> String {
            conn.query_row("SELECT status FROM messages WHERE id = ?1", params![id], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(status(1), "read");
        assert_eq!(status(2), "read");
        assert_eq!(status(3), "read", "already-read remains read");
        assert_eq!(status(4), "sent", "beyond-watermark outbound not advanced");
        assert_eq!(status(5), "received", "inbound never touched by read receipts");

        // Replay is a no-op (forward-only).
        assert_eq!(apply_read_receipt(&conn, 500, &wm).unwrap(), 0);
    }

    #[test]
    fn invalid_watermark_is_an_error_not_silent_success() {
        let conn = fixture_conn();
        assert!(apply_read_receipt(&conn, 500, "not-a-number").is_err());
    }

    #[test]
    fn reaction_quarantine_is_idempotent_per_item_index() {
        let conn = fixture_conn();
        quarantine_reaction(&conn, 800, "facebook", 42, 0, "react_mid_1", r#"{"x":1}"#, "timestamp_out_of_range").unwrap();
        quarantine_reaction(&conn, 800, "facebook", 42, 0, "react_mid_1", r#"{"x":1}"#, "timestamp_out_of_range").unwrap();
        quarantine_reaction(&conn, 800, "facebook", 42, 1, "react_mid_2", r#"{"x":2}"#, "timestamp_out_of_range").unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM channel_sync_dead_letters", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2, "same (event,item_index) dedupes; distinct indexes kept");

        let idx: i64 = conn
            .query_row("SELECT item_index FROM channel_sync_dead_letters WHERE event_key='react_mid_2'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(idx, 1, "item_index mirrors array position");
    }

    #[test]
    fn reaction_timestamp_range_check() {
        let now: i64 = 1_800_000_000_000; // fixed "now" in ms
        assert!(is_reaction_timestamp_valid(now as f64, now));
        assert!(is_reaction_timestamp_valid((now - 86_400_000) as f64, now), "a day old is fine");
        assert!(!is_reaction_timestamp_valid(-1.0, now));
        assert!(!is_reaction_timestamp_valid(f64::NAN, now));
        assert!(!is_reaction_timestamp_valid(f64::INFINITY, now));
        assert!(!is_reaction_timestamp_valid((now + 6 * 60 * 1000) as f64, now), "beyond 5-min future skew rejected");
        assert!(is_reaction_timestamp_valid((now + 60_000) as f64, now), "small clock skew tolerated");
    }
}
