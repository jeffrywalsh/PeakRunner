//! Gameplay wire protocol shared only by client and match server.
use serde::{Deserialize, Serialize};
use peakrunner_core::sim::{Command, Snapshot};
pub use peakrunner_discovery::{PROTOCOL, ServerAdvert};
pub mod packets;
pub const GAME_VERSION: &str = "0.1.0-private.20260921.1";

/// A different local map must never silently join a server simulating another
/// layout. Keep directory discovery independent of gameplay/map assets.
pub fn game_protocol() -> String {
    use peakrunner_core::{map_pack, terrain::MapId};
    match map_pack::active() {
        // maps6: every rotation slot is an embedded original map. Tower Complex,
        // Cairnhold, Frostline and Dustreach hold the four former clone slots;
        // no private reference packs remain.
        // muzzle1: player shots are clamped to the shooter's side of walls.
        // equipment4: sensors and fixed turrets carry generator-powered shields,
        // and equipment snapshots include shield state.
        Some(pack) => format!("{PROTOCOL}:equipment4:blast3:chat2:names1:ping1:fov1:muzzle1:maps6:{}:{}:{}:{}:{}",pack.fingerprint,
            map_pack::on(MapId::BroadsideClone).expect("Tower Complex").fingerprint,
            map_pack::on(MapId::StonehengeClone).expect("Cairnhold").fingerprint,
            map_pack::on(MapId::SnowblindClone).expect("Frostline").fingerprint,
            map_pack::on(MapId::DesertOfDeathClone).expect("Dustreach").fingerprint),
        None => format!("{PROTOCOL}:equipment4:blast3:chat2:names1:ping1:fov1:muzzle1"),
    }
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
    fn equipment_shield_state_survives_the_wire() {
        let mut m=peakrunner_core::sim::Match::new(peakrunner_core::terrain::MapId::Raindance);
        assert!(!m.world.equipment.is_empty(),"Raindance has equipment");
        m.world.equipment[0].shield=123.5;m.world.equipment[0].health=40.;
        let text=serde_json::to_string(&super::ServerMsg::Snapshot{state:m.snapshot()}).unwrap();
        let super::ServerMsg::Snapshot{state}=serde_json::from_str(&text).unwrap() else {panic!()};
        assert_eq!((state.equipment[0].shield,state.equipment[0].health),(123.5,40.));
        assert!(!text.contains("since_hit"),"the server-side regen timer is not sent");
    }
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
