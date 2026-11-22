//! Faults found by the UI2 QA that live in the core: unknown channels, edits of rows that are
//! gone, ids that point at nothing. Each test names the item of the ui2-core brief.

use super::commands;

#[test]
fn item2_unknown_template_channels_are_invalid() {
    for channel in ["bogus", "", "ZALO", " zalo", "sms"] {
        let error = commands::validate_template_channel(channel).unwrap_err();
        assert!(error.contains("invalid"), "{channel:?}: {error}");
    }
    for channel in commands::TEMPLATE_CHANNELS {
        assert_eq!(commands::validate_template_channel(channel), Ok(()));
    }
}
