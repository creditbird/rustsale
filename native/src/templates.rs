//! Email templates, the multi-channel template centre and SMTP accounts.

use crate::error::CoreApiError;
use crate::runtime::{
    args, body, checked_id64, checked_optional_id32, checked_optional_id64, id32, id64,
    on_blocking, on_core, state_of,
};
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

impl SmtpInput {
    /// Checks the id and the port serde accepted, before the core stores a row or opens a socket.
    fn checked(mut self) -> Result<Self, CoreApiError> {
        self.id = checked_optional_id32(self.id)?;
        self.port = smtp_port(f64::from(self.port))?;
        Ok(self)
    }
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

/// A JS number as a TCP port: whole and in 1..=65535, the ports a TCP socket accepts.
fn smtp_port(value: f64) -> Result<i32, CoreApiError> {
    if value.is_finite() && value.fract() == 0.0 && (1.0..=65_535.0).contains(&value) {
        Ok(value as i32)
    } else {
        Err(CoreApiError::invalid("port is out of range"))
    }
}

/// Email templates, multi-channel templates with their folders, and SMTP accounts. Records keep the
/// core's field names (snake_case).
#[module]
pub struct Templates;

#[module]
impl Templates {
    /// The email templates of the outreach screen.
    pub async fn email_templates() -> Result<JsonValue, JsError> {
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
    ) -> Result<JsonValue, JsError> {
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
        id: JsonValue,
        name: String,
        subject: String,
        content: String,
    ) -> Result<JsonValue, JsError> {
        let id = id32(id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_email_template(
                id, name, subject, content, state,
            )?)
        })
        .await
    }

    /// Deletes email template `id`.
    pub async fn delete_email_template(id: JsonValue) -> Result<JsonValue, JsError> {
        let id = id32(id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_email_template(id, state)?)
        })
        .await
    }

    /// The multi-channel templates matching `filter` (`{ channel?, folder_id?, stage?,
    /// search_query?, tag? }`); absent or `null` means all.
    pub async fn list(filter: Option<JsonValue>) -> Result<JsonValue, JsError> {
        let mut filter: TemplateFilter = args(filter)?;
        filter.folder_id = checked_optional_id64(filter.folder_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_templates(Some(filter), state)?)
        })
        .await
    }

    /// Template `id`, or `null` when there is none.
    pub async fn template(id: JsonValue) -> Result<JsonValue, JsError> {
        let id = id64(id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_template_by_id(id, state)?)
        })
        .await
    }

    /// Creates a multi-channel template from `{ folder_id?, name, channel, stage?, subject?,
    /// content, media_url?, ai_prompt?, tags_json? }` and resolves its id.
    pub async fn create_template(input: JsonValue) -> Result<JsonValue, JsError> {
        let mut input: CreateTemplateInput = body(input)?;
        input.folder_id = checked_optional_id64(input.folder_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::create_template(input, state)?)
        })
        .await
    }

    /// Replaces a multi-channel template from `{ id, folder_id?, name, channel, stage?, subject?,
    /// content, media_url?, ai_prompt?, tags_json? }`.
    pub async fn update_template(input: JsonValue) -> Result<JsonValue, JsError> {
        let mut input: UpdateTemplateInput = body(input)?;
        input.id = checked_id64(input.id)?;
        input.folder_id = checked_optional_id64(input.folder_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_template(input, state)?)
        })
        .await
    }

    /// Deletes multi-channel template `id`.
    pub async fn delete_template(id: JsonValue) -> Result<JsonValue, JsError> {
        let id = id64(id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_template(id, state)?)
        })
        .await
    }

    /// The template folders, with their colours.
    pub async fn folders() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_template_folders(state)?)
        })
        .await
    }

    /// Creates a template folder named `name`, with `color` optional, and resolves its id.
    pub async fn create_folder(name: String, color: Option<String>) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::create_template_folder(name, color, state)?)
        })
        .await
    }

    /// Deletes template folder `id`; its templates stay, without a folder.
    pub async fn delete_folder(id: JsonValue) -> Result<JsonValue, JsError> {
        let id = id64(id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_template_folder(id, state)?)
        })
        .await
    }

    /// Counts one more use of template `id`.
    pub async fn increment_usage(id: JsonValue) -> Result<JsonValue, JsError> {
        let id = id64(id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::increment_template_usage(id, state)?)
        })
        .await
    }

    /// The SMTP accounts, without their passwords.
    pub async fn smtp_accounts() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_managed_smtp_accounts(state)?)
        })
        .await
    }

    /// Adds or updates an SMTP server from `{ id?, name, host, port, username?, password?,
    /// from_email, from_name?, security?, created_at? }` and resolves its id. A blank password
    /// keeps the stored one.
    pub async fn save_smtp_server(server: JsonValue) -> Result<JsonValue, JsError> {
        let server = SmtpInput::checked(body(server)?)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::save_smtp_server(server.into(), state)?)
        })
        .await
    }

    /// Deletes SMTP server `id`.
    pub async fn delete_smtp_server(id: JsonValue) -> Result<JsonValue, JsError> {
        let id = id32(id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_smtp_server(id, state)?)
        })
        .await
    }

    /// Opens a TCP connection to `host:port` and resolves the core's message. It touches the
    /// network, so screens call it only on an explicit test.
    pub async fn test_smtp_connection(host: String, port: f64) -> Result<JsonValue, JsError> {
        let port = smtp_port(port)?;
        on_core(move |_h| async move {
            Ok::<_, CoreApiError>(commands::test_smtp_connection(host, port).await?)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::Templates;
    use serde_json::json;

    #[tokio::test]
    async fn oversized_template_ids_are_invalid_requests() {
        for result in [
            Templates::delete_smtp_server(json!(4_294_967_297.0_f64)).await,
            Templates::delete_email_template(json!(4_294_967_297.0_f64)).await,
            Templates::delete_email_template(json!(-1.0_f64)).await,
        ] {
            let error = match result {
                Ok(_) => panic!("an id past i32 must be refused"),
                Err(error) => error,
            };
            assert_eq!(error.code(), Some("invalid_request"));
            assert_eq!(error.message(), "id is out of range");
        }
    }

    #[tokio::test]
    async fn template_ids_of_the_wrong_type_are_invalid_requests() {
        for id in [
            json!(1.5),
            json!(null),
            json!("7"),
            json!(true),
            json!({"id": 1}),
        ] {
            for result in [
                Templates::delete_smtp_server(id.clone()).await,
                Templates::delete_email_template(id.clone()).await,
            ] {
                let error = match result {
                    Ok(_) => panic!("a wrong-typed id must be refused: {id}"),
                    Err(error) => error,
                };
                assert_eq!(error.code(), Some("invalid_request"), "{id}");
                assert_eq!(
                    error.message(),
                    "id has the wrong type: it must be a whole number",
                    "{id}"
                );
            }
        }
    }

    fn smtp_input() -> super::SmtpInput {
        super::SmtpInput {
            id: None,
            name: "QA".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 2525,
            username: String::new(),
            password: String::new(),
            from_email: "qa@example.test".to_owned(),
            from_name: String::new(),
            security: String::new(),
            created_at: String::new(),
        }
    }

    #[test]
    fn smtp_ports_outside_one_to_65535_are_invalid_requests() {
        for good in [1.0, 25.0, 2525.0, 65_535.0] {
            assert_eq!(super::smtp_port(good).ok(), Some(good as i32));
        }
        for bad in [0.0, -1.0, 65_536.0, 70_000.0, 1.5, f64::NAN] {
            let error = match super::smtp_port(bad) {
                Ok(_) => panic!("a bad port must be refused"),
                Err(error) => error,
            };
            assert_eq!(error.code, "invalid_request", "{bad}");
            assert_eq!(error.message, "port is out of range", "{bad}");
        }
    }

    #[test]
    fn smtp_input_checks_its_id_and_port_after_serde() {
        let mut input = smtp_input();
        input.id = Some(-1);
        let error = match input.checked() {
            Ok(_) => panic!("a negative id must be refused"),
            Err(error) => error,
        };
        assert_eq!(error.code, "invalid_request");

        let mut input = smtp_input();
        input.port = 70_000;
        let error = match input.checked() {
            Ok(_) => panic!("a port past 65535 must be refused"),
            Err(error) => error,
        };
        assert_eq!(error.message, "port is out of range");

        let mut input = smtp_input();
        input.id = Some(7);
        input.port = 65_535;
        let checked = match input.checked() {
            Ok(checked) => checked,
            Err(error) => panic!("a valid input must pass: {error}"),
        };
        assert_eq!(checked.id, Some(7));
        assert_eq!(checked.port, 65_535);
    }
}
