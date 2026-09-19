//! Gameplay wire protocol shared only by client and match server.
use serde::{Deserialize, Serialize};
use peakrunner_core::sim::{Command, Snapshot};
pub use peakrunner_discovery::{PROTOCOL, ServerAdvert};
pub mod packets;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMsg {
    Hello { name: String, protocol: String, password: String },
    Input { command: Command },
    Leave,
    Status,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome { player_id: u32, match_name: String },
    Snapshot { state: Snapshot },
    Reject { message: String },
    Status { status: peakrunner_discovery::MatchStatus },
}
