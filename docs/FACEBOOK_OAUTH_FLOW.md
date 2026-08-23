# Facebook Page Connection & Token Vault Architecture

## Overview
RustSale Desktop provides secure Facebook Page & Messenger connectivity without relying on external intermediate servers. This architecture document defines the security boundaries, authentication flow, and token vault implementation.

---

## 1. Authentication & Token Acquisition Flow

### Client-side Meta OAuth Dialog with Cryptographic State Nonce
1. **Nonce Generation**:
   - The desktop client generates a 128-bit cryptographically secure random nonce (`crypto.getRandomValues`) before opening the Meta authentication window.
   - The state nonce is bound to the active session in memory.
2. **Meta Authorization**:
   - The client launches `https://www.facebook.com/v26.0/dialog/oauth` with parameters:
     - `client_id`: Meta App ID
     - `redirect_uri`: `https://facebook.com/connect/login_success.html`
     - `response_type`: `token`
     - `state`: `<cryptographic_nonce>`
     - `scope`: `pages_show_list,pages_read_engagement,pages_manage_posts,pages_messaging`
3. **Callback & State Verification**:
   - When the authorization response is received, the client strictly validates that `state` matches the session nonce to prevent cross-site request forgery (CSRF) and injection attacks. Callbacks lacking `state` or with mismatched nonces are rejected fail-closed.

---

## 2. Token Validation & Capability Verification

### Strict Identity & Capability Proof via Meta Graph API
1. **Token Transport**:
   - All Meta Graph API calls exclusively transmit access tokens via the HTTP `Authorization: Bearer <token>` header. Tokens are never included in query strings to prevent leakage into error URLs, logs, or diagnostic dumps.
2. **Principal Identity Verification**:
   - Before saving a Page connection, the backend queries the token-relative `GET https://graph.facebook.com/v26.0/me?fields=id,name,category,tasks`.
   - The returned principal `id` must strictly match the target `page_id`. Non-matching principals (such as personal user accounts) are rejected.
3. **Fail-Closed Capability Resolution**:
   - Granted scopes (`pages_messaging`, `pages_manage_posts`, `pages_read_engagement`) are exclusively extracted from Meta's verified `tasks` array.
   - Missing tasks/permissions never default to full capabilities; unverified scopes are omitted (fail-closed).

---

## 3. Storage & Encryption (AES-256 Keyring Token Vault)

1. **Hardware-backed OS Keyring**:
   - A master cryptographic key is generated and stored in the OS Keyring (Linux Secret Service / Windows Credential Manager / macOS Keychain).
2. **Authenticated Encryption (AES-256-GCM)**:
   - Each page token is encrypted using AES-256-GCM with a unique 96-bit nonce per encryption.
   - The `channel_account_id` and `page_id` are bound as **Authenticated Additional Data (AAD)**, preventing ciphertext substitution attacks between different accounts or databases.
3. **Database Separation**:
   - Only the encrypted ciphertext and nonce are stored in SQLite `facebook_page_tokens`. Plaintext tokens are never written to disk or logged.
