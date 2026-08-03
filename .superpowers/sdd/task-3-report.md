# Task 3 Report: Add Phase 2A Contracts, Capabilities, and Errors

## Summary
Task 3 of Zalo Rich Chat Phase 2A has been successfully implemented using Test-Driven Development (TDD) and verified with the full Rust conversation test suite.

## Summary of Changes

### 1. Extended Domain Contracts & Types (`src-tauri/src/conversation/types.rs`)
- Added `ConversationReactionCode` enum (`Heart`, `Like`, `Haha`, `Wow`, `Cry`, `Angry`) with `snake_case` serde serialization.
- Added Mentions domain types: `ConversationMentionInput`, `ConversationMention`, `ConversationMentionCandidate`, and `ConversationMentionCapabilities`.
- Added Reactions mutation contracts: `SetConversationReactionRequest`, `ReactionMutationStatus`, `ConversationReactionMutationResult`, and `ConversationReactionCapabilities`.
- Added Message Forwarding contracts: `ForwardConversationMessageRequest`, `ForwardOperationStatus`, `ForwardTargetStatus`, `ConversationForwardTargetResult`, `ForwardConversationMessageResult`, and `ConversationForwardCapabilities`.
- Added Quick Templates contracts: `ConversationQuickTemplate`, `CreateConversationQuickTemplateRequest`, `UpdateConversationQuickTemplateRequest`, `ListConversationQuickTemplatesRequest`, `DeleteConversationQuickTemplateRequest`, `MarkConversationQuickTemplateUsedRequest`, and `ConversationQuickTemplateCapabilities`.
- Extended `ConversationCapabilities` with `reactions`, `forward`, `mentions`, and `quick_templates`.
  - `ConversationCapabilities::zalo_phase_one()` exposes 6 reaction codes, max 10 forward targets, max 20 mentions per message, and max 500 templates.
  - `ConversationCapabilities::text_only()` exposes none / disabled for all Phase 2A capabilities.
- Extended `ConversationMessage` with `mentions: Vec<ConversationMention>` and `actions: ConversationMessageActions` (with `forward`, `react`, `recall`, `reply`).
- Extended `ConversationSendPartContent::Text` with `mentions: Vec<ConversationMentionInput>` and added `ConversationSendPartContent::Forward` variant for internal outbox processing.

### 2. Extended Error Codes (`src-tauri/src/conversation/error.rs`)
Added 13 new `ConversationErrorCode` variants with `snake_case` serialization mapping:
- `ReactionNotSupported` (`"reaction_not_supported"`)
- `ReactionRemovalNotVerified` (`"reaction_removal_not_verified"`)
- `ReactionDeliveryUnknown` (`"reaction_delivery_unknown"`)
- `ForwardNotSupported` (`"forward_not_supported"`)
- `ForwardSourceMissingProviderIds` (`"forward_source_missing_provider_ids"`)
- `ForwardTargetAccountMismatch` (`"forward_target_account_mismatch"`)
- `ForwardTargetLimitExceeded` (`"forward_target_limit_exceeded"`)
- `ForwardDeliveryUnknown` (`"forward_delivery_unknown"`)
- `TemplateLimitExceeded` (`"template_limit_exceeded"`)
- `TemplateShortcutConflict` (`"template_shortcut_conflict"`)
- `InvalidMentionRange` (`"invalid_mention_range"`)
- `MentionParticipantNotFound` (`"mention_participant_not_found"`)
- `MentionDirectConversationUnsupported` (`"mention_direct_conversation_unsupported"`)

### 3. Extended Provider Adapter Interfaces (`src-tauri/src/conversation/adapter.rs`)
- Added `ProviderReactionRequest`, `ProviderReactionResult`, `ProviderForwardRequest`, and `ProviderForwardResult`.
- Extended `ChannelSender` trait with default async methods `set_reaction` and `forward_message` returning unsupported error for non-Zalo/mock adapters.

### 4. Verification & Testing
- **Contract Tests**: Added `phase2a_contract_capabilities` and `phase2a_contract_serialization` tests to `types.rs`.
- **TDD Failure Check**: Confirmed build failure prior to type definitions.
- **Contract Test Suite**: `cargo test --manifest-path src-tauri/Cargo.toml phase2a_contract` passed 2/2 tests.
- **Full Conversation Suite**: `cargo test --manifest-path src-tauri/Cargo.toml conversation::` passed 126/126 tests cleanly.
- **GitNexus Scope Verification**: Ran `gitnexus_detect_changes` to verify affected scope.
