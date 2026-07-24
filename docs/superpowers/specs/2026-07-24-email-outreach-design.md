# Email Outreach Feature Design Specification

## 1. Overview
The Email Outreach feature adds email marketing and messaging capabilities to RustSale_v2, mirroring the workflow and user interface of the Zalo Outreach feature. Instead of Zalo QR code / session cookies, Email Outreach uses configurable SMTP Server accounts (e.g. Gmail, SendGrid, custom SMTP servers) to send personalized batch emails to CRM leads.

## 2. Database Schema & Migration Specification

Modify `src-tauri/src/db.rs` to create the following tables upon application startup:

### 2.1 Bảng `smtp_servers`
```sql
CREATE TABLE IF NOT EXISTS smtp_servers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 587,
    username TEXT NOT NULL,
    password TEXT NOT NULL,
    from_email TEXT NOT NULL,
    from_name TEXT NOT NULL,
    security TEXT NOT NULL DEFAULT 'STARTTLS',
    created_at TEXT NOT NULL
);
```

### 2.2 Bảng `email_templates`
```sql
CREATE TABLE IF NOT EXISTS email_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    subject TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

### 2.3 Bảng `email_outreach_history`
```sql
CREATE TABLE IF NOT EXISTS email_outreach_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lead_id INTEGER NOT NULL,
    template_id INTEGER NOT NULL,
    smtp_id INTEGER NOT NULL,
    status TEXT NOT NULL,
    error_message TEXT,
    sent_at TEXT NOT NULL,
    FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE,
    FOREIGN KEY(template_id) REFERENCES email_templates(id) ON DELETE CASCADE,
    FOREIGN KEY(smtp_id) REFERENCES smtp_servers(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_email_outreach_history_lead ON email_outreach_history(lead_id);
```

## 3. Data Models (`src-tauri/src/models.rs`)

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmtpServer {
    pub id: Option<i32>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
    pub security: String, // "STARTTLS", "SSL_TLS", "NONE"
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailTemplate {
    pub id: Option<i32>,
    pub name: String,
    pub subject: String,
    pub content: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailOutreachHistory {
    pub id: Option<i32>,
    pub lead_id: i32,
    pub template_id: i32,
    pub smtp_id: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub sent_at: String,
}
```

## 4. Backend Architecture & IPC Commands (`src-tauri/src/commands.rs`)

### 4.1 SMTP Server IPC Commands
- `get_smtp_servers() -> Result<Vec<SmtpServer>, String>`
- `save_smtp_server(server: SmtpServer) -> Result<i32, String>`
- `delete_smtp_server(id: i32) -> Result<(), String>`
- `test_smtp_connection(id: i32) -> Result<String, String>`

### 4.2 Email Template IPC Commands
- `get_email_templates() -> Result<Vec<EmailTemplate>, String>`
- `create_email_template(name: String, subject: String, content: String) -> Result<i32, String>`
- `update_email_template(id: i32, name: String, subject: String, content: String) -> Result<(), String>`
- `delete_email_template(id: i32) -> Result<(), String>`

### 4.3 Email Outreach Execution
- `send_email_outreach(app_handle: AppHandle, smtp_id: i32, lead_ids: Vec<i32>, template_id: i32) -> Result<(), String>`
  - Async task spawned via Tokio.
  - Replaces placeholders: `{{name}}`, `{{company}}`, `{{email}}`, `{{phone}}`.
  - Uses `lettre::AsyncSmtpTransport` for secure delivery.
  - Emits realtime events: `email-outreach-progress`, `email-outreach-log`, `email-outreach-complete`.
  - Logs results to `email_outreach_history` and `activities` tables.

## 5. Frontend Components & Navigation

### 5.1 Components
- `EmailOutreach.svelte`: Component styled symmetrically to `ZaloOutreach.svelte`.
  - Section 1: SMTP Server Management (List of configured SMTP servers + modal for adding/editing/testing connection).
  - Section 2: Email Template Manager (List of templates + modal for editing name, subject, body content with placeholder badges).
  - Section 3: Target Lead Selection (Search bar, category dropdown, segmentation dropdown, bulk selection checkboxes).
  - Section 4: Live Campaign Console (Progress bar, start/stop buttons, real-time log terminal).

### 5.2 Navigation Integration
- Update `Sidebar.svelte`: Add `{ id: 'email_outreach', label: 'Email Outreach', icon: 'mail' }`.
- Update `+page.svelte`: Bind `activeTab` to render `<EmailOutreach />`.

## 6. Testing Strategy
- Unit tests in `commands_test.rs` for database functions and template interpolation logic.
- UI tests for `EmailOutreach.svelte` in `src/lib/components/__tests__/EmailOutreach.spec.ts`.
