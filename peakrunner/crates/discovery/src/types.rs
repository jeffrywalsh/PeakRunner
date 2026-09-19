use serde::{Deserialize, Serialize};

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
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MatchStatus {
    pub name: String, pub map: String, pub players: u32, pub max_players: u32,
    pub tick: u64, pub round: u32, pub phase: String, pub score: [u32; 2],
    pub time_left: f32, pub password_required: bool,
}
#[derive(Serialize, Deserialize)]
pub struct PublicDirectory { pub protocol: String, pub servers: Vec<ServerAdvert> }

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum StatusRequest { Status }
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum StatusReply { Status { status: MatchStatus } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_cannot_decode_or_issue_gameplay_commands() {
        assert_eq!(serde_json::to_string(&StatusRequest::Status).unwrap(), r#"{"op":"status"}"#);
        for payload in [r#"{"op":"input","command":{}}"#, r#"{"op":"hello","name":"x"}"#] {
            assert!(serde_json::from_str::<StatusRequest>(payload).is_err());
        }
        assert!(serde_json::from_str::<StatusReply>(r#"{"op":"snapshot","state":{}}"#).is_err());
    }
}
