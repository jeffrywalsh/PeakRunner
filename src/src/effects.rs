//! Client-only weapon effects and first-person animation state.
//!
//! Nothing here feeds back into the simulation. It watches the world the client
//! already has (explosions, bullets, shot counters, the local player) and adds
//! lingering visuals the authoritative state does not carry: smoke that outlives
//! the 0.55 s blast record, thrown debris and casings, impact sparks, scorch
//! marks, plus the viewmodel's switch/bob/spin/heat animation. Every pool is a
//! fixed-size ring allocated once, so a busy fight never allocates per spark.
use glam::{Mat4, Vec2, Vec3};
use peakrunner_core::sim::{MatchState, World};
use peakrunner_core::terrain::{self, MapId};

use crate::drawlist::{EmitDraw, LitDraw, MeshId};

/// Hard cap on live particles of every kind (smoke, sparks, debris, casings,
/// scorch marks). The oldest is overwritten when the ring is full.
pub const MAX_PARTICLES: usize = 360;
/// Bullet impacts spawned per frame; a chaingun storm cannot flood the pool.
pub const MAX_IMPACTS_PER_FRAME: usize = 10;
pub const MAX_CASINGS: usize = 40;
pub const MAX_SCORCH: usize = 12;
/// Beyond this range explosions keep only the stateless flash layers.
pub const DETAIL_RANGE: f32 = 220.0;
const IGNORE_RANGE: f32 = 450.0;
const SWITCH_TIME: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind { Smoke, Spark, Debris, Casing, Scorch, Flash }

#[derive(Clone, Copy)]
struct Particle {
    kind: Kind,
    pos: Vec3,
    vel: Vec3,
    normal: Vec3,
    age: f32,
    life: f32,
    size: f32,
    grow: f32,
    color: [f32; 3],
    alpha: f32,
    spin: f32,
    drag: f32,
    gravity: f32,
}

impl Particle {
    const DEAD: Particle = Particle { kind: Kind::Flash, pos: Vec3::ZERO, vel: Vec3::ZERO, normal: Vec3::Y,
        age: 1.0, life: 0.0, size: 0.0, grow: 0.0, color: [0.0; 3], alpha: 0.0, spin: 0.0, drag: 0.0, gravity: 0.0 };
    fn alive(&self) -> bool { self.age < self.life }
}

/// First-person animation. Recoil and flashes are read from the weapon
/// cooldown (stateless, so replays and captures are exact); switch, bob,
/// barrel spin, heat and the grenade cylinder need memory across frames.
#[derive(Clone, Debug)]
pub struct ViewAnim {
    pub weapon: u8,
    pub from: u8,
    pub switch: f32,
    pub spin: f32,
    pub spin_rate: f32,
    pub heat: f32,
    pub bob_phase: f32,
    pub bob: f32,
    pub ski: f32,
    pub air: f32,
    pub lift: f32,
    pub cylinder: u32,
    pub ready_flash: f32,
    last_shots: u32,
    last_cooldown: f32,
    started: bool,
}

impl Default for ViewAnim {
    fn default() -> Self {
        Self { weapon: 0, from: 0, switch: SWITCH_TIME, spin: 0.0, spin_rate: 0.0, heat: 0.0,
            bob_phase: 0.0, bob: 0.0, ski: 0.0, air: 0.0, lift: 0.0, cylinder: 0, ready_flash: 1.0,
            last_shots: 0, last_cooldown: 0.0, started: false }
    }
}

impl ViewAnim {
    /// Progress through the 0.25 s switch: the old weapon drops for the first
    /// half, the new one rises for the second. Returns (weapon shown, drop 0..1).
    pub fn shown(&self) -> (u8, f32) {
        let t = (self.switch / SWITCH_TIME).clamp(0.0, 1.0);
        if t >= 1.0 || self.from == self.weapon { return (self.weapon, 0.0); }
        if t < 0.5 { (self.from, ease(t * 2.0)) } else { (self.weapon, ease((1.0 - t) * 2.0)) }
    }

    fn update(&mut self, p: &peakrunner_core::sim::Player, dt: f32) {
        if !self.started {
            *self = Self { weapon: p.weapon, from: p.weapon, last_shots: p.shots, last_cooldown: p.cooldown,
                started: true, ..Self::default() };
        }
        if p.weapon != self.weapon {
            // Switching mid-switch lowers whatever is currently on screen.
            self.from = self.shown().0;
            self.weapon = p.weapon;
            self.switch = 0.0;
        }
        self.switch += dt;
        let fired = p.shots != self.last_shots || p.cooldown > self.last_cooldown + 0.01;
        if fired && p.weapon == 2 { self.cylinder = self.cylinder.wrapping_add(1); }
        if p.cooldown <= 0.0 && self.last_cooldown > 0.0 { self.ready_flash = 0.0; }
        self.ready_flash += dt;
        self.last_shots = p.shots;
        self.last_cooldown = p.cooldown;

        // Chaingun barrels spin up while the trigger cycles and coast down
        // after release; the jacket heats and cools. Cosmetic only.
        let firing = p.weapon == 1 && p.cooldown > 0.0;
        let target = if firing { 42.0 } else { 0.0 };
        let accel = if target > self.spin_rate { 110.0 } else { 20.0 };
        self.spin_rate += (target - self.spin_rate).clamp(-accel * dt, accel * dt);
        self.spin = (self.spin + self.spin_rate * dt) % std::f32::consts::TAU;
        self.heat = (self.heat + if firing { dt * 0.45 } else { -dt * 0.3 }).clamp(0.0, 1.0);

        // Bob follows the body: a stride on foot, a low glide while skiing,
        // and a slow float in the air. Weights ease so states blend.
        let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
        let ski = if p.skiing && p.on_ground { 1.0 } else { 0.0 };
        let air = if p.on_ground { 0.0 } else { 1.0 };
        let k = (dt * 6.0).min(1.0);
        self.ski += (ski - self.ski) * k;
        self.air += (air - self.air) * k;
        let run = (1.0 - self.ski) * (1.0 - self.air);
        let amp = run * (speed / 9.0).min(1.0) + self.ski * 0.25 + self.air * 0.15;
        self.bob += (amp - self.bob) * k;
        let freq = run * (1.4 + speed * 0.07).min(2.6) + self.ski * 0.7 + self.air * 0.45;
        self.bob_phase = (self.bob_phase + dt * freq * std::f32::consts::TAU) % (std::f32::consts::TAU * 64.0);
        let lift = (-p.vel.y * 0.0025).clamp(-0.03, 0.03) * self.air;
        self.lift += (lift - self.lift) * k;
    }

    /// View-space offset from bob and air lag.
    pub fn bob_offset(&self, time: f32) -> Vec3 {
        let s = self.bob_phase.sin();
        let c = self.bob_phase.cos();
        let idle = (time * 1.4).sin() * 0.004;
        Vec3::new(s * 0.014 * self.bob + idle, -c.abs() * 0.018 * self.bob + self.lift, 0.0)
    }
}

fn ease(t: f32) -> f32 { let t = t.clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }

pub struct Effects {
    parts: Vec<Particle>,
    next: usize,
    seen_blasts: Vec<(Vec3, u8, f32)>,
    prev_bullets: Vec<(Vec3, Vec3)>,
    cur_bullets: Vec<(Vec3, Vec3)>,
    matched: Vec<bool>,
    shots: Vec<(u32, u32)>,
    clock: f32,
    seed: std::cell::Cell<u32>,
    map: Option<MapId>,
    /// Each player's water immersion last frame, for splash-on-entry.
    wet: Vec<f32>,
    wake_clock: f32,
    pub anim: ViewAnim,
    /// Third-person landing and weapon-switch cues, per player.
    pub players: crate::player_model::AnimTracker,
}

impl Default for Effects { fn default() -> Self { Self::new() } }

impl Effects {
    pub fn new() -> Self {
        Self { parts: vec![Particle::DEAD; MAX_PARTICLES], next: 0, seen_blasts: Vec::with_capacity(64),
            prev_bullets: Vec::with_capacity(128), cur_bullets: Vec::with_capacity(128), matched: Vec::with_capacity(128),
            shots: Vec::with_capacity(16), clock: 0.0, seed: std::cell::Cell::new(0x9E37_79B9), map: None, wet: Vec::with_capacity(16), wake_clock: 0.0, anim: ViewAnim::default(), players: Default::default() }
    }

    fn rand(&self) -> f32 {
        let mut x = self.seed.get();
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        self.seed.set(x);
        (x >> 8) as f32 / 16_777_216.0
    }

    fn rand_dir(&self, up: f32) -> Vec3 {
        let a = self.rand() * std::f32::consts::TAU;
        let y = up + self.rand() * (1.0 - up);
        let r = (1.0 - y * y).max(0.0).sqrt();
        Vec3::new(a.cos() * r, y, a.sin() * r)
    }

    #[cfg(test)]
    pub fn live(&self) -> usize { self.parts.iter().filter(|p| p.alive()).count() }
    fn count(&self, kind: Kind) -> usize { self.parts.iter().filter(|p| p.alive() && p.kind == kind).count() }

    fn spawn(&mut self, p: Particle) {
        self.parts[self.next] = p;
        self.next = (self.next + 1) % MAX_PARTICLES;
    }

    /// Advance everything by `dt` and pick up new explosions, impacts and shots.
    pub fn update(&mut self, world: &World, eye: Vec3, dt: f32) {
        self.players.update(&world.players, dt, &|a, b| world.clear_distance(a, b));
        if self.map != Some(world.map) {
            // A new map or round: nothing lingers from the last one.
            self.parts.fill(Particle::DEAD);
            self.seen_blasts.clear();
            self.prev_bullets.clear();
            self.shots.clear();
            self.map = Some(world.map);
        }
        self.clock += dt;
        let map = world.map;
        for i in 0..self.parts.len() {
            let mut p = self.parts[i];
            if !p.alive() { continue; }
            p.age += dt;
            p.vel.y -= p.gravity * dt;
            p.vel *= (1.0 - p.drag * dt).max(0.0);
            p.pos += p.vel * dt;
            p.spin += dt * 9.0;
            if matches!(p.kind, Kind::Debris | Kind::Casing) && p.vel.y < 0.0 {
                let floor = ground_y(map, p.pos + Vec3::Y * 0.5);
                if p.pos.y < floor + p.size * 0.5 {
                    p.pos.y = floor + p.size * 0.5;
                    p.vel = Vec3::new(p.vel.x * 0.55, -p.vel.y * 0.3, p.vel.z * 0.55);
                }
            }
            self.parts[i] = p;
        }

        let clock = self.clock;
        self.seen_blasts.retain(|s| s.2 > clock);
        for e in &world.explosions {
            if e.age > 0.3 { continue; }
            if self.seen_blasts.iter().any(|s| s.1 == e.kind && s.0.distance_squared(e.pos) < 0.25) { continue; }
            if self.seen_blasts.len() < 64 { self.seen_blasts.push((e.pos, e.kind, clock + 1.5)); }
            self.blast(map, e.pos, e.kind, e.max_r, eye);
        }
        if world.state != MatchState::Flyby {
            self.impacts(world, eye, dt);
            self.watch_shots(world, eye);
        }
        self.water(world, eye, dt);
        if let Some(p) = world.players.get(world.player_id) { self.anim.update(p, dt); }
    }

    fn blast(&mut self, map: MapId, pos: Vec3, kind: u8, radius: f32, eye: Vec3) {
        let distance = eye.distance(pos);
        if distance > IGNORE_RANGE { return; }
        let detailed = distance < DETAIL_RANGE;
        let floor = ground_y(map, pos + Vec3::Y * 0.5);
        let near_ground = pos.y - floor < 2.5;
        let ground = ground_tint(map);
        let (smoke, sparks, debris, dark, spark_color, rise) = match kind {
            0 => (5, 7, 0, [0.52, 0.58, 0.64], [0.45, 0.85, 1.0], 0.9),
            3 => (2, 8, 0, [0.42, 0.52, 0.40], [0.45, 1.0, 0.3], 0.6),
            // Mortar: green sparks, dark smoke.
            5 => (10, 12, 8, [0.16, 0.2, 0.15], [0.45, 1.0, 0.25], 1.5),
            4 => (14, 16, 12, [0.10, 0.095, 0.09], [1.0, 0.62, 0.2], 2.0),
            _ => (8, 8, 7, [0.19, 0.18, 0.17], [1.0, 0.58, 0.16], 1.3),
        };
        if !detailed {
            // Distant blasts keep one smoke puff so the column still reads.
            self.puff(pos, Vec3::Y * rise, dark, 0.55, radius * 0.25, radius * 0.6, 1.6);
            return;
        }
        let scale = (radius / 8.0).clamp(0.5, 1.6);
        for _ in 0..smoke {
            let d = self.rand_dir(0.1);
            let spread = self.rand() * radius * 0.35;
            let life = 1.3 + self.rand() * 0.7 + if kind == 4 { 0.8 } else { 0.0 };
            // Smaller puffs with more lift break into a rising column rather
            // than one dark dome.
            let v = d * (1.5 + self.rand() * 2.0) * scale + Vec3::Y * rise * (1.0 + self.rand() * 1.4);
            let big = if kind == 4 { 1.5 } else { 1.0 };
            self.puff(pos + d * spread, v, dark, if kind == 4 { 0.66 } else { 0.5 },
                (0.4 + self.rand() * 0.3) * scale * big, (1.0 + self.rand() * 0.9) * scale * 1.3 * big, life);
        }
        if near_ground {
            let ground_pos = Vec3::new(pos.x, floor + 0.2, pos.z);
            for _ in 0..3 {
                let d = self.rand_dir(0.0) * Vec3::new(1.0, 0.25, 1.0);
                let v = d * (3.0 + self.rand() * 2.0) * scale + Vec3::Y * 0.6;
                self.puff(ground_pos, v, ground, 0.5, 0.6 * scale, 2.2 * scale, 0.9 + self.rand() * 0.4);
            }
            if self.count(Kind::Scorch) < MAX_SCORCH && kind != 3 {
                let n = ground_normal(map, ground_pos);
                self.spawn(Particle { kind: Kind::Scorch, pos: Vec3::new(pos.x, floor + 0.05, pos.z), vel: Vec3::ZERO,
                    normal: n, age: 0.0, life: 3.5, size: radius * 0.32, grow: 0.0, color: [0.05, 0.045, 0.04],
                    alpha: 0.55, spin: self.rand() * 6.3, drag: 0.0, gravity: 0.0 });
            }
        }
        for _ in 0..sparks {
            let d = self.rand_dir(0.05);
            let s = (8.0 + self.rand() * 12.0) * scale;
            self.spawn(Particle { kind: Kind::Spark, pos, vel: d * s, normal: Vec3::Y, age: 0.0,
                life: 0.35 + self.rand() * 0.3, size: 0.05, grow: 0.0, color: spark_color, alpha: 1.0,
                spin: 0.0, drag: 1.2, gravity: 14.0 });
        }
        let metal = [0.26, 0.28, 0.31];
        for _ in 0..debris {
            let d = self.rand_dir(0.35);
            let s = (6.0 + self.rand() * 7.0) * scale;
            let c = if kind == 4 { metal } else { [ground[0] * 0.7, ground[1] * 0.7, ground[2] * 0.7] };
            self.spawn(Particle { kind: Kind::Debris, pos: pos + Vec3::Y * 0.3, vel: d * s, normal: Vec3::Y,
                age: 0.0, life: 1.1 + self.rand() * 0.5, size: 0.12 + self.rand() * 0.14 * scale, grow: 0.0,
                color: c, alpha: 1.0, spin: self.rand() * 6.3, drag: 0.3, gravity: 20.0 });
        }
    }

    /// Splash when a player enters water (scaled by speed) and a light wake
    /// while they move through it.
    fn water(&mut self, world: &World, eye: Vec3, dt: f32) {
        const SPRAY: [f32; 3] = [0.82, 0.88, 0.92];
        self.wet.resize(world.players.len(), 0.0);
        self.wake_clock += dt;
        let wake_now = self.wake_clock > 0.09;
        if wake_now { self.wake_clock = 0.0; }
        for (i, pl) in world.players.iter().enumerate() {
            let (wet, volume) = if pl.alive {
                peakrunner_core::water::immersion(world.map, &world.staged_water, pl.pos)
            } else { (0.0, None) };
            let was = std::mem::replace(&mut self.wet[i], wet);
            let Some(volume) = volume else { continue };
            if pl.pos.distance(eye) > DETAIL_RANGE { continue; }
            let at = Vec3::new(pl.pos.x, volume.surface + 0.05, pl.pos.z);
            let speed = pl.vel.length();
            if was <= 0.0 && wet > 0.0 && speed > 2.0 {
                let strength = (speed / 25.0).clamp(0.3, 2.0);
                for _ in 0..(6.0 * strength) as usize {
                    let d = self.rand_dir(0.55);
                    self.spawn(Particle { kind: Kind::Spark, pos: at, vel: d * (4.0 + self.rand() * 6.0) * strength,
                        normal: Vec3::Y, age: 0.0, life: 0.5 + self.rand() * 0.3, size: 0.07, grow: 0.0,
                        color: SPRAY, alpha: 0.9, spin: 0.0, drag: 0.6, gravity: 16.0 });
                }
                for _ in 0..(2.0 * strength).ceil() as usize {
                    let d = self.rand_dir(0.0) * Vec3::new(1.0, 0.2, 1.0);
                    self.puff(at, d * 2.5 * strength + Vec3::Y * 1.5, SPRAY, 0.35, 0.5 * strength, 1.6 * strength, 0.7);
                }
            } else if wake_now && wet > 0.0 && wet < 0.95 && Vec2::new(pl.vel.x, pl.vel.z).length() > 3.0 {
                let back = -Vec3::new(pl.vel.x, 0.0, pl.vel.z).normalize_or_zero();
                self.puff(at + back * 0.6, back * 1.2 + Vec3::Y * 0.4, SPRAY, 0.22, 0.35, 1.1, 0.55);
            }
        }
    }

    fn puff(&mut self, pos: Vec3, vel: Vec3, color: [f32; 3], alpha: f32, size: f32, grow: f32, life: f32) {
        let spin = self.rand() * 6.3;
        self.spawn(Particle { kind: Kind::Smoke, pos, vel, normal: Vec3::Y, age: 0.0, life, size, grow,
            color, alpha, spin, drag: 1.4, gravity: -0.4 });
    }

    /// A chaingun round that vanishes between frames and whose last step
    /// crosses a surface struck that surface: sparks and a dust puff there.
    fn impacts(&mut self, world: &World, eye: Vec3, dt: f32) {
        self.cur_bullets.clear();
        for d in &world.discs {
            if d.kind == 1 && self.cur_bullets.len() < 128 { self.cur_bullets.push((d.pos, d.vel)); }
        }
        self.matched.clear();
        self.matched.resize(self.cur_bullets.len(), false);
        let mut spawned = 0;
        for k in 0..self.prev_bullets.len() {
            let (pos, vel) = self.prev_bullets[k];
            let step = vel * dt;
            let predicted = pos + step;
            let tolerance = (step.length() * 0.4).max(1.0);
            let dir = vel.normalize_or_zero();
            let found = self.cur_bullets.iter().enumerate().position(|(i, (p, v))| {
                !self.matched[i] && p.distance(predicted) < tolerance && v.normalize_or_zero().dot(dir) > 0.95
            });
            if let Some(i) = found { self.matched[i] = true; continue; }
            if spawned >= MAX_IMPACTS_PER_FRAME || pos.distance(eye) > DETAIL_RANGE { continue; }
            let end = pos + vel * (dt * 1.6 + 0.03);
            if let Some((at, n)) = surface_hit(world.map, pos, end) {
                spawned += 1;
                self.impact(world.map, at, n, dir);
            }
        }
        std::mem::swap(&mut self.prev_bullets, &mut self.cur_bullets);
    }

    fn impact(&mut self, map: MapId, at: Vec3, normal: Vec3, dir: Vec3) {
        let reflect = (dir - normal * 2.0 * dir.dot(normal)).normalize_or(normal);
        for _ in 0..5 {
            let jitter = self.rand_dir(-1.0) * 0.55;
            let v = (reflect + normal * 0.4 + jitter).normalize_or(normal) * (5.0 + self.rand() * 7.0);
            self.spawn(Particle { kind: Kind::Spark, pos: at + normal * 0.05, vel: v, normal, age: 0.0,
                life: 0.18 + self.rand() * 0.18, size: 0.03, grow: 0.0, color: [1.0, 0.78, 0.35], alpha: 1.0,
                spin: 0.0, drag: 2.0, gravity: 12.0 });
        }
        self.spawn(Particle { kind: Kind::Flash, pos: at + normal * 0.08, vel: Vec3::ZERO, normal, age: 0.0,
            life: 0.05, size: 0.16, grow: 0.0, color: [1.0, 0.85, 0.55], alpha: 0.9, spin: 0.0, drag: 0.0, gravity: 0.0 });
        let dust = if normal.y > 0.6 { ground_tint(map) } else { [0.52, 0.52, 0.50] };
        self.puff(at + normal * 0.15, normal * 1.2, dust, 0.45, 0.14, 0.55, 0.55 + self.rand() * 0.25);
    }

    /// Casings and third-person muzzle flashes from the shot counters.
    fn watch_shots(&mut self, world: &World, eye: Vec3) {
        let me = world.player_id;
        for (i, p) in world.players.iter().enumerate() {
            let id = if p.net_id != 0 { p.net_id } else { i as u32 + 1 };
            let before = match self.shots.iter_mut().find(|s| s.0 == id) {
                Some(s) => { let b = s.1; s.1 = p.shots; b }
                None => { if self.shots.len() < 16 { self.shots.push((id, p.shots)); } continue; }
            };
            if p.shots <= before || !p.alive || p.pos.distance(eye) > 120.0 { continue; }
            let yaw = if i == me { world.players[me].yaw } else { p.yaw };
            let base = Mat4::from_translation(p.pos) * Mat4::from_rotation_y(yaw);
            let right = base.transform_vector3(Vec3::X);
            let forward = base.transform_vector3(Vec3::NEG_Z);
            let (gun, muzzle) = if i == me {
                let (e, d, _) = world.camera();
                (e + right * 0.28 - Vec3::Y * 0.22 + d * 0.35, e + d * 0.9)
            } else {
                (base.transform_point3(Vec3::new(0.29, 1.0, -0.3)), base.transform_point3(Vec3::new(0.29, 0.99, -0.86)))
            };
            if p.weapon == 1 && self.count(Kind::Casing) < MAX_CASINGS && p.shots % 2 == 0 {
                let v = right * (2.2 + self.rand()) + Vec3::Y * (1.8 + self.rand()) - forward * 0.4;
                self.spawn(Particle { kind: Kind::Casing, pos: gun, vel: v, normal: Vec3::Y, age: 0.0,
                    life: 0.9, size: 0.03, grow: 0.0, color: [0.82, 0.62, 0.26], alpha: 1.0,
                    spin: self.rand() * 6.3, drag: 0.2, gravity: 16.0 });
            }
            if i != me {
                let color = match p.weapon { 0 => [0.45, 0.85, 1.0], 1 => [1.0, 0.8, 0.35], _ => [1.0, 0.55, 0.2] };
                self.spawn(Particle { kind: Kind::Flash, pos: muzzle, vel: Vec3::ZERO, normal: forward, age: 0.0,
                    life: 0.06, size: if p.weapon == 1 { 0.22 } else { 0.32 }, grow: 0.0, color, alpha: 0.9,
                    spin: 0.0, drag: 0.0, gravity: 0.0 });
            }
            if p.weapon == 2 {
                // A grey puff leaves the wide bore on every grenade.
                self.puff(muzzle + forward * 0.2, forward * 1.5 + Vec3::Y * 0.4, [0.62, 0.62, 0.60], 0.35, 0.18, 0.7, 0.7);
            }
        }
    }

    /// Append this frame's effect draws.
    pub fn draw(&self, lit: &mut Vec<LitDraw>, emit: &mut Vec<EmitDraw>, smoke: &mut Vec<EmitDraw>, eye: Vec3) {
        for p in self.parts.iter().filter(|p| p.alive()) {
            let t = (p.age / p.life).clamp(0.0, 1.0);
            match p.kind {
                Kind::Smoke => {
                    let size = p.size + p.grow * (1.0 - (1.0 - t) * (1.0 - t));
                    let fade_in = (p.age / 0.08).min(1.0);
                    let near = ((p.pos.distance(eye) - size * 0.6) / 1.0).clamp(0.0, 1.0);
                    let a = p.alpha * (1.0 - t).powf(1.4) * fade_in * near;
                    if a < 0.01 { continue; }
                    smoke.push(EmitDraw { mesh: MeshId::Sphere,
                        model: Mat4::from_translation(p.pos) * Mat4::from_rotation_y(p.spin * 0.05)
                            * Mat4::from_scale(Vec3::new(size, size * 0.86, size)),
                        color: [p.color[0], p.color[1], p.color[2], a] });
                }
                Kind::Scorch => {
                    let a = p.alpha * (1.0 - t);
                    let rot = glam::Quat::from_rotation_arc(Vec3::Y, p.normal.normalize_or(Vec3::Y));
                    smoke.push(EmitDraw { mesh: MeshId::Decal,
                        model: Mat4::from_translation(p.pos) * Mat4::from_quat(rot) * Mat4::from_rotation_y(p.spin)
                            * Mat4::from_scale(Vec3::new(p.size, 1.0, p.size * 0.8)),
                        color: [p.color[0], p.color[1], p.color[2], a] });
                }
                Kind::Spark => {
                    let dir = p.vel.normalize_or(Vec3::Y);
                    let len = (p.vel.length() * 0.035).clamp(0.05, 0.6);
                    let mut side = dir.cross(Vec3::Y);
                    if side.length_squared() < 1e-4 { side = Vec3::X; }
                    let side = side.normalize();
                    let up = side.cross(dir);
                    let w = p.size * (1.0 - t * 0.5);
                    emit.push(EmitDraw { mesh: MeshId::Sphere,
                        model: Mat4::from_cols((side * w).extend(0.0), (up * w).extend(0.0),
                            (dir * len).extend(0.0), p.pos.extend(1.0)),
                        color: [p.color[0], p.color[1], p.color[2], (1.0 - t) * p.alpha] });
                }
                Kind::Flash => {
                    let s = p.size * (0.6 + t * 0.8);
                    emit.push(EmitDraw { mesh: MeshId::Sphere,
                        model: Mat4::from_translation(p.pos) * Mat4::from_scale(Vec3::splat(s)),
                        color: [p.color[0], p.color[1], p.color[2], (1.0 - t) * p.alpha] });
                }
                Kind::Debris | Kind::Casing => {
                    // Shrink out over the last third rather than popping.
                    let shrink = ((1.0 - t) * 3.0).min(1.0);
                    let s = p.size * shrink;
                    if s < 0.004 { continue; }
                    let scale = if p.kind == Kind::Casing { Vec3::new(s, s, s * 2.4) } else { Vec3::new(s, s * 0.8, s * 1.1) };
                    lit.push(LitDraw { mesh: if p.kind == Kind::Casing { MeshId::Disc } else { MeshId::Bevel },
                        model: Mat4::from_translation(p.pos) * Mat4::from_rotation_y(p.spin)
                            * Mat4::from_rotation_x(p.spin * 1.3) * Mat4::from_scale(scale),
                        color: Vec3::from_array(p.color), emit: if p.kind == Kind::Casing { 0.15 } else { 0.0 }, mode: 0.0 });
                }
            }
        }
    }
}

/// Soil or snow colour thrown up by blasts on each map.
pub fn ground_tint(map: MapId) -> [f32; 3] {
    match map {
        MapId::SnowblindClone | MapId::Valley => [0.86, 0.88, 0.91],
        MapId::DesertOfDeathClone => [0.80, 0.66, 0.46],
        MapId::StonehengeClone => [0.46, 0.43, 0.38],
        MapId::BroadsideClone => [0.42, 0.38, 0.30],
        MapId::Raindance => [0.40, 0.34, 0.25],
        MapId::Longfield => [0.36, 0.40, 0.26],
        MapId::Highgoal => [0.78, 0.66, 0.48],
        MapId::OzarkticBlast => [0.44, 0.46, 0.38],
        MapId::Reefbreak => [0.8, 0.74, 0.6],
    }
}

/// Highest support under `pos`: terrain or a building floor.
fn ground_y(map: MapId, pos: Vec3) -> f32 {
    let terrain = terrain::height_on(map, pos.x, pos.z);
    let floor = peakrunner_core::map_pack::on(map).and_then(|p| p.floor(pos)).map(|f| f.0);
    match floor { Some(f) if f > terrain => f, _ => terrain }
}

fn ground_normal(map: MapId, at: Vec3) -> Vec3 {
    if let Some((_, n)) = peakrunner_core::map_pack::on(map).and_then(|p| p.floor(at + Vec3::Y * 0.3)) {
        if n.y > 0.3 { return n; }
    }
    terrain::normal_on(map, at.x, at.z)
}

/// First surface a segment crosses (terrain or map geometry), with its normal.
pub fn surface_hit(map: MapId, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
    let mut best: Option<(f32, Vec3)> = None;
    if let Some(t) = terrain::segment_hit(map, a, b, 0.0) {
        let p = a.lerp(b, t);
        best = Some((t, terrain::normal_on(map, p.x, p.z)));
    }
    if let Some((t, n)) = peakrunner_core::map_pack::on(map).and_then(|p| p.sweep(a, b, 0.0)) {
        if best.is_none_or(|(old, _)| t < old) { best = Some((t, n)); }
    }
    best.map(|(t, n)| {
        let n = if n.dot(b - a) > 0.0 { -n } else { n };
        (a.lerp(b, t), n.normalize_or(Vec3::Y))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use peakrunner_core::sim::{Disc, Explosion, Team};

    fn world() -> World {
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        w.start_match(true);
        w.state = MatchState::Playing;
        w
    }

    #[test]
    fn a_blast_spawns_lingering_capped_effects_once() {
        let mut w = world();
        let eye = w.camera().0;
        let pos = eye + Vec3::new(0.0, -2.0, -20.0);
        w.explosions.push(Explosion { pos, age: 0.0, max_r: 9.0, kind: 2 });
        let mut fx = Effects::new();
        fx.update(&w, eye, 1.0 / 60.0);
        let first = fx.live();
        assert!(first > 10, "a grenade blast spawns smoke, sparks and debris: {first}");
        // The same blast seen again in later snapshots is not re-spawned.
        for _ in 0..10 { fx.update(&w, eye, 1.0 / 60.0); }
        assert!(fx.live() <= first);
        // Smoke outlives the 0.55 s blast record.
        w.explosions.clear();
        for _ in 0..60 { fx.update(&w, eye, 1.0 / 60.0); }
        assert!(fx.count(Kind::Smoke) > 0);
        // Dozens of blasts never exceed the pool.
        for k in 0..60 {
            w.explosions.push(Explosion { pos: pos + Vec3::X * k as f32, age: 0.0, max_r: 9.0, kind: 4 });
            fx.update(&w, eye, 1.0 / 60.0);
        }
        assert!(fx.live() <= MAX_PARTICLES);
        assert!(fx.count(Kind::Scorch) <= MAX_SCORCH);
        let (mut lit, mut emit, mut smoke) = (Vec::new(), Vec::new(), Vec::new());
        fx.draw(&mut lit, &mut emit, &mut smoke, eye);
        for d in lit.iter() { assert!(d.model.is_finite()); }
        for d in emit.iter().chain(&smoke) { assert!(d.model.is_finite() && d.color.iter().all(|c| c.is_finite())); }
    }

    #[test]
    fn scorch_marks_draw_as_unflattened_decals() {
        // A scorch drawn as a disc flattened by a tiny Y scale lost its faces to
        // the facing fade, leaving only a thin rim on screen.
        let mut w = world();
        let eye = w.camera().0;
        w.explosions.push(Explosion { pos: eye + Vec3::new(0.0, -2.0, -20.0), age: 0.0, max_r: 9.0, kind: 2 });
        let mut fx = Effects::default();
        fx.update(&w, eye, 1.0 / 60.0);
        assert_eq!(fx.count(Kind::Scorch), 1);
        let (mut lit, mut emit, mut smoke) = (Vec::new(), Vec::new(), Vec::new());
        fx.draw(&mut lit, &mut emit, &mut smoke, eye);
        let decals: Vec<_> = smoke.iter().filter(|d| d.mesh == MeshId::Decal).collect();
        assert_eq!(decals.len(), 1);
        assert!(!smoke.iter().any(|d| d.mesh == MeshId::Disc));
        let up = decals[0].model.transform_vector3(Vec3::Y).length();
        assert!(up > 0.5, "decal flattened by its model scale: {up}");
    }

    #[test]
    fn distant_blasts_skip_debris_and_sparks() {
        let mut w = world();
        let eye = w.camera().0;
        w.explosions.push(Explosion { pos: eye + Vec3::new(0.0, 0.0, -300.0), age: 0.0, max_r: 9.0, kind: 2 });
        let mut fx = Effects::new();
        fx.update(&w, eye, 1.0 / 60.0);
        assert_eq!(fx.count(Kind::Spark) + fx.count(Kind::Debris), 0);
        assert!(fx.live() <= 1);
    }

    #[test]
    fn a_round_that_vanishes_into_the_ground_sparks_and_one_in_flight_does_not() {
        let mut w = world();
        let eye = w.camera().0;
        let target = Vec3::new(eye.x, terrain::height_on(w.map, eye.x, eye.z - 15.0), eye.z - 15.0);
        let from = target + Vec3::new(0.0, 3.0, 3.0);
        let vel = (target - from).normalize() * 420.0;
        w.discs.push(Disc { pos: from, vel, team: Team::Ember, owner: 0, life: 1.0, kind: 1, spin: 0.0 });
        let mut fx = Effects::new();
        fx.update(&w, eye, 1.0 / 60.0);
        assert_eq!(fx.count(Kind::Spark), 0, "a live round is not an impact");
        w.discs.clear();
        fx.update(&w, eye, 1.0 / 60.0);
        assert!(fx.count(Kind::Spark) >= 5, "the vanished round struck the ground");
        // A round that simply leaves (expires in open air) spawns nothing.
        let mut fx = Effects::new();
        w.discs.push(Disc { pos: eye + Vec3::Y * 40.0, vel: Vec3::Y * 420.0, team: Team::Ember, owner: 0, life: 0.01, kind: 1, spin: 0.0 });
        fx.update(&w, eye, 1.0 / 60.0);
        w.discs.clear();
        fx.update(&w, eye, 1.0 / 60.0);
        assert_eq!(fx.count(Kind::Spark), 0);
    }

    #[test]
    fn switch_drops_the_old_weapon_then_raises_the_new_one() {
        let mut w = world();
        let me = w.player_id;
        w.players[me].weapon = 0;
        let mut fx = Effects::new();
        let eye = w.camera().0;
        fx.update(&w, eye, 0.016);
        assert_eq!(fx.anim.shown(), (0, 0.0));
        w.players[me].weapon = 1;
        fx.update(&w, eye, 0.06);
        let (shown, drop) = fx.anim.shown();
        assert_eq!(shown, 0);
        assert!(drop > 0.2);
        fx.update(&w, eye, 0.1);
        assert_eq!(fx.anim.shown().0, 1);
        fx.update(&w, eye, 0.2);
        assert_eq!(fx.anim.shown(), (1, 0.0));
    }

    #[test]
    fn chaingun_barrels_spin_up_and_coast_down() {
        let mut w = world();
        let me = w.player_id;
        w.players[me].weapon = 1;
        let mut fx = Effects::new();
        let eye = w.camera().0;
        fx.update(&w, eye, 0.016);
        for _ in 0..60 {
            w.players[me].cooldown = 0.05;
            fx.update(&w, eye, 0.016);
        }
        let spun = fx.anim.spin_rate;
        assert!(spun > 35.0 && fx.anim.heat > 0.3);
        w.players[me].cooldown = 0.0;
        fx.update(&w, eye, 0.1);
        assert!(fx.anim.spin_rate > 0.0 && fx.anim.spin_rate < spun, "coasting, not snapping still");
        for _ in 0..200 { fx.update(&w, eye, 0.016); }
        assert_eq!(fx.anim.spin_rate, 0.0);
        assert!(fx.anim.heat < 0.01);
    }
}
