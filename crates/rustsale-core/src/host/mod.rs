//! Host abstraction: the seam between the core and whatever shell embeds it.
//!
//! * [`ports`] — traits the shell implements (events, notifications, dialogs,
//!   window, tray, opener, updater, autostart).
//! * [`AppHandle`] / [`State`] — typed state registry + event emission handed
//!   to every command.
//! * [`mock`] — in-memory ports for tests (`test-support` feature).

mod channel;
mod error;
mod handle;
pub mod ports;

#[cfg(any(test, feature = "test-support"))]
pub mod mock;

pub use channel::Channel;
pub use error::{HostError, PortError};
pub use handle::{AppHandle, State};
pub use ports::{
    AutostartPort, DesktopNotifier, EventSink, FileDialog, FileFilter, NoopPorts, OpenerPort,
    PickFileRequest, Ports, SaveFileRequest, TrayPort, UpdateInfo, UpdaterPort, WindowPort,
};
