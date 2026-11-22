//! RustSale v3 native module: what `App.tsx` imports from `rustsale/native`.
//! The only place TSX reaches `rustsale-core`.

mod conversations;
mod error;
mod host;
mod leads;
mod maps;
mod ports;
mod runtime;
mod settings;

use nsh_core::prelude::*;

/// The crate's exports (`nsh.json` `"rust"`).
#[native_module("rustsale/native")]
fn native(registry: &mut Registry) {
    registry
        .module::<host::Host>()
        .module::<leads::Leads>()
        .module::<conversations::Conversations>()
        .module::<maps::GoogleMaps>()
        .module::<settings::Settings>();
}
