//! RustSale v3 native module: what `App.tsx` imports from `rustsale/native`.
//! The only place TSX reaches `rustsale-core`.

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
        .module::<categories::Categories>()
        .module::<conversations::Conversations>()
        .module::<conversation_actions::ConversationActions>()
        .module::<maps::GoogleMaps>()
        .module::<templates::Templates>()
        .module::<settings::Settings>();
}
