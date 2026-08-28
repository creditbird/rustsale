use std::str::FromStr;
pub use whatsapp_rust::Jid;

pub const WHATSAPP_DIRECT_SERVER: &str = "s.whatsapp.net";
pub const WHATSAPP_GROUP_SERVER: &str = "g.us";
pub const WHATSAPP_LID_SERVER: &str = "lid";
pub const WHATSAPP_BROADCAST_SERVER: &str = "broadcast";
pub const WHATSAPP_NEWSLETTER_SERVER: &str = "newsletter";

/// Checks whether a given JID is a WhatsApp Linked Identity (`@lid`).
pub fn is_lid_jid(jid: &str) -> bool {
    jid.ends_with(&format!("@{WHATSAPP_LID_SERVER}"))
}

/// Extracts the LID user string from a LID JID.
pub fn extract_lid_user(jid: &str) -> Option<String> {
    if !is_lid_jid(jid) {
        return None;
    }
    let user_part = jid.split('@').next()?;
    let user_base = user_part.split(':').next()?;
    if !user_base.is_empty() {
        Some(user_base.to_string())
    } else {
        None
    }
}

/// Resolves a JID or LID to an E.164 phone string.
/// For `@s.whatsapp.net` or `@c.us`, extracts the phone directly.
/// For `@lid`, queries the `whatsapp_lid_pn_mappings` cache table.
pub fn resolve_lid_or_jid_to_phone(conn: &rusqlite::Connection, jid: &str) -> Option<String> {
    if is_direct_jid(jid) {
        jid_to_phone(jid)
    } else if is_lid_jid(jid) {
        let lid_user = extract_lid_user(jid)?;
        conn.query_row(
            "SELECT phone_e164 FROM whatsapp_lid_pn_mappings WHERE lid = ?1",
            rusqlite::params![lid_user],
            |row| row.get::<_, String>(0),
        )
        .ok()
    } else {
        None
    }
}

/// Saves or updates a LID -> Phone mapping into `whatsapp_lid_pn_mappings`.
pub fn save_lid_pn_mapping(
    conn: &rusqlite::Connection,
    lid: &str,
    phone_e164: &str,
) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let clean_lid = extract_lid_user(lid).unwrap_or_else(|| lid.to_string());
    conn.execute(
        "INSERT INTO whatsapp_lid_pn_mappings (lid, phone_e164, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?3)
         ON CONFLICT(lid) DO UPDATE SET phone_e164 = EXCLUDED.phone_e164, updated_at = EXCLUDED.updated_at",
        rusqlite::params![clean_lid, phone_e164, now],
    )?;
    Ok(())
}

/// Normalizes a raw phone number into E.164 digits format (without leading '+').
/// Returns None if the input contains invalid characters or does not satisfy
/// standard E.164 length requirements (7 to 15 digits).
pub fn normalize_e164(raw: &str, default_country: Option<&str>) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    let has_plus = raw.starts_with('+');
    let content = if has_plus { &raw[1..] } else { raw };

    // Check for invalid characters
    for c in content.chars() {
        if !c.is_ascii_digit()
            && c != ' '
            && c != '-'
            && c != '('
            && c != ')'
            && c != '.'
            && c != '/'
        {
            return None;
        }
    }

    let digits: String = content.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }

    let normalized_country = default_country.map(|c| c.trim_start_matches('+').trim());

    let final_digits = if has_plus {
        // Explicit international format with '+'
        digits
    } else if let Some(rest) = digits.strip_prefix("00") {
        // International dialing prefix '00'
        rest.to_string()
    } else if let Some(country) = normalized_country {
        if let Some(rest) = digits.strip_prefix(&format!("0{country}")) {
            // E.g., "084901234567" with country "84" -> "84901234567"
            format!("{country}{rest}")
        } else if let Some(rest) = digits.strip_prefix('0') {
            // E.g., "0901234567" with country "84" -> "84901234567"
            format!("{country}{rest}")
        } else if digits.starts_with(country) {
            // E.g., "84901234567" with country "84"
            digits
        } else if digits.len() <= 10 {
            // Domestic subscriber number without leading 0
            format!("{country}{digits}")
        } else {
            digits
        }
    } else if let Some(rest) = digits.strip_prefix("084") {
        format!("84{rest}")
    } else if digits.starts_with('0') {
        // Domestic format without known country code cannot be reliably converted to E.164
        return None;
    } else {
        digits
    };

    // E.164 length constraint: 7 to 15 digits
    if final_digits.len() >= 7
        && final_digits.len() <= 15
        && final_digits.chars().all(|c| c.is_ascii_digit())
    {
        Some(final_digits)
    } else {
        None
    }
}

/// Formats an E.164 phone string into a direct WhatsApp JID (`<phone>@s.whatsapp.net`).
pub fn phone_to_direct_jid(e164: &str) -> String {
    let clean: String = e164.chars().filter(|c| c.is_ascii_digit()).collect();
    format!("{clean}@{WHATSAPP_DIRECT_SERVER}")
}

/// Checks whether a given JID string belongs to a WhatsApp Group (`@g.us`).
pub fn is_group_jid(jid: &str) -> bool {
    jid.ends_with(&format!("@{WHATSAPP_GROUP_SERVER}"))
}

/// Checks whether a given JID string belongs to a direct WhatsApp user (`@s.whatsapp.net` or legacy `@c.us`).
pub fn is_direct_jid(jid: &str) -> bool {
    jid.ends_with(&format!("@{WHATSAPP_DIRECT_SERVER}")) || jid.ends_with("@c.us")
}

/// Extracts the phone/user part from a direct WhatsApp JID if applicable.
pub fn jid_to_phone(jid: &str) -> Option<String> {
    if !is_direct_jid(jid) {
        return None;
    }
    let user_part = jid.split('@').next()?;
    let user_base = user_part.split(':').next()?;
    let clean: String = user_base.chars().filter(|c| c.is_ascii_digit()).collect();
    if !clean.is_empty() {
        Some(clean)
    } else {
        None
    }
}

/// Parses a raw JID string into a `whatsapp_rust::Jid`.
pub fn parse_raw_jid(jid_str: &str) -> Option<Jid> {
    Jid::from_str(jid_str).ok()
}
