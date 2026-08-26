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
fn test_parse_raw_jid() {
    let direct = parse_raw_jid("84901234567@s.whatsapp.net");
    assert!(direct.is_some());
    let jid = direct.unwrap();
    assert_eq!(jid.user.as_str(), "84901234567");

    let group = parse_raw_jid("123456789-987654@g.us");
    assert!(group.is_some());

    let invalid = parse_raw_jid("not_a_jid");
    assert!(invalid.is_none());
}
