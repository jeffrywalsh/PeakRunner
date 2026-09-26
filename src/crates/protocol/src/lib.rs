//! Gameplay wire protocol shared only by client and match server.
use serde::{Deserialize, Serialize};
use peakrunner_core::sim::{Command, Snapshot};
pub use peakrunner_discovery::{PROTOCOL, ServerAdvert};
pub mod packets;
pub const GAME_VERSION: &str = "0.1.0-playtest.20260926.1";

/// A different local map must never silently join a server simulating another
/// layout. Keep directory discovery independent of gameplay/map assets.
pub fn game_protocol() -> String {
    use peakrunner_core::{map_pack, terrain::MapId};
    match map_pack::active() {
        // maps10: Reefbreak joins the CTF maps.
        // maps9: Ozarktic Blast joins the CTF maps.
        // maps8: Highgoal, the raised-goal Football arena, joins Longfield.
        // maps7: Longfield, the Football stadium, joins the five CTF maps.
        // maps6: every rotation slot is an embedded original map. Tower Complex,
        // Cairnhold, Frostline and Dustreach hold the four former clone slots;
        // no private reference packs remain.
        // muzzle1: player shots are clamped to the shooter's side of walls.
        // equipment5: generator-powered shields regenerate continuously, destroyed
        // equipment stays offline until repaired past half hull (sent in
        // snapshots), and generators explode visibly. kit1: repair kits (now the repair tool, loadout1).
        // cnh1: control points (state in snapshots, drain field in the shared
        // sim) and the Capture & Hold mode.
        // arc1: fixed turrets face the field with a limited field of fire beyond
        // 15 m (equipment::Definition::in_arc), so open doorways need no baffles.
        // water1: manifest water volumes slow movement and projectiles in the
        // shared sim (crate::water).
        // bump1: enemies collide and exchange momentum, with light damage
        // (sim::bump). ball1: the Football mode and its ball in snapshots.
        // kill1: the Ctrl+K suicide intent in commands.
        // loadout1: light and heavy armor, limited ammo, the mortar, the repair
        // tool (was the kit), inventory purchases and deployables (sim::loadout,
        // sim::deploy).
        // throw1: hand grenades and mines thrown with G and M (sim::throwables).
        // loot2: death packs roll 1-5 discs and third-weapon rounds, all chaingun rounds and 1+ hand grenades; pickers take only matching ammo (sim::loadout).
        // dm1: Deathmatch and Team Deathmatch, with per-round conditions in snapshots (sim::deathmatch).
        // rifle1: the rifle slot bought at stations: laser (light) and railgun (heavy) (sim::rifles).
        // arc2: outside the field of fire, turrets still engage anyone under open sky.
        Some(pack) => format!("{PROTOCOL}:equipment5:blast3:chat2:names1:ping1:fov1:muzzle1:kit1:cnh1:arc2:water1:bump1:ball1:kill1:loadout1:throw1:rifle1:dm1:loot2:maps10:{}:{}:{}:{}:{}:{}:{}:{}:{}",pack.fingerprint,
            map_pack::on(MapId::BroadsideClone).expect("Tower Complex").fingerprint,
            map_pack::on(MapId::StonehengeClone).expect("Cairnhold").fingerprint,
            map_pack::on(MapId::SnowblindClone).expect("Frostline").fingerprint,
            map_pack::on(MapId::DesertOfDeathClone).expect("Dustreach").fingerprint,
            map_pack::on(MapId::Longfield).expect("Longfield").fingerprint,
            map_pack::on(MapId::Highgoal).expect("Highgoal").fingerprint,
            map_pack::on(MapId::OzarkticBlast).expect("Ozarktic Blast").fingerprint,
            map_pack::on(MapId::Reefbreak).expect("Reefbreak").fingerprint),
        None => format!("{PROTOCOL}:equipment5:blast3:chat2:names1:ping1:fov1:muzzle1:kit1:cnh1:arc2:water1:bump1:ball1:kill1:loadout1:throw1:rifle1:dm1:loot2"),
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
        m.world.equipment[1].offline=true;
        let text=serde_json::to_string(&super::ServerMsg::Snapshot{state:m.snapshot()}).unwrap();
        let super::ServerMsg::Snapshot{state}=serde_json::from_str(&text).unwrap() else {panic!()};
        assert!(state.equipment[1].offline,"the offline state survives the wire");
    }
    #[test]
    fn loadout_intents_ammo_armor_pack_and_deployables_survive_the_wire() {
        use peakrunner_core::sim::{loadout, deploy, ArmorClass};
        // Older clients called the repair intent `kit`; it still parses.
        let old: peakrunner_core::sim::Command = serde_json::from_str(
            r#"{"seq":1,"move_x":0,"move_z":0,"yaw":0,"pitch":0,"jump":false,"jet":false,"fire":false,"kit":true,"weapon":0}"#).unwrap();
        assert!(old.repair);
        let command=peakrunner_core::sim::Command{seq:1,repair:true,buy:loadout::BUY_HEAVY,deploy:true,..Default::default()};
        let text=serde_json::to_string(&command).unwrap();
        let back: peakrunner_core::sim::Command = serde_json::from_str(&text).unwrap();
        assert!(back.repair && back.deploy && back.buy==loadout::BUY_HEAVY);
        let mut m=peakrunner_core::sim::Match::new(peakrunner_core::terrain::MapId::Raindance);
        let Some(slot)=m.join(7,"kit") else {panic!()};
        let v=m.world.players[slot].pos;
        let p=&mut m.world.players[slot];
        p.armor=ArmorClass::Heavy;p.ammo=[3,40,2,7];p.rifle=true;p.pack=Some(deploy::DeployKind::Field);p.repair_beam=Some(v);
        m.world.deployables.push(deploy::Deployable{kind:deploy::DeployKind::Wall,team:peakrunner_core::sim::Team::Glacier,
            pos:v,yaw:0.5,health:320.,cooldown:0.,aim:v});
        let text=serde_json::to_string(&super::ServerMsg::Snapshot{state:m.snapshot()}).unwrap();
        let super::ServerMsg::Snapshot{state}=serde_json::from_str(&text).unwrap() else {panic!()};
        let p=&state.players[slot];
        assert!(p.rifle);assert_eq!((p.armor,p.ammo,p.pack,p.repair_beam),(ArmorClass::Heavy,[3,40,2,7],Some(deploy::DeployKind::Field),Some(v)));
        assert_eq!(state.deployables,m.world.deployables);
    }

    #[test]
    fn control_points_and_mode_survive_the_wire() {
        use peakrunner_core::{control, map_catalog::SupportedMode};
        let mut m=peakrunner_core::sim::Match::new(peakrunner_core::terrain::MapId::Raindance);
        m.world.set_mode(SupportedMode::CaptureAndHold);
        m.world.set_control_points(vec![control::Definition{id:"beacon".into(),name:"Beacon".into(),pos:[1.,2.,3.],
            radius:12.,ctf_active:false,drain:Some(control::Drain{radius:60.,rate:10.})}]);
        m.world.points[0].owner=Some(1);m.world.points[0].progress=0.4;m.world.points[0].capturing=Some(0);m.world.points[0].contested=true;
        m.world.score=[120,45];
        let text=serde_json::to_string(&super::ServerMsg::Snapshot{state:m.snapshot()}).unwrap();
        let super::ServerMsg::Snapshot{state}=serde_json::from_str(&text).unwrap() else {panic!()};
        assert_eq!(state.mode,SupportedMode::CaptureAndHold);
        assert_eq!(state.points,m.world.points);
        assert_eq!(state.score,[120,45]);
        assert!(super::game_protocol().contains(":cnh1"));
        assert!(super::game_protocol().contains(":arc2"));
        assert!(super::game_protocol().contains(":water1:bump1:ball1"));
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
