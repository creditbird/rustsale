//! Linking an identity to a lead checks both exist first: a well-formed id that points at
//! nothing is "not found", never a FOREIGN KEY failure.

use super::commands_test::{direct_message, ServiceHarness};

#[test]
fn linking_to_a_missing_lead_is_not_found_and_changes_nothing() {
    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-link", "m-1"))
        .unwrap();
    let identity_id = applied.identity_id.unwrap();
    let error = harness
        .service
        .link_identity(identity_id, 424_242)
        .unwrap_err()
        .to_string();
    assert_eq!(error, "lead 424242 not found");
}

#[test]
fn linking_a_missing_identity_is_not_found() {
    let harness = ServiceHarness::new();
    harness.insert_lead(7, "Lead Seven");
    let error = harness
        .service
        .link_identity(424_242, 7)
        .unwrap_err()
        .to_string();
    assert_eq!(error, "identity 424242 not found");
}
