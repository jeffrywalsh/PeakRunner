//! QA-only, env-gated staging for screenshots, like `QA_FLYCAM`. Applied to the
//! client's view of the world every frame; the server never sees it. Absent the
//! env vars this does nothing.
//!
//! `QA_EQUIPMENT="3=120/0,5=250/200,gen0=down"`: set equipment `index=health/shield`;
//! `genN=down` destroys team N's generators so its circuit loses power.
//! `QA_PLAYERS="Ridge:0:1010,245,860;Echo:1:1040,245,870"`: stage extra players
//! (`name:team:x,y,z`, team 0 = Ember) for name-tag captures.
use glam::Vec3;
use peakrunner_core::sim::{Team, World};
use std::sync::OnceLock;

struct Overrides { equipment: Vec<(usize, f32, f32)>, down: Vec<u8>, players: Vec<(String, Team, Vec3)> }

fn parse() -> Option<&'static Overrides> {
    static CELL: OnceLock<Option<Overrides>> = OnceLock::new();
    CELL.get_or_init(|| {
        let eq = std::env::var("QA_EQUIPMENT").ok();
        let pl = std::env::var("QA_PLAYERS").ok();
        if eq.is_none() && pl.is_none() { return None; }
        let mut o = Overrides { equipment: Vec::new(), down: Vec::new(), players: Vec::new() };
        for item in eq.iter().flat_map(|s| s.split(',')) {
            let Some((key, value)) = item.split_once('=') else { continue; };
            if let Some(team) = key.strip_prefix("gen") { if let Ok(t) = team.parse() { o.down.push(t); } continue; }
            let (Ok(i), Some((h, s))) = (key.trim().parse(), value.split_once('/')) else { continue; };
            if let (Ok(h), Ok(s)) = (h.parse(), s.parse()) { o.equipment.push((i, h, s)); }
        }
        for item in pl.iter().flat_map(|s| s.split(';')) {
            let parts: Vec<_> = item.split(':').collect();
            if parts.len() != 3 { continue; }
            let xyz: Vec<f32> = parts[2].split(',').filter_map(|v| v.trim().parse().ok()).collect();
            if xyz.len() != 3 { continue; }
            let team = if parts[1] == "0" { Team::Ember } else { Team::Glacier };
            o.players.push((parts[0].to_string(), team, Vec3::new(xyz[0], xyz[1], xyz[2])));
        }
        Some(o)
    }).as_ref()
}

pub fn apply(world: &mut World) {
    let Some(o) = parse() else { return; };
    let defs = peakrunner_core::equipment::definitions(world.map);
    for &team in &o.down {
        for (d, s) in defs.iter().zip(&mut world.equipment) {
            if d.kind == peakrunner_core::equipment::Kind::Generator && d.team == team { s.health = 0.0; }
        }
    }
    peakrunner_core::equipment::power(defs, &mut world.equipment);
    for &(i, h, s) in &o.equipment {
        if let Some(state) = world.equipment.get_mut(i) { state.health = h; if state.powered { state.shield = s; } }
    }
    let Some(template) = world.players.get(world.player_id).cloned() else { return; };
    for (k, (name, team, pos)) in o.players.iter().enumerate() {
        let id = 60_000 + k as u32;
        let slot = match world.players.iter().position(|p| p.net_id == id) {
            Some(i) => i,
            None => { world.players.push(template.clone()); world.players.len() - 1 }
        };
        let p = &mut world.players[slot];
        p.net_id = id; p.name = name.clone(); p.team = *team; p.pos = *pos; p.vel = Vec3::ZERO; p.alive = true;
    }
}
