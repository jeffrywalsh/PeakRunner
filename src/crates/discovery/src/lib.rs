//! Discovery contracts and bounded networking helpers. No gameplay dependency.
pub mod lan;
pub mod http;
pub mod quic;
pub mod wire;
mod types;
pub use types::*;
