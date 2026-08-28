use crate::conversation::error::{ConversationError, ConversationErrorCode, ConversationResult};
use crate::conversation::projection::load_message_reaction_groups;
use crate::conversation::types::{
    utc_now, ConversationReactionGroup, SetConversationReactionRequest,
};
use crate::db::open_db_connection;
use rusqlite::{params, OptionalExtension, Transaction};
use std::path::PathBuf;

const MAX_REACTION_BATCH_COUNT: u32 = 1_000;

pub(crate) fn canonical_reaction_code(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "❤️" | "❤" | "/-heart" | "/:heart" | "heart" => Some("heart"),
        "👍" | "/-strong" | "/:strong" | "/-like" | "/:like" | "like" => Some("like"),
        "😆" | "😄" | ":>" | "/-haha" | "/:haha" | "haha" => Some("haha"),
        "😮" | ":o" | "/-wow" | "/:wow" | "wow" => Some("wow"),
        ":-(" | ":-[" | "/-sad" | "/:sad" => Some("cry"),
        "😭" | "😢" | ":-((" | ":((" | "--b" | "/-cry" | "/:cry" | "cry" | "sad" => Some("cry"),
        "😡" | ":-h" | "/-angry" | "/:angry" | "angry" => Some("angry"),
        _ => None,
    }
}

pub(crate) fn mark_reaction_removed(
    tx: &Transaction<'_>,
    message_id: i64,
    actor_external_id: &str,
    reaction_code: Option<&str>,
    _occurred_at: &str,
) -> rusqlite::Result<usize> {
    let target = reaction_code
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("unknown"))
        .map(|value| canonical_reaction_code(value).unwrap_or(value).to_string());

    let rows = {
        let mut stmt = tx.prepare(
            "SELECT rowid, reaction FROM message_reactions
             WHERE message_id = ?1 AND actor_external_id = ?2 AND removed_at IS NULL",
        )?;
        let mapped_rows = stmt.query_map(params![message_id, actor_external_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        mapped_rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut updated = 0;
    for (row_id, stored_reaction) in rows {
        let stored_code = canonical_reaction_code(&stored_reaction).unwrap_or(&stored_reaction);
        if target
            .as_deref()
            .is_none_or(|target_code| stored_code.eq_ignore_ascii_case(target_code))
        {
            updated += tx.execute(
                "DELETE FROM message_reactions WHERE rowid = ?1",
                params![row_id],
            )?;
        }
    }

    Ok(updated)
}

fn apply_reaction_increment(
    tx: &Transaction<'_>,
    message_id: i64,
    actor_external_id: &str,
    reaction_code: &str,
    increment_by: u32,
    occurred_at: &str,
) -> rusqlite::Result<usize> {
    let stored_reaction_code = canonical_reaction_code(reaction_code).unwrap_or(reaction_code);
    tx.execute(
        "INSERT INTO message_reactions (message_id, actor_external_id, reaction, count, reacted_at, removed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL)
         ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET
             count = message_reactions.count + excluded.count,
             reacted_at = COALESCE(message_reactions.reacted_at, excluded.reacted_at),
             removed_at = NULL",
        params![
            message_id,
            actor_external_id,
            stored_reaction_code,
            increment_by,
            occurred_at
        ],
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeginReactionMutation {
    pub client_mutation_id: String,
    pub message_id: i64,
    pub reaction_code: String,
    pub active: bool,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub is_existing: bool,
}

#[derive(Clone)]
pub struct ReactionMutationRepository {
    db_path: PathBuf,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PartialReactionFailure<'a> {
    pub occurred_at: &'a str,
    pub successful_count: u32,
    pub code: &'a str,
    pub message: &'a str,
    pub unknown: bool,
}

impl ReactionMutationRepository {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    fn db_path_str(&self) -> &str {
        self.db_path.to_str().unwrap_or_default()
    }

    pub fn begin(
        &self,
        request: &SetConversationReactionRequest,
        _account_id: i64,
    ) -> ConversationResult<BeginReactionMutation> {
        let client_mutation_id = request.client_mutation_id.trim().to_string();
        if client_mutation_id.is_empty() {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                "client_mutation_id must not be empty",
            ));
        }

        let reaction_code = request.reaction.as_str().to_string();
        let active = request.active;

        let conn = open_db_connection(self.db_path_str())?;
        let now = utc_now();
        let react_count = if active {
            request.count.unwrap_or(1).max(1)
        } else {
            1
        };
        if react_count > MAX_REACTION_BATCH_COUNT {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!("Reaction batch count must not exceed {MAX_REACTION_BATCH_COUNT}"),
            ));
        }
        let inserted = conn.execute(
            "INSERT INTO reaction_mutations (client_mutation_id, message_id, reaction_code, active, count, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6, ?6)
             ON CONFLICT(client_mutation_id) DO NOTHING",
            params![
                &client_mutation_id,
                request.message_id,
                &reaction_code,
                if active { 1 } else { 0 },
                react_count,
                now,
            ],
        )?;

        let existing: (
            i64,
            String,
            bool,
            u32,
            String,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT message_id, reaction_code, active, COALESCE(count, 1), status, error_code, error_message
                 FROM reaction_mutations WHERE client_mutation_id = ?1",
                params![&client_mutation_id],
                |row| {
                    let active_int: i32 = row.get(2)?;
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        active_int != 0,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    format!(
                        "Mutation client_id '{}' was not persisted",
                        client_mutation_id
                    ),
                )
            })?;

        let (
            message_id,
            existing_code,
            existing_active,
            existing_count,
            status,
            error_code,
            error_message,
        ) = existing;
        if message_id != request.message_id
            || existing_code != reaction_code
            || existing_active != active
            || existing_count != react_count
        {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!(
                    "Conflicting request for client_mutation_id '{}'",
                    client_mutation_id
                ),
            ));
        }

        Ok(BeginReactionMutation {
            client_mutation_id,
            message_id,
            reaction_code: existing_code,
            active: existing_active,
            status,
            error_code,
            error_message,
            is_existing: inserted == 0,
        })
    }

    pub fn succeed(
        &self,
        client_id: &str,
        self_external_id: &str,
        occurred_at: &str,
    ) -> ConversationResult<Vec<ConversationReactionGroup>> {
        let mut conn = open_db_connection(self.db_path_str())?;
        let tx = conn.transaction()?;

        let row: Option<(i64, String, bool, u32, String)> = tx
            .query_row(
                "SELECT message_id, reaction_code, active, COALESCE(count, 1), status
                 FROM reaction_mutations WHERE client_mutation_id = ?1",
                params![client_id],
                |r| {
                    let active_int: i32 = r.get(2)?;
                    let count_val: u32 = r.get(3)?;
                    Ok((r.get(0)?, r.get(1)?, active_int != 0, count_val, r.get(4)?))
                },
            )
            .optional()?;

        let (message_id, reaction_code, active, react_count, status) = match row {
            Some(r) => r,
            None => {
                return Err(ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    format!("Mutation client_id '{}' not found", client_id),
                ));
            }
        };

        if status != "succeeded" {
            let now = utc_now();
            tx.execute(
                "UPDATE reaction_mutations
                 SET status = 'succeeded', error_code = NULL, error_message = NULL, updated_at = ?1
                 WHERE client_mutation_id = ?2",
                params![now, client_id],
            )?;

            if active {
                apply_reaction_increment(
                    &tx,
                    message_id,
                    self_external_id,
                    &reaction_code,
                    react_count,
                    occurred_at,
                )?;
            } else {
                mark_reaction_removed(
                    &tx,
                    message_id,
                    self_external_id,
                    (!reaction_code.is_empty()).then_some(reaction_code.as_str()),
                    occurred_at,
                )?;
            }
        }

        tx.commit()?;

        let read_conn = open_db_connection(self.db_path_str())?;
        let groups = load_message_reaction_groups(&read_conn, message_id)?;
        Ok(groups)
    }

    pub fn succeed_single_active(
        &self,
        client_id: &str,
        self_external_id: &str,
        occurred_at: &str,
    ) -> ConversationResult<Vec<ConversationReactionGroup>> {
        let mut conn = open_db_connection(self.db_path_str())?;
        let tx = conn.transaction()?;

        let row: Option<(i64, String, bool, u32, String)> = tx
            .query_row(
                "SELECT message_id, reaction_code, active, COALESCE(count, 1), status
                 FROM reaction_mutations WHERE client_mutation_id = ?1",
                params![client_id],
                |r| {
                    let active_int: i32 = r.get(2)?;
                    let count_val: u32 = r.get(3)?;
                    Ok((r.get(0)?, r.get(1)?, active_int != 0, count_val, r.get(4)?))
                },
            )
            .optional()?;

        let (message_id, reaction_code, active, _react_count, status) = match row {
            Some(r) => r,
            None => {
                return Err(ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    format!("Mutation client_id '{}' not found", client_id),
                ));
            }
        };

        if status != "succeeded" {
            let now = utc_now();
            tx.execute(
                "UPDATE reaction_mutations
                 SET status = 'succeeded', error_code = NULL, error_message = NULL, updated_at = ?1
                 WHERE client_mutation_id = ?2",
                params![now, client_id],
            )?;

            // In single-active mode (e.g. WhatsApp, Messenger), delete any prior reaction of this actor on this message
            mark_reaction_removed(
                &tx,
                message_id,
                self_external_id,
                None,
                occurred_at,
            )?;

            if active {
                apply_reaction_increment(
                    &tx,
                    message_id,
                    self_external_id,
                    &reaction_code,
                    1,
                    occurred_at,
                )?;
            }
        }

        tx.commit()?;

        let read_conn = open_db_connection(self.db_path_str())?;
        let groups = load_message_reaction_groups(&read_conn, message_id)?;
        Ok(groups)
    }

    pub(crate) fn fail_after_partial_success(
        &self,
        client_id: &str,
        self_external_id: &str,
        failure: PartialReactionFailure<'_>,
    ) -> ConversationResult<Vec<ConversationReactionGroup>> {
        let PartialReactionFailure {
            occurred_at,
            successful_count,
            code,
            message,
            unknown,
        } = failure;
        let mut conn = open_db_connection(self.db_path_str())?;
        let tx = conn.transaction()?;
        let row: Option<(i64, String, bool, u32, String)> = tx
            .query_row(
                "SELECT message_id, reaction_code, active, COALESCE(count, 1), status
                 FROM reaction_mutations WHERE client_mutation_id = ?1",
                params![client_id],
                |row| {
                    let active: i32 = row.get(2)?;
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        active != 0,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?;
        let (message_id, reaction_code, active, requested_count, status) =
            row.ok_or_else(|| {
                ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    format!("Mutation client_id '{}' not found", client_id),
                )
            })?;

        if !active || successful_count == 0 || successful_count > requested_count {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!(
                    "Invalid partial reaction count for client_mutation_id '{}'",
                    client_id
                ),
            ));
        }

        if status == "running" {
            apply_reaction_increment(
                &tx,
                message_id,
                self_external_id,
                &reaction_code,
                successful_count,
                occurred_at,
            )?;
        }

        let new_status = if unknown {
            "delivery_unknown"
        } else {
            "failed"
        };
        tx.execute(
            "UPDATE reaction_mutations
             SET status = ?1, error_code = ?2, error_message = ?3, updated_at = ?4
             WHERE client_mutation_id = ?5",
            params![new_status, code, message, utc_now(), client_id],
        )?;
        tx.commit()?;

        let read_conn = open_db_connection(self.db_path_str())?;
        Ok(load_message_reaction_groups(&read_conn, message_id)?)
    }

    pub fn fail(
        &self,
        client_id: &str,
        code: &str,
        message: &str,
        unknown: bool,
    ) -> ConversationResult<()> {
        let conn = open_db_connection(self.db_path_str())?;
        let new_status = if unknown {
            "delivery_unknown"
        } else {
            "failed"
        };
        let now = utc_now();

        conn.execute(
            "UPDATE reaction_mutations SET status = ?1, error_code = ?2, error_message = ?3, updated_at = ?4 WHERE client_mutation_id = ?5",
            params![new_status, code, message, now, client_id],
        )?;
        Ok(())
    }

    pub fn mark_running_delivery_unknown(&self, now: &str) -> ConversationResult<usize> {
        let conn = open_db_connection(self.db_path_str())?;
        let count = conn.execute(
            "UPDATE reaction_mutations SET status = 'delivery_unknown', updated_at = ?1 WHERE status = 'running'",
            params![now],
        )?;
        Ok(count)
    }

    pub fn reconcile_reaction_mutation(
        &self,
        message_id: i64,
        reaction_code: &str,
        active: bool,
        now: &str,
    ) -> ConversationResult<Option<String>> {
        let conn = open_db_connection(self.db_path_str())?;
        let active_int = if active { 1 } else { 0 };

        let client_id: Option<String> = conn
            .query_row(
                "SELECT client_mutation_id FROM reaction_mutations
                 WHERE message_id = ?1 AND reaction_code = ?2 AND active = ?3 AND status IN ('running', 'delivery_unknown')
                 ORDER BY created_at DESC LIMIT 1",
                params![message_id, reaction_code, active_int],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(ref cid) = client_id {
            conn.execute(
                "UPDATE reaction_mutations SET status = 'succeeded', updated_at = ?1 WHERE client_mutation_id = ?2",
                params![now, cid],
            )?;
        }

        Ok(client_id)
    }
}
