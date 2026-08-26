use super::spintax::{parse_spintax, render_template};
use std::collections::HashMap;

#[test]
fn test_spintax_single_level_variations() {
    let pattern = "{Xin chào|Chào bạn} [name]";
    let mut found_xin_chao = false;
    let mut found_chao_ban = false;

    // Over multiple runs, both variations should be sampled
    for _ in 0..100 {
        let rendered = parse_spintax(pattern);
        if rendered == "Xin chào [name]" {
            found_xin_chao = true;
        } else if rendered == "Chào bạn [name]" {
            found_chao_ban = true;
        } else {
            panic!("Unexpected rendered spintax output: {}", rendered);
        }
    }

    assert!(found_xin_chao, "Expected to sample 'Xin chào'");
    assert!(found_chao_ban, "Expected to sample 'Chào bạn'");
}

#[test]
fn test_spintax_nested_levels() {
    let pattern = "{Hello {there|friend}|Hi {world|everyone}}!";
    let mut variations = std::collections::HashSet::new();

    for _ in 0..200 {
        let rendered = parse_spintax(pattern);
        variations.insert(rendered);
    }

    assert!(variations.contains("Hello there!"));
    assert!(variations.contains("Hello friend!"));
    assert!(variations.contains("Hi world!"));
    assert!(variations.contains("Hi everyone!"));
    assert_eq!(variations.len(), 4);
}

#[test]
fn test_spintax_edge_cases_no_panic() {
    // No spintax
    assert_eq!(parse_spintax("Hello world"), "Hello world");
    assert_eq!(parse_spintax(""), "");

    // Single choice in braces
    assert_eq!(parse_spintax("{Hello}"), "Hello");

    // Empty choices
    let res = parse_spintax("{|}");
    assert!(res.is_empty());

    // Unbalanced opening brace
    let unclosed = parse_spintax("Hello {world");
    assert_eq!(unclosed, "Hello {world");

    // Unbalanced closing brace
    let unopened = parse_spintax("Hello world}");
    assert_eq!(unopened, "Hello world}");

    // Orphan braces
    let orphan = parse_spintax("}{");
    assert_eq!(orphan, "}{");

    // Unbalanced nested
    let unclosed_nested = parse_spintax("{A|{B|C}");
    assert!(unclosed_nested == "{A|B" || unclosed_nested == "{A|C");
}

#[test]
fn test_render_template_variable_substitution() {
    let template = "{Xin chào|Chào bạn} [Name]! Số điện thoại của bạn là [phone] tại công ty [COMPANY]. Ghi chú: [notes]. Chức vụ: [Title], Địa chỉ: [address], Email: [EMAIL].";

    let mut vars = HashMap::new();
    vars.insert("name".to_string(), "Trần Văn A".to_string());
    vars.insert("phone".to_string(), "0901234567".to_string());
    vars.insert("company".to_string(), "RustSale Corp".to_string());
    vars.insert("notes".to_string(), "VIP customer".to_string());
    vars.insert("title".to_string(), "Giám đốc".to_string());
    vars.insert("address".to_string(), "123 Đường Lê Lợi".to_string());
    vars.insert("email".to_string(), "vana@example.com".to_string());

    let rendered = render_template(template, &vars);

    assert!(rendered.contains("Trần Văn A!"));
    assert!(rendered.contains("Số điện thoại của bạn là 0901234567"));
    assert!(rendered.contains("tại công ty RustSale Corp."));
    assert!(rendered.contains("Ghi chú: VIP customer."));
    assert!(rendered.contains("Chức vụ: Giám đốc"));
    assert!(rendered.contains("Địa chỉ: 123 Đường Lê Lợi"));
    assert!(rendered.contains("Email: vana@example.com"));
    assert!(rendered.starts_with("Xin chào") || rendered.starts_with("Chào bạn"));
}

#[test]
fn test_render_template_missing_vars_and_custom_tokens() {
    let template = "Chào [name], mã giới thiệu của bạn là [REFERRAL_CODE] và [Unknown_Token].";
    let mut vars = HashMap::new();
    vars.insert("Name".to_string(), "Bình".to_string());
    vars.insert("referral_code".to_string(), "REF123".to_string());

    let rendered = render_template(template, &vars);
    assert!(rendered.contains("Chào Bình"));
    assert!(rendered.contains("mã giới thiệu của bạn là REF123"));
    // Unmatched token left intact
    assert!(rendered.contains("[Unknown_Token]"));
}
