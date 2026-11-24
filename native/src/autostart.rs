//! Starting RustSale at sign-in on Linux: an XDG autostart `.desktop` file under
//! `$XDG_CONFIG_HOME/autostart` (or `~/.config/autostart`).
//!
//! The file is ours alone (`rustsale.desktop`); turning the switch off removes it. Other systems
//! answer `unsupported` from the export (see `settings.rs`): nothing is written there.

use crate::error::CoreApiError;
use std::path::{Path, PathBuf};

/// The entry's file name, so another app's autostart file is never touched.
const FILE_NAME: &str = "rustsale.desktop";

/// Where the entry lives and what it starts.
pub(crate) struct Paths {
    /// The XDG config folder (`$XDG_CONFIG_HOME` or `~/.config`).
    pub(crate) config_home: PathBuf,
    /// The program the entry starts.
    pub(crate) exe: PathBuf,
}

/// The paths of this run, or an error when the system names no config folder or program.
pub(crate) fn paths() -> Result<Paths, CoreApiError> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .ok_or_else(|| {
            CoreApiError::new(
                "unavailable",
                "no config folder: neither XDG_CONFIG_HOME nor HOME is set",
            )
        })?;
    let exe = std::env::current_exe().map_err(|error| {
        CoreApiError::internal(format!("cannot find the RustSale program: {error}"))
    })?;
    Ok(Paths { config_home, exe })
}

/// The autostart entry of `config_home`.
pub(crate) fn desktop_path(config_home: &Path) -> PathBuf {
    config_home.join("autostart").join(FILE_NAME)
}

/// Whether the entry exists: what the screen's switch shows.
pub(crate) fn is_enabled_at(config_home: &Path) -> bool {
    desktop_path(config_home).is_file()
}

/// Writes or removes the entry. Removing one that is already gone is fine.
pub(crate) fn set_enabled_at(
    config_home: &Path,
    exe: &Path,
    enabled: bool,
) -> Result<(), CoreApiError> {
    let path = desktop_path(config_home);
    if enabled {
        let folder = path
            .parent()
            .ok_or_else(|| CoreApiError::internal("the autostart entry has no folder"))?;
        std::fs::create_dir_all(folder).map_err(|error| {
            CoreApiError::internal(format!("cannot create the autostart folder: {error}"))
        })?;
        std::fs::write(&path, entry(exe)).map_err(|error| {
            CoreApiError::internal(format!("cannot write the autostart entry: {error}"))
        })?;
        return Ok(());
    }
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CoreApiError::internal(format!(
            "cannot remove the autostart entry: {error}"
        ))),
    }
}

/// The entry's text, per the Desktop Entry specification: the program is quoted as one argument
/// when it holds a reserved character, then the whole value is escaped.
pub(crate) fn entry(exe: &Path) -> String {
    let command = escape_value(&quote_arg(&exe.to_string_lossy()));
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=RustSale\n\
         Comment=Zalo, WhatsApp and Facebook when you sign in\n\
         Exec={command}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

/// Quotes one `Exec` argument when it holds a reserved character, as the specification says.
fn quote_arg(text: &str) -> String {
    let reserved = |c: char| c.is_whitespace() || "\"'\\><~|&;$*?#()`".contains(c);
    if !text.chars().any(reserved) {
        return text.to_owned();
    }
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

/// The general string escape of the specification, applied to the quoted argument.
fn escape_value(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config() -> Result<tempfile::TempDir, std::io::Error> {
        tempfile::tempdir()
    }

    #[test]
    fn enabling_writes_an_entry_and_disabling_removes_it() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = temp_config()?;
        let config = root.path();
        let exe = Path::new("/usr/bin/rustsale");
        assert!(!is_enabled_at(config));
        assert!(!desktop_path(config).exists());

        set_enabled_at(config, exe, true)?;
        assert!(is_enabled_at(config));
        let text = std::fs::read_to_string(desktop_path(config))?;
        assert!(text.starts_with("[Desktop Entry]\n"));
        assert!(text.contains("Exec=/usr/bin/rustsale\n"));
        assert!(text.contains("X-GNOME-Autostart-enabled=true\n"));

        // Enabling again is a plain overwrite.
        set_enabled_at(config, exe, true)?;
        assert!(is_enabled_at(config));

        set_enabled_at(config, exe, false)?;
        assert!(!is_enabled_at(config));
        // Disabling what is already gone is not an error.
        set_enabled_at(config, exe, false)?;
        Ok(())
    }

    #[test]
    fn a_program_path_with_spaces_is_one_quoted_argument() {
        let entry = entry(Path::new("/opt/RustSale App/rustsale"));
        assert!(
            entry.contains("Exec=\"/opt/RustSale App/rustsale\"\n"),
            "{entry}"
        );
    }

    #[test]
    fn a_reserved_character_is_quoted_and_escaped() {
        let entry = entry(Path::new("/opt/$HOME/rustsale"));
        // `$` is quoted inside Exec, then the backslash is escaped for the value itself.
        assert!(
            entry.contains("Exec=\"/opt/\\\\$HOME/rustsale\"\n"),
            "{entry}"
        );
    }

    #[test]
    fn the_entry_starts_the_program_itself_with_no_extra_argument() {
        let text = entry(Path::new("/usr/bin/rustsale"));
        let exec = text
            .lines()
            .find_map(|line| line.strip_prefix("Exec="))
            .unwrap_or_default();
        assert_eq!(exec, "/usr/bin/rustsale");
    }

    #[test]
    fn the_entry_is_ours_alone() {
        assert_eq!(
            desktop_path(Path::new("/home/x/.config")),
            PathBuf::from("/home/x/.config/autostart/rustsale.desktop")
        );
    }
}
