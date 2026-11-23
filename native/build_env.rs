//! Reads build-time settings (`VITE_SUPABASE_*`) from the environment or a `.env` file that is never committed.
//! Shared by `build.rs` and, for its tests, by the library (`#[path]`), so the parsing has one copy.

/// A value of `.env` syntax: `KEY=value`, optional `export `, optional single or double quotes, `#` comments.
pub fn parse_dotenv(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (name, value) = line.split_once('=')?;
        if name.trim() != key {
            return None;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        Some(value.to_owned())
    })
}

/// Whether `value` is a real setting: not empty and not the placeholder `.env.example` ships.
pub fn is_real(value: &str) -> bool {
    let v = value.trim();
    !(v.is_empty()
        || v.contains("your-project-ref")
        || v.contains("your_supabase")
        || v.contains("xyzcompany"))
}

/// The value for `key`: the process environment wins over the `.env` text.
pub fn resolve(env: Option<String>, dotenv: Option<&str>, key: &str) -> Option<String> {
    env.filter(|v| is_real(v)).or_else(|| {
        dotenv
            .and_then(|t| parse_dotenv(t, key))
            .filter(|v| is_real(v))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotenv_values_are_read_with_quotes_and_comments() {
        let text =
            "# c\nVITE_A=one\nexport VITE_B=\"two words\"\nVITE_C='three'\n  VITE_D = four \n";
        assert_eq!(parse_dotenv(text, "VITE_A").as_deref(), Some("one"));
        assert_eq!(parse_dotenv(text, "VITE_B").as_deref(), Some("two words"));
        assert_eq!(parse_dotenv(text, "VITE_C").as_deref(), Some("three"));
        assert_eq!(parse_dotenv(text, "VITE_D").as_deref(), Some("four"));
        assert_eq!(parse_dotenv(text, "VITE_E"), None);
    }

    #[test]
    fn the_example_placeholders_never_count_as_settings() {
        let example = "VITE_SUPABASE_URL=https://your-project-ref.supabase.co\nVITE_SUPABASE_ANON_KEY=your_supabase_anon_key_here\n";
        assert_eq!(resolve(None, Some(example), "VITE_SUPABASE_URL"), None);
        assert_eq!(resolve(None, Some(example), "VITE_SUPABASE_ANON_KEY"), None);
        assert_eq!(
            resolve(Some(String::new()), None, "VITE_SUPABASE_URL"),
            None
        );
    }

    #[test]
    fn the_environment_wins_over_the_file() {
        let file = "VITE_SUPABASE_URL=https://file.supabase.co\n";
        assert_eq!(
            resolve(
                Some("https://env.supabase.co".into()),
                Some(file),
                "VITE_SUPABASE_URL"
            )
            .as_deref(),
            Some("https://env.supabase.co")
        );
        assert_eq!(
            resolve(None, Some(file), "VITE_SUPABASE_URL").as_deref(),
            Some("https://file.supabase.co")
        );
    }
}
