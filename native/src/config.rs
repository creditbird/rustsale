//! Settings fixed when the app was built (see `build.rs`): the Supabase project of the release.

/// The Supabase project the build was configured with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupabaseConfig {
    /// Project URL (`VITE_SUPABASE_URL` at build time).
    pub url: &'static str,
    /// Public anon key (`VITE_SUPABASE_ANON_KEY` at build time).
    pub anon_key: &'static str,
}

/// The Supabase settings, or `None` when the build had no real `VITE_SUPABASE_URL` and
/// `VITE_SUPABASE_ANON_KEY` (a dev build): callers treat that as "sync is off", never as a default project.
pub fn supabase() -> Option<SupabaseConfig> {
    pair(
        option_env!("RUSTSALE_BUILD_SUPABASE_URL"),
        option_env!("RUSTSALE_BUILD_SUPABASE_ANON_KEY"),
    )
}

fn pair(url: Option<&'static str>, anon_key: Option<&'static str>) -> Option<SupabaseConfig> {
    match (url, anon_key) {
        (Some(url), Some(anon_key)) if url.starts_with("https://") && !anon_key.is_empty() => {
            Some(SupabaseConfig { url, anon_key })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_values_are_needed_and_the_url_must_be_https() {
        assert_eq!(pair(None, Some("k")), None);
        assert_eq!(pair(Some("https://p.supabase.co"), None), None);
        assert_eq!(pair(Some("http://p.supabase.co"), Some("k")), None);
        assert_eq!(
            pair(Some("https://p.supabase.co"), Some("k")),
            Some(SupabaseConfig {
                url: "https://p.supabase.co",
                anon_key: "k"
            })
        );
    }
}
