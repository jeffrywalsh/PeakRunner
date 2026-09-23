//! Gameplay wire protocol shared only by client and match server.
use serde::{Deserialize, Serialize};
use peakrunner_core::sim::{Command, Snapshot};
pub use peakrunner_discovery::{PROTOCOL, ServerAdvert};
pub mod packets;
pub const GAME_VERSION: &str = "0.1.0-private.20260921.1";

/// A different local map must never silently join a server simulating another
/// layout. Keep directory discovery independent of gameplay/map assets.
pub fn game_protocol() -> String {
    let mut protocol = match peakrunner_core::map_pack::active() {
        // maps4: the broadside-clone slot is the embedded Tower Complex and the
        // stonehenge-clone slot is the embedded Cairnhold.
        // muzzle1: player shots are clamped to the shooter's side of walls.
        Some(pack) => format!("{PROTOCOL}:equipment3:blast3:chat2:names1:ping1:fov1:muzzle1:maps4:{}:{}:{}:{}",pack.fingerprint,
            peakrunner_core::map_pack::on(peakrunner_core::terrain::MapId::Skybreak).expect("Skybreak").fingerprint,
            peakrunner_core::map_pack::on(peakrunner_core::terrain::MapId::BroadsideClone).expect("Tower Complex").fingerprint,
            peakrunner_core::map_pack::on(peakrunner_core::terrain::MapId::StonehengeClone).expect("Cairnhold").fingerprint),
        None => format!("{PROTOCOL}:equipment3:blast3:chat2:names1:ping1:fov1:muzzle1"),
    };
    // Installed reference packs join the compatibility marker. A missing or
    // changed pack is rejected before a player slot is granted.
    use peakrunner_core::{map_pack, terrain::MapId};
    for map in [MapId::SnowblindClone, MapId::DesertOfDeathClone] {
        if let Some(pack) = map_pack::on(map) {
            protocol.push_str(&format!(":private1:{}:{}", map.key(), pack.fingerprint));
        }
    }
    protocol
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMsg {
    Hello { name: String, protocol: String, password: String },
    Input { command: Command },
    Chat { text: String },
    TeamChat { text: String },
    Rename { name: String },
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

#[cfg(test)]
mod tests {
    #[test]
    fn chat_cannot_supply_identity_or_frag_outcomes() {
        for text in [r#"{"op":"chat","text":"hello","sender":"Admin"}"#,
                     r#"{"op":"team_chat","text":"secret","team":"Glacier"}"#,
                     r#"{"op":"rename","name":"Admin","player_id":2}"#,
                     r#"{"op":"frag","killer":"Alice","victim":"Bob"}"#] {
            assert!(serde_json::from_str::<super::ClientMsg>(text).is_err());
        }
    }
}
