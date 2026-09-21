//! Client-only match connection and discovery API. No server or directory runtime.
mod client;
mod transport;
mod quic;
use peakrunner_protocol as proto;
use peakrunner_discovery::{lan as directory, wire};
pub use client::{browse, connect, connect_private, Lobby, Session};
pub use peakrunner_discovery::{ServerAdvert, PROTOCOL};
