//! Email templates, the multi-channel template centre and SMTP accounts.

use crate::error::CoreApiError;
use crate::runtime::{args, body, on_blocking, on_core, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::{self, DbState};
use rustsale_core::models::{CreateTemplateInput, SmtpServer, TemplateFilter, UpdateTemplateInput};
use serde::Deserialize;

/// An SMTP server as the accounts screen sends it; `createdAt` empty means now.
#[derive(Debug, Deserialize)]
struct SmtpInput {
    #[serde(default)]
    id: Option<i32>,
    name: String,
    host: String,
    port: i32,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    from_email: String,
    #[serde(default)]
    from_name: String,
    #[serde(default)]
    security: String,
    #[serde(default)]
    created_at: String,
}

impl From<SmtpInput> for SmtpServer {
    fn from(input: SmtpInput) -> Self {
        Self {
            id: input.id,
            name: input.name,
            host: input.host,
            port: input.port,
            username: input.username,
            password: input.password,
            from_email: input.from_email,
            from_name: input.from_name,
            security: input.security,
            created_at: input.created_at,
        }
    }
}

/// Email templates, multi-channel templates with their folders, and SMTP accounts. Records keep the
/// core's field names (snake_case).
#[module]
pub struct Templates;

#[module]
impl Templates {
    /// The email templates of the outreach screen.
    pub async fn email_templates() -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_email_templates(state)?)
        })
        .await
    }

    /// Creates an email template and resolves its id.
    pub async fn create_email_template(
        name: String,
        subject: String,
        content: String,
    ) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::create_email_template(
                name, subject, content, state,
            )?)
        })
        .await
    }

    /// Replaces the subject and body of email template `id`.
    pub async fn update_email_template(
        id: i32,
        name: String,
        subject: String,
        content: String,
    ) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_email_template(
                id, name, subject, content, state,
            )?)
        })
        .await
    }

    /// Deletes email template `id`.
    pub async fn delete_email_template(id: i32) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_email_template(id, state)?)
        })
        .await
    }

    /// The multi-channel templates matching `filter` (`{ channel?, folder_id?, stage?,
    /// search_query?, tag? }`); absent or `null` means all.
    pub async fn list(filter: Option<JsonValue>) -> Result<JsonValue, CoreApiError> {
        let filter: TemplateFilter = args(filter)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_templates(Some(filter), state)?)
        })
        .await
    }

    /// Template `id`, or `null` when there is none.
    pub async fn template(id: i64) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_template_by_id(id, state)?)
        })
        .await
    }

    /// Creates a multi-channel template from `{ folder_id?, name, channel, stage?, subject?,
    /// content, media_url?, ai_prompt?, tags_json? }` and resolves its id.
    pub async fn create_template(input: JsonValue) -> Result<JsonValue, CoreApiError> {
        let input: CreateTemplateInput = body(input)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::create_template(input, state)?)
        })
        .await
    }

    /// Replaces a multi-channel template from `{ id, folder_id?, name, channel, stage?, subject?,
    /// content, media_url?, ai_prompt?, tags_json? }`.
    pub async fn update_template(input: JsonValue) -> Result<JsonValue, CoreApiError> {
        let input: UpdateTemplateInput = body(input)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_template(input, state)?)
        })
        .await
    }

    /// Deletes multi-channel template `id`.
    pub async fn delete_template(id: i64) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_template(id, state)?)
        })
        .await
    }

    /// The template folders, with their colours.
    pub async fn folders() -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_template_folders(state)?)
        })
        .await
    }

    /// Creates a template folder named `name`, with `color` optional, and resolves its id.
    pub async fn create_folder(
        name: String,
        color: Option<String>,
    ) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::create_template_folder(name, color, state)?)
        })
        .await
    }

    /// Deletes template folder `id`; its templates stay, without a folder.
    pub async fn delete_folder(id: i64) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_template_folder(id, state)?)
        })
        .await
    }

    /// Counts one more use of template `id`.
    pub async fn increment_usage(id: i64) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::increment_template_usage(id, state)?)
        })
        .await
    }

    /// The SMTP accounts, without their passwords.
    pub async fn smtp_accounts() -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_managed_smtp_accounts(state)?)
        })
        .await
    }

    /// Adds or updates an SMTP server from `{ id?, name, host, port, username?, password?,
    /// from_email, from_name?, security?, created_at? }` and resolves its id. A blank password
    /// keeps the stored one.
    pub async fn save_smtp_server(server: JsonValue) -> Result<JsonValue, CoreApiError> {
        let server: SmtpInput = body(server)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::save_smtp_server(server.into(), state)?)
        })
        .await
    }

    /// Deletes SMTP server `id`.
    pub async fn delete_smtp_server(id: i32) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_smtp_server(id, state)?)
        })
        .await
    }

    /// Opens a TCP connection to `host:port` and resolves the core's message. It touches the
    /// network, so screens call it only on an explicit test.
    pub async fn test_smtp_connection(host: String, port: i32) -> Result<JsonValue, CoreApiError> {
        on_core(move |_h| async move {
            Ok::<_, CoreApiError>(commands::test_smtp_connection(host, port).await?)
        })
        .await
    }
}
