//! RustSale v3 native module: what `App.tsx` imports from `rustsale/native`.
//! The only place TSX reaches `rustsale-core`.

mod ai;
mod autostart;
#[cfg(test)]
#[path = "../build_env.rs"]
mod build_env;
mod campaigns;
mod categories;
pub mod config;
mod conversation_actions;
mod conversation_network;
mod conversations;
mod deals;
#[cfg(any(feature = "demo", rustsale_demo))]
mod demo;
mod desktop;
mod error;
mod facebook;
mod host;
mod import;
mod leads;
mod leads_more;
mod maps;
mod maps_more;
mod ports;
mod qr;
mod runtime;
mod settings;
mod templates;
mod util;
mod zalo;

use nsh_core::prelude::*;

/// The crate's exports (`nsh.json` `"rust"`).
#[native_module("rustsale/native")]
fn native(registry: &mut Registry) {
    registry
        .module::<host::Host>()
        .module::<ai::Ai>()
        .module::<leads::Leads>()
        .module::<leads_more::LeadsMore>()
        .module::<deals::Deals>()
        .module::<desktop::Desktop>()
        .module::<import::ImportLeads>()
        .module::<campaigns::Campaigns>()
        .module::<categories::Categories>()
        .module::<facebook::Facebook>()
        .module::<conversations::Conversations>()
        .module::<conversation_actions::ConversationActions>()
        .module::<conversation_network::ConversationNetwork>()
        .module::<maps::GoogleMaps>()
        .module::<maps_more::GoogleMapsMore>()
        .module::<templates::Templates>()
        .module::<settings::Settings>()
        .module::<util::Util>()
        .module::<zalo::Zalo>();
    // Synthetic data for tests: not exported by a release or `nsh package` build.
    #[cfg(any(feature = "demo", rustsale_demo))]
    registry.module::<demo::Demo>();
}

#[cfg(test)]
mod tests {
    use nsh_core::element::declarations;

    #[test]
    fn every_export_has_a_doc_comment() {
        let missing = super::registry().undocumented();
        assert!(missing.is_empty(), "undocumented exports: {missing:?}");
    }

    #[cfg(not(any(feature = "demo", rustsale_demo)))]
    #[test]
    fn a_release_build_has_no_demo_export() {
        let text = declarations(&super::registry(), &[]);
        assert!(
            !text.contains("Demo"),
            "Demo must not be exported without the demo feature"
        );
        assert!(
            !text.contains("seed"),
            "no seed export without the demo feature"
        );
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
