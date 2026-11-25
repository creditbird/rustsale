//! Build-time settings for the native module:
//! - turns the demo exports on when RUSTSALE_DEMO=1 is set while building (tests and screenshots only);
//! - hands `VITE_SUPABASE_URL` / `VITE_SUPABASE_ANON_KEY` to the crate from the environment or an
//!   uncommitted `.env` (repo root or `native/`); nothing is read at run time and nothing is committed.

#[path = "build_env.rs"]
mod build_env;

const SETTINGS: [(&str, &str); 2] = [
    ("VITE_SUPABASE_URL", "RUSTSALE_BUILD_SUPABASE_URL"),
    ("VITE_SUPABASE_ANON_KEY", "RUSTSALE_BUILD_SUPABASE_ANON_KEY"),
];

fn main() {
    println!("cargo:rustc-check-cfg=cfg(rustsale_demo)");
    println!("cargo:rerun-if-env-changed=RUSTSALE_DEMO");
    if std::env::var("RUSTSALE_DEMO").is_ok_and(|v| v == "1") {
        println!("cargo:rustc-cfg=rustsale_demo");
    }

    println!("cargo:rerun-if-changed=build_env.rs");
    println!("cargo:rerun-if-changed=../.env");
    println!("cargo:rerun-if-changed=.env");
    let dotenv: String = [".env", "../.env"]
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .collect::<Vec<_>>()
        .join("\n");
    for (key, out) in SETTINGS {
        println!("cargo:rerun-if-env-changed={key}");
        if let Some(value) = build_env::resolve(std::env::var(key).ok(), Some(&dotenv), key) {
            // A newline would end the directive early, so such a value is dropped, not truncated.
            if !value.contains(['\n', '\r']) {
                println!("cargo:rustc-env={out}={value}");
            }
        }
    }
}
