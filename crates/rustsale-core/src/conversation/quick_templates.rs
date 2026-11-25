use crate::conversation::error::{ConversationError, ConversationErrorCode, ConversationResult};
use crate::conversation::types::{
    utc_now, ConversationQuickTemplate, CreateConversationQuickTemplateRequest,
    UpdateConversationQuickTemplateRequest,
};
use crate::db::open_db_connection;
use rusqlite::{params, OptionalExtension};
use std::cmp::Ordering;

pub fn normalize_shortcut(shortcut: &str) -> String {
    shortcut
        .trim()
        .trim_start_matches('/')
        .trim()
        .to_lowercase()
}

#[derive(Debug, Clone)]
pub struct QuickTemplateRepository {
    db_path: String,
}

impl QuickTemplateRepository {
    pub fn new(db_path: impl Into<String>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    fn validate_fields(
        shortcut: &str,
        title: &str,
        content: &str,
    ) -> ConversationResult<(String, String, String)> {
        let norm_sc = normalize_shortcut(shortcut);
        let sc_len = norm_sc.chars().count();
        if sc_len == 0 || sc_len > 32 {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                "Shortcut length must be between 1 and 32 characters",
            ));
        }

        let title_trimmed = title.trim().to_string();
        let title_len = title_trimmed.chars().count();
        if title_len == 0 || title_len > 100 {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                "Title length must be between 1 and 100 characters",
            ));
        }

        let body_trimmed = content.trim().to_string();
        let body_len = body_trimmed.chars().count();
        if body_len == 0 || body_len > 4000 {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                "Body length must be between 1 and 4000 characters",
            ));
        }

        Ok((norm_sc, title_trimmed, body_trimmed))
    }

    fn normalize_category(category: Option<String>) -> Option<String> {
        category
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
    }

    pub fn create(
        &self,
        req: CreateConversationQuickTemplateRequest,
    ) -> ConversationResult<ConversationQuickTemplate> {
        let (norm_sc, title, body_text) =
            Self::validate_fields(&req.shortcut, &req.title, &req.content)?;
        let category = Self::normalize_category(req.category);

        let conn = open_db_connection(&self.db_path)?;

        let active_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM conversation_quick_templates WHERE is_active = 1",
            [],
            |row| row.get(0),
        )?;
        if active_count >= 500 {
            return Err(ConversationError::new(
                ConversationErrorCode::TemplateLimitExceeded,
                "Template limit of 500 active templates reached",
            ));
        }

        let existing: Option<(i64, i32)> = conn
            .query_row(
                "SELECT id, is_active FROM conversation_quick_templates WHERE shortcut = ?1",
                params![norm_sc],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((existing_id, is_active)) = existing {
            if is_active == 1 {
                return Err(ConversationError::new(
                    ConversationErrorCode::TemplateShortcutConflict,
                    "Template with this shortcut already exists",
                ));
            } else {
                conn.execute(
                    "DELETE FROM conversation_quick_templates WHERE id = ?1",
                    params![existing_id],
                )?;
            }
        }

        let now = utc_now();
        let res = conn.execute(
            "INSERT INTO conversation_quick_templates (shortcut, title, body_text, category, is_active, usage_count, last_used_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, 0, NULL, ?5, ?5)",
            params![norm_sc, title, body_text, category, now],
        );

        match res {
            Ok(_) => {
                let id = conn.last_insert_rowid();
                Ok(ConversationQuickTemplate {
                    id,
                    shortcut: norm_sc,
                    title,
                    content: body_text,
                    category,
                    is_active: true,
                    usage_count: 0,
                    last_used_at: None,
                    created_at: now.clone(),
                    updated_at: now,
                })
            }
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(ConversationError::new(
                    ConversationErrorCode::TemplateShortcutConflict,
                    "Template with this shortcut already exists",
                ))
            }
            Err(err) => Err(err.into()),
        }
    }

    pub fn update(
        &self,
        req: UpdateConversationQuickTemplateRequest,
    ) -> ConversationResult<ConversationQuickTemplate> {
        let (norm_sc, title, body_text) =
            Self::validate_fields(&req.shortcut, &req.title, &req.content)?;
        let category = Self::normalize_category(req.category);

        let conn = open_db_connection(&self.db_path)?;

        let current: Option<(i32, u32, Option<String>, String)> = conn
            .query_row(
                "SELECT is_active, usage_count, last_used_at, created_at FROM conversation_quick_templates WHERE id = ?1",
                params![req.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        let (current_is_active, usage_count, last_used_at, created_at) = match current {
            Some((active, usage, last_used, created)) => (active == 1, usage, last_used, created),
            None => {
                return Err(ConversationError::new(
                    ConversationErrorCode::NotFound,
                    "Template not found",
                ));
            }
        };

        if req.is_active && !current_is_active {
            let active_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM conversation_quick_templates WHERE is_active = 1",
                [],
                |row| row.get(0),
            )?;
            if active_count >= 500 {
                return Err(ConversationError::new(
                    ConversationErrorCode::TemplateLimitExceeded,
                    "Template limit of 500 active templates reached",
                ));
            }
        }

        let existing: Option<(i64, i32)> = conn
            .query_row(
                "SELECT id, is_active FROM conversation_quick_templates WHERE shortcut = ?1 AND id != ?2",
                params![norm_sc, req.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((other_id, other_active)) = existing {
            if other_active == 1 {
                return Err(ConversationError::new(
                    ConversationErrorCode::TemplateShortcutConflict,
                    "Shortcut already in use by another template",
                ));
            } else {
                conn.execute(
                    "DELETE FROM conversation_quick_templates WHERE id = ?1",
                    params![other_id],
                )?;
            }
        }

        let now = utc_now();
        conn.execute(
            "UPDATE conversation_quick_templates
             SET shortcut = ?1, title = ?2, body_text = ?3, category = ?4, is_active = ?5, updated_at = ?6
             WHERE id = ?7",
            params![
                norm_sc,
                title,
                body_text,
                category,
                req.is_active,
                now,
                req.id
            ],
        )?;

        Ok(ConversationQuickTemplate {
            id: req.id,
            shortcut: norm_sc,
            title,
            content: body_text,
            category,
            is_active: req.is_active,
            usage_count,
            last_used_at,
            created_at,
            updated_at: now,
        })
    }

    pub fn delete(&self, id: i64) -> ConversationResult<()> {
        let conn = open_db_connection(&self.db_path)?;
        let deleted = conn.execute(
            "DELETE FROM conversation_quick_templates WHERE id = ?1",
            params![id],
        )?;
        if deleted == 0 {
            Err(ConversationError::new(
                ConversationErrorCode::NotFound,
                "Template not found",
            ))
        } else {
            Ok(())
        }
    }

    pub fn mark_used(&self, id: i64) -> ConversationResult<ConversationQuickTemplate> {
        let conn = open_db_connection(&self.db_path)?;
        let exists: Option<i32> = conn
            .query_row(
                "SELECT is_active FROM conversation_quick_templates WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;

        if exists != Some(1) {
            return Err(ConversationError::new(
                ConversationErrorCode::NotFound,
                "Template not found",
            ));
        }

        let now = utc_now();
        conn.execute(
            "UPDATE conversation_quick_templates
             SET usage_count = usage_count + 1, last_used_at = ?1, updated_at = ?1
             WHERE id = ?2",
            params![now, id],
        )?;

        let tmpl = conn.query_row(
            "SELECT id, shortcut, title, body_text, category, is_active, usage_count, last_used_at, created_at, updated_at
             FROM conversation_quick_templates WHERE id = ?1",
            params![id],
            |row| {
                Ok(ConversationQuickTemplate {
                    id: row.get(0)?,
                    shortcut: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    category: row.get(4)?,
                    is_active: row.get(5)?,
                    usage_count: row.get(6)?,
                    last_used_at: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )?;

        Ok(tmpl)
    }

    pub fn list(
        &self,
        include_inactive: bool,
    ) -> ConversationResult<Vec<ConversationQuickTemplate>> {
        let conn = open_db_connection(&self.db_path)?;
        let sql = if include_inactive {
            "SELECT id, shortcut, title, body_text, category, is_active, usage_count, last_used_at, created_at, updated_at
             FROM conversation_quick_templates ORDER BY shortcut ASC, id ASC"
        } else {
            "SELECT id, shortcut, title, body_text, category, is_active, usage_count, last_used_at, created_at, updated_at
             FROM conversation_quick_templates WHERE is_active = 1 ORDER BY shortcut ASC, id ASC"
        };

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(ConversationQuickTemplate {
                id: row.get(0)?,
                shortcut: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                category: row.get(4)?,
                is_active: row.get(5)?,
                usage_count: row.get(6)?,
                last_used_at: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;

        let mut templates = Vec::new();
        for row in rows {
            templates.push(row?);
        }
        Ok(templates)
    }

    pub fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> ConversationResult<Vec<ConversationQuickTemplate>> {
        let limit = limit.clamp(1, 50);
        let active_templates = self.list(false)?;

        let norm_query = normalize_shortcut(query);

        let mut matched: Vec<(u8, ConversationQuickTemplate)> = Vec::new();

        for tmpl in active_templates {
            if norm_query.is_empty() {
                matched.push((0, tmpl));
                continue;
            }

            let sc = normalize_shortcut(&tmpl.shortcut);
            let title = tmpl.title.to_lowercase();
            let category = tmpl.category.as_deref().unwrap_or("").to_lowercase();
            let body = tmpl.content.to_lowercase();

            let bucket = if sc == norm_query {
                Some(0)
            } else if sc.starts_with(&norm_query) {
                Some(1)
            } else if title.starts_with(&norm_query) {
                Some(2)
            } else if !category.is_empty()
                && (category == norm_query
                    || category.starts_with(&norm_query)
                    || category.contains(&norm_query))
            {
                Some(3)
            } else if body.contains(&norm_query) {
                Some(4)
            } else {
                None
            };

            if let Some(b) = bucket {
                matched.push((b, tmpl));
            }
        }

        matched.sort_by(|(b1, t1), (b2, t2)| {
            b1.cmp(b2)
                .then_with(|| t2.usage_count.cmp(&t1.usage_count))
                .then_with(|| match (&t1.last_used_at, &t2.last_used_at) {
                    (Some(a), Some(b)) => b.cmp(a),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                })
                .then_with(|| t1.id.cmp(&t2.id))
        });

        Ok(matched
            .into_iter()
            .map(|(_, tmpl)| tmpl)
            .take(limit)
            .collect())
    }
}
