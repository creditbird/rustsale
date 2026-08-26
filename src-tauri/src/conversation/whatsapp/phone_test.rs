use super::phone::*;

#[test]
fn test_normalize_vietnamese_phones() {
    assert_eq!(
        normalize_e164("0901234567", Some("84")),
        Some("84901234567".to_string())
    );
    assert_eq!(
        normalize_e164("+84 901 234 567", None),
        Some("84901234567".to_string())
    );
    assert_eq!(
        normalize_e164("084901234567", Some("84")),
        Some("84901234567".to_string())
    );
    assert_eq!(
        normalize_e164("0084901234567", None),
        Some("84901234567".to_string())
    );
    assert_eq!(
        normalize_e164("0901234567", Some("+84")),
        Some("84901234567".to_string())
    );
}

#[test]
fn test_normalize_international_phones() {
    assert_eq!(
        normalize_e164("+1 (555) 123-4567", None),
        Some("15551234567".to_string())
    );
    assert_eq!(
        normalize_e164("+65 9123 4567", None),
        Some("6591234567".to_string())
    );
    assert_eq!(
        normalize_e164("0015551234567", None),
        Some("15551234567".to_string())
    );
}

#[test]
fn test_invalid_phones() {
    assert_eq!(normalize_e164("abc12345", None), None);
    assert_eq!(normalize_e164("123", None), None);
    assert_eq!(normalize_e164("", None), None);
    assert_eq!(normalize_e164("   ", None), None);
    assert_eq!(normalize_e164("0901234567", None), None); // Missing default country code
    assert_eq!(normalize_e164("+1234567890123456789", None), None); // Over 15 digits
}

#[test]
fn test_jid_formatting() {
    assert_eq!(
        phone_to_direct_jid("84901234567"),
        "84901234567@s.whatsapp.net"
    );
    assert_eq!(
        phone_to_direct_jid("+84 901 234 567"),
        "84901234567@s.whatsapp.net"
    );
    assert!(is_group_jid("123456789-987654@g.us"));
    assert!(!is_group_jid("84901234567@s.whatsapp.net"));
    assert!(is_direct_jid("84901234567@s.whatsapp.net"));
    assert!(is_direct_jid("84901234567@c.us"));
    assert!(!is_direct_jid("123456789-987654@g.us"));
}

#[test]
fn test_jid_to_phone() {
    assert_eq!(
        jid_to_phone("84901234567@s.whatsapp.net"),
        Some("84901234567".to_string())
    );
    assert_eq!(
        jid_to_phone("84901234567:1@s.whatsapp.net"),
        Some("84901234567".to_string())
    );
}

#[test]
fn test_lid_detection_and_resolution() {
    assert!(is_lid_jid("123456789012345@lid"));
    assert!(is_lid_jid("123456789012345:2@lid"));
    assert!(!is_lid_jid("84901234567@s.whatsapp.net"));
    assert_eq!(
        extract_lid_user("123456789012345:2@lid"),
        Some("123456789012345".to_string())
    );

    // Test DB resolution
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

    // Prior to mapping: returns None for LID
    assert_eq!(
        resolve_lid_or_jid_to_phone(&conn, "123456789012345@lid"),
        None
    );

    // Direct JID resolves immediately
    assert_eq!(
        resolve_lid_or_jid_to_phone(&conn, "84901234567@s.whatsapp.net"),
        Some("84901234567".to_string())
    );

    // Save LID-PN mapping
    save_lid_pn_mapping(&conn, "123456789012345", "84901234567").unwrap();

    // After mapping: resolves to phone
    assert_eq!(
        resolve_lid_or_jid_to_phone(&conn, "123456789012345@lid"),
        Some("84901234567".to_string())
    );
    assert_eq!(
        resolve_lid_or_jid_to_phone(&conn, "123456789012345:1@lid"),
        Some("84901234567".to_string())
    );
}
