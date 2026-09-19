use glam::{Mat4, Vec2, Vec3};
use serde::{Deserialize, Serialize};
#[path = "matchplay.rs"]
mod matchplay;
pub use matchplay::*;

use crate::terrain::{
    height, pillars, MapId, Pillar, EMBER_HOME, EYE, GLACIER_HOME, PLAYER_RADIUS,
};

pub const STEP: f32 = 1.0 / 60.0;
// Ascend-inspired tuning in meters/seconds, not a binary-exact engine port.
// Keep the existing jump thresholds, but use smooth held skiing and air control.
const T2: f32 = 11.2 / 15.0;
const GRAVITY: f32 = 20.0;
const MASS: f32 = 90.0;
/// `jumpForce / mass` from the Classic light armor, applied along the ground normal.
const JUMP_IMPULSE: f32 = 8.34;
const MIN_JUMP_SPEED: f32 = 20.0 * T2;
const MAX_JUMP_SPEED: f32 = 30.0 * T2;
/// Safety ceiling, not a cruising-speed target. Routes can exceed 300 km/h.
const HORIZ_MAX: f32 = 125.0;
const UP_MAX: f32 = 52.0 * T2;
const UP_RESIST_SPEED: f32 = 20.89 * T2;
const UP_RESIST: f32 = 0.35;

const WALK_ACCEL: f32 = 78.0;
const WALK_MAX: f32 = 11.2;
const GROUND_BRAKE: f32 = 32.0;
const AIR_ACCEL: f32 = 6.0;
const SKI_TURN_ACCEL: f32 = 28.0;
/// Tuned downhill assistance for an Ascend-like ski, with ordinary uphill gravity.
const SKI_SLOPE_GRAVITY: f32 = 2.0;
/// Climb accel. Not spent when a movement key is held.
const JET_ACCEL: f32 = 37.28;
/// Extra sideways accel under the cap. Added on top of the climb, not taken from it.
const JET_HORIZ_ACCEL: f32 = 22.0;
/// 72 km/h. Thrust falls off here so jetting alone cannot make a ski line.
const JET_THRUST_CAP: f32 = 72.0 / 3.6;
/// Four-second tank with a five-second recharge, tuned for continuous ski routes.
pub const ENERGY_MAX: f32 = 60.0;
const ENERGY_JET: f32 = 15.0;
const ENERGY_REGEN: f32 = 12.0;
const MIN_JET_ENERGY: f32 = 3.0;
/// Classic `dryVelocity`. The disc is a linear round, not a mortar.
const DISC_SPEED: f32 = 95.0;
/// Classic `velInheritFactor`. Without this the disc runs away and you cannot disc jump.
const DISC_INHERIT: f32 = 0.75;
pub const DISC_RELOAD: f32 = 1.05;
/// Viewmodel pose. The world muzzle uses the same numbers, or the disc leaves the eye.
pub const VM_FOV: f32 = 65.0;
pub const VM_TURN_Y: f32 = 0.18;
pub const VM_TURN_X: f32 = -0.08;
pub const VM_DISC_TURN_Y: f32 = 0.005;
pub const VM_DISC_TURN_X: f32 = 0.004;
pub const VM_ANCHOR_DISC: Vec3 = Vec3::new(0.40, -0.30, -0.70);
pub const VM_ANCHOR_BOLT: Vec3 = Vec3::new(0.32, -0.28, -0.62);
pub const VM_MUZZLE_DISC: Vec3 = Vec3::new(0.0, 0.075, -0.79);
pub const VM_MUZZLE_BOLT: Vec3 = Vec3::new(0.0, 0.02, -0.59);
/// Classic `damageRadius`.
const DISC_RADIUS: f32 = 7.5;
/// Classic `kickBackStrength` 2000 on a 90 kg body, at the center of the blast.
const DISC_KICK: f32 = 2000.0 / 90.0;
const BOLT_SPEED: f32 = 420.0;
pub fn weapon_reload(kind: u8) -> f32 {
    match kind { 0 => DISC_RELOAD, 1 => 0.075, _ => 0.85 }
}
const MATCH_TIME: f32 = 8.0 * 60.0;
const CAPTURES: u32 = 3;
const SENS: f32 = 0.00235;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Team {
    Ember = 0,
    Glacier = 1,
}

impl Team {
    pub fn other(self) -> Team {
        match self {
            Team::Ember => Team::Glacier,
            Team::Glacier => Team::Ember,
        }
    }
    pub fn idx(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchState {
    Flyby = 0,
    Playing = 1,
    Paused = 2,
    Ended = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum BotRole {
    Offense,
    Defense,
    Hunter,
}

#[derive(Clone, Debug)]
pub struct Input {
    pub move_x: f32,
    pub move_z: f32,
    pub jump: bool,
    pub jet: bool,
    pub fire: bool,
    pub weapon: u8,
    pub look_stick_x: f32,
    pub look_stick_y: f32,
    #[allow(dead_code)]
    pub keys_override: Option<Vec<String>>,
    jump_prev: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            move_x: 0.0,
            move_z: 0.0,
            jump: false,
            jet: false,
            fire: false,
            weapon: 0,
            look_stick_x: 0.0,
            look_stick_y: 0.0,
            keys_override: None,
            jump_prev: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub net_id: u32,
    pub name: String,
    pub frags: u32,
    pub losses: u32,
    pub shots: u32,
    pub hits: u32,
    jump_prev: bool,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub energy: f32,
    pub team: Team,
    pub alive: bool,
    pub respawn: f32,
    pub carrying: Option<Team>,
    pub is_bot: bool,
    pub remote: bool,
    pub on_ground: bool,
    pub skiing: bool,
    pub jetting: bool,
    pub cooldown: f32,
    pub weapon: u8,
    bot_role: BotRole,
    bot_goal: Vec3,
    bot_think: f32,
    coyote: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Disc {
    pub pos: Vec3,
    pub vel: Vec3,
    pub team: Team,
    pub owner: usize,
    pub life: f32,
    pub kind: u8, // 0 disc, 1 bullet, 2 grenade
    pub spin: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Explosion {
    pub pos: Vec3,
    pub age: f32,
    pub max_r: f32,
    pub kind: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SmokePuff {
    pub pos: Vec3,
    pub age: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Flag {
    pub team: Team,
    pub pos: Vec3,
    pub home: Vec3,
    pub carrier: Option<usize>,
    pub drop_timer: f32,
}

pub struct World {
    pub players: Vec<Player>,
    pub discs: Vec<Disc>,
    pub explosions: Vec<Explosion>,
    pub smoke: Vec<SmokePuff>,
    pub flags: [Flag; 2],
    pub pillars: Vec<Pillar>,
    pub score: [u32; 2],
    pub time_left: f32,
    pub state: MatchState,
    pub acc: f32,
    pub flyby: f32,
    pub player_id: usize,
    pub input: Input,
    pub message: String,
    pub message_t: f32,
    pub hitmarker: f32,
    pub damage_flash: f32,
    pub trauma: f32,
    pub kills: u32,
    pub deaths: u32,
    pub events: String,
    pub time: f32,
    pub map: MapId,
    pub net_camera_offset: Vec3,
    pub blast_serial: u64,
    network_inputs: Vec<Input>,
    predicting: bool,
    rng: u32,
}

struct Rng(u32);
impl Rng {
    fn f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 16_777_216.0
    }
}

pub fn camera_fov(horiz_speed: f32) -> f32 {
    76.0 + (horiz_speed * 0.14).min(17.0)
}

/// Camera-space muzzle, widened to the world field of view so it sits on the gun.
fn muzzle_origin(eye: Vec3, forward: Vec3, weapon: u8, fov_deg: f32) -> Vec3 {
    let anchor = if weapon == 0 { VM_ANCHOR_DISC } else { VM_ANCHOR_BOLT };
    let local = if weapon == 0 { VM_MUZZLE_DISC } else { VM_MUZZLE_BOLT };
    let (yaw, pitch) = if weapon == 0 { (VM_DISC_TURN_Y, VM_DISC_TURN_X) }
        else { (VM_TURN_Y, VM_TURN_X) };
    let turned = Mat4::from_rotation_y(yaw) * Mat4::from_rotation_x(pitch);
    let cam = anchor + turned.transform_point3(local);
    let widen = (fov_deg * 0.5).to_radians().tan() / (VM_FOV * 0.5).to_radians().tan();
    let (right, up) = view_basis(forward);
    eye + right * cam.x * widen + up * cam.y * widen + forward * (-cam.z)
}

fn view_basis(forward: Vec3) -> (Vec3, Vec3) {
    let mut right = forward.cross(Vec3::Y);
    if right.length_squared() < 1.0e-6 {
        right = forward.cross(Vec3::Z);
    }
    let right = right.normalize();
    let up = right.cross(forward).normalize();
    (right, up)
}

fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    let cp = pitch.cos();
    Vec3::new(-yaw.sin() * cp, pitch.sin(), -yaw.cos() * cp)
}

fn move_basis(yaw: f32) -> (Vec3, Vec3) {
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    (forward, right)
}

impl World {
    pub fn new() -> Self {
        let pillars = pillars();
        let ember_h = height(EMBER_HOME.x, EMBER_HOME.z);
        let glac_h = height(GLACIER_HOME.x, GLACIER_HOME.z);
        Self {
            players: Vec::new(),
            discs: Vec::new(),
            explosions: Vec::new(),
            smoke: Vec::new(),
            flags: [
                Flag {
                    team: Team::Ember,
                    pos: Vec3::new(EMBER_HOME.x, ember_h + 0.2, EMBER_HOME.z),
                    home: Vec3::new(EMBER_HOME.x, ember_h + 0.2, EMBER_HOME.z),
                    carrier: None,
                    drop_timer: 0.0,
                },
                Flag {
                    team: Team::Glacier,
                    pos: Vec3::new(GLACIER_HOME.x, glac_h + 0.2, GLACIER_HOME.z),
                    home: Vec3::new(GLACIER_HOME.x, glac_h + 0.2, GLACIER_HOME.z),
                    carrier: None,
                    drop_timer: 0.0,
                },
            ],
            pillars,
            score: [0, 0],
            time_left: MATCH_TIME,
            state: MatchState::Flyby,
            acc: 0.0,
            flyby: 0.0,
            player_id: 0,
            input: Input::default(),
            message: String::new(),
            message_t: 0.0,
            hitmarker: 0.0,
            damage_flash: 0.0,
            trauma: 0.0,
            kills: 0,
            deaths: 0,
            events: String::new(),
            time: 0.0,
            map: MapId::Valley,
            net_camera_offset: Vec3::ZERO,
            blast_serial: 0,
            network_inputs: Vec::new(),
            predicting: false,
            rng: 0xC0FFEE,
        }
    }

    fn rng(&mut self) -> f32 {
        let mut r = Rng(self.rng);
        let v = r.f32();
        self.rng = r.0;
        v
    }

    pub fn set_map(&mut self, id: MapId) {
        self.map = id;
        self.pillars = crate::terrain::pillars_on(id);
        if self.players.is_empty() {
            self.place_flags();
        }
    }

    fn ground(&self, x: f32, z: f32) -> f32 {
        crate::terrain::height_on(self.map, x, z)
    }

    fn map_size(&self) -> f32 {
        crate::terrain::info(self.map).size
    }

    fn stand(&self, ember: bool) -> Vec3 {
        crate::terrain::spawn_on(self.map, ember)
    }

    pub fn set_paused(&mut self, paused: bool) {
        match (self.state, paused) {
            (MatchState::Playing, true) => self.state = MatchState::Paused,
            (MatchState::Paused, false) => self.state = MatchState::Playing,
            _ => {}
        }
    }

    pub fn start_match(&mut self, ember: bool) {
        self.network_inputs.clear();
        self.net_camera_offset = Vec3::ZERO;
        let team = if ember { Team::Ember } else { Team::Glacier };
        self.players.clear();
        self.discs.clear();
        self.explosions.clear();
        self.smoke.clear();
        self.score = [0, 0];
        self.kills = 0;
        self.deaths = 0;
        self.time_left = MATCH_TIME;
        self.state = MatchState::Playing;
        self.acc = 0.0;
        self.trauma = 0.0;
        self.message = "DEPLOYED — TAKE THEIR FLAG".into();
        self.message_t = 3.2;
        self.events = "start".into();

        // Face the valley, not the rim. Ember sits at low Z.
        let yaw = if ember { std::f32::consts::PI } else { 0.0 };
        self.players.push(make_player(team, false, self.stand(ember), yaw, BotRole::Offense));
        self.player_id = 0;
        self.fill_match(ember, team, yaw);
    }

    /// A joined rift: you on the snow, no local bots. Other skiers come from the server.
    pub fn start_rift(&mut self, ember: bool) {
        let team = if ember { Team::Ember } else { Team::Glacier };
        self.players.clear();
        self.discs.clear();
        self.explosions.clear();
        self.smoke.clear();
        self.score = [0, 0];
        self.kills = 0;
        self.deaths = 0;
        self.time_left = MATCH_TIME;
        self.state = MatchState::Playing;
        self.acc = 0.0;
        self.trauma = 0.0;
        self.message = "IN THE RIFT".into();
        self.message_t = 2.4;
        self.events = "start".into();
        let yaw = if ember { std::f32::consts::PI } else { 0.0 };
        self.players.push(make_player(team, false, self.stand(ember), yaw, BotRole::Offense));
        self.player_id = 0;
        self.place_flags();
    }

    fn fill_match(&mut self, ember: bool, team: Team, yaw: f32) {
        for i in 0..2 {
            let o = Vec3::new((i as f32 - 0.5) * 6.0, 0.0, 4.0);
            let mut p = self.stand(ember) + o;
            p.y = self.ground(p.x, p.z) + 1.2;
            let role = if i == 0 { BotRole::Offense } else { BotRole::Defense };
            self.players.push(make_player(team, true, p, yaw, role));
        }
        let other = team.other();
        let other_ember = other == Team::Ember;
        let oyaw = if other_ember { std::f32::consts::PI } else { 0.0 };
        for i in 0..3 {
            let o = Vec3::new((i as f32 - 1.0) * 5.5, 0.0, 3.0);
            let mut p = self.stand(other_ember) + o;
            p.y = self.ground(p.x, p.z) + 1.2;
            let role = if i == 2 { BotRole::Defense } else { BotRole::Offense };
            self.players.push(make_player(other, true, p, oyaw, role));
        }

        self.place_flags();
    }

    fn place_flags(&mut self) {
        let ember = crate::terrain::info(self.map).ember;
        let glacier = crate::terrain::info(self.map).glacier;
        let eh = self.ground(ember.x, ember.z);
        let gh = self.ground(glacier.x, glacier.z);
        self.flags[0] = Flag {
            team: Team::Ember,
            pos: Vec3::new(ember.x, eh + 0.2, ember.z),
            home: Vec3::new(ember.x, eh + 0.2, ember.z),
            carrier: None,
            drop_timer: 0.0,
        };
        self.flags[1] = Flag {
            team: Team::Glacier,
            pos: Vec3::new(glacier.x, gh + 0.2, glacier.z),
            home: Vec3::new(glacier.x, gh + 0.2, glacier.z),
            carrier: None,
            drop_timer: 0.0,
        };
    }

    pub fn add_look(&mut self, dx: f32, dy: f32) {
        if self.state != MatchState::Playing || self.players.is_empty() {
            return;
        }
        let p = &mut self.players[self.player_id];
        if !p.alive {
            return;
        }
        p.yaw -= dx * SENS;
        p.pitch = (p.pitch - dy * SENS).clamp(-1.52, 1.52);
    }

    #[allow(dead_code)]
    pub fn apply_override(&mut self) {
        let Some(codes) = self.input.keys_override.clone() else {
            return;
        };
        let has = |c: &str| codes.iter().any(|k| k == c);
        let mut mx = 0.0;
        let mut mz = 0.0;
        if has("KeyW") || has("ArrowUp") {
            mz += 1.0;
        }
        if has("KeyS") || has("ArrowDown") {
            mz -= 1.0;
        }
        if has("KeyD") || has("ArrowRight") {
            mx += 1.0;
        }
        if has("KeyA") || has("ArrowLeft") {
            mx -= 1.0;
        }
        self.input.move_x = mx;
        self.input.move_z = mz;
        self.input.jump = has("Space");
        self.input.jet = has("Mouse2");
        if has("Mouse0") || has("KeyF") {
            self.input.fire = true;
        }
        if has("Digit2") {
            self.input.weapon = 1;
        }
        if has("Digit1") {
            self.input.weapon = 0;
        }
        if has("Digit3") { self.input.weapon = 2; }
    }

    pub fn tick(&mut self, dt: f32) {
        self.events.clear();
        self.flyby += dt;
        self.time += dt;
        self.hitmarker = (self.hitmarker - dt * 3.5).max(0.0);
        self.damage_flash = (self.damage_flash - dt * 2.8).max(0.0);
        self.trauma = (self.trauma - dt * 1.6).max(0.0);
        self.message_t = (self.message_t - dt).max(0.0);
        if self.message_t <= 0.0 {
            self.message.clear();
        }

        if self.state == MatchState::Flyby {
            return;
        }
        if self.state != MatchState::Playing {
            return;
        }

        if !self.players.is_empty() {
            let p = &mut self.players[self.player_id];
            p.weapon = self.input.weapon.min(2);
            p.yaw -= self.input.look_stick_x * 1.8 * dt;
            p.pitch = (p.pitch - self.input.look_stick_y * 1.4 * dt).clamp(-1.52, 1.52);
        }

        self.acc += dt.min(0.1);
        let mut n = 0;
        while self.acc >= STEP && n < 5 {
            self.physics_step();
            self.acc -= STEP;
            n += 1;
        }
    }

    fn physics_step(&mut self) {
        let dt = STEP;
        self.time_left -= dt;
        if self.time_left <= 0.0 {
            self.time_left = 0.0;
            self.state = MatchState::Ended;
            self.msg("TIME — MATCH OVER", 4.0);
            self.push_event("end");
            return;
        }

        self.think_bots(dt);
        self.step_players(dt);
        self.step_discs(dt);
        self.step_flags(dt);
        self.step_explosions(dt);
    }

    fn think_bots(&mut self, dt: f32) {
        let n = self.players.len();
        for i in 0..n {
            if !self.players[i].is_bot || !self.players[i].alive {
                continue;
            }
            self.players[i].bot_think -= dt;
            if self.players[i].bot_think > 0.0 {
                continue;
            }
            let wait = 0.18 + self.rng() * 0.22;
            let jx = (self.rng() - 0.5) * 12.0;
            let jz = (self.rng() - 0.5) * 8.0;
            self.players[i].bot_think = wait;
            let team = self.players[i].team;
            let role = self.players[i].bot_role;
            let carrying = self.players[i].carrying;
            let pos = self.players[i].pos;
            let enemy_flag_pos = self.flags[team.other().idx()].pos;
            let enemy_carrier = self.flags[team.other().idx()].carrier;
            let own_home = self.flags[team.idx()].home;
            let own_carrier = self.flags[team.idx()].carrier;

            let mut goal = enemy_flag_pos;
            if carrying.is_some() {
                goal = own_home;
            } else if own_carrier.is_some() && role != BotRole::Offense {
                if let Some(c) = own_carrier {
                    if c < n {
                        goal = self.players[c].pos;
                    }
                }
            } else if role == BotRole::Defense {
                goal = own_home + Vec3::new(jx, 0.0, jz);
            } else if let Some(c) = enemy_carrier {
                if c < n {
                    goal = self.players[c].pos;
                }
            }

            // Hunt nearby enemies.
            if role == BotRole::Hunter || carrying.is_none() {
                let mut best = 80.0;
                for (j, other) in self.players.iter().enumerate() {
                    if j == i || !other.alive || other.team == team {
                        continue;
                    }
                    let d = pos.distance(other.pos);
                    if d < best {
                        best = d;
                        if d < 48.0 && role != BotRole::Defense {
                            goal = other.pos;
                        }
                    }
                }
            }

            self.players[i].bot_goal = goal;

            // Aim at nearest enemy with lead.
            let mut aim_at: Option<Vec3> = None;
            let mut aim_d = 95.0;
            for (j, other) in self.players.iter().enumerate() {
                if j == i || !other.alive || other.team == team {
                    continue;
                }
                let d = pos.distance(other.pos);
                if d < aim_d {
                    aim_d = d;
                    let lead = d / DISC_SPEED;
                    aim_at = Some(other.pos + Vec3::Y * 1.1 + other.vel * lead * 0.85);
                }
            }
            if let Some(at) = aim_at {
                let dir = (at - (pos + Vec3::Y * 1.4)).normalize_or_zero();
                if dir.length_squared() > 0.1 {
                    self.players[i].yaw = (-dir.x).atan2(-dir.z);
                    self.players[i].pitch = dir.y.asin().clamp(-0.7, 0.7);
                }
                self.players[i].weapon = if aim_d < 28.0 { 1 } else { 0 };
            }
        }
    }

    fn step_players(&mut self, dt: f32) {
        let n = self.players.len();
        let jump_edge_player = self.input.jump && !self.input.jump_prev;
        self.input.jump_prev = self.input.jump;

        for i in 0..n {
            if self.players[i].remote {
                continue;
            }
            if !self.players[i].alive {
                self.players[i].respawn -= dt;
                if self.players[i].respawn <= 0.0 {
                    self.respawn(i);
                }
                continue;
            }

            let is_local = i == self.player_id && !self.players[i].is_bot;
            let (wish_x, wish_z, jump_held, jump_edge, fire, jet) = if let Some(input) = self.network_inputs.get(i) {
                let edge = input.jump && !self.players[i].jump_prev;
                self.players[i].jump_prev = input.jump;
                (input.move_x, input.move_z, input.jump, edge, input.fire, input.jet)
            } else if is_local {
                (
                    self.input.move_x.clamp(-1.0, 1.0),
                    self.input.move_z.clamp(-1.0, 1.0),
                    self.input.jump,
                    jump_edge_player,
                    self.input.fire,
                    self.input.jet,
                )
            } else {
                self.bot_wish(i)
            };

            let map = self.map;
            let edge = self.map_size() - 6.0;
            let disc_ready = is_local && self.players[i].weapon == 0
                && self.players[i].cooldown > 0.0 && self.players[i].cooldown <= dt;
            let land_hit = {
                let p = &mut self.players[i];
                p.cooldown = (p.cooldown - dt).max(0.0);
                let (fwd, right) = move_basis(p.yaw);
                let mut wish = right * wish_x + fwd * wish_z;
                let wlen = wish.length();
                if wlen > 1.0 {
                    wish /= wlen;
                }

                p.vel.y -= gravity_for_speed(Vec2::new(p.vel.x, p.vel.z).length()) * dt;

                let h = crate::terrain::height_on(map, p.pos.x, p.pos.z);
                let nrm = crate::terrain::normal_on(map, p.pos.x, p.pos.z);
                let ground_y = h + PLAYER_RADIUS;
                let was_ground = p.on_ground;
                // Contact depends on motion relative to the slope, not world Y:
                // a skier climbing a ramp can have positive Y velocity.
                let contact_normal = ski_normal(map, p.pos.x, p.pos.z);
                p.on_ground = p.pos.y <= ground_y + 0.12
                    && p.vel.dot(contact_normal) <= 0.5;
                if p.on_ground {
                    p.coyote = 0.14;
                } else {
                    p.coyote = (p.coyote - dt).max(0.0);
                }

                if p.pos.y < ground_y {
                    p.pos.y = ground_y;
                    // A held jump is frictionless. Don't let the bumpy contact
                    // normal eat the slide; the ski projects gravity itself.
                    if !jump_held {
                        let vn = p.vel.dot(nrm);
                        if vn < 0.0 {
                            p.vel -= nrm * vn;
                        }
                    }
                    p.on_ground = true;
                }

                let grounded = p.on_ground || p.coyote > 0.0;
                let horiz_speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
                let mass = player_mass(p);
                // A fresh press can hop; holding through landing always skis.
                // This removes the repeated low-speed pogo of the earlier hybrid.
                let steep = nrm.y < 0.82;
                let hop = if jump_edge && grounded && !steep {
                    jump_scale(horiz_speed, p.vel.y)
                } else {
                    0.0
                };
                if hop > 0.01 {
                    let impulse = (JUMP_IMPULSE * MASS / mass) * hop;
                    p.vel += nrm * impulse;
                    p.on_ground = false;
                    p.coyote = 0.0;
                }
                p.skiing = jump_held && p.on_ground && (steep || hop <= 0.01);

                p.jetting = false;
                if jet && p.energy >= MIN_JET_ENERGY {
                    p.jetting = true;
                    // Look does not aim the jet. Climb stays up. WASD is the
                    // direction: it can turn a fast line, and it only adds speed
                    // up to 72 km/h.
                    let scale = MASS / mass;
                    let up_speed = p.vel.y.max(0.0);
                    p.vel.y += JET_ACCEL * scale * jet_falloff(up_speed) * dt;
                    let before = Vec3::new(p.vel.x, 0.0, p.vel.z);
                    let before_h = before.length();
                    if wish.length_squared() > 0.04 {
                        let wish_dir = wish.normalize_or_zero();
                        let along = before.dot(wish_dir).max(0.0);
                        let push = JET_HORIZ_ACCEL * scale * jet_falloff(along) * wish.length().min(1.0);
                        p.vel += wish_dir * push * dt;
                        let after = Vec3::new(p.vel.x, 0.0, p.vel.z);
                        let now = after.length();
                        let cap = if before_h >= JET_THRUST_CAP {
                            before_h
                        } else {
                            JET_THRUST_CAP
                        };
                        if now > cap && now > 0.01 {
                            let s = cap / now;
                            p.vel.x = after.x * s;
                            p.vel.z = after.z * s;
                        }
                    }
                    p.on_ground = false;
                    p.skiing = false;
                    p.coyote = 0.0;
                    p.energy = (p.energy - ENERGY_JET * dt).max(0.0);
                } else {
                    p.energy = (p.energy + ENERGY_REGEN * dt).min(ENERGY_MAX);
                }

                if p.on_ground && !p.skiing {
                    // Approach the desired walking speed; releasing ski brakes
                    // progressively instead of instantly deleting route momentum.
                    let horizontal = Vec3::new(p.vel.x, 0.0, p.vel.z);
                    let delta = wish * WALK_MAX - horizontal;
                    let accel = if horizontal.length() > WALK_MAX || wish.length_squared() < 0.01 {
                        GROUND_BRAKE
                    } else { WALK_ACCEL };
                    p.vel += delta.clamp_length_max(accel * dt);
                } else if p.skiing {
                    ride_ski(map, p, wish, dt);
                } else {
                    // Ascend-style air control: coast without drag, steer with
                    // WASD. Above walking speed, steering cannot add energy.
                    if !p.jetting {
                        steer_horizontal(p, wish, AIR_ACCEL, WALK_MAX, dt);
                    }
                }
                apply_speed_limits(p, dt);

                let impact_speed = move_over_terrain(map, p, jump_held, dt);
                if p.skiing {
                    let ground_y = crate::terrain::height_on(map, p.pos.x, p.pos.z) + PLAYER_RADIUS;
                    let drop = p.pos.y - ground_y;
                    let nrm = ski_normal(map, p.pos.x, p.pos.z);
                    // Catch a nearby descending surface, never pull an outgoing
                    // velocity down to a crest. Ramps must launch the player.
                    if drop <= 0.0 || (drop < 0.15 && p.vel.dot(nrm) <= 0.0) {
                        p.pos.y = ground_y;
                        let vn = p.vel.dot(nrm);
                        if vn < 0.0 {
                            p.vel -= nrm * vn;
                        }
                        p.on_ground = true;
                    } else {
                        p.on_ground = false;
                        p.skiing = false;
                    }
                }

                // Bounds.
                if p.pos.x < 6.0 {
                    p.pos.x = 6.0;
                    p.vel.x = p.vel.x.abs() * 0.4;
                }
                if p.pos.x > edge {
                    p.pos.x = edge;
                    p.vel.x = -p.vel.x.abs() * 0.4;
                }
                if p.pos.z < 6.0 {
                    p.pos.z = 6.0;
                    p.vel.z = p.vel.z.abs() * 0.4;
                }
                if p.pos.z > edge {
                    p.pos.z = edge;
                    p.vel.z = -p.vel.z.abs() * 0.4;
                }

                let mut land_hit = 0.0;
                if !was_ground && p.on_ground && impact_speed > 18.0 {
                    if !self.predicting { p.health -= landing_damage(impact_speed); }
                    land_hit = 0.25;
                }
                land_hit
            };

            if land_hit > 0.0 && i == self.player_id {
                self.trauma = (self.trauma + land_hit).min(1.0);
            }
            if disc_ready { self.push_event("disc_ready"); }

            self.collide_pillars(i);

            if fire && self.players[i].cooldown <= 0.0 && self.players[i].alive {
                self.shoot(i);
            }

            if !self.predicting && self.players[i].health <= 0.0 && self.players[i].alive {
                self.kill(i, None);
            }
        }
    }

    fn bot_wish(&mut self, i: usize) -> (f32, f32, bool, bool, bool, bool) {
        let p = &self.players[i];
        let to = p.bot_goal - p.pos;
        let yaw = p.yaw;
        let pitch = p.pitch;
        let team = p.team;
        let pos = p.pos;
        let on_ground = p.on_ground;
        let (fwd, right) = move_basis(yaw);
        let horiz = Vec3::new(to.x, 0.0, to.z);
        let hlen = horiz.length();
        let wish = if hlen > 1.0 { horiz / hlen } else { horiz };
        let mx = wish.dot(right).clamp(-1.0, 1.0);
        let mz = wish.dot(fwd).clamp(-1.0, 1.0);
        let mut fire = false;
        for (j, o) in self.players.iter().enumerate() {
            if j == i || !o.alive || o.team == team {
                continue;
            }
            let d = pos.distance(o.pos);
            let dir = look_dir(yaw, pitch);
            let to_e = ((o.pos + Vec3::Y) - (pos + Vec3::Y * 1.4)).normalize_or_zero();
            if d < 78.0 && dir.dot(to_e) > 0.86 {
                fire = true;
            }
        }
        let energy = p.energy;
        let uphill = to.y > 5.0;
        let r = self.rng();
        let jump_edge = on_ground && r < 0.02;
        let jet = energy >= MIN_JET_ENERGY && (!on_ground || uphill);
        (mx, mz.max(0.15), true, jump_edge, fire && r < 0.7, jet)
    }

    fn collide_pillars(&mut self, i: usize) {
        let pos = self.players[i].pos;
        let mut vel = self.players[i].vel;
        let mut new_pos = pos;
        let map = self.map;
        let pillars: Vec<(f32, f32, f32, f32)> = self.pillars.iter().map(|p| (p.x, p.z, p.r, p.h)).collect();
        for (px, pz, pr, ph) in pillars {
            let base_y = crate::terrain::height_on(map, px, pz);
            if pos.y > base_y + ph + 1.2 {
                continue;
            }
            let dx = new_pos.x - px;
            let dz = new_pos.z - pz;
            let d = dx.hypot(dz);
            let min = pr + PLAYER_RADIUS;
            if d < min {
                let (nx, nz) = if d > 1e-3 { (dx / d, dz / d) } else { (1.0, 0.0) };
                new_pos.x = px + nx * min;
                new_pos.z = pz + nz * min;
                let vn = vel.x * nx + vel.z * nz;
                if vn < 0.0 {
                    vel.x -= nx * vn * 1.35;
                    vel.z -= nz * vn * 1.35;
                }
            }
        }
        self.players[i].pos = new_pos;
        self.players[i].vel = vel;
    }

    fn shoot(&mut self, i: usize) {
        self.players[i].shots += 1;
        let p = &self.players[i];
        let dir = look_dir(p.yaw, p.pitch);
        let kind = p.weapon;
        let eye = p.pos + Vec3::Y * EYE;
        let spd = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
        let origin = muzzle_origin(eye, dir, kind, camera_fov(spd));
        // Leave the muzzle, but steer onto the crosshair so a close shot still hits.
        let aim = eye + dir * 80.0;
        let shot = (aim - origin).normalize_or_zero();
        let shot = if shot.length_squared() > 0.5 { shot } else { dir };
        let speed = match kind { 0 => DISC_SPEED, 1 => BOLT_SPEED, _ => 48.0 };
        let inherit = if kind == 0 { DISC_INHERIT } else { 0.75 };
        let mut vel = shot * speed + p.vel * inherit;
        let is_bot = p.is_bot;
        let team = p.team;
        if kind == 1 {
            let (right, up) = view_basis(dir);
            vel += (right * (self.rng() - 0.5) + up * (self.rng() - 0.5)) * 6.0;
        }
        if is_bot {
            let a = self.rng() - 0.5;
            let b = self.rng() - 0.5;
            let c = self.rng() - 0.5;
            vel += Vec3::new(a, b, c) * 3.2;
        }
        self.discs.push(Disc {
            pos: origin,
            vel,
            team,
            owner: i,
            life: match kind { 0 => 5.0, 1 => 1.2, _ => 2.0 },
            kind,
            spin: 0.0,
        });
        {
            let p = &mut self.players[i];
            p.cooldown = weapon_reload(kind);
            if (i == self.player_id || !self.network_inputs.is_empty()) && kind != 0 {
                p.pitch = (p.pitch + if kind == 1 { 0.002 } else { 0.012 }).min(1.5);
            }
        }
        if i == self.player_id {
            self.trauma = (self.trauma + if kind == 1 { 0.015 } else { 0.08 }).min(1.0);
            self.push_event(match kind { 0 => "disc", 1 => "chain", _ => "grenade" });
        }
    }

    fn step_discs(&mut self, dt: f32) {
        for puff in &mut self.smoke {
            puff.age += dt;
            puff.pos.y += dt * 0.35;
        }
        self.smoke.retain(|p| p.age < 0.5);
        let n_sub = 4;
        let sdt = dt / n_sub as f32;
        let mut explode: Vec<(Vec3, usize, u8, Team)> = Vec::new();
        let mut keep = Vec::new();
        let mut bullet_hits = Vec::new();

        let map = self.map;
        let far = self.map_size() - 1.0;
        for mut d in self.discs.drain(..) {
            if d.kind == 2 && self.smoke.len() < 256 {
                self.smoke.push(SmokePuff { pos: d.pos, age: 0.0 });
            }
            d.life -= dt;
            if d.life <= 0.0 {
                if d.kind != 1 { explode.push((d.pos, d.owner, d.kind, d.team)); }
                continue;
            }
            let mut dead = false;
            for _ in 0..n_sub {
                d.spin += sdt * 42.0;
                let grav = if d.kind == 2 { 1.0 } else { 0.0 };
                d.vel.y -= GRAVITY * grav * sdt;
                let next = d.pos + d.vel * sdt;
                let mut hit = obstacle_hit(map, &self.pillars, d.pos, next, 0.12);
                // Keep the actual boundary impact height, rather than moving
                // an airborne explosion down onto the terrain.
                for (a, b) in [(d.pos.x, next.x), (d.pos.z, next.z)] {
                    if b < 1.0 || b > far {
                        let t = if (b - a).abs() > 1e-6 {
                            ((b.clamp(1.0, far) - a) / (b - a)).clamp(0.0, 1.0)
                        } else { 0.0 };
                        hit = Some(hit.map_or(t, |old| old.min(t)));
                    }
                }
                let mut victim = None;
                for (idx, pl) in self.players.iter().enumerate() {
                    if !pl.alive || pl.team == d.team {
                        continue;
                    }
                    let c = pl.pos + Vec3::Y * 0.9;
                    if let Some(t) = segment_sphere(d.pos, next, c, PLAYER_RADIUS + 0.45) {
                        if hit.is_none_or(|old| t < old) {
                            hit = Some(t);
                            victim = Some(idx);
                        }
                    }
                }
                if let Some(t) = hit {
                    let contact = d.pos.lerp(next, t);
                    if d.kind == 2 {
                        // Brief launch safety lets close surfaces bounce the
                        // shell. Armed shells detonate on their next contact.
                        if d.life <= 1.65 {
                            explode.push((contact, d.owner, d.kind, d.team));
                            dead = true;
                            break;
                        }
                        let normal = if let Some(idx) = victim {
                            (contact - (self.players[idx].pos + Vec3::Y * 0.9)).normalize_or_zero()
                        } else { grenade_contact_normal(map, &self.pillars, contact, far) };
                        let inward = d.vel.dot(normal);
                        if inward < 0.0 { d.vel -= normal * inward * 1.5; }
                        d.vel *= 0.82;
                        d.pos = contact + normal * 0.03;
                        continue;
                    }
                    if d.kind == 1 {
                        if let Some(idx) = victim { bullet_hits.push((idx, d.owner)); }
                    } else { explode.push((contact, d.owner, d.kind, d.team)); }
                    dead = true;
                    break;
                }
                d.pos = next;
            }
            if !dead {
                keep.push(d);
            }
        }
        self.discs = keep;
        for (idx, owner) in bullet_hits {
            if !self.players[idx].alive { continue; }
            self.players[idx].health -= 8.0;
            if let Some(p) = self.players.get_mut(owner) { p.hits += 1; }
            if owner == self.player_id { self.hitmarker = 1.0; self.push_event("hit"); }
            if idx == self.player_id { self.damage_flash = 0.35; self.push_event("pain"); }
            if self.players[idx].health <= 0.0 {
                self.kill(idx, Some(owner));
                if owner == self.player_id { self.kills += 1; self.msg("FRAG", 1.1); }
            }
        }
        for (pos, owner, kind, team) in explode {
            self.explode(pos, owner, kind, team);
        }
    }

    fn explode(&mut self, pos: Vec3, owner: usize, kind: u8, team: Team) {
        self.blast_serial += 1;
        let max_r = if kind == 0 { DISC_RADIUS } else { 9.0 };
        self.explosions.push(Explosion {
            pos,
            age: 0.0,
            max_r,
            kind,
        });
        self.push_event("boom");
        let dmg_core = if kind == 0 { 52.0 } else { 68.0 };
        let mut killed_by_player = false;
        let pid = self.player_id;
        for i in 0..self.players.len() {
            if !self.players[i].alive {
                continue;
            }
            let d = self.players[i].pos.distance(pos);
            if d > max_r {
                continue;
            }
            let fall = (1.0 - d / max_r).clamp(0.0, 1.0);
            let same = self.players[i].team == team;
            if same && i != owner {
                continue;
            }
            let body = self.players[i].pos + Vec3::Y * 0.7;
            // Terrain and midfield pillars shield splash as well as projectiles.
            if obstacle_hit(self.map, &self.pillars, pos + Vec3::Y * 0.02, body, 0.0)
                .is_some_and(|t| t < 0.995) { continue; }
            let mul = if i == owner { 0.4 } else { 1.0 };
            // Preserve point-blank damage, but taper to zero at the radius;
            // the old constant 12 caused a hard damage jump at its edge.
            let dmg = (12.0 * fall + dmg_core * fall * fall) * mul;
            self.players[i].health -= dmg;
            // Kick from the blast, not from the chest, so a disc at your feet throws you up.
            let away = (body - pos).normalize_or_zero();
            let kick = if kind == 0 { DISC_KICK } else { 6.0 };
            let push = away * kick * fall;
            self.players[i].vel += push;
            if push.y > 4.0 {
                self.players[i].on_ground = false;
                self.players[i].skiing = false;
                self.players[i].coyote = 0.0;
            }
            if i == pid {
                self.damage_flash = 0.7;
                self.trauma = (self.trauma + 0.35 * fall).min(1.0);
                self.push_event("pain");
            }
            if owner == pid && i != pid && !same {
                self.hitmarker = 1.0;
                self.push_event("hit");
            }
            if i != owner && !same {
                if let Some(p) = self.players.get_mut(owner) { p.hits += 1; }
            }
            if self.players[i].health <= 0.0 {
                if owner == pid && i != pid {
                    killed_by_player = true;
                }
                self.kill(i, Some(owner));
            }
        }
        if killed_by_player {
            self.kills += 1;
            self.msg("FRAG", 1.1);
        }
        if owner == pid {
            self.trauma = (self.trauma + 0.12).min(1.0);
        }
    }

    fn kill(&mut self, i: usize, killer: Option<usize>) {
        if !self.players[i].alive {
            return;
        }
        self.players[i].alive = false;
        self.players[i].losses += 1;
        if let Some(k) = killer.filter(|&k| k != i && k < self.players.len()
            && self.players[k].team != self.players[i].team) {
            self.players[k].frags += 1;
        }
        self.players[i].health = 0.0;
        self.players[i].respawn = 3.4;
        self.players[i].jetting = false;
        self.players[i].skiing = false;
        if let Some(team) = self.players[i].carrying.take() {
            self.drop_flag(team, self.players[i].pos);
        }
        if i == self.player_id {
            self.deaths += 1;
            self.msg("YOU WERE FRAGGED", 2.2);
            self.push_event("death");
            self.trauma = 0.8;
        }
        self.explosions.push(Explosion {
            pos: self.players[i].pos + Vec3::Y,
            age: 0.0,
            max_r: 4.5,
            kind: 2,
        });
    }

    fn respawn(&mut self, i: usize) {
        let ember = self.players[i].team == Team::Ember;
        let mut pos = self.stand(ember);
        pos.x += (self.rng() - 0.5) * 8.0;
        pos.z += (self.rng() - 0.5) * 6.0;
        pos.y = self.ground(pos.x, pos.z) + 1.2;
        let p = &mut self.players[i];
        p.pos = pos;
        p.vel = Vec3::ZERO;
        p.health = 100.0;
        p.energy = ENERGY_MAX;
        p.alive = true;
        p.carrying = None;
        p.yaw = if ember { std::f32::consts::PI } else { 0.0 };
        p.pitch = 0.0;
        p.cooldown = 0.4;
        p.skiing = false;
        p.jetting = false;
        p.on_ground = true;
        p.coyote = 0.0;
        p.jump_prev = false;
        if i == self.player_id {
            self.msg("REDEPLOYED", 1.4);
        }
    }

    fn drop_flag(&mut self, team: Team, pos: Vec3) {
        let y = self.ground(pos.x, pos.z) + 0.4;
        let f = &mut self.flags[team.idx()];
        f.carrier = None;
        f.pos = Vec3::new(pos.x, y, pos.z);
        f.drop_timer = 14.0;
        self.msg(
            if team == self.players[self.player_id].team {
                "YOUR FLAG DROPPED"
            } else {
                "ENEMY FLAG DROPPED"
            },
            2.0,
        );
        self.push_event("drop");
    }

    fn step_flags(&mut self, dt: f32) {
        for fi in 0..2 {
            if let Some(c) = self.flags[fi].carrier {
                if c < self.players.len() && self.players[c].alive {
                    self.flags[fi].pos = self.players[c].pos + Vec3::Y * 2.2;
                    continue;
                } else {
                    let pos = self.flags[fi].pos;
                    let team = self.flags[fi].team;
                    self.drop_flag(team, pos);
                }
            }
            if self.flags[fi].carrier.is_none() && self.flags[fi].drop_timer > 0.0 {
                self.flags[fi].drop_timer -= dt;
                let pos = self.flags[fi].pos;
                let y = self.ground(pos.x, pos.z) + 0.4;
                self.flags[fi].pos.y = y;
                if self.flags[fi].drop_timer <= 0.0 {
                    self.return_flag(fi);
                }
            }
        }

        // Pickups / captures.
        let n = self.players.len();
        for i in 0..n {
            if !self.players[i].alive {
                continue;
            }
            let team = self.players[i].team;
            let pos = self.players[i].pos;
            let enemy = team.other().idx();
            let own = team.idx();

            // Return own dropped flag.
            if self.flags[own].carrier.is_none() {
                let at_home = self.flags[own].pos.distance(self.flags[own].home) < 2.5;
                if !at_home && pos.distance(self.flags[own].pos) < 2.4 {
                    self.return_flag(own);
                    if i == self.player_id {
                        self.msg("FLAG RETURNED", 2.0);
                    }
                }
            }

            // Grab enemy flag.
            if self.players[i].carrying.is_none() && self.flags[enemy].carrier.is_none() {
                if pos.distance(self.flags[enemy].pos) < 2.6 {
                    self.flags[enemy].carrier = Some(i);
                    self.flags[enemy].drop_timer = 0.0;
                    self.players[i].carrying = Some(self.flags[enemy].team);
                    self.push_event("flag");
                    if i == self.player_id {
                        self.msg("YOU HAVE THE FLAG — GET HOME", 3.0);
                    } else if self.players[i].team != self.players[self.player_id].team {
                        self.msg("YOUR FLAG HAS BEEN TAKEN", 3.0);
                    } else {
                        self.msg("ALLY HAS THEIR FLAG", 2.0);
                    }
                }
            }

            // Capture.
            if self.players[i].carrying.is_some() {
                let own_home = self.flags[own].home;
                let own_at_home = self.flags[own].carrier.is_none()
                    && self.flags[own].pos.distance(own_home) < 3.0;
                if own_at_home && pos.distance(own_home) < 4.2 {
                    self.capture(i);
                }
            }
        }
    }

    fn return_flag(&mut self, fi: usize) {
        let home = self.flags[fi].home;
        self.flags[fi].pos = home;
        self.flags[fi].carrier = None;
        self.flags[fi].drop_timer = 0.0;
        self.push_event("return");
    }

    fn capture(&mut self, i: usize) {
        let team = self.players[i].team;
        if let Some(ft) = self.players[i].carrying.take() {
            self.return_flag(ft.idx());
        }
        self.score[team.idx()] += 1;
        self.push_event("capture");
        self.trauma = 0.55;
        if i == self.player_id {
            self.msg("CAPTURE", 3.2);
        } else if team == self.players[self.player_id].team {
            self.msg("ALLY CAPTURED THE FLAG", 3.0);
        } else {
            self.msg("ENEMY CAPTURED YOUR FLAG", 3.0);
        }
        if self.score[team.idx()] >= CAPTURES {
            self.state = MatchState::Ended;
            self.msg(
                if team == self.players[self.player_id].team {
                    "VICTORY"
                } else {
                    "DEFEAT"
                },
                8.0,
            );
            self.push_event("end");
        }
    }

    fn step_explosions(&mut self, dt: f32) {
        for e in &mut self.explosions {
            e.age += dt;
        }
        self.explosions.retain(|e| e.age < 0.55);
    }

    fn msg(&mut self, s: &str, t: f32) {
        self.message = s.into();
        self.message_t = t;
    }

    fn push_event(&mut self, e: &str) {
        if !self.events.is_empty() {
            self.events.push(',');
        }
        self.events.push_str(e);
    }

    pub fn player_yaw(&self) -> f32 {
        self.players.get(self.player_id).map(|p| p.yaw).unwrap_or(0.0)
    }
    pub fn player_speed(&self) -> f32 {
        self.players
            .get(self.player_id)
            .map(|p| Vec2::new(p.vel.x, p.vel.z).length())
            .unwrap_or(0.0)
    }

    pub fn player_pos(&self) -> Vec3 {
        self.players
            .get(self.player_id)
            .map(|p| p.pos)
            .unwrap_or(Vec3::ZERO)
    }

    pub fn camera(&self) -> (Vec3, Vec3, f32) {
        if self.state == MatchState::Flyby || self.players.is_empty() {
            let t = self.flyby;
            let c = self.map_size() * 0.5;
            let span = (self.map_size() * 0.22).clamp(52.0, 380.0);
            let eye = Vec3::new(
                c + (t * 0.18).sin() * span * 0.6,
                self.ground(c, c) + 28.0,
                c + (t * 0.18).cos() * span,
            );
            let target = Vec3::new(c, self.ground(c, c) + 8.0, c);
            let dir = (target - eye).normalize_or_zero();
            return (eye, dir, 62.0);
        }
        let p = &self.players[self.player_id];
        let shake = self.trauma * self.trauma;
        let t = self.time;
        let off = Vec3::new(
            (t * 37.1).sin() * shake * 0.18,
            (t * 41.7).cos() * shake * 0.14,
            (t * 29.3).sin() * shake * 0.12,
        );
        let spd = Vec2::new(p.vel.x, p.vel.z).length();
        let bob = if p.on_ground && !p.skiing && spd > 2.0 && p.alive {
            (self.time * 9.5).sin() * 0.04
        } else {
            0.0
        };
        let eye = p.pos + Vec3::Y * EYE + off + Vec3::Y * bob + self.net_camera_offset;
        let dir = look_dir(p.yaw, p.pitch);
        let fov = camera_fov(spd);
        (eye, dir, fov)
    }

    pub fn hud_json(&self) -> String {
        let p = self.players.get(self.player_id);
        let health = p.map(|x| x.health.max(0.0)).unwrap_or(0.0);
        let energy = p.map(|x| x.energy).unwrap_or(0.0);
        let speed = self.player_speed();
        let yaw = self.player_yaw();
        let pitch = p.map(|x| x.pitch).unwrap_or(0.0);
        let pos = self.player_pos();
        let team = p.map(|x| x.team.idx()).unwrap_or(0);
        let weapon = p.map(|x| x.weapon).unwrap_or(0);
        let carrying = p.and_then(|x| x.carrying).map(|t| t.idx() as i32).unwrap_or(-1);
        let on_g = p.map(|x| if x.on_ground { 1 } else { 0 }).unwrap_or(0);
        let ski = p.map(|x| if x.skiing { 1 } else { 0 }).unwrap_or(0);
        let jet = p.map(|x| if x.jetting { 1 } else { 0 }).unwrap_or(0);
        let alive = p.map(|x| if x.alive { 1 } else { 0 }).unwrap_or(0);
        let cd = p.map(|x| x.cooldown).unwrap_or(0.0);
        let own_flag_home = self.flags.get(team).map(|f| {
            if f.carrier.is_none() && f.pos.distance(f.home) < 3.0 {
                1
            } else if f.carrier.is_some() {
                2
            } else {
                3
            }
        }).unwrap_or(1);
        let winner = if self.state == MatchState::Ended {
            if self.score[0] > self.score[1] {
                0
            } else if self.score[1] > self.score[0] {
                1
            } else {
                -1
            }
        } else {
            -2
        };
        let mut blips = String::new();
        for (i, pl) in self.players.iter().enumerate() {
            if !pl.alive {
                continue;
            }
            if !blips.is_empty() {
                blips.push(';');
            }
            blips.push_str(&format!(
                "{:.0},{:.0},{},{}",
                pl.pos.x,
                pl.pos.z,
                pl.team.idx(),
                if i == self.player_id { 1 } else { 0 }
            ));
        }
        for f in &self.flags {
            blips.push_str(&format!(
                ";{:.0},{:.0},{},2",
                f.pos.x,
                f.pos.z,
                f.team.idx()
            ));
        }
        let msg = self.message.replace('"', "");
        format!(
            "{{\"health\":{:.1},\"energy\":{:.1},\"speed\":{:.1},\"yaw\":{:.4},\"pitch\":{:.4},\"px\":{:.2},\"py\":{:.2},\"pz\":{:.2},\"ember\":{},\"glacier\":{},\"time\":{:.1},\"state\":{},\"weapon\":{},\"flag\":{},\"ownFlag\":{},\"hit\":{:.2},\"flash\":{:.2},\"msg\":\"{}\",\"kills\":{},\"deaths\":{},\"winner\":{},\"team\":{},\"onGround\":{},\"ski\":{},\"jet\":{},\"alive\":{},\"cd\":{:.2},\"events\":\"{}\",\"blips\":\"{}\",\"mapSize\":{:.0}}}",
            health,
            energy,
            speed,
            yaw,
            pitch,
            pos.x,
            pos.y,
            pos.z,
            self.score[0],
            self.score[1],
            self.time_left,
            self.state as u8,
            weapon,
            carrying,
            own_flag_home,
            self.hitmarker,
            self.damage_flash,
            msg,
            self.kills,
            self.deaths,
            winner,
            team,
            on_g,
            ski,
            jet,
            alive,
            cd,
            self.events,
            blips,
            self.map_size()
        )
    }
}

#[cfg(test)]
mod controls {
    use super::*;

    fn solo_airborne() -> World {
        let mut world = World::new();
        world.start_match(true);
        world.players.truncate(1);
        let p = &mut world.players[0];
        p.pos = Vec3::new(128.0, 150.0, 128.0);
        p.vel = Vec3::ZERO;
        p.yaw = 0.0;
        p.pitch = 0.0;
        p.on_ground = false;
        world
    }

    #[test]
    fn bullets_hit_one_target_without_splash_or_expiry_explosions() {
        let mut world = solo_airborne();
        for x in [128.0, 130.0] {
            world.players.push(make_player(Team::Glacier, false,
                Vec3::new(x, 150.0, 120.0), 0.0, BotRole::Defense));
        }
        world.discs.push(Disc { pos: Vec3::new(128.0, 150.9, 125.0),
            vel: Vec3::new(0.0, 0.0, -1000.0), team: Team::Ember,
            owner: 0, life: 1.0, kind: 1, spin: 0.0 });
        world.step_discs(STEP);
        assert_eq!(world.players[1].health, 92.0);
        assert_eq!(world.players[2].health, 100.0);
        assert!(world.explosions.is_empty());
        assert!(world.discs.is_empty());
        world.players[0].weapon = 1;
        world.shoot(0);
        world.discs[0].life = 0.001;
        world.step_discs(STEP);
        assert!(world.explosions.is_empty());
        assert!(world.discs.is_empty());
    }

    #[test]
    fn grenade_bounces_then_explodes_once_at_fuse_end() {
        for map in [MapId::Valley, MapId::Raindance] {
            let mut world = solo_airborne();
            world.set_map(map);
            let home = crate::terrain::info(map).ember;
            let ground = crate::terrain::height_on(map, home.x, home.z);
            world.discs.push(Disc { pos: Vec3::new(home.x, ground + 0.25, home.z),
                vel: Vec3::new(0.0, -30.0, 0.0), team: Team::Ember,
                owner: 0, life: 2.0, kind: 2, spin: 0.0 });
            world.step_discs(STEP);
            assert_eq!(world.discs.len(), 1);
            assert!(world.discs[0].vel.y > 0.0);
            assert!(world.explosions.is_empty());
            world.discs[0].life = 0.001;
            world.step_discs(STEP);
            assert!(world.discs.is_empty());
            assert_eq!(world.explosions.len(), 1);
            world.step_discs(STEP);
            assert_eq!(world.explosions.len(), 1);
        }
    }

    #[test]
    fn pillar_stops_bullets_and_bounces_grenades_without_detonation() {
        for kind in [1, 2] {
            let mut world = solo_airborne();
            world.pillars = vec![Pillar { x: 128.0, z: 124.0, r: 1.0, h: 200.0 }];
            world.discs.push(Disc { pos: Vec3::new(128.0, 150.0, 126.0),
                vel: Vec3::new(0.0, 0.0, -400.0), team: Team::Ember,
                owner: 0, life: 2.0, kind, spin: 0.0 });
            world.step_discs(STEP);
            assert!(world.explosions.is_empty());
            if kind == 1 { assert!(world.discs.is_empty()); }
            else { assert_eq!(world.discs.len(), 1); assert!(world.discs[0].vel.z > 0.0); }
        }
    }

    #[test]
    fn armed_grenade_detonates_on_contact_and_smoke_outlives_it() {
        let mut world = solo_airborne();
        world.pillars = vec![Pillar { x: 128.0, z: 124.0, r: 1.0, h: 200.0 }];
        world.discs.push(Disc { pos: Vec3::new(128.0, 150.0, 126.0),
            vel: Vec3::new(0.0, 0.0, -400.0), team: Team::Ember,
            owner: 0, life: 1.5, kind: 2, spin: 0.0 });
        world.step_discs(STEP);
        assert!(world.discs.is_empty());
        assert_eq!(world.explosions.len(), 1);
        assert!(!world.smoke.is_empty());
        world.step_discs(0.6);
        assert!(world.smoke.is_empty());
        assert_eq!(world.explosions.len(), 1);
    }

    #[test]
    fn grenade_arcs_and_third_slot_can_fire() {
        let mut world = solo_airborne();
        world.input.weapon = 2;
        world.input.fire = true;
        step(&mut world, 1);
        world.input.fire = false;
        assert_eq!(world.players[0].weapon, 2);
        assert_eq!(world.discs[0].kind, 2);
        let vy = world.discs[0].vel.y;
        world.step_discs(0.1);
        assert!(world.discs[0].vel.y < vy - 1.0);
        assert!(world.explosions.is_empty());
    }

    #[test]
    fn landing_damage_is_halved_and_rounded_down() {
        for (speed, damage) in [(0.0, 0.0), (18.0, 0.0), (19.0, 0.0),
            (20.0, 1.0), (23.0, 3.0), (30.0, 8.0), (80.0, 11.0)] {
            assert_eq!(landing_damage(speed), damage);
        }
    }

    #[test]
    fn a_fast_landing_is_resolved_before_the_frame_is_drawn() {
        for map in [MapId::Valley, MapId::Raindance] {
            let mut world = solo_airborne();
            world.set_map(map);
            let home = crate::terrain::info(map).ember;
            let floor = crate::terrain::height_on(map, home.x, home.z) + PLAYER_RADIUS;
            world.players[0].pos = Vec3::new(home.x, floor + 0.25, home.z);
            world.players[0].vel = Vec3::new(50.0, -80.0, 0.0);
            world.input.jump = true;
            step(&mut world, 1);
            let p = &world.players[0];
            let floor = crate::terrain::height_on(map, p.pos.x, p.pos.z) + PLAYER_RADIUS;
            assert!(p.pos.y >= floor - 0.001, "{map:?}: a fast landing must not render below ground");
            assert!(p.vel.is_finite() && p.pos.is_finite());
        }
    }

    #[test]
    fn fast_discs_hit_the_nearest_target_between_sample_points() {
        let mut world = solo_airborne();
        for z in [118.0, 125.0] {
            world.players.push(make_player(Team::Glacier, false,
                Vec3::new(128.0, 150.0 + EYE - 0.9, z), 0.0, BotRole::Defense));
        }
        world.shoot(0);
        world.discs[0].vel = Vec3::new(0.0, 0.0, -2000.0);
        world.step_discs(STEP);
        assert!(world.discs.is_empty());
        assert_eq!(world.explosions.len(), 1);
        assert!(world.explosions[0].pos.z > 125.0, "nearest target wins regardless of player list order");
        assert!(world.players[2].health < 100.0);
    }

    #[test]
    fn pillars_stop_discs_and_shield_targets_from_splash() {
        let mut world = solo_airborne();
        world.pillars = vec![Pillar { x: 128.0, z: 124.0, r: 1.0, h: 200.0 }];
        world.players.push(make_player(Team::Glacier, false,
            Vec3::new(128.0, 150.0, 122.0), 0.0, BotRole::Defense));
        world.shoot(0);
        world.discs[0].vel = Vec3::new(0.0, 0.0, -2000.0);
        world.step_discs(STEP);
        assert!(world.discs.is_empty());
        assert!(world.explosions[0].pos.z > 124.8);
        assert_eq!(world.players[1].health, 100.0, "solid cover must shield splash");
    }

    #[test]
    fn disc_aim_stays_on_the_eye_line_and_does_not_drift_up() {
        let mut world = solo_airborne();
        let initial_pitch = 0.3;
        world.players[0].pitch = initial_pitch;
        for _ in 0..5 { world.shoot(0); }
        assert_eq!(world.players[0].pitch, initial_pitch);
        let p = &world.players[0];
        let eye = p.pos + Vec3::Y * EYE;
        let dir = look_dir(0.0, initial_pitch);
        let d = &world.discs[0];
        let (right, up) = view_basis(dir);
        let rel = d.pos - eye;
        assert!(rel.dot(up) < -0.2, "the disc should leave below the eye, at the gun");
        assert!(rel.dot(right) > 0.1, "the disc should leave from the right-hand weapon");
        let aim = (eye + dir * 80.0 - d.pos).normalize();
        assert!(d.vel.normalize().dot(aim) > 0.999, "the shot still converges on the crosshair");
    }

    #[test]
    fn splash_damage_tapers_to_zero_at_the_edge() {
        let mut world = solo_airborne();
        let blast = world.players[0].pos + Vec3::X * (DISC_RADIUS - 0.01);
        world.explode(blast, usize::MAX, 0, Team::Glacier);
        let damage = 100.0 - world.players[0].health;
        assert!(damage > 0.0 && damage < 0.05, "edge damage should be negligible, got {damage}");
    }

    #[test]
    fn reload_ready_cue_occurs_once_and_fire_stays_gated() {
        let mut world = solo_airborne();
        world.input.fire = true;
        step(&mut world, 1);
        assert!(world.players[0].cooldown > DISC_RELOAD - STEP * 2.0);
        step(&mut world, 30);
        assert_eq!(world.discs.len(), 1, "holding fire cannot bypass reload");
        world.input.fire = false;
        let mut cues = 0;
        for _ in 0..80 {
            step(&mut world, 1);
            cues += world.events.matches("disc_ready").count();
        }
        assert_eq!(cues, 1);
    }

    #[test]
    fn airborne_momentum_survives_coasting_and_flag_pickup() {
        for flag in [None, Some(Team::Glacier)] {
            let mut world = solo_airborne();
            world.players[0].vel = Vec3::new(0.0, 0.0, -50.0);
            world.players[0].carrying = flag;
            step(&mut world, 60);
            assert!((world.players[0].vel.z + 50.0).abs() < 0.01, "coasting must preserve route speed");
            assert!(world.players[0].alive);
        }
    }

    #[test]
    fn air_strafe_steers_left_and_right_without_free_speed() {
        for direction in [-1.0, 1.0] {
            let mut world = solo_airborne();
            world.players[0].vel = Vec3::new(0.0, 0.0, -50.0);
            world.input.move_x = direction;
            world.input.move_z = 1.0;
            step(&mut world, 30);
            let p = &world.players[0];
            assert!(p.pos.x * direction > 128.0 * direction + 0.2, "strafe sign must agree with the view");
            assert!(p.vel.x * direction > 1.5);
            assert!(Vec2::new(p.vel.x, p.vel.z).length() <= 50.01);
        }
    }

    #[test]
    fn holding_ski_through_landing_does_not_repeat_the_jump() {
        let mut world = World::new();
        world.start_match(true);
        world.players.truncate(1);
        world.input.jump = true;
        step(&mut world, 120);
        let p = &world.players[0];
        assert!(p.skiing && p.on_ground, "holding Space should settle into skiing");
        assert!(p.vel.y.abs() < 0.1, "no automatic second hop");
        world.input.jump = false;
        step(&mut world, 1);
        world.input.jump = true;
        step(&mut world, 1);
        assert!(world.players[0].vel.y > 4.0, "a new press can still jump");
    }

    #[test]
    fn ski_contact_does_not_pull_an_outgoing_velocity_back_to_terrain() {
        let mut world = World::new();
        world.start_match(true);
        world.players.truncate(1);
        let p = &mut world.players[0];
        p.vel = Vec3::new(30.0, 8.0, 0.0);
        p.on_ground = true;
        let start_y = p.pos.y;
        world.input.jump = true;
        step(&mut world, 1);
        let p = &world.players[0];
        assert!(!p.on_ground && !p.skiing, "outgoing motion must launch from a crest");
        assert!(p.pos.y > start_y + 0.1);
        assert!(p.vel.x > 29.9);
    }

    #[test]
    fn empty_jetpack_recharges_in_five_seconds() {
        let mut world = solo_airborne();
        world.players[0].pos.y = 1000.0;
        world.players[0].energy = 0.0;
        step(&mut world, 150);
        assert!((world.players[0].energy - 30.0).abs() < 0.1);
        step(&mut world, 150);
        assert!((world.players[0].energy - ENERGY_MAX).abs() < 0.1);
    }

    #[test]
    fn discs_inherit_velocity_and_fly_without_gravity() {
        let mut world = solo_airborne();
        world.players[0].vel = Vec3::new(20.0, 0.0, 0.0);
        world.shoot(0);
        let d = &world.discs[0];
        let inherited = Vec3::new(20.0, 0.0, 0.0) * DISC_INHERIT;
        let weapon = d.vel - inherited;
        assert!(
            (weapon.length() - DISC_SPEED).abs() < 0.05,
            "retain weapon velocity inheritance, weapon speed {}",
            weapon.length()
        );
        assert!(weapon.z < -90.0, "the round still flies downrange");
        let y = d.pos.y;
        let vy = d.vel.y;
        for _ in 0..30 { world.step_discs(STEP); }
        assert_eq!(world.discs.len(), 1);
        let expected = y + vy * 30.0 * STEP;
        assert!(
            (world.discs[0].pos.y - expected).abs() < 0.002,
            "disc should not sag"
        );
    }

    #[test]
    fn running_reaches_its_advertised_speed() {
        let mut world = World::new();
        world.start_match(true);
        world.players.truncate(1);
        world.players[0].yaw = 0.0;
        world.input.move_z = 1.0;
        step(&mut world, 30);
        let v = world.players[0].vel;
        assert!(Vec2::new(v.x, v.z).length() > WALK_MAX * 0.95);
    }

    #[test]
    fn fast_routes_get_a_smooth_gravity_reduction() {
        assert_eq!(gravity_for_speed(50.0), GRAVITY);
        assert!((gravity_for_speed(250.0 / 3.6 - 0.001)
            - gravity_for_speed(250.0 / 3.6 + 0.001)).abs() < 0.001);
        assert!(gravity_for_speed(90.0) < GRAVITY * 0.8);
        assert!(gravity_for_speed(120.0) >= GRAVITY * 0.65 - 0.001);
    }

    fn step(world: &mut World, frames: u32) {
        for _ in 0..frames {
            world.tick(STEP);
        }
    }

    #[test]
    fn a_strafes_left_when_facing_negative_z() {
        let mut world = World::new();
        world.start_match(true);
        world.players[world.player_id].yaw = 0.0;
        let x0 = world.player_pos().x;
        world.input.move_x = -1.0;
        step(&mut world, 180);
        assert!(
            world.player_pos().x < x0 - 0.4,
            "A should move toward -X (left of a -Z facing), x0={x0} x={}",
            world.player_pos().x
        );
    }

    #[test]
    fn d_strafes_right_when_facing_negative_z() {
        let mut world = World::new();
        world.start_match(true);
        world.players[world.player_id].yaw = 0.0;
        let x0 = world.player_pos().x;
        world.input.move_x = 1.0;
        step(&mut world, 180);
        assert!(
            world.player_pos().x > x0 + 0.4,
            "D should move toward +X (right of a -Z facing), x0={x0} x={}",
            world.player_pos().x
        );
    }

    #[test]
    fn tap_space_jumps_from_a_stop() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..30 {
            world.tick(STEP);
        }
        world.input.jump = true;
        world.tick(STEP);
        world.input.jump = false;
        world.tick(STEP);
        let p = &world.players[world.player_id];
        assert!(p.vel.y > 4.0, "a tap of space from a stop should jump, vel.y={}", p.vel.y);
        assert!(!p.skiing, "a standing jump is not a ski");
    }

    #[test]
    fn hold_space_skis_from_a_stop_and_a_hill_builds_speed() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..20 {
            world.tick(STEP);
        }
        {
            let p = &mut world.players[world.player_id];
            p.pos = Vec3::new(128.0, height(128.0, 46.0) + PLAYER_RADIUS, 46.0);
            p.vel = Vec3::new(0.0, 0.0, MAX_JUMP_SPEED + 2.0);
            p.on_ground = true;
            p.yaw = std::f32::consts::PI;
        }
        let mut peak = 0.0f32;
        let mut skied = false;
        for _ in 0..100 {
            world.input.jump = true;
            world.input.move_z = 1.0;
            world.tick(STEP);
            let p = &world.players[world.player_id];
            let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
            peak = peak.max(speed);
            skied |= p.skiing;
        }
        assert!(skied, "holding jump on the drop should enter the ski path");
        assert!(
            peak > MAX_JUMP_SPEED + 2.0,
            "gravity on frictionless snow should add speed, peak={peak:.1}"
        );
    }

    #[test]
    fn a_flat_ski_does_not_add_speed() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..30 {
            world.tick(STEP);
        }
        {
            let p = &mut world.players[world.player_id];
            p.pos = Vec3::new(EMBER_HOME.x, height(EMBER_HOME.x, EMBER_HOME.z) + PLAYER_RADIUS, EMBER_HOME.z);
            p.vel = Vec3::new(MAX_JUMP_SPEED + 4.0, 0.0, 0.0);
            p.on_ground = true;
        }
        let before = MAX_JUMP_SPEED + 4.0;
        for _ in 0..12 {
            world.input.jump = true;
            world.tick(STEP);
        }
        let p = &world.players[world.player_id];
        let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
        assert!(p.skiing, "a fast hold on the pad should still be a ski");
        assert!(
            speed <= before + 0.4,
            "skiing must not drive you faster on a flat, before={before:.1} after={speed:.1}"
        );
    }

    #[test]
    fn downhill_ski_outruns_plain_gravity() {
        let x = 128.0;
        let z = 46.0;
        let n = ski_normal(MapId::Valley, x, z);
        let g = Vec3::new(0.0, -GRAVITY, 0.0);
        let tangent = g - n * g.dot(n);
        let down = Vec3::new(tangent.x, 0.0, tangent.z);
        assert!(
            down.length() > 3.0,
            "the drop should be a real slope, accel={}",
            down.length()
        );
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..10 {
            world.tick(STEP);
        }
        {
            let p = &mut world.players[world.player_id];
            // Start tangent to the slope. A horizontal velocity pointing out
            // of this downhill face is a launch, not supported skiing.
            let start = tangent.normalize_or_zero() * (MAX_JUMP_SPEED + 4.0);
            p.pos = Vec3::new(x, height(x, z) + PLAYER_RADIUS, z);
            p.vel = start;
            p.on_ground = true;
            p.yaw = start.x.atan2(start.z);
        }
        let before = {
            let p = &world.players[world.player_id];
            Vec3::new(p.vel.x, 0.0, p.vel.z).length()
        };
        let mut contact_frames = 0;
        for _ in 0..45 {
            world.input.jump = true;
            world.tick(STEP);
            contact_frames += u32::from(world.players[world.player_id].skiing);
        }
        let p = &world.players[world.player_id];
        let after = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
        let gain = after - before;
        let plain = down.length() * 45.0 * STEP;
        assert!(contact_frames > 10, "the slope should support sustained ski contact, got {contact_frames}");
        assert!(
            gain > plain * 1.25,
            "downhill ski should outrun plain gravity, gain={gain:.2} plain={plain:.2}"
        );
    }

    #[test]
    fn a_single_press_does_not_keep_friction_off() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..30 {
            world.tick(STEP);
        }
        world.input.jump = true;
        world.tick(STEP);
        world.input.jump = false;
        for _ in 0..40 {
            world.tick(STEP);
        }
        let p = &world.players[world.player_id];
        assert!(!p.skiing, "a released jump must not keep the ski path on");
    }

    #[test]
    fn hold_space_skis_once_moving() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..30 {
            world.tick(STEP);
        }
        {
            let p = &mut world.players[world.player_id];
            p.vel.x = MAX_JUMP_SPEED + 4.0;
            p.vel.y = 0.0;
            p.on_ground = true;
        }
        world.input.jump = true;
        world.tick(STEP);
        let p = &world.players[world.player_id];
        assert!(
            p.skiing && p.on_ground,
            "above maxJumpSpeed a held jump is a ski, not a hop"
        );
    }

    #[test]
    fn a_slow_hold_still_hops() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..30 {
            world.tick(STEP);
        }
        world.input.jump = true;
        world.tick(STEP);
        let p = &world.players[world.player_id];
        assert!(!p.on_ground, "below minJumpSpeed a hold still hops");
        assert!(p.vel.y > 3.0, "the hop is a real impulse, vy={}", p.vel.y);
    }

    #[test]
    fn flag_pickup_does_not_change_jump_strength() {
        let mut light = World::new();
        let mut heavy = World::new();
        for world in [&mut light, &mut heavy] {
            world.start_match(true);
            for _ in 0..30 {
                world.tick(STEP);
            }
        }
        heavy.players[heavy.player_id].carrying = Some(Team::Glacier);
        light.input.jump = true;
        heavy.input.jump = true;
        light.tick(STEP);
        heavy.tick(STEP);
        let a = light.players[light.player_id].vel.y;
        let b = heavy.players[heavy.player_id].vel.y;
        assert!((b - a).abs() < 0.01, "a flag must not alter movement, light={a:.2} carrier={b:.2}");
    }

    #[test]
    fn air_control_can_start_a_slow_drift() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..40 {
            world.tick(STEP);
        }
        world.input.jump = true;
        world.tick(STEP);
        world.input.jump = false;
        world.tick(STEP);
        {
            let p = &mut world.players[world.player_id];
            p.on_ground = false;
            p.vel.y = 8.0;
            p.pos.y += 1.5;
        }
        world.input.move_z = 1.0;
        let h0 = {
            let p = &world.players[world.player_id];
            Vec3::new(p.vel.x, 0.0, p.vel.z).length()
        };
        for _ in 0..12 {
            world.input.move_z = 1.0;
            world.tick(STEP);
        }
        let p = &world.players[world.player_id];
        let h1 = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
        assert!(!p.on_ground, "still airborne");
        assert!(
            h1 > h0 + 0.8 && h1 <= WALK_MAX,
            "W should give limited air control, h0={h0:.2} h1={h1:.2}"
        );
    }

    #[test]
    fn jet_keeps_climb_and_wasd_adds_a_push() {
        let mut idle = World::new();
        let mut held = World::new();
        let mut looked = World::new();
        for world in [&mut idle, &mut held, &mut looked] {
            world.start_match(true);
            for _ in 0..30 {
                world.tick(STEP);
            }
            world.players[world.player_id].on_ground = false;
            world.players[world.player_id].vel = Vec3::ZERO;
            world.players[world.player_id].pos.y += 6.0;
            world.players[world.player_id].yaw = 0.0;
        }
        looked.players[looked.player_id].pitch = 1.1;
        for _ in 0..16 {
            idle.input.jet = true;
            held.input.jet = true;
            held.input.move_z = 1.0;
            looked.input.jet = true;
            idle.tick(STEP);
            held.tick(STEP);
            looked.tick(STEP);
        }
        let a = &idle.players[idle.player_id];
        let b = &held.players[held.player_id];
        let c = &looked.players[looked.player_id];
        let b_h = Vec3::new(b.vel.x, 0.0, b.vel.z).length();
        let a_h = Vec3::new(a.vel.x, 0.0, a.vel.z).length();
        assert!(
            (a.vel.y - b.vel.y).abs() < 0.5,
            "WASD must not spend climb, idle vy={} held vy={}",
            a.vel.y,
            b.vel.y
        );
        assert!(b_h > a_h + 1.0, "WASD should add a push, held={b_h:.1} idle={a_h:.1}");
        assert!(
            (a.vel.y - c.vel.y).abs() < 0.4,
            "look must not change the jet, idle vy={} looked vy={}",
            a.vel.y,
            c.vel.y
        );
    }

    #[test]
    fn a_fast_ski_keeps_full_jet_climb() {
        let mut straight = World::new();
        let mut held = World::new();
        for world in [&mut straight, &mut held] {
            world.start_match(true);
            for _ in 0..20 {
                world.tick(STEP);
            }
            let p = &mut world.players[world.player_id];
            p.on_ground = false;
            p.pos.y += 12.0;
            p.vel = Vec3::new(JET_THRUST_CAP + 10.0, 0.0, 0.0);
            p.energy = ENERGY_MAX;
            p.yaw = 0.0;
        }
        for _ in 0..20 {
            straight.input.jet = true;
            held.input.jet = true;
            held.input.move_z = 1.0;
            straight.tick(STEP);
            held.tick(STEP);
        }
        let a = straight.players[straight.player_id].vel.y;
        let b = held.players[held.player_id].vel.y;
        assert!(
            (a - b).abs() < 0.4,
            "past the thrust cap a move key must not change climb, straight={a:.2} held={b:.2}"
        );
    }

    #[test]
    fn jet_tank_supports_a_four_second_route_burn() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..20 {
            world.tick(STEP);
        }
        {
            let p = &mut world.players[world.player_id];
            p.on_ground = false;
            p.pos.y += 40.0;
            p.vel = Vec3::ZERO;
            p.energy = ENERGY_MAX;
        }
        let mut burned = 0;
        for _ in 0..400 {
            world.input.jet = true;
            world.tick(STEP);
            if world.players[world.player_id].jetting {
                burned += 1;
            } else if burned > 0 {
                break;
            }
        }
        let seconds = burned as f32 * STEP;
        assert!(
            (seconds - 3.8).abs() < 0.1,
            "tank should burn for roughly four seconds before reserve, got {seconds:.2}"
        );
        assert!(
            world.players[world.player_id].energy < MIN_JET_ENERGY,
            "the burn should stop at minJetEnergy"
        );
    }

    #[test]
    fn jet_alone_does_not_make_a_ski_line() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..20 {
            world.tick(STEP);
        }
        {
            let p = &mut world.players[world.player_id];
            p.on_ground = false;
            p.pos.y += 40.0;
            p.vel = Vec3::ZERO;
            p.energy = ENERGY_MAX;
            p.yaw = 0.0;
        }
        for _ in 0..180 {
            world.input.jet = true;
            world.input.move_z = 1.0;
            world.tick(STEP);
            if !world.players[world.player_id].jetting && world.players[world.player_id].energy < MIN_JET_ENERGY {
                break;
            }
        }
        let p = &world.players[world.player_id];
        let h = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
        assert!(
            h <= JET_THRUST_CAP + 1.0,
            "jetting with W should fade by 72 km/h, h={h:.2} cap={:.2}",
            JET_THRUST_CAP
        );
    }

    #[test]
    fn a_disc_at_your_feet_throws_you() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..20 {
            world.tick(STEP);
        }
        let id = world.player_id;
        {
            let p = &mut world.players[id];
            p.pitch = -1.2;
            p.vel = Vec3::ZERO;
            p.weapon = 0;
            p.cooldown = 0.0;
            p.on_ground = true;
        }
        world.input.weapon = 0;
        world.input.fire = true;
        world.tick(STEP);
        world.input.fire = false;
        let mut peak = 0.0_f32;
        for _ in 0..50 {
            world.tick(STEP);
            peak = peak.max(world.players[id].vel.y);
        }
        assert!(
            peak > 8.0,
            "a disc into the snow at your feet should throw you, peak vy={peak:.2}"
        );
    }

    #[test]
    fn jet_steers_a_fast_line_without_speeding_it_up() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..20 {
            world.tick(STEP);
        }
        {
            let p = &mut world.players[world.player_id];
            p.on_ground = false;
            p.pos.y += 30.0;
            p.vel = Vec3::new(JET_THRUST_CAP + 15.0, 0.0, 0.0);
            p.energy = ENERGY_MAX;
            p.yaw = 0.0;
        }
        let before = {
            let p = &world.players[world.player_id];
            Vec3::new(p.vel.x, 0.0, p.vel.z).length()
        };
        for _ in 0..30 {
            world.input.jet = true;
            world.input.move_z = 1.0;
            world.tick(STEP);
        }
        let p = &world.players[world.player_id];
        let h = Vec3::new(p.vel.x, 0.0, p.vel.z);
        assert!(
            h.z < -1.0,
            "holding W while jetting should steer a fast line, vz={}",
            h.z
        );
        assert!(
            (h.length() - before).abs() < 1.5,
            "steering must not build a faster line, before={before:.1} after={:.1}",
            h.length()
        );
    }

    #[test]
    fn jet_lifts_off_the_snow_even_looking_down() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..30 {
            world.tick(STEP);
        }
        {
            let p = &mut world.players[world.player_id];
            p.pitch = -0.2;
            p.vel = Vec3::ZERO;
        }
        world.input.jet = true;
        for _ in 0..30 {
            world.tick(STEP);
        }
        let p = &world.players[world.player_id];
        assert!(p.jetting, "right click should jet");
        assert!(!p.on_ground, "jet should leave the snow");
        assert!(p.vel.y > 3.0, "jet should lift, vy={}", p.vel.y);
        assert!(p.vel.y < 12.0, "a half-second of jet should not be a missile, vy={}", p.vel.y);
    }

    #[test]
    fn right_click_jets_without_space() {
        let mut world = World::new();
        world.start_match(true);
        for _ in 0..20 {
            world.tick(STEP);
        }
        world.input.jet = true;
        let before = world.players[world.player_id].energy;
        world.tick(STEP);
        let p = &world.players[world.player_id];
        assert!(p.jetting, "jet is its own button");
        assert!(p.energy < before, "jet spends energy");
        assert!(!p.skiing, "jet does not start a ski");
    }

    #[test]
    fn w_moves_along_facing() {
        let mut world = World::new();
        world.start_match(true);
        world.players[world.player_id].yaw = 0.0;
        let z0 = world.player_pos().z;
        world.input.move_z = 1.0;
        step(&mut world, 180);
        assert!(
            world.player_pos().z < z0 - 0.4,
            "W should move toward -Z at yaw 0, z0={z0} z={}",
            world.player_pos().z
        );
    }
}

fn segment_sphere(start: Vec3, end: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let offset = start - center;
    let c = offset.length_squared() - radius * radius;
    if c <= 0.0 { return Some(0.0); }
    let travel = end - start;
    let a = travel.length_squared();
    if a < 1e-10 { return None; }
    let b = offset.dot(travel);
    let discriminant = b * b - a * c;
    if discriminant < 0.0 { return None; }
    let t = (-b - discriminant.sqrt()) / a;
    (0.0..=1.0).contains(&t).then_some(t)
}

fn segment_box(start: Vec3, end: Vec3, low: Vec3, high: Vec3) -> Option<f32> {
    let delta = end - start;
    let mut entry = 0.0_f32;
    let mut exit = 1.0_f32;
    for axis in 0..3 {
        if delta[axis].abs() < 1e-7 {
            if start[axis] < low[axis] || start[axis] > high[axis] { return None; }
        } else {
            let a = (low[axis] - start[axis]) / delta[axis];
            let b = (high[axis] - start[axis]) / delta[axis];
            entry = entry.max(a.min(b));
            exit = exit.min(a.max(b));
            if entry > exit { return None; }
        }
    }
    Some(entry)
}

fn obstacle_hit(map: MapId, pillars: &[Pillar], start: Vec3, end: Vec3, radius: f32) -> Option<f32> {
    let mut hit = crate::terrain::segment_hit(map, start, end, radius);
    for pillar in pillars {
        let y = crate::terrain::height_on(map, pillar.x, pillar.z);
        // Match the rendered square column, not an unrelated circular volume.
        let half = pillar.r * 0.8;
        let padding = Vec3::splat(radius);
        let low = Vec3::new(pillar.x - half, y, pillar.z - half) - padding;
        let high = Vec3::new(pillar.x + half, y + pillar.h, pillar.z + half) + padding;
        if let Some(t) = segment_box(start, end, low, high) {
            hit = Some(hit.map_or(t, |old| old.min(t)));
        }
    }
    hit
}

fn player_mass(_p: &Player) -> f32 {
    MASS
}

/// Sweep across the rendered ground in short steps, resolving contacts in the
/// same tick as movement. This changes collision, not gravity/thrust/steering.
fn move_over_terrain(map: MapId, p: &mut Player, ski_held: bool, dt: f32) -> f32 {
    let steps = ((p.vel.length() * dt / 0.75).ceil() as usize).clamp(1, 64);
    let sub_dt = dt / steps as f32;
    let mut impact = 0.0_f32;
    for _ in 0..steps {
        let start = p.pos;
        let end = start + p.vel * sub_dt;
        if p.skiing {
            // Supported skiing keeps the existing smooth contact response once
            // per tick. Re-projecting at every substep would add artificial
            // friction and change the movement the player already approved.
            p.pos = end;
            p.pos.y = p.pos.y.max(crate::terrain::height_on(map, end.x, end.z) + PLAYER_RADIUS);
            continue;
        }
        if let Some(t) = crate::terrain::segment_hit(map, start, end, PLAYER_RADIUS) {
            p.pos = start.lerp(end, t);
            let (ground, face) = crate::terrain::surface_on(map, p.pos.x, p.pos.z);
            p.pos.y = ground + PLAYER_RADIUS;
            let inward = p.vel.dot(face);
            if inward < 0.0 {
                impact = impact.max(-inward);
                p.vel -= face * inward;
                p.on_ground = true;
                p.skiing = ski_held && !p.jetting;
            }
            p.pos += p.vel * (sub_dt * (1.0 - t));
            let floor = crate::terrain::height_on(map, p.pos.x, p.pos.z) + PLAYER_RADIUS;
            p.pos.y = p.pos.y.max(floor);
        } else {
            p.pos = end;
        }
    }
    impact
}

fn gravity_for_speed(speed: f32) -> f32 {
    // Gentle high-speed float, blended rather than a sudden threshold change.
    let blend = ((speed - 250.0 / 3.6) / (100.0 / 3.6)).clamp(0.0, 1.0);
    GRAVITY * (1.0 - 0.35 * blend * blend * (3.0 - 2.0 * blend))
}

fn steer_horizontal(p: &mut Player, wish: Vec3, accel: f32, low_speed_cap: f32, dt: f32) {
    let before = Vec3::new(p.vel.x, 0.0, p.vel.z);
    let cap = before.length().max(low_speed_cap);
    let after = (before + wish * accel * dt).clamp_length_max(cap);
    p.vel.x = after.x;
    p.vel.z = after.z;
}

/// Full hop below minJumpSpeed, none at maxJumpSpeed. Vertical speed also fades
/// the hop so a rising player does not stack impulses.
fn jump_scale(horiz: f32, vy: f32) -> f32 {
    let h = if horiz >= MAX_JUMP_SPEED {
        0.0
    } else if horiz <= MIN_JUMP_SPEED {
        1.0
    } else {
        1.0 - (horiz - MIN_JUMP_SPEED) / (MAX_JUMP_SPEED - MIN_JUMP_SPEED)
    };
    let v = if vy >= MAX_JUMP_SPEED {
        0.0
    } else if vy <= 0.0 {
        1.0
    } else {
        1.0 - (vy / MAX_JUMP_SPEED).min(1.0)
    };
    h * v
}

fn ski_normal(map: MapId, x: f32, z: f32) -> Vec3 {
    let e = if crate::terrain::info(map).size > 1000.0 { 12.0 } else { 6.0 };
    let h = |x, z| crate::terrain::height_on(map, x, z);
    let hl = h(x - e, z);
    let hr = h(x + e, z);
    let hd = h(x, z - e);
    let hu = h(x, z + e);
    Vec3::new(hl - hr, 2.0 * e, hd - hu).normalize_or_zero()
}

fn landing_damage(impact_speed: f32) -> f32 {
    (((impact_speed - 18.0).max(0.0) * 1.4).min(22.0) * 0.5).floor()
}

fn grenade_contact_normal(map: MapId, pillars: &[Pillar], p: Vec3, far: f32) -> Vec3 {
    if p.x <= 1.001 { return Vec3::X; }
    if p.x >= far - 0.001 { return -Vec3::X; }
    if p.z <= 1.001 { return Vec3::Z; }
    if p.z >= far - 0.001 { return -Vec3::Z; }
    let (height, normal) = crate::terrain::surface_on(map, p.x, p.z);
    if p.y <= height + 0.15 { return normal; }
    for pillar in pillars {
        let half = pillar.r * 0.8 + 0.12;
        let base = crate::terrain::height_on(map, pillar.x, pillar.z);
        if (p.x - pillar.x).abs() <= half + 0.02 && (p.z - pillar.z).abs() <= half + 0.02
            && p.y >= base - 0.14 && p.y <= base + pillar.h + 0.14 {
            let faces = [((p.x - pillar.x - half).abs(), Vec3::X),
                ((p.x - pillar.x + half).abs(), -Vec3::X),
                ((p.z - pillar.z - half).abs(), Vec3::Z),
                ((p.z - pillar.z + half).abs(), -Vec3::Z),
                ((p.y - base - pillar.h - 0.12).abs(), Vec3::Y)];
            return faces.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)).unwrap().1;
        }
    }
    normal
}

fn jet_falloff(speed: f32) -> f32 {
    let knee = JET_THRUST_CAP * 0.8;
    if speed <= knee {
        1.0
    } else if speed >= JET_THRUST_CAP {
        0.0
    } else {
        1.0 - (speed - knee) / (JET_THRUST_CAP - knee)
    }
}

fn ride_ski(map: MapId, p: &mut Player, wish: Vec3, dt: f32) {
    // Gravity was already applied straight down. On frictionless snow that
    // into-ground part becomes slide. Descending multiplies that slide.
    let n = ski_normal(map, p.pos.x, p.pos.z);
    let vn = p.vel.dot(n);
    if vn < 0.0 {
        p.vel -= n * vn;
    }
    let g = Vec3::new(0.0, -gravity_for_speed(Vec2::new(p.vel.x, p.vel.z).length()), 0.0);
    let tangent = g - n * g.dot(n);
    let down = Vec3::new(tangent.x, 0.0, tangent.z);
    let moving_down = Vec3::new(p.vel.x, 0.0, p.vel.z).dot(down) > 0.0;
    if moving_down {
        p.vel += tangent * (SKI_SLOPE_GRAVITY - 1.0) * dt;
    }
    let speed = p.vel.length();
    if speed > 1.2 && wish.length_squared() > 0.04 {
        // Steer in the slope plane, preserving total speed. Fixed acceleration
        // gives wide high-speed turns instead of the old instant rail-like carve.
        let tangent_wish = wish - n * wish.dot(n);
        p.vel = (p.vel + tangent_wish * SKI_TURN_ACCEL * dt).normalize_or_zero() * speed;
    }
}

fn apply_speed_limits(p: &mut Player, dt: f32) {
    let h = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
    if h > 0.01 {
        let mut nh = h;
        if nh > HORIZ_MAX {
            nh = HORIZ_MAX;
        }
        let s = nh / h;
        p.vel.x *= s;
        p.vel.z *= s;
    }
    if p.vel.y > UP_RESIST_SPEED {
        let excess = p.vel.y - UP_RESIST_SPEED;
        p.vel.y = UP_RESIST_SPEED + excess * (1.0 - UP_RESIST * dt).max(0.0);
    }
    if p.vel.y > UP_MAX {
        p.vel.y = UP_MAX;
    }
}

fn make_player(team: Team, bot: bool, pos: Vec3, yaw: f32, role: BotRole) -> Player {
    Player {
        net_id: 0,
        name: String::new(),
        frags: 0,
        losses: 0,
        shots: 0,
        hits: 0,
        jump_prev: false,
        pos,
        vel: Vec3::ZERO,
        yaw,
        pitch: -0.08,
        health: 100.0,
        energy: ENERGY_MAX,
        team,
        alive: true,
        respawn: 0.0,
        carrying: None,
        is_bot: bot,
        remote: false,
        on_ground: true,
        skiing: false,
        jetting: false,

        cooldown: 0.0,
        weapon: 0,
        bot_role: role,
        bot_goal: pos,
        bot_think: 0.0,
        coyote: 0.0,
    }
}
