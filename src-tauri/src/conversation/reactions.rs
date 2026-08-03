use crate::conversation::error::{ConversationError, ConversationErrorCode, ConversationResult};
use crate::conversation::projection::load_message_reaction_groups;
use crate::conversation::types::{
    utc_now, ConversationReactionGroup, SetConversationReactionRequest,
};
use crate::db::open_db_connection;
use rusqlite::{params, OptionalExtension};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeginReactionMutation {
    pub client_mutation_id: String,
    pub message_id: i64,
    pub reaction_code: String,
    pub active: bool,
    pub status: String,
    pub is_existing: bool,
}

#[derive(Clone)]
pub struct ReactionMutationRepository {
    db_path: PathBuf,
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
        let client_mutation_id = match &request.client_mutation_id {
            Some(id) if !id.trim().is_empty() => id.trim().to_string(),
            _ => format!("mut-{}-{}", rand::random::<u64>(), rand::random::<u64>()),
        };

        let reaction_code = request
            .reaction_code
            .map(|c| c.as_str().to_string())
            .unwrap_or_default();
        let active = request.reaction_code.is_some();

        let conn = open_db_connection(self.db_path_str())?;

        let existing: Option<(i64, String, bool, String)> = conn
            .query_row(
                "SELECT message_id, reaction_code, active, status FROM reaction_mutations WHERE client_mutation_id = ?1",
                params![client_mutation_id],
                |row| {
                    let active_int: i32 = row.get(2)?;
                    Ok((row.get(0)?, row.get(1)?, active_int != 0, row.get(3)?))
                },
            )
            .optional()?;

        if let Some((ex_msg_id, ex_code, ex_active, ex_status)) = existing {
            if ex_msg_id != request.message_id || ex_code != reaction_code || ex_active != active {
                return Err(ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    format!(
                        "Conflicting request for client_mutation_id '{}'",
                        client_mutation_id
                    ),
                ));
            }
            return Ok(BeginReactionMutation {
                client_mutation_id,
                message_id: request.message_id,
                reaction_code,
                active,
                status: ex_status,
                is_existing: true,
            });
        }

        let now = utc_now();
        conn.execute(
            "INSERT INTO reaction_mutations (client_mutation_id, message_id, reaction_code, active, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?5)",
            params![
                client_mutation_id,
                request.message_id,
                reaction_code,
                if active { 1 } else { 0 },
                now,
            ],
        )?;

        Ok(BeginReactionMutation {
            client_mutation_id,
            message_id: request.message_id,
            reaction_code,
            active,
            status: "running".to_string(),
            is_existing: false,
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

        let row: Option<(i64, String, bool)> = tx
            .query_row(
                "SELECT message_id, reaction_code, active FROM reaction_mutations WHERE client_mutation_id = ?1",
                params![client_id],
                |r| {
                    let active_int: i32 = r.get(2)?;
                    Ok((r.get(0)?, r.get(1)?, active_int != 0))
                },
            )
            .optional()?;

        let (message_id, reaction_code, active) = match row {
            Some(r) => r,
            None => {
                return Err(ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    format!("Mutation client_id '{}' not found", client_id),
                ));
            }
        };

        let now = utc_now();
        tx.execute(
            "UPDATE reaction_mutations SET status = 'succeeded', updated_at = ?1 WHERE client_mutation_id = ?2",
            params![now, client_id],
        )?;

        if active {
            tx.execute(
                "INSERT INTO message_reactions (message_id, actor_external_id, reaction, reacted_at, removed_at)
                 VALUES (?1, ?2, ?3, ?4, NULL)
                 ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET
                     reacted_at = COALESCE(message_reactions.reacted_at, excluded.reacted_at),
                     removed_at = NULL",
                params![message_id, self_external_id, reaction_code, occurred_at],
            )?;
        } else {
            if reaction_code.is_empty() {
                tx.execute(
                    "UPDATE message_reactions SET removed_at = ?1 WHERE message_id = ?2 AND actor_external_id = ?3 AND removed_at IS NULL",
                    params![occurred_at, message_id, self_external_id],
                )?;
            } else {
                tx.execute(
                    "UPDATE message_reactions SET removed_at = ?1 WHERE message_id = ?2 AND actor_external_id = ?3 AND reaction = ?4 AND removed_at IS NULL",
                    params![occurred_at, message_id, self_external_id, reaction_code],
                )?;
            }
        }

        tx.commit()?;

        let read_conn = open_db_connection(self.db_path_str())?;
        let groups = load_message_reaction_groups(&read_conn, message_id)?;
        Ok(groups)
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
