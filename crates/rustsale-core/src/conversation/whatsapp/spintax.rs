use rand::Rng;
use std::collections::HashMap;

/// Parses single-level or nested Spintax patterns `{Option 1|Option 2|Option 3}`.
/// Handles empty options, unbalanced braces, and nested patterns gracefully.
pub fn parse_spintax(input: &str) -> String {
    parse_spintax_with_rng(input, &mut rand::thread_rng())
}

/// Parses spintax using a provided random number generator.
pub fn parse_spintax_with_rng<R: Rng>(input: &str, rng: &mut R) -> String {
    let mut text = input.to_string();
    let mut start_search = 0;

    while let Some(close_rel) = text[start_search..].find('}') {
        let close_idx = start_search + close_rel;
        if let Some(open_rel) = text[..close_idx].rfind('{') {
            let inner = &text[open_rel + 1..close_idx];
            let choices: Vec<&str> = inner.split('|').collect();
            if !choices.is_empty() {
                let chosen_idx = rng.gen_range(0..choices.len());
                let chosen = choices[chosen_idx].to_string();
                text.replace_range(open_rel..=close_idx, &chosen);
                start_search = open_rel;
            } else {
                text.replace_range(open_rel..=close_idx, "");
                start_search = open_rel;
            }
        } else {
            // Unmatched closing brace, advance search index to prevent infinite loop
            start_search = close_idx + 1;
        }
    }

    text
}

/// Substitutes placeholder tokens like `[name]`, `[phone]`, `[company]`, `[email]`, `[address]`, `[notes]`, `[title]`
/// (with case-insensitive keys) using variables from `lead_vars`, and then passes the result through `parse_spintax`.
pub fn render_template(template: &str, lead_vars: &HashMap<String, String>) -> String {
    let mut normalized_vars: HashMap<String, &str> = HashMap::new();
    for (k, v) in lead_vars {
        normalized_vars.insert(k.trim().to_lowercase(), v.as_str());
    }

    let mut rendered = String::with_capacity(template.len());
    let mut chars = template.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        if ch == '[' {
            // Check for matching ']'
            let mut found_close = None;
            let peek_chars = chars.clone();
            for (c_idx, c_ch) in peek_chars {
                if c_ch == ']' {
                    found_close = Some(c_idx);
                    break;
                } else if c_ch == '[' || c_ch == '\n' {
                    // Nested '[' or newline breaks bracket token
                    break;
                }
            }

            if let Some(close_idx) = found_close {
                let token_content = &template[idx + 1..close_idx];
                let key_lower = token_content.trim().to_lowercase();
                if let Some(val) = normalized_vars.get(&key_lower) {
                    rendered.push_str(val);
                    while let Some(&(c_idx, _)) = chars.peek() {
                        if c_idx <= close_idx {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    continue;
                }
            }
        }
        rendered.push(ch);
    }

    parse_spintax(&rendered)
}
