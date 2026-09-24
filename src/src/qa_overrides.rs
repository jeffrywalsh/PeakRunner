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
//! An optional fourth field stages a pose for animation captures: `dive`
//! (airborne, pitched down, firing), `land@T` (falling until `T` seconds,
//! then grounded: a hard landing) or `switch@T` (disc launcher until `T`,
//! then chaingun). The landing and switch play through the client's normal
//! frame-to-frame animation tracker. `splash@T` treats `y` as a water
//! surface: the player skis along it 2 m up until `T` seconds, then drops
//! in at 22 m/s, so the entry splash plays through the normal effects path.
//! `QA_POINTS="Beacon:1010,120,860:drain;West:900,100,1000"` stages temporary
//! control points (`name:x,y,z[:drain][:rRADIUS]`) on any map, `QA_POINT_STATE="0=1/0.4/0/c"`
//! forces point `index=owner/progress/capturing[/c for contested]` (`-` = none),
//! and `QA_MODE=cnh` with `QA_SCORE=120,45` shows Capture & Hold on the client.
//! `QA_ROUNDS="1:1010,246,870:0,0,-300;0:..."` stages projectiles in flight
//! (`kind:x,y,z:vx,vy,vz`, kind 0 disc, 1 bullet, 2 grenade, 3 plasma). Each
//! loops along its velocity over 0.25 s so incoming fire can be captured.
//! `QA_BLASTS="0:1010,244,860@3;2:1016,244,866@3.2"` stages one explosion each
//! (`kind:x,y,z@seconds`) through the normal effects path, scorch included.
use glam::Vec3;
use peakrunner_core::sim::{Team, World};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
enum StagedPose { Stand, Dive, Land(f32), Switch(f32), Splash(f32) }

struct Overrides { equipment: Vec<(usize, f32, f32)>, down: Vec<u8>, boom: Vec<u8>, boom_at: f32, kit_heal: bool, players: Vec<(String, Team, Vec3, bool, StagedPose)>, started: std::time::Instant, carry_at: f32, rounds: Vec<(u8, Vec3, Vec3)>, blasts: Vec<(u8, Vec3, f32)> }

/// Marks QA-staged rounds so each frame replaces them rather than piling up.
const STAGED_ROUND_SPIN: f32 = -4242.0;

fn parse() -> Option<&'static Overrides> {
    static CELL: OnceLock<Option<Overrides>> = OnceLock::new();
    CELL.get_or_init(|| {
        let eq = std::env::var("QA_EQUIPMENT").ok();
        let pl = std::env::var("QA_PLAYERS").ok();
        let kit_heal = std::env::var("QA_KIT").is_ok_and(|v| v == "heal");
        let rd = std::env::var("QA_ROUNDS").ok();
        let bl = std::env::var("QA_BLASTS").ok();
        if eq.is_none() && pl.is_none() && !kit_heal && rd.is_none() && bl.is_none() { return None; }
        let boom_at = std::env::var("QA_BOOM_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(5.0);
        let carry_at = std::env::var("QA_CARRY_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0);
        let mut o = Overrides { equipment: Vec::new(), down: Vec::new(), boom: Vec::new(), boom_at, kit_heal, players: Vec::new(), started: std::time::Instant::now(), carry_at, rounds: Vec::new(), blasts: Vec::new() };
        let vec3 = |s: &str| -> Option<Vec3> {
            let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
        };
        for item in rd.iter().flat_map(|s| s.split(';')) {
            let parts: Vec<_> = item.split(':').collect();
            if parts.len() != 3 { continue; }
            if let (Ok(kind), Some(pos), Some(vel)) = (parts[0].trim().parse(), vec3(parts[1]), vec3(parts[2])) {
                o.rounds.push((kind, pos, vel));
            }
        }
        for item in bl.iter().flat_map(|s| s.split(';')) {
            let Some((head, at)) = item.split_once('@') else { continue; };
            let Some((kind, xyz)) = head.split_once(':') else { continue; };
            if let (Ok(kind), Some(pos), Ok(at)) = (kind.trim().parse(), vec3(xyz), at.trim().parse()) {
                o.blasts.push((kind, pos, at));
            }
        }
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
            if parts.len() != 3 && parts.len() != 4 { continue; }
            let xyz: Vec<f32> = parts[2].split(',').filter_map(|v| v.trim().parse().ok()).collect();
            if xyz.len() != 3 { continue; }
            let team = if parts[1] == "0" { Team::Ember } else { Team::Glacier };
            let (name, carries) = match parts[0].strip_suffix('*') { Some(n) => (n, true), None => (parts[0], false) };
            let at = |tag: &str| parts[3].strip_prefix(tag).and_then(|t| t.parse().ok());
            let pose = match parts.get(3) {
                None => StagedPose::Stand,
                Some(&"dive") => StagedPose::Dive,
                Some(_) => at("land@").map(StagedPose::Land).or_else(|| at("switch@").map(StagedPose::Switch))
                    .or_else(|| at("splash@").map(StagedPose::Splash)).unwrap_or(StagedPose::Stand),
            };
            o.players.push((name.to_string(), team, Vec3::new(xyz[0], xyz[1], xyz[2]), carries, pose));
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
    for &(kind, pos, at) in &o.blasts {
        let age = o.started.elapsed().as_secs_f32() - at;
        if (0.0..0.55).contains(&age) && !world.explosions.iter().any(|e| e.kind == kind && e.pos.distance(pos) < 0.1) {
            world.explosions.push(peakrunner_core::sim::Explosion { pos, age, max_r: 9.0, kind });
        }
    }
    if !o.rounds.is_empty() {
        world.discs.retain(|d| d.spin != STAGED_ROUND_SPIN);
        let phase = (o.started.elapsed().as_secs_f32() % 0.25) - 0.125;
        for &(kind, pos, vel) in &o.rounds {
            let life = match kind { 0 => 4.0, 1 => 0.9, 2 => 1.5, _ => 2.0 };
            world.discs.push(peakrunner_core::sim::Disc { pos: pos + vel * phase, vel, team: Team::Glacier,
                owner: 0, life, kind, spin: STAGED_ROUND_SPIN });
        }
    }
    let Some(template) = world.players.get(world.player_id).cloned() else { return; };
    let carry_now = o.started.elapsed().as_secs_f32() >= o.carry_at;
    let now = o.started.elapsed().as_secs_f32();
    for (k, (name, team, pos, carries, pose)) in o.players.iter().enumerate() {
        let id = 60_000 + k as u32;
        let slot = match world.players.iter().position(|p| p.net_id == id) {
            Some(i) => i,
            None => { world.players.push(template.clone()); world.players.len() - 1 }
        };
        let p = &mut world.players[slot];
        p.net_id = id; p.name = name.clone(); p.team = *team; p.pos = *pos; p.vel = Vec3::ZERO; p.alive = true;
        p.on_ground = true; p.jetting = false; p.skiing = false;
        match *pose {
            StagedPose::Stand => {}
            StagedPose::Dive => {
                p.on_ground = false; p.vel = Vec3::new(0.0, -24.0, -12.0); p.pitch = -1.0; p.weapon = 0;
                p.cooldown = peakrunner_core::sim::weapon_reload(0) - 0.03;
            }
            StagedPose::Land(t) => if now < t { p.on_ground = false; p.vel = Vec3::new(0.0, -28.0, 0.0); },
            StagedPose::Switch(t) => p.weapon = if now < t { 0 } else { 1 },
            StagedPose::Splash(t) => {
                p.on_ground = false; p.skiing = true; p.vel = Vec3::new(22.0, -2.0, 0.0);
                p.pos.y += if now < t { 2.0 } else { -0.6 };
            }
        }
        if *carries && carry_now {
            let enemy = team.other();
            p.carrying = Some(enemy);
            let f = &mut world.flags[enemy.idx()];
            f.carrier = Some(slot); f.pos = *pos + Vec3::Y * 2.2;
        }
    }
}

fn qa_points() -> Option<&'static Vec<peakrunner_core::control::Definition>> {
    static CELL: OnceLock<Option<Vec<peakrunner_core::control::Definition>>> = OnceLock::new();
    CELL.get_or_init(|| {
        let raw = std::env::var("QA_POINTS").ok()?;
        let defs: Vec<_> = raw.split(';').enumerate().filter_map(|(i, item)| {
            let parts: Vec<_> = item.split(':').collect();
            let xyz: Vec<f32> = parts.get(1)?.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            if xyz.len() != 3 { return None; }
            let drain = parts.iter().any(|d| *d == "drain").then(|| peakrunner_core::control::Drain {
                radius: peakrunner_core::control::DEFAULT_DRAIN_RADIUS, rate: peakrunner_core::control::DEFAULT_DRAIN_RATE });
            let radius = parts.iter().find_map(|p| p.strip_prefix('r').and_then(|r| r.parse().ok()))
                .unwrap_or(peakrunner_core::control::DEFAULT_RADIUS);
            Some(peakrunner_core::control::Definition { id: format!("qa-{i}"), name: parts[0].to_string(),
                pos: [xyz[0], xyz[1], xyz[2]], radius, ctf_active: true, drain })
        }).collect();
        (!defs.is_empty()).then_some(defs)
    }).as_ref()
}

/// Number of QA-staged control points, if any are staged.
pub fn staged_point_count() -> Option<usize> { qa_points().map(|d| d.len()) }

/// Put QA-staged control points into a world (offline start and client view).
pub fn stage_points(world: &mut World) {
    if let Some(defs) = qa_points() { world.set_control_points(defs.clone()); }
}

/// Client-view staging for control points, mode and score; see the module docs.
pub fn apply_points(world: &mut World) {
    let Some(defs) = qa_points() else { return; };
    if world.points.len() != defs.len() { world.set_control_points(defs.clone()); }
    if std::env::var("QA_MODE").is_ok_and(|m| m == "cnh") {
        world.mode = peakrunner_core::map_catalog::SupportedMode::CaptureAndHold;
        for p in &mut world.points { p.active = true; }
        if let Some(score) = std::env::var("QA_SCORE").ok().and_then(|s| {
            let v: Vec<u32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (v.len() == 2).then(|| [v[0], v[1]]) }) { world.score = score; }
    }
    for item in std::env::var("QA_POINT_STATE").unwrap_or_default().split(',') {
        let Some((i, v)) = item.split_once('=') else { continue; };
        let Ok(i) = i.trim().parse::<usize>() else { continue; };
        let Some(p) = world.points.get_mut(i) else { continue; };
        let f: Vec<&str> = v.split('/').collect();
        let team = |s: &str| s.parse::<u8>().ok().filter(|t| *t < 2);
        p.owner = f.first().and_then(|s| team(s));
        p.progress = f.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        p.capturing = f.get(2).and_then(|s| team(s));
        p.contested = f.get(3) == Some(&"c");
    }
}
