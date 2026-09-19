use serde::{Deserialize, Serialize};
use peakrunner_core::sim::{Command, Snapshot};

pub const PROTOCOL: &str = "peakrunner-4";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerAdvert {
    pub id: String, pub name: String, pub host: String, pub port: u16,
    pub players: u32, pub max_players: u32, pub map: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum DirRequest {
    Register { name: String, host: String, port: u16, players: u32, max_players: u32, map: String },
    Heartbeat { id: String, token: String, players: u32 },
    List,
    Unregister { id: String, token: String },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DirResponse {
    Registered { id: String, token: String }, Servers { servers: Vec<ServerAdvert> },
    Ok, Error { message: String },
}
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
    Status { status: crate::public::MatchStatus },
}
