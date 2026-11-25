//! Synthetic conversations and scrape results for screen tests and screenshots.
//!
//! Nothing here runs by itself: the native module calls it only on a throwaway data folder.

use rusqlite::{params, Connection};

/// What a seed run inserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoSeed {
    /// Id of the conversation with the most messages.
    pub long_conversation_id: i64,
    /// Messages in that conversation.
    pub long_conversation_messages: u32,
}

const ACCOUNT_KEY: &str = "demo-account";

/// Inserts one connected Zalo account, three conversations (one long) and their messages.
pub fn seed_demo_conversations(conn: &Connection) -> rusqlite::Result<DemoSeed> {
    let now = "2026-07-20T09:00:00.000Z";
    let account: i64 = conn.query_row(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, display_name, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', ?1, 'Tài khoản demo', 'connected', '{}', ?2, ?2)
         RETURNING id",
        params![ACCOUNT_KEY, now],
        |row| row.get(0),
    )?;
    let people = [
        ("demo-thread-1", "Nguyễn Văn An", 1, 30_u32),
        ("demo-thread-2", "Trần Thị Bình", 2, 4),
        ("demo-thread-3", "Lê Quang Cường", 3, 2),
    ];
    let mut long_conversation_id = 0;
    for (thread, name, lead_id, count) in people {
        let identity: i64 = conn.query_row(
            "INSERT INTO channel_identities
             (lead_id, channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
             VALUES (?1, 'zalo', ?2, 'zalo_uid', ?3, ?4, ?5, ?5)
             RETURNING id",
            params![lead_id, account, thread, name, now],
            |row| row.get(0),
        )?;
        let conversation: i64 = conn.query_row(
            "INSERT INTO channel_conversations
             (channel_account_id, external_thread_id, kind, title, status, last_message_at, last_synced_at, created_at, updated_at)
             VALUES (?1, ?2, 'direct', ?3, 'active', ?4, ?4, ?4, ?4)
             RETURNING id",
            params![account, thread, name, now],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO conversation_participants (conversation_id, identity_id, joined_at) VALUES (?1, ?2, ?3)",
            params![conversation, identity, now],
        )?;
        if count == 30 {
            long_conversation_id = conversation;
        }
        for i in 0..count {
            let inbound = i % 2 == 0;
            let stamp = format!("2026-07-20T08:{:02}:00.000Z", i.min(59));
            conn.execute(
                "INSERT INTO messages
                 (conversation_id, sender_identity_id, direction, provider_message_id, content_type, text_content,
                  content_json, provider_timestamp, ingested_at, status)
                 VALUES (?1, ?2, ?3, ?4, 'text', ?5, '{}', ?6, ?6, ?7)",
                params![
                    conversation,
                    if inbound { Some(identity) } else { None },
                    if inbound { "inbound" } else { "outbound" },
                    format!("{thread}-m{i}"),
                    if inbound {
                        format!("Anh/chị cho em hỏi báo giá lô hàng số {i} ạ?")
                    } else {
                        format!("Dạ em gửi báo giá lô {i} trong hôm nay ạ.")
                    },
                    stamp,
                    if inbound { "received" } else { "sent" },
                ],
            )?;
        }
    }
    // A sticker and a photo without text: the screen must label them, not draw empty bubbles.
    let third: i64 = conn.query_row(
        "SELECT id FROM channel_conversations WHERE external_thread_id = 'demo-thread-3'",
        [],
        |row| row.get(0),
    )?;
    for (i, kind) in ["sticker", "image"].into_iter().enumerate() {
        conn.execute(
            "INSERT INTO messages
             (conversation_id, sender_identity_id, direction, provider_message_id, content_type, text_content,
              content_json, provider_timestamp, ingested_at, status)
             VALUES (?1, NULL, 'outbound', ?2, ?3, NULL, '{}', ?4, ?4, 'sent')",
            params![
                third,
                format!("demo-thread-3-{kind}"),
                kind,
                format!("2026-07-20T08:{:02}:00.000Z", 10 + i)
            ],
        )?;
    }
    // One customer line after the last reply: the inbox shows it as unread until the chat is opened.
    conn.execute(
        "INSERT INTO messages
         (conversation_id, sender_identity_id, direction, provider_message_id, content_type, text_content,
          content_json, provider_timestamp, ingested_at, status)
         VALUES (?1, (SELECT id FROM channel_identities WHERE external_id = 'demo-thread-3'), 'inbound',
                 'demo-thread-3-unread', 'text', 'Bên mình còn hàng không ạ?', '{}', ?2, ?2, 'received')",
        params![third, "2026-07-20T08:20:00.000Z"],
    )?;
    Ok(DemoSeed {
        long_conversation_id,
        long_conversation_messages: 30,
    })
}

/// Inserts a finished scrape job with `places` places and a paused one with five.
pub fn seed_demo_scrape(conn: &Connection, places: u32) -> rusqlite::Result<()> {
    let now = "2026-07-20T09:00:00.000Z";
    for (job, status, stage, count, keyword) in [
        ("demo-job-1", "completed", "cleanup", places, "quán cà phê"),
        ("demo-job-2", "paused", "scraping", 5, "nhà hàng"),
        // Two finished-with-gaps jobs, the retry buttons' fixtures: 3 has one failed cell, 4 one failed email lookup.
        ("demo-job-3", "partial", "scraping", 3, "tiệm làm tóc"),
        ("demo-job-4", "partial", "email_enrichment", 3, "phòng gym"),
    ] {
        conn.execute(
            "INSERT INTO google_maps_scrape_jobs
             (id, mode, keyword, location, headless, profile_dir, status, stage, total_units, processed_units,
              businesses_discovered, businesses_qualified, created_at, started_at, updated_at)
             VALUES (?1, 'keyword', ?2, 'Quận 1, TP.HCM', 1, '', ?3, ?4, 10, ?5, ?6, ?6, ?7, ?7, ?7)",
            params![job, keyword, status, stage, count.min(10), count, now],
        )?;
        conn.execute(
            "INSERT INTO google_maps_scrape_units (id, job_id, unit_key, unit_type, payload_json, status, created_at, updated_at)
             VALUES (?1, ?2, 'unit', 'keyword', '{}', 'completed', ?3, ?3)",
            params![format!("{job}-unit"), job, now],
        )?;
        for i in 0..count {
            conn.execute(
                "INSERT INTO google_maps_scrape_results
                 (id, job_id, source_unit_id, dedup_key, name, phone, address, detail_status, enrichment_status,
                  import_status, first_seen_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'completed', 'pending', 'pending', ?8, ?8)",
                params![
                    format!("{job}-p{i:03}"),
                    job,
                    format!("{job}-unit"),
                    format!("{job}:{i}"),
                    format!("{keyword} số {i}"),
                    if i % 3 == 0 { None } else { Some(format!("09{:08}", 10_000_000 + i * 137)) },
                    format!("{} Lê Lợi, Quận 1, TP.HCM", i + 1),
                    format!("2026-07-20T09:{:02}:00.000Z", i.min(59)),
                ],
            )?;
        }
    }
    conn.execute(
        "INSERT INTO google_maps_scrape_units
         (id, job_id, unit_key, unit_type, payload_json, status, attempt_count, last_error_kind,
          last_error_message, created_at, updated_at)
         VALUES ('demo-job-3-failed', 'demo-job-3', 'unit-failed', 'keyword', '{}', 'failed', 4,
                 'navigation_timeout', 'navigation timed out', ?1, ?1)",
        params![now],
    )?;
    conn.execute(
        "UPDATE google_maps_scrape_jobs SET failed_units = 1, total_units = 2, processed_units = 1 WHERE id = 'demo-job-3'",
        [],
    )?;
    conn.execute(
        "UPDATE google_maps_scrape_results SET enrichment_status = 'failed' WHERE id = 'demo-job-4-p000'",
        [],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;

    #[test]
    fn seeds_fit_the_schema_and_count_up() -> rusqlite::Result<()> {
        let conn = init_db(":memory:")?;
        // The conversations link to leads 1..3, which the app's debug seed creates.
        for id in 1..=3 {
            conn.execute(
                "INSERT INTO leads (id, name, company, status, last_activity, owner, email, phone)
                 VALUES (?1, ?2, '', 'New', '', '', '', '')",
                params![id, format!("Lead {id}")],
            )?;
        }
        let seed = seed_demo_conversations(&conn)?;
        let n: u32 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [seed.long_conversation_id],
            |r| r.get(0),
        )?;
        assert_eq!(n, seed.long_conversation_messages);
        seed_demo_scrape(&conn, 25)?;
        let places: u32 = conn.query_row(
            "SELECT COUNT(*) FROM google_maps_scrape_results WHERE job_id = 'demo-job-1'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(places, 25);
        Ok(())
    }
}
