use super::mentions::{utf16_len, utf16_slice, validate_mentions, MentionParticipant};
use crate::conversation::error::ConversationErrorCode;
use crate::conversation::types::ConversationMentionInput;

#[test]
fn mention_utf16_helpers() {
    // ASCII
    assert_eq!(utf16_len("hello"), 5);
    assert_eq!(utf16_slice("hello", 0, 5), Some("hello".into()));
    assert_eq!(utf16_slice("hello", 1, 3), Some("ell".into()));

    // Combining characters
    let combining = "e\u{0301}"; // e + combining acute accent
    assert_eq!(utf16_len(combining), 2);
    assert_eq!(utf16_slice(combining, 0, 2), Some(combining.into()));

    // Precomposed Vietnamese
    let vn = "Xin @Hà nhé";
    assert_eq!(utf16_len(vn), 11);
    assert_eq!(utf16_slice(vn, 4, 3), Some("@Hà".into()));

    // Emoji (surrogate pairs in UTF-16)
    let emoji = "A😀B";
    assert_eq!(utf16_len(emoji), 4);
    assert_eq!(utf16_slice(emoji, 0, 1), Some("A".into()));
    assert_eq!(utf16_slice(emoji, 1, 2), Some("😀".into()));
    assert_eq!(utf16_slice(emoji, 3, 1), Some("B".into()));
    // Splitting surrogate pair returns None
    assert_eq!(utf16_slice(emoji, 1, 1), None);

    // Multiple tokens
    let complex = "Xin @Hà😀 nhé";
    // "Xin " (4) + "@Hà" (3) + "😀" (2) + " nhé" (4) = 13
    assert_eq!(utf16_len(complex), 13);
    assert_eq!(utf16_slice(complex, 4, 5), Some("@Hà😀".into()));
}

#[test]
fn mention_outbound_mentions_require_active_group_participants() {
    let text = "Hello @Alice and @Bob!";
    let inputs = vec![
        ConversationMentionInput {
            pos: 6,
            len: 6,
            target_identity_id: Some(101),
            is_all: false,
        },
        ConversationMentionInput {
            pos: 17,
            len: 4,
            target_identity_id: Some(102),
            is_all: false,
        },
    ];

    // Case 1: Direct conversation (empty participants) fails with MentionDirectConversationUnsupported
    let err = validate_mentions(text, &inputs, &[]).unwrap_err();
    assert_eq!(
        err.code,
        ConversationErrorCode::MentionDirectConversationUnsupported
    );

    // Case 2: Foreign participant not found in list fails with MentionParticipantNotFound
    let participants = vec![MentionParticipant {
        identity_id: 101,
        external_id: Some("uid-101".into()),
        display_name: "Alice".into(),
        is_active: true,
    }];
    let err = validate_mentions(text, &inputs, &participants).unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::MentionParticipantNotFound);

    // Case 3: Inactive participant fails with MentionParticipantNotFound
    let participants_inactive = vec![
        MentionParticipant {
            identity_id: 101,
            external_id: Some("uid-101".into()),
            display_name: "Alice".into(),
            is_active: true,
        },
        MentionParticipant {
            identity_id: 102,
            external_id: Some("uid-102".into()),
            display_name: "Bob".into(),
            is_active: false,
        },
    ];
    let err = validate_mentions(text, &inputs, &participants_inactive).unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::MentionParticipantNotFound);

    // Case 4: Active participants succeed
    let active_participants = vec![
        MentionParticipant {
            identity_id: 101,
            external_id: Some("uid-101".into()),
            display_name: "Alice".into(),
            is_active: true,
        },
        MentionParticipant {
            identity_id: 102,
            external_id: Some("uid-102".into()),
            display_name: "Bob".into(),
            is_active: true,
        },
    ];
    let resolved = validate_mentions(text, &inputs, &active_participants).unwrap();
    assert_eq!(resolved.len(), 2);
}

#[test]
fn mention_backend_resolves_mention_external_uid() {
    let text = "Chào @Nguyễn Văn A!";
    let inputs = vec![ConversationMentionInput {
        pos: 5,
        len: 13,
        target_identity_id: Some(42),
        is_all: false,
    }];
    let participants = vec![MentionParticipant {
        identity_id: 42,
        external_id: Some("zalo-uid-user-999".into()),
        display_name: "Nguyễn Văn A".into(),
        is_active: true,
    }];

    let resolved = validate_mentions(text, &inputs, &participants).unwrap();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].target_identity_id, Some(42));
    assert_eq!(
        resolved[0].target_external_id.as_deref(),
        Some("zalo-uid-user-999")
    );
    assert_eq!(resolved[0].display_name.as_deref(), Some("Nguyễn Văn A"));
    assert_eq!(resolved[0].pos, 5);
    assert_eq!(resolved[0].len, 13);
    assert!(!resolved[0].is_all);
}
