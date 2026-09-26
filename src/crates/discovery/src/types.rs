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
    #[serde(default)]
    pub game_version: String,
    #[serde(default)]
    pub game_protocol: String,
    #[serde(default)]
    pub roster: Vec<PlayerStats>,
}
/// Public scoreboard only: no IP addresses, private chat, positions or credentials.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerStats {
    pub id: u32, pub name: String, pub team: String, pub frags: u32, pub deaths: u32,
}
#[derive(Serialize, Deserialize)]
pub struct PublicDirectory { pub protocol: String, pub servers: Vec<ServerAdvert> }

/// A hosted server telling the public directory where it is (`POST
/// /announce`). Only the address: the directory learns everything else by
/// querying the server's status there, and lists it only after that works.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Announce { pub url: String }

/// Stable directory ID for an announced address: `play.peakrunner.net:7778`
/// becomes `play-peakrunner-net-7778`. None for anything that isn't a plain
/// quic://host[:port] URL.
pub fn announce_id(url: &str) -> Option<String> {
    let u = crate::quic::endpoint_url(url).ok()?;
    let host = u.host_str()?.trim_matches(['[', ']']).to_ascii_lowercase();
    if host.is_empty() || host.len() > 200 { return None; }
    let clean: String = host.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    Some(format!("{clean}-{}", u.port().unwrap_or(7777)))
}

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
    fn announce_ids_are_stable_and_reject_other_schemes() {
        assert_eq!(announce_id("quic://play.peakrunner.net:7778").as_deref(), Some("play-peakrunner-net-7778"));
        assert_eq!(announce_id("quic://Play.PeakRunner.net").as_deref(), Some("play-peakrunner-net-7777"));
        assert!(announce_id("https://play.peakrunner.net:7778").is_none());
        assert!(serde_json::from_str::<Announce>(r#"{"url":"quic://a:1","name":"x"}"#).is_err());
    }

    #[test]
    fn discovery_cannot_decode_or_issue_gameplay_commands() {
        assert_eq!(serde_json::to_string(&StatusRequest::Status).unwrap(), r#"{"op":"status"}"#);
        for payload in [r#"{"op":"input","command":{}}"#, r#"{"op":"hello","name":"x"}"#] {
            assert!(serde_json::from_str::<StatusRequest>(payload).is_err());
        }
        assert!(serde_json::from_str::<StatusReply>(r#"{"op":"snapshot","state":{}}"#).is_err());
    }
}
