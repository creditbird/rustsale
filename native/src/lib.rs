//! RustSale v3 native module: what `App.tsx` imports from `rustsale/native`.
//! The only place TSX reaches `rustsale-core`.

mod campaigns;
mod categories;
mod conversation_actions;
mod conversations;
mod deals;
mod error;
mod host;
mod leads;
mod leads_more;
mod maps;
mod ports;
mod runtime;
mod settings;
mod templates;

use nsh_core::prelude::*;

/// The crate's exports (`nsh.json` `"rust"`).
#[native_module("rustsale/native")]
fn native(registry: &mut Registry) {
    registry
        .module::<host::Host>()
        .module::<leads::Leads>()
        .module::<leads_more::LeadsMore>()
        .module::<deals::Deals>()
        .module::<campaigns::Campaigns>()
        .module::<categories::Categories>()
        .module::<conversations::Conversations>()
        .module::<conversation_actions::ConversationActions>()
        .module::<maps::GoogleMaps>()
        .module::<templates::Templates>()
        .module::<settings::Settings>();
}

#[cfg(test)]
mod tests {
    use nsh_core::element::declarations;

    #[test]
    fn every_export_has_a_doc_comment() {
        let missing = super::registry().undocumented();
        assert!(missing.is_empty(), "undocumented exports: {missing:?}");
    }

    #[test]
    fn legacy_commands_are_not_exported_to_tsx() {
        let text = declarations(&super::registry(), &[]);
        for legacy in [
            "send_zalo_campaign",
            "sendZaloCampaign",
            "get_campaign_leads",
            "campaignLeads",
            "start_google_maps_scrape",
            "startGoogleMapsScrape",
            "stop_google_maps_scrape",
            "stopGoogleMapsScrape",
            "verify_all_zalo_sessions",
            "verifyAllZaloSessions",
        ] {
            assert!(!text.contains(legacy), "legacy command leaked: {legacy}");
        }
    }
}
