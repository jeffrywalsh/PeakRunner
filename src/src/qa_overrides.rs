//! QA-only, env-gated staging for screenshots, like `QA_FLYCAM`. Applied to the
//! client's view of the world every frame; the server never sees it. Absent the
//! env vars this does nothing.
//!
//! `QA_EQUIPMENT="3=120/0,5=250/200,gen0=down"`: set equipment `index=health/shield`;
//! `genN=down` destroys team N's generators so its circuit loses power.
//! `QA_PLAYERS="Ridge:0:1010,245,860;Echo:1:1040,245,870"`: stage extra players
//! (`name:team:x,y,z`, team 0 = Ember) for name-tag captures. A trailing `*` on
//! the name (`Budster*:0:...`) hands that player the enemy flag once
//! `QA_CARRY_AT` seconds (default 6) have passed, so the pickup announcement
//! fires through the normal state-change path before the 8 s capture.
//! `genN=boom` destroys team N's generators `QA_BOOM_AT` seconds in (default
//! 5): the hull drops to zero, the generator blast plays, and the wreck and
//! announcement follow through the normal paths. `QA_KIT=heal` shows the local
//! player mid-way through a repair kit.
use glam::Vec3;
use peakrunner_core::sim::{Team, World};
use std::sync::OnceLock;

struct Overrides { equipment: Vec<(usize, f32, f32)>, down: Vec<u8>, boom: Vec<u8>, boom_at: f32, kit_heal: bool, players: Vec<(String, Team, Vec3, bool)>, started: std::time::Instant, carry_at: f32 }

fn parse() -> Option<&'static Overrides> {
    static CELL: OnceLock<Option<Overrides>> = OnceLock::new();
    CELL.get_or_init(|| {
        let eq = std::env::var("QA_EQUIPMENT").ok();
        let pl = std::env::var("QA_PLAYERS").ok();
        let kit_heal = std::env::var("QA_KIT").is_ok_and(|v| v == "heal");
        if eq.is_none() && pl.is_none() && !kit_heal { return None; }
        let boom_at = std::env::var("QA_BOOM_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(5.0);
        let carry_at = std::env::var("QA_CARRY_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0);
        let mut o = Overrides { equipment: Vec::new(), down: Vec::new(), boom: Vec::new(), boom_at, kit_heal, players: Vec::new(), started: std::time::Instant::now(), carry_at };
        for item in eq.iter().flat_map(|s| s.split(',')) {
            let Some((key, value)) = item.split_once('=') else { continue; };
            if let Some(team) = key.strip_prefix("gen") {
                if let Ok(t) = team.parse() { if value == "boom" { o.boom.push(t); } else { o.down.push(t); } }
                continue;
            }
            let (Ok(i), Some((h, s))) = (key.trim().parse(), value.split_once('/')) else { continue; };
            if let (Ok(h), Ok(s)) = (h.parse(), s.parse()) { o.equipment.push((i, h, s)); }
        }
        for item in pl.iter().flat_map(|s| s.split(';')) {
            let parts: Vec<_> = item.split(':').collect();
            if parts.len() != 3 { continue; }
            let xyz: Vec<f32> = parts[2].split(',').filter_map(|v| v.trim().parse().ok()).collect();
            if xyz.len() != 3 { continue; }
            let team = if parts[1] == "0" { Team::Ember } else { Team::Glacier };
            let (name, carries) = match parts[0].strip_suffix('*') { Some(n) => (n, true), None => (parts[0], false) };
            o.players.push((name.to_string(), team, Vec3::new(xyz[0], xyz[1], xyz[2]), carries));
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
    let since_boom = o.started.elapsed().as_secs_f32() - o.boom_at;
    if since_boom >= 0.0 {
        for &team in &o.boom {
            for (d, s) in defs.iter().zip(&mut world.equipment) {
                if d.kind != peakrunner_core::equipment::Kind::Generator || d.team != team { continue; }
                s.health = 0.0; s.offline = true;
                // Snapshots replace the explosion list, so keep the blast alive
                // at its true age until it has played out.
                if since_boom < 0.55 && !world.explosions.iter().any(|e| e.kind == 4) {
                    world.explosions.push(peakrunner_core::sim::Explosion { pos: d.pos() + Vec3::Y * 2.0, age: since_boom,
                        max_r: peakrunner_core::sim::GENERATOR_BLAST_RADIUS, kind: 4 });
                }
            }
        }
    }
    peakrunner_core::equipment::power(defs, &mut world.equipment);
    if o.kit_heal {
        if let Some(p) = world.players.get_mut(world.player_id) { p.kits = 0; p.kit_heal = 35.0; p.health = 55.0; }
    }
    for &(i, h, s) in &o.equipment {
        if let Some(state) = world.equipment.get_mut(i) { state.health = h; if state.powered { state.shield = s; } }
    }
    let Some(template) = world.players.get(world.player_id).cloned() else { return; };
    let carry_now = o.started.elapsed().as_secs_f32() >= o.carry_at;
    for (k, (name, team, pos, carries)) in o.players.iter().enumerate() {
        let id = 60_000 + k as u32;
        let slot = match world.players.iter().position(|p| p.net_id == id) {
            Some(i) => i,
            None => { world.players.push(template.clone()); world.players.len() - 1 }
        };
        let p = &mut world.players[slot];
        p.net_id = id; p.name = name.clone(); p.team = *team; p.pos = *pos; p.vel = Vec3::ZERO; p.alive = true;
        if *carries && carry_now {
            let enemy = team.other();
            p.carrying = Some(enemy);
            let f = &mut world.flags[enemy.idx()];
            f.carrier = Some(slot); f.pos = *pos + Vec3::Y * 2.2;
        }
    }
}
