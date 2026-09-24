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
pub(crate) const GRAVITY: f32 = 20.0;
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
pub(crate) const JET_ACCEL: f32 = 37.28;
/// Extra sideways accel under the cap. Added on top of the climb, not taken from it.
pub(crate) const JET_HORIZ_ACCEL: f32 = 22.0;
/// 72 km/h. Thrust falls off here so jetting alone cannot make a ski line.
pub(crate) const JET_THRUST_CAP: f32 = 72.0 / 3.6;
/// Four-second tank with a five-second recharge, tuned for continuous ski routes.
pub const ENERGY_MAX: f32 = 60.0;
pub(crate) const ENERGY_JET: f32 = 15.0;
const ENERGY_REGEN: f32 = 12.0;
pub(crate) const MIN_JET_ENERGY: f32 = 3.0;
/// Classic `dryVelocity`. The disc is a linear round, not a mortar.
const DISC_SPEED: f32 = 95.0;
/// Bots notice enemies inside this range, and only with a clear line of sight.
const BOT_SIGHT_RANGE: f32 = 95.0;
/// Most sight rays one bot casts per think (nearest enemies first).
const BOT_SIGHT_RAYS: usize = 3;
/// Seconds a bot keeps heading for an enemy's last-seen spot after losing sight.
const BOT_MEMORY: f32 = 2.5;
/// Route searches allowed per tick across all bots, and the node budget of
/// each: bounds the worst tick when every bot re-plans at once.
const BOT_PLANS_PER_TICK: u32 = 2;
const BOT_SEARCH_BUDGET: usize = 40_000;
/// Re-plan at least this often, and when a bot makes no progress this long.
const BOT_REPLAN_SECONDS: f32 = 6.0;
const BOT_STUCK_SECONDS: f32 = 2.5;
/// Flyers: horizontal distance at which they hand back to the ground route.
const BOT_FLY_HANDOVER: f32 = 30.0;
/// Flyers: cruise height above the highest ground ahead.
const BOT_FLY_CLEARANCE: f32 = 28.0;
/// Flyers: how far a target below can be for a dive.
const BOT_DIVE_RANGE: f32 = 90.0;
/// Flyers: jet again once the tank refills past this, coast below the floor.
const BOT_FLY_REFILL: f32 = 45.0;
const BOT_FLY_EMPTY: f32 = 8.0;
/// Flyers: height a single tank reliably climbs; higher goals take the route.
const BOT_FLY_CLIMB: f32 = 30.0;
/// Flyers: a flyer on the ground route takes off again only beyond this.
const BOT_FLY_TAKEOFF: f32 = 320.0;
/// Flyers: within this of a goal out of climbing reach, land and take the
/// route from the open field (routes from under a floating base fail).
const BOT_FLY_CLIMB_RANGE: f32 = 260.0;
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
pub(crate) const BOLT_SPEED: f32 = 420.0;
/// Collision radius of player discs, bolts and grenades against map geometry.
const SHOT_RADIUS: f32 = 0.12;
/// Repair kits per life, the armor one restores, and how long it takes.
pub const KITS_PER_LIFE: u8 = 1;
pub const KIT_HEAL: f32 = 60.0;
pub const KIT_SECONDS: f32 = 2.0;
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
    pub interact: bool,
    /// Use a repair kit (Q). An intent: the server decides whether it applies.
    pub kit: bool,
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
            interact: false,
            kit: false,
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
    /// Repair kits carried; refilled on respawn and at inventory stations.
    #[serde(default)]
    pub kits: u8,
    /// Armor still to restore from a kit in use.
    #[serde(default)]
    pub kit_heal: f32,
    bot_role: BotRole,
    bot_goal: Vec3,
    bot_think: f32,
    /// Enemy this bot can see, chosen at its last think. Server-only state.
    #[serde(skip)]
    bot_target: Option<usize>,
    /// Where it last saw an enemy, and for how long it keeps hunting there.
    #[serde(skip)]
    bot_seen: Vec3,
    #[serde(skip)]
    bot_memory: f32,
    /// Personality and route-following state. Server-only; never sent.
    #[serde(skip)]
    bot_profile: crate::bot_nav::BotProfile,
    #[serde(skip)]
    bot_path: Vec<u32>,
    #[serde(skip)]
    bot_path_i: usize,
    #[serde(skip)]
    bot_path_goal: Vec3,
    #[serde(skip)]
    bot_replan: f32,
    #[serde(skip)]
    bot_stuck: f32,
    #[serde(skip)]
    bot_best: f32,
    #[serde(skip)]
    bot_link: Option<crate::bot_nav::Link>,
    /// The next link is a jet climb: save energy for it on the way.
    #[serde(skip)]
    bot_saving: bool,
    /// Seconds the current target has stayed in sight (reaction time).
    #[serde(skip)]
    bot_seen_for: f32,
    /// This think's aim error, yaw and pitch radians.
    #[serde(skip)]
    bot_aim: Vec2,
    #[serde(skip)]
    bot_kit: bool,
    /// Links this bot got stuck on recently; its routes avoid them.
    #[serde(skip)]
    bot_avoid: Vec<(u32, u32)>,
    /// Flyers: crossing high on jet arcs instead of following a ground route.
    #[serde(skip)]
    bot_fly: bool,
    coyote: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Disc {
    pub pos: Vec3,
    pub vel: Vec3,
    pub team: Team,
    pub owner: usize,
    pub life: f32,
    pub kind: u8, // 0 disc, 1 bullet, 2 grenade, 3 turret plasma (not a player weapon slot)
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
    pub feed: Vec<crate::feed::Entry>,
    pub equipment: Vec<crate::equipment::State>,
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
    pub spatial_sounds: Vec<(&'static str, Vec3)>,
    pub time: f32,
    pub map: MapId,
    pub net_camera_offset: Vec3,
    pub blast_serial: u64,
    /// Game mode rules in force (CTF or Capture & Hold).
    pub mode: crate::map_catalog::SupportedMode,
    /// Control points, with state. Sent whole in snapshots.
    pub points: Vec<crate::control::Point>,
    control_defs: Vec<crate::control::Definition>,
    /// Extra water volumes beside the map's own: QA staging (`QA_WATER`) and
    /// tests. Empty in normal play.
    pub staged_water: Vec<crate::water::Volume>,
    cnh_acc: [f32; 2],
    network_inputs: Vec<Input>,
    predicting: bool,
    rng: u32,
    last_spawn: [usize; 2],
    /// Offline bot mix; see `crate::bot_nav::Difficulty`.
    pub bot_difficulty: crate::bot_nav::Difficulty,
    /// Route searches left this tick, so bots re-planning together can't
    /// stall one frame.
    bot_plans: u32,
}

struct Rng(u32);
impl Rng {
    fn f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 16_777_216.0
    }
}

pub fn camera_fov(horiz_speed: f32) -> f32 {
    // Walking/short strafe taps never change the lens. Ease into the skiing
    // boost with zero slope at both ends, keeping camera and muzzle in sync.
    let t = ((horiz_speed - 20.0) / 100.0).clamp(0.0, 1.0);
    76.0 + 12.0 * t * t * (3.0 - 2.0 * t)
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
            equipment: Vec::new(),
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
            spatial_sounds: Vec::new(),
            time: 0.0,
            map: MapId::Valley,
            net_camera_offset: Vec3::ZERO,
            blast_serial: 0,
            mode: crate::map_catalog::SupportedMode::Ctf,
            points: Vec::new(),
            control_defs: Vec::new(),
            staged_water: crate::water::qa_staged(),
            cnh_acc: [0.0; 2],
            network_inputs: Vec::new(),
            predicting: false,
            feed: Vec::new(),
            rng: 0xC0FFEE,
            last_spawn: [usize::MAX; 2],
            bot_difficulty: crate::bot_nav::Difficulty::default(),
            bot_plans: 0,
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
        self.equipment=crate::equipment::fresh(id);
        self.pillars = crate::terrain::pillars_on(id);
        self.control_defs = crate::map_pack::on(id).map(|p| p.manifest.control_points.clone()).unwrap_or_default();
        self.reset_points();
        if self.players.is_empty() {
            self.place_flags();
        }
    }

    /// Fresh, neutral points for the current map and mode. CTF only runs the
    /// points marked `ctf_active`; Capture & Hold runs them all.
    pub fn reset_points(&mut self) {
        let cnh = self.mode == crate::map_catalog::SupportedMode::CaptureAndHold;
        self.points = self.control_defs.iter()
            .map(|d| crate::control::Point::from_def(d, cnh || d.ctf_active)).collect();
        self.cnh_acc = [0.0; 2];
    }

    pub fn set_mode(&mut self, mode: crate::map_catalog::SupportedMode) {
        self.mode = mode;
        self.reset_points();
    }

    /// Replace the map's control points (tests and QA fixtures only use this;
    /// real maps declare them in the manifest).
    pub fn set_control_points(&mut self, defs: Vec<crate::control::Definition>) {
        self.control_defs = defs;
        self.reset_points();
    }

    pub fn control_point_count(&self) -> usize { self.control_defs.len() }

    fn ground(&self, x: f32, z: f32) -> f32 {
        crate::terrain::height_on(self.map, x, z)
    }

    fn map_size(&self) -> f32 {
        crate::terrain::info(self.map).size
    }

    fn stand(&self, ember: bool) -> Vec3 {
        crate::terrain::spawn_on(self.map, ember)
    }

    /// Server-chosen spawn among a map's authored points: random, avoiding
    /// the point this team used last and points with a live enemy nearby when
    /// any other choice exists. `None` keeps the legacy single-spawn path.
    fn pick_spawn(&mut self, team: Team) -> Option<(Vec3, f32)> {
        let points = crate::terrain::spawn_points_on(self.map, team == Team::Ember);
        if points.is_empty() { return None; }
        let last = self.last_spawn[team.idx()];
        let safe = |p: Vec3, players: &[Player]| players.iter()
            .all(|o| !o.alive || o.team == team || (o.pos - p).length() > 30.0);
        let mut pool: Vec<usize> = (0..points.len())
            .filter(|&i| i != last && safe(points[i].0, &self.players)).collect();
        if pool.is_empty() { pool = (0..points.len()).filter(|&i| i != last).collect(); }
        if pool.is_empty() { pool = vec![0]; }
        let pick = pool[((self.rng() * pool.len() as f32) as usize).min(pool.len() - 1)];
        self.last_spawn[team.idx()] = pick;
        Some(points[pick])
    }

    pub fn set_paused(&mut self, paused: bool) {
        match (self.state, paused) {
            (MatchState::Playing, true) => self.state = MatchState::Paused,
            (MatchState::Paused, false) => self.state = MatchState::Playing,
            _ => {}
        }
    }

    pub fn start_match(&mut self, ember: bool) {
        self.feed.clear();
        self.equipment=crate::equipment::fresh(self.map);
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
        self.message = if self.mode == crate::map_catalog::SupportedMode::CaptureAndHold {
            "DEPLOYED — HOLD THE POINTS".into() } else { "DEPLOYED — TAKE THEIR FLAG".into() };
        self.reset_points();
        self.message_t = 3.2;
        self.events = "start".into();

        // Face the valley, not the rim. Ember sits at low Z.
        let yaw = if ember { std::f32::consts::PI } else { 0.0 };
        self.last_spawn = [usize::MAX; 2];
        let (pos, face) = self.pick_spawn(team).unwrap_or((self.stand(ember), yaw));
        self.players.push(make_player(team, false, pos, face, BotRole::Offense));
        self.player_id = 0;
        self.fill_match(ember, team, yaw);
    }

    /// A joined rift: you on the snow, no local bots. Other skiers come from the server.
    pub fn start_rift(&mut self, ember: bool) {
        self.feed.clear();
        self.equipment=crate::equipment::fresh(self.map);
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
        self.reset_points();
        self.message_t = 2.4;
        self.events = "start".into();
        let yaw = if ember { std::f32::consts::PI } else { 0.0 };
        self.last_spawn = [usize::MAX; 2];
        let (pos, face) = self.pick_spawn(team).unwrap_or((self.stand(ember), yaw));
        self.players.push(make_player(team, false, pos, face, BotRole::Offense));
        self.player_id = 0;
        self.place_flags();
    }

    fn fill_match(&mut self, ember: bool, team: Team, yaw: f32) {
        for i in 0..2 {
            let o = Vec3::new((i as f32 - 0.5) * 6.0, 0.0, 4.0);
            let mut p = self.stand(ember) + o;
            p.y = self.ground(p.x, p.z) + 1.2;
            let (p, face) = self.pick_spawn(team).unwrap_or((p, yaw));
            let role = if i == 0 { BotRole::Offense } else { BotRole::Defense };
            self.players.push(make_player(team, true, p, face, role));
        }
        let other = team.other();
        let other_ember = other == Team::Ember;
        let oyaw = if other_ember { std::f32::consts::PI } else { 0.0 };
        for i in 0..3 {
            let o = Vec3::new((i as f32 - 1.0) * 5.5, 0.0, 3.0);
            let mut p = self.stand(other_ember) + o;
            p.y = self.ground(p.x, p.z) + 1.2;
            let (p, face) = self.pick_spawn(other).unwrap_or((p, oyaw));
            let role = if i == 2 { BotRole::Defense } else { BotRole::Offense };
            self.players.push(make_player(other, true, p, face, role));
        }

        for i in 0..self.players.len() {
            if self.players[i].is_bot { self.assign_personality(i); }
        }
        self.place_flags();
    }

    /// Give bot `i` a personality from the difficulty mix, a matching role
    /// and a readable name ("Rider", "Ace 2").
    fn assign_personality(&mut self, i: usize) {
        let profile = self.bot_difficulty.pick(self.rng());
        let same = self.players.iter().enumerate()
            .filter(|(j, o)| *j < i && o.is_bot && o.bot_profile.name == profile.name).count();
        let p = &mut self.players[i];
        p.bot_profile = profile;
        p.name = if same == 0 { profile.name.into() } else { format!("{} {}", profile.name, same + 1) };
        match profile.name {
            "Anchor" => p.bot_role = BotRole::Defense,
            "Skirmisher" | "Ace" if p.bot_role != BotRole::Defense => p.bot_role = BotRole::Hunter,
            _ => {}
        }
    }

    /// Offline bot mix for the next match.
    pub fn set_bot_difficulty(&mut self, difficulty: crate::bot_nav::Difficulty) {
        self.bot_difficulty = difficulty;
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
        if let Some(pack)=crate::map_pack::on(self.map) {
            for (flag,source) in self.flags.iter_mut().zip(pack.manifest.flags) {
                flag.pos=Vec3::from_array(source);flag.home=flag.pos;
            }
        }
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
        self.spatial_sounds.clear();
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
        self.step_kits(dt);
        self.step_equipment(dt);
        if self.mode == crate::map_catalog::SupportedMode::Ctf { self.step_flags(dt); }
        self.step_points(dt);
        if self.state == MatchState::Ended { return; }
        self.step_explosions(dt);
    }

    /// Control points: capture progress and ownership, and in Capture & Hold
    /// the score. Clients announce ownership changes by diffing snapshots, so
    /// replayed snapshots never re-announce.
    fn step_points(&mut self, dt: f32) {
        if self.predicting || self.points.iter().all(|p| !p.active) { return; }
        let players: Vec<(u8, Vec3)> = self.players.iter().filter(|p| p.alive)
            .map(|p| (p.team.idx() as u8, p.pos)).collect();
        for e in crate::control::step(&mut self.points, &players, dt) {
            match e {
                crate::control::Event::Captured { .. } => self.push_event("point"),
                crate::control::Event::ContestedStart { .. } => self.push_event("contest"),
            }
        }
        if self.mode != crate::map_catalog::SupportedMode::CaptureAndHold { return; }
        let gained = crate::control::score(&self.points, &mut self.cnh_acc, dt);
        for t in 0..2 { self.score[t] += gained[t]; }
        if let Some(t) = (0..2).find(|&t| self.score[t] >= crate::control::CNH_TARGET) {
            self.score[t] = crate::control::CNH_TARGET;
            self.state = MatchState::Ended;
            self.msg(if t == 0 { "EMBER HOLDS THE FIELD" } else { "GLACIER HOLDS THE FIELD" }, 4.0);
            self.push_event("end");
        }
    }

    /// Server-side repair kits: a use intent starts a heal when the player is
    /// alive, has a kit and is not already healing. Taking damage does not
    /// cancel it; death does.
    fn step_kits(&mut self, dt:f32) {
        if self.predicting {return;}
        for i in 0..self.players.len() {
            let wants=if self.players[i].is_bot {self.players[i].bot_kit}
                else if self.network_inputs.is_empty() {i==self.player_id && self.input.kit}
                else {self.network_inputs.get(i).is_some_and(|c|c.kit)};
            let p=&mut self.players[i];
            if !p.alive {p.kit_heal=0.;continue;}
            if wants && p.kits>0 && p.kit_heal<=0. && p.health<100. {
                p.kits-=1;p.kit_heal=KIT_HEAL;
                if i==self.player_id {self.events.push_str("kit,");}
            }
            if p.kit_heal>0. {
                let step=(KIT_HEAL/KIT_SECONDS*dt).min(p.kit_heal);
                p.kit_heal-=step;p.health=(p.health+step).min(100.);
                if p.health>=100. {p.kit_heal=0.;}
            }
        }
    }

    fn step_equipment(&mut self, dt:f32) {
        use crate::equipment::{self,Kind};
        if self.predicting {return;}
        let defs=equipment::definitions(self.map);
        equipment::power(defs,&mut self.equipment);
        let sensed:Vec<_>=defs.iter().zip(&self.equipment)
            .filter(|(d,s)|d.kind==Kind::Sensor && s.powered).map(|(d,_)|d.team).collect();
        for (i,(d,s)) in defs.iter().zip(&mut self.equipment).enumerate() {
            s.cooldown=(s.cooldown-dt).max(0.);s.contacts=0;
            for (pid,p) in self.players.iter_mut().enumerate() {
                // Do not cast map-wide service rays for distant players. These
                // proximity rejects are also part of the server trust boundary.
                if !p.alive || p.team.idx()!=d.team as usize
                    || p.pos.distance_squared(d.pos())>(d.radius+1.5).powi(2) {continue;}
                let input=if self.network_inputs.is_empty() {
                    if pid==self.player_id {self.input.interact} else {false}
                } else {self.network_inputs.get(pid).is_some_and(|c|c.interact)};
                let end=d.pos();let start=p.pos+Vec3::Y*0.7;
                let delta=end-start;
                // Station interaction points are in free space; repairable
                // solids use their near surface. Buildings still occlude use.
                let trim=if matches!(d.kind,Kind::Generator|Kind::Turret|Kind::Sensor) {d.radius} else {0.};
                let end=end-delta.normalize_or_zero()*trim.min(delta.length());
                if obstacle_hit(self.map,&self.pillars,start,end,0.).is_none() {
                    equipment::service(d,s,p,input,dt);
                }
            }
            if !s.powered {continue;}
            let Some(profile)=equipment::profile(d.kind,d.weapon) else {continue};
            let (map,pillars)=(self.map,&self.pillars);
            let clear=|a:Vec3,b:Vec3|obstacle_hit(map,pillars,a,b,0.).is_none();
            let candidates=self.players.iter().enumerate().filter(|(_,p)|p.alive && d.in_arc(p.pos))
                .map(|(index,p)|equipment::Candidate {index,team:p.team.idx() as u8,pos:p.pos,vel:p.vel});
            let Some(hit)=equipment::acquire_target(d.pos(),d.radius,d.team,&profile,
                sensed.contains(&d.team),candidates,clear) else {continue};
            s.contacts=1;s.aim=hit.aim;
            if profile.fires && s.cooldown<=0. && self.discs.len()<256 {
                if !clear(hit.muzzle,hit.aim_point) {continue;}
                self.discs.push(Disc {pos:hit.muzzle,vel:hit.aim*profile.speed,
                    team:if d.team==0 {Team::Ember} else {Team::Glacier},owner:MAX_PLAYERS,
                    life:profile.life,kind:profile.projectile,spin:i as f32});
                s.cooldown=profile.cooldown;
            }
        }
        equipment::power(defs,&mut self.equipment);
        for (d,s) in defs.iter().zip(&mut self.equipment) {s.regen(d,dt);}
    }

    /// Clear sight between two points through terrain, pillars and the map
    /// collision mesh. Client overlays use it cosmetically; positions already
    /// arrive in snapshots, so it reveals nothing new.
    pub fn sight_clear(&self, from:Vec3, to:Vec3)->bool {
        obstacle_hit(self.map,&self.pillars,from,to,0.).is_none()
    }

    pub fn equipment_prompt(&self)->Option<String> {
        use crate::equipment::Kind;
        let p=self.players.get(self.player_id)?;
        if !p.alive {return None;}
        crate::equipment::definitions(self.map).iter().zip(&self.equipment)
            .filter(|(d,_)|d.team as usize==p.team.idx() && p.pos.distance(d.pos())<d.radius+1.5)
            .min_by(|(a,_),(b,_)|p.pos.distance_squared(a.pos()).total_cmp(&p.pos.distance_squared(b.pos())))
            .map(|(d,s)| {
                if s.health<d.max_health() {format!("Hold E: repair {:?} · {:.0}%",d.kind,100.*s.health/d.max_health())}
                else if !s.powered {"Power offline — repair generator".into()}
                else if d.kind==Kind::Inventory {"Hold E: refit · 1/2/3: weapon".into()}
                else if d.kind==Kind::Repair {"Repair pad: restoring health / energy".into()}
                else {format!("{:?} ONLINE",d.kind)}
            })
    }

    fn think_bots(&mut self, dt: f32) {
        self.bot_plans = BOT_PLANS_PER_TICK;
        let nav = crate::bot_nav::ready(self.map);
        let n = self.players.len();
        for i in 0..n {
            if !self.players[i].is_bot || !self.players[i].alive {
                continue;
            }
            self.players[i].bot_think -= dt;
            self.players[i].bot_memory = (self.players[i].bot_memory - dt).max(0.0);
            self.players[i].bot_replan -= dt;
            if self.players[i].bot_think > 0.0 {
                continue;
            }
            let wait = 0.18 + self.rng() * 0.22;
            let jx = (self.rng() - 0.5) * 12.0;
            let jz = (self.rng() - 0.5) * 8.0;
            // Aim error for this think: a rough normal sample, per profile.
            let spread = self.players[i].bot_profile.aim_error;
            let gauss = |w: &mut Self| (w.rng() + w.rng() + w.rng() - 1.5) * 2.0;
            let aim_err = Vec2::new(gauss(self), gauss(self)) * spread;
            self.players[i].bot_aim = aim_err;
            self.players[i].bot_think = wait;
            let profile = self.players[i].bot_profile;
            let team = self.players[i].team;
            let role = self.players[i].bot_role;
            let carrying = self.players[i].carrying;
            let pos = self.players[i].pos;
            let health = self.players[i].health;
            let enemy_flag_pos = self.flags[team.other().idx()].pos;
            let enemy_carrier = self.flags[team.other().idx()].carrier;
            let own_home = self.flags[team.idx()].home;
            let own_carrier = self.flags[team.idx()].carrier;

            // Heal: a kit when hurt, and cautious personalities fall back to
            // an inventory station when out of kits.
            self.players[i].bot_kit = profile.uses_kit && health < 55.0 && self.players[i].kits > 0;
            let retreat = carrying.is_none() && self.players[i].kits == 0 && health < profile.retreat_health;

            let mut goal = enemy_flag_pos;
            let cnh_goal = if self.mode == crate::map_catalog::SupportedMode::CaptureAndHold {
                // Minimal Capture & Hold play: defenders guard the nearest held
                // point, everyone else heads for the nearest point not yet theirs.
                let mine = |p: &crate::control::Point| p.owner == Some(team.idx() as u8);
                let pick = |want_mine: bool| self.points.iter().filter(|p| p.active && mine(p) == want_mine)
                    .min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos))).map(|p| p.pos);
                if role == BotRole::Defense { pick(true).or_else(|| pick(false)) } else { pick(false).or_else(|| pick(true)) }
                    .map(|g| g + Vec3::new(jx * 0.5, 1.2, jz * 0.5))
            } else { None };
            let station = retreat.then(|| crate::equipment::definitions(self.map).iter()
                .filter(|d| d.kind == crate::equipment::Kind::Inventory && d.team as usize == team.idx())
                .map(|d| d.pos())
                .min_by(|a, b| a.distance(pos).total_cmp(&b.distance(pos)))).flatten();
            if let Some(g) = station {
                goal = g;
            } else if let Some(g) = cnh_goal {
                goal = g;
            } else if carrying.is_some() {
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

            // Sight uses the turrets' line-of-sight rule (eye to chest against
            // terrain, map collision and pillars), testing only the nearest
            // few enemies so each think casts at most BOT_SIGHT_RAYS rays.
            let eye = pos + Vec3::Y * EYE;
            let mut near: Vec<(f32, usize)> = self.players.iter().enumerate()
                .filter(|(j, o)| *j != i && o.alive && o.team != team)
                .map(|(j, o)| (pos.distance(o.pos), j))
                .filter(|(d, _)| *d < BOT_SIGHT_RANGE)
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            let map = self.map;
            let seen = near.iter().take(BOT_SIGHT_RAYS).find(|(_, j)| {
                let chest = self.players[*j].pos + Vec3::Y * crate::equipment::CHEST_HEIGHT;
                obstacle_hit(map, &self.pillars, eye, chest, 0.0).is_none()
            }).copied();

            // Hunt what it can see; once sight breaks, head for where the enemy
            // was last seen for a moment instead of tracking it through walls.
            // Aggressive personalities chase farther.
            let chase = 20.0 + 60.0 * profile.aggression;
            let hunts = (role == BotRole::Hunter || carrying.is_none()) && role != BotRole::Defense && !retreat;
            let mut direct = false;
            match seen {
                Some((d, j)) => {
                    let same = self.players[i].bot_target == Some(j);
                    self.players[i].bot_seen_for = if same { self.players[i].bot_seen_for + wait } else { 0.0 };
                    self.players[i].bot_target = Some(j);
                    self.players[i].bot_seen = self.players[j].pos;
                    self.players[i].bot_memory = BOT_MEMORY;
                    if hunts && d < chase {
                        goal = self.players[j].pos;
                        // In sight and close: fight it out without a route.
                        direct = d < 28.0;
                    }
                }
                None => {
                    self.players[i].bot_target = None;
                    self.players[i].bot_seen_for = 0.0;
                    let last = self.players[i].bot_seen;
                    if hunts && self.players[i].bot_memory > 0.0 && pos.distance(last) < chase {
                        goal = last;
                    }
                }
            }

            self.players[i].bot_goal = goal;
            // Flyers cross high instead of following the ground route, dive on
            // targets below them, and hand back to the route for the landing.
            let flyer = profile.style == crate::bot_nav::MoveStyle::Flyer;
            let flying = self.players[i].bot_fly;
            // Only take off under open sky: indoors, the route leads out first.
            let open_sky = flyer && (flying || crate::map_pack::on(map).map_or(true, |pack| {
                pack.sweep(pos + Vec3::Y * 0.5, pos + Vec3::Y * 20.0, PLAYER_RADIUS).is_none()
            }));
            let dive = open_sky && seen.is_some_and(|(d, j)| d < BOT_DIVE_RANGE && pos.y > self.players[j].pos.y + 6.0);
            let flat = Vec2::new(goal.x - pos.x, goal.z - pos.z).length();
            // Hysteresis: once landed on the route, take off again only when
            // well away from the goal, so it doesn't flip at the boundary.
            // Near the goal the route owns the approach (its jet climbs must
            // not be interrupted), so a landed flyer only takes off again far out.
            let far = if flying { BOT_FLY_HANDOVER } else { BOT_FLY_TAKEOFF };
            // Keep flying until level with the goal, so it lands at deck height
            // (a floating base) rather than under it.
            // A goal well above it nearby (a floating deck) is out of reach of
            // one tank: take the route's pads and ledges up instead of hovering
            // under the hull.
            let out_of_reach = flat < BOT_FLY_CLIMB_RANGE && pos.y + BOT_FLY_CLIMB < goal.y;
            let fly = open_sky && (dive || (!direct && !out_of_reach
                && (flat > far || (flying && (pos.y - goal.y).abs() > 10.0))));
            if fly {
                let p = &mut self.players[i];
                p.bot_fly = true;
                p.bot_link = None;
                p.bot_path.clear();
            } else {
                self.players[i].bot_fly = false;
                if !direct {
                    if let Some(g) = nav { self.follow_route(i, g, goal); }
                }
            }

            // Aim at the visible target with this personality's lead and
            // error, or face its last-seen spot, or face the way it's going.
            let aim = match seen {
                Some((d, j)) => {
                    let o = &self.players[j];
                    let lead = d / DISC_SPEED * profile.lead;
                    Some((o.pos + Vec3::Y * 1.1 + o.vel * lead, Some(d)))
                }
                None if self.players[i].bot_memory > 0.0 => Some((self.players[i].bot_seen + Vec3::Y * 1.1, None)),
                None => None,
            };
            if let Some((at, dist)) = aim {
                let dir = (at - (pos + Vec3::Y * 1.4)).normalize_or_zero();
                if dir.length_squared() > 0.1 {
                    self.players[i].yaw = (-dir.x).atan2(-dir.z) + aim_err.x;
                    // Flyers can aim steeply down onto a target below.
                    let down = if profile.style == crate::bot_nav::MoveStyle::Flyer { -1.3 } else { -0.7 };
                    self.players[i].pitch = (dir.y.asin() + aim_err.y).clamp(down, 0.7);
                }
                if let Some(d) = dist {
                    self.players[i].weapon = if d < 28.0 { 1 } else { 0 };
                }
            } else {
                let to = self.players[i].bot_goal - pos;
                if Vec2::new(to.x, to.z).length() > 1.0 {
                    self.players[i].yaw = (-to.x).atan2(-to.z);
                    self.players[i].pitch = -0.05;
                }
            }
        }
    }

    /// Point bot `i` at the next waypoint of a route to `goal`, planning or
    /// re-planning when the goal moves, the route runs out, or the bot stalls.
    fn follow_route(&mut self, i: usize, g: &crate::bot_nav::NavGraph, goal: Vec3) {
        use crate::bot_nav::Link;
        let pos = self.players[i].pos;
        // Last few metres with nothing in the way: go straight in rather
        // than threading waypoints that can sit behind a lip or railing.
        if pos.distance(goal) < 14.0 && (goal.y - pos.y).abs() < 3.0 {
            // A chest-height line: plinths and steps are hopped, not walls.
            let body = crate::map_pack::on(self.map).map_or(true, |pack| {
                let chest = Vec3::Y * 1.2;
                pack.sweep(pos + chest, goal + chest, 0.35).is_none()
            });
            if body {
                let p = &mut self.players[i];
                p.bot_goal = goal;
                p.bot_link = None;
                p.bot_saving = false;
                p.bot_stuck = 0.0;
                return;
            }
        }
        let p = &self.players[i];
        // Never swap routes mid-climb or mid-fall: the new route would start
        // from the floor below and drop the move in progress.
        let committed = !p.on_ground && matches!(p.bot_link, Some(Link::Jet) | Some(Link::Drop))
            && p.bot_path_i < p.bot_path.len() && p.bot_stuck < BOT_STUCK_SECONDS * 2.0;
        let stale = !committed && (p.bot_path.is_empty() || p.bot_path_i >= p.bot_path.len()
            || p.bot_path_goal.distance(goal) > 12.0 || p.bot_replan <= 0.0 || p.bot_stuck > BOT_STUCK_SECONDS);
        if stale && self.bot_plans > 0 {
            self.bot_plans -= 1;
            // Stuck: remember the link that failed so the new route avoids it.
            let p = &mut self.players[i];
            if p.bot_stuck > BOT_STUCK_SECONDS && p.bot_path_i > 0 && p.bot_path_i < p.bot_path.len() {
                let bad = (p.bot_path[p.bot_path_i - 1], p.bot_path[p.bot_path_i]);
                if p.bot_avoid.len() >= 8 { p.bot_avoid.remove(0); }
                p.bot_avoid.push(bad);
            }
            let weights = p.bot_profile.route;
            let avoid = p.bot_avoid.clone();
            let mut forgive = false;
            let path = match (g.nearest_in_reach(self.map, pos), g.nearest(goal)) {
                // The failed links may have been the only way: forget them
                // and take the plain route again.
                (Some(a), Some(b)) => g.path_avoiding(a, b, weights, BOT_SEARCH_BUDGET, &avoid)
                    .or_else(|| { forgive = true; g.path(a, b, weights, BOT_SEARCH_BUDGET) })
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            let p = &mut self.players[i];
            if forgive { p.bot_avoid.clear(); }
            p.bot_path = path;
            p.bot_path_i = 0;
            p.bot_path_goal = goal;
            p.bot_replan = BOT_REPLAN_SECONDS;
            p.bot_stuck = 0.0;
            p.bot_best = f32::MAX;
        }
        let p = &mut self.players[i];
        // Advance past waypoints already reached.
        while p.bot_path_i < p.bot_path.len() {
            let w = g.nodes[p.bot_path[p.bot_path_i] as usize];
            let flat = Vec2::new(w.x - pos.x, w.z - pos.z).length();
            let reach = if g.terrain[p.bot_path[p.bot_path_i] as usize] { 5.0 } else { 1.8 };
            // Already past it at speed: the next waypoint is nearer.
            let passed = p.bot_path.get(p.bot_path_i + 1).is_some_and(|&n| {
                let nx = g.nodes[n as usize];
                g.terrain[p.bot_path[p.bot_path_i] as usize] && nx.distance(pos) < w.distance(pos)
            });
            if passed || (flat < reach && (w.y - pos.y).abs() < 2.8) {
                p.bot_path_i += 1;
                p.bot_best = f32::MAX;
                p.bot_stuck = 0.0;
            } else {
                break;
            }
        }
        if p.bot_path_i < p.bot_path.len() {
            let here = p.bot_path[p.bot_path_i];
            p.bot_goal = g.nodes[here as usize];
            p.bot_link = Some(if p.bot_path_i == 0 { Link::Walk } else { g.link(p.bot_path[p.bot_path_i - 1], here) });
            p.bot_saving = p.bot_path.get(p.bot_path_i + 1).is_some_and(|&next| g.link(here, next) == Link::Jet);
            let d = p.bot_goal.distance(pos);
            if d < p.bot_best - 0.5 { p.bot_best = d; p.bot_stuck = 0.0; }
        } else {
            p.bot_link = None;
            p.bot_saving = false;
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
            let drain = crate::control::drain_at(&self.points, self.players[i].team.idx() as u8, self.players[i].pos);
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

                let (h,face) = crate::terrain::support_on(map,p.pos);
                let nrm = if crate::map_pack::on(map).is_some() {face} else {crate::terrain::normal_on(map, p.pos.x, p.pos.z)};
                let ground_y = h + PLAYER_RADIUS;
                let was_ground = p.on_ground;
                // Contact depends on motion relative to the slope, not world Y:
                // a skier climbing a ramp can have positive Y velocity.
                let ground_normal = contact_normal(map,p.pos);
                p.on_ground = p.pos.y <= ground_y + 0.12
                    && p.vel.dot(ground_normal) <= 0.5;
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
                // How much of the body is under water (0 = dry: nothing below changes).
                let (wet, wet_volume) = crate::water::immersion(map, &self.staged_water, p.pos);
                let jet_cost = if wet >= crate::water::DEEP {crate::water::JET_ENERGY_FACTOR} else {1.0};

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
                    p.energy = (p.energy - ENERGY_JET * jet_cost * dt).max(0.0);
                } else {
                    p.energy = (p.energy + ENERGY_REGEN * dt).min(ENERGY_MAX);
                }
                // A held control point's drain field saps its enemies' energy.
                p.energy = (p.energy - drain * dt).max(0.0);

                if p.on_ground && !p.skiing {
                    // Approach the desired walking speed; releasing ski brakes
                    // progressively instead of instantly deleting route momentum.
                    let horizontal = Vec3::new(p.vel.x, 0.0, p.vel.z);
                    let walk_max = WALK_MAX * (1.0 - crate::water::WADE_SLOW * wet);
                    let delta = wish * walk_max - horizontal;
                    let accel = if horizontal.length() > walk_max || wish.length_squared() < 0.01 {
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
                if let (true, Some(volume)) = (wet > 0.0, wet_volume) {
                    let g = gravity_for_speed(Vec2::new(p.vel.x, p.vel.z).length());
                    crate::water::apply(&mut p.vel, wet, p.skiing, volume, g, dt);
                }
                apply_speed_limits(p, dt);

                let impact_speed = move_over_terrain(map, p, jump_held, dt);
                if p.skiing {
                    let ground_y = crate::terrain::support_on(map,p.pos).0 + PLAYER_RADIUS;
                    let drop = p.pos.y - ground_y;
                    let nrm = contact_normal(map,p.pos);
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
                self.kill(i, None, "Fall");
            }
        }
    }

    /// A flyer's steering while crossing high: (wish, jet, ski).
    ///
    /// Cruises `BOT_FLY_CLEARANCE` above the highest ground over the next
    /// 120 m, pulsing the jet between a full tank and empty (energy refills
    /// while it coasts), and climbs over a structure ahead. Near the goal it
    /// glides down a slope onto it; above a visible target it dives.
    fn fly_controls(&mut self, i: usize, target_in_view: bool) -> (Vec3, bool, bool) {
        let map = self.map;
        let p = &self.players[i];
        let (pos, vel, goal, energy, on_ground) = (p.pos, p.vel, p.bot_goal, p.energy, p.on_ground);
        let target = p.bot_target.and_then(|j| self.players.get(j)).map(|o| o.pos);
        let to = goal - pos;
        let horiz = Vec3::new(to.x, 0.0, to.z);
        let flat = horiz.length();
        let dir = if flat > 0.5 { horiz / flat } else { Vec3::ZERO };
        let height = |x: f32, z: f32| crate::terrain::height_on(map, x, z);
        let below_me = pos.y - height(pos.x, pos.z);
        // Dive: fall onto a target below, braking with the jet near the ground.
        if let Some(t) = target.filter(|t| target_in_view && pos.y > t.y + 6.0) {
            let at = Vec3::new(t.x - pos.x, 0.0, t.z - pos.z).normalize_or_zero();
            let stop = vel.y * vel.y / (2.0 * (JET_ACCEL - GRAVITY));
            let brake = vel.y < -10.0 && below_me < stop + 4.0 && energy >= MIN_JET_ENERGY;
            return (at, brake, false);
        }
        let mut ground = height(pos.x, pos.z);
        for k in 1..=4 {
            let s = pos + dir * (30.0 * k as f32).min(flat);
            ground = ground.max(height(s.x, s.z));
        }
        // Far out: clear the ground ahead and never cruise below the goal
        // (a floating base's deck). Over the last 150 m: glide down onto it.
        let glide = goal.y + 4.0 + (flat - BOT_FLY_HANDOVER).max(0.0) * 0.3;
        let cruise = if flat > 150.0 {
            (ground + BOT_FLY_CLEARANCE).max(goal.y + 10.0)
        } else {
            glide.max(ground + 8.0)
        };
        let horiz_speed = Vec2::new(vel.x, vel.z).length();
        let reach = (horiz_speed + 10.0).min(40.0);
        let blocked = crate::map_pack::on(map).is_some_and(|pack| {
            pack.sweep(pos + Vec3::Y * 0.5, pos + Vec3::Y * 0.5 + dir * reach, PLAYER_RADIUS).is_some()
        });
        // Pulse: jet from a full tank down to empty, then coast to refill.
        let mut coast = self.players[i].bot_saving;
        if energy < BOT_FLY_EMPTY { coast = true; } else if energy > BOT_FLY_REFILL { coast = false; }
        self.players[i].bot_saving = coast;
        let low = pos.y < cruise;
        let jet = !coast && energy >= MIN_JET_ENERGY && (low || blocked || horiz_speed < 18.0);
        // A wall ahead (a courtyard, a hull): stop pushing into it and climb
        // straight up on a full tank until the way is clear.
        let wish = if blocked { Vec3::ZERO } else { dir };
        // Grounded while refilling: ski on toward the goal.
        let ski = on_ground && coast && !blocked;
        (wish, jet, ski)
    }

    fn bot_wish(&mut self, i: usize) -> (f32, f32, bool, bool, bool, bool) {
        use crate::bot_nav::{Link, MoveStyle};
        let p = &self.players[i];
        let to = p.bot_goal - p.pos;
        let yaw = p.yaw;
        let pitch = p.pitch;
        let team = p.team;
        let pos = p.pos;
        let vel = p.vel;
        let on_ground = p.on_ground;
        let energy = p.energy;
        let profile = p.bot_profile;
        let link = p.bot_link;
        let saving = p.bot_saving;
        let goal = p.bot_goal;
        let seen_for = p.bot_seen_for;
        let target = p.bot_target;
        let (fwd, right) = move_basis(yaw);
        let horiz = Vec3::new(to.x, 0.0, to.z);
        let hlen = horiz.length();
        let mut wish = if hlen > 1.0 { horiz / hlen } else { horiz };
        // Fire only at the enemy it can see, re-checking sight at the trigger,
        // and only once it has had the target in view for its reaction time.
        let mut fire = false;
        let mut target_in_view = false;
        if let Some(o) = target.and_then(|j| self.players.get(j)) {
            let d = pos.distance(o.pos);
            let dir = look_dir(yaw, pitch);
            let to_e = ((o.pos + Vec3::Y) - (pos + Vec3::Y * 1.4)).normalize_or_zero();
            target_in_view = o.alive && o.team != team;
            if target_in_view && d < 78.0 && dir.dot(to_e) > 0.86 && seen_for >= profile.reaction {
                let chest = o.pos + Vec3::Y * crate::equipment::CHEST_HEIGHT;
                fire = obstacle_hit(self.map, &self.pillars, pos + Vec3::Y * EYE, chest, 0.0).is_none();
            }
        }
        let r = self.rng();
        let horiz_speed = Vec2::new(vel.x, vel.z).length();
        let fly = self.players[i].bot_fly;
        let (jet, ski) = if fly {
            let (w, jet, ski) = self.fly_controls(i, target_in_view);
            wish = w;
            (jet, ski)
        } else { match link {
            // Climbs: wait for a tank first, then jet until a coast would
            // carry it over the waypoint, braking horizontally near the end.
            Some(Link::Jet) => {
                let above = pos.y > goal.y + 1.0;
                let brake = horiz_speed * horiz_speed / (2.0 * JET_HORIZ_ACCEL);
                if !above && hlen < brake + 1.0 && horiz_speed > 3.0 {
                    let back = -Vec3::new(vel.x, 0.0, vel.z).normalize_or_zero();
                    wish = back;
                }
                // Under an overhang (a roof's eave over its wall): the climb
                // straight up is blocked, so back away from the wall until it
                // clears, then rise and cross, instead of pinning under it.
                if pos.y < goal.y + 0.5 && hlen < 4.0 && hlen > 0.05 {
                    let blocked = crate::map_pack::on(self.map).is_some_and(|pack| {
                        let top = goal.y + 1.5;
                        top > pos.y + 0.5 && pack.sweep(pos + Vec3::Y * 0.5, Vec3::new(pos.x, top, pos.z), PLAYER_RADIUS).is_some()
                    });
                    if blocked { wish = -horiz / hlen; }
                }
                let tank = energy >= ENERGY_MAX * crate::bot_nav::JET_PLAN_ENERGY - 4.0;
                if on_ground && !tank && (hlen < 12.0 || horiz_speed < 10.0) {
                    // Recharge in place before committing to the climb.
                    wish = Vec3::ZERO;
                    (false, false)
                } else {
                    // Hold the jet while a coast from here would arrive
                    // under the waypoint, not just until it's briefly above.
                    let t = hlen / horiz_speed.max(4.0);
                    let arrive = pos.y + vel.y * t - 0.5 * GRAVITY * t * t;
                    (arrive < goal.y + 1.5 && energy >= MIN_JET_ENERGY && (!on_ground || tank), on_ground)
                }
            }
            // Drops: walk off, then burn jet near the bottom to soften the landing.
            Some(Link::Drop) => {
                let fall = (pos.y - goal.y).max(0.0);
                let stop = vel.y * vel.y / (2.0 * (JET_ACCEL - GRAVITY));
                (vel.y < -12.0 && fall < stop + 2.0 && energy >= MIN_JET_ENERGY, false)
            }
            // Terrain: skis down on descents and at speed; jet up climbs and
            // across flats while the tank is healthy, as players do.
            Some(Link::Ski) => {
                let downhill = to.y < -0.5;
                let ski = profile.style == MoveStyle::Skier || downhill || horiz_speed > 15.0;
                let push = !saving && ((to.y > 1.0 && energy > 15.0) || (!downhill && horiz_speed < 18.0 && energy > 40.0));
                (push, ski)
            }
            // Floors and ramps: walk, so a slide doesn't carry it through
            // doors; hop a lip or step with a short jet.
            Some(Link::Walk) => (!saving && to.y > 1.5 && hlen < 4.0 && energy > MIN_JET_ENERGY, false),
            // No route yet (or a close fight): the original steering.
            None => {
                let uphill = to.y > 5.0;
                (energy >= MIN_JET_ENERGY && (!on_ground || uphill), true)
            }
        } };
        // Jetters fight from the air when they have the energy for it.
        let jet = jet || (profile.style == MoveStyle::Jetter && target_in_view && energy > 30.0 && r < 0.6);
        let mx = wish.dot(right).clamp(-1.0, 1.0);
        let mz = wish.dot(fwd).clamp(-1.0, 1.0);
        // Hop a step or plinth edge too tall to walk up; elsewhere the odd
        // random hop off rough ground.
        let step_up = matches!(link, Some(Link::Walk) | None) && to.y > 0.4 && hlen < 3.0;
        let jump_edge = on_ground && (step_up || (link != Some(Link::Walk) && r < 0.02));
        // Stall tracking: time without getting closer to the waypoint (the
        // route follower resets it on progress), except while recharging.
        // A climb that keeps falling short counts, so it gets re-planned.
        if link.is_some() && !(on_ground && !jet && wish == Vec3::ZERO) {
            self.players[i].bot_stuck += STEP;
        }
        let forward = if link.is_some() { mz } else { mz.max(0.15) };
        (mx, forward, ski, jump_edge, fire && r < 0.7, jet)
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
        // The viewmodel muzzle sits up to ~2 m ahead of the eye at wide FOV,
        // farther than a wall is thick: never spawn a shot past a surface.
        let origin = match obstacle_hit(self.map, &self.pillars, eye, origin, SHOT_RADIUS) {
            Some(t) => eye.lerp(origin, (t - 0.05 / eye.distance(origin).max(0.05)).max(0.0)),
            None => origin,
        };
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
        } else if self.network_inputs.is_empty() {
            self.spatial_sounds.push((match kind { 0 => "disc", 1 => "chain", _ => "grenade" }, origin));
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
        let mut explode: Vec<(Vec3, usize, u8, Team, Option<usize>)> = Vec::new();
        let mut keep = Vec::new();
        let mut bullet_hits = Vec::new();
        let mut equipment_hits = Vec::new();

        let map = self.map;
        let far = self.map_size() - 1.0;
        for mut d in self.discs.drain(..) {
            if d.kind == 2 && self.smoke.len() < 256 {
                self.smoke.push(SmokePuff { pos: d.pos, age: 0.0 });
            }
            d.life -= dt;
            if d.life <= 0.0 {
                if d.kind != 1 { explode.push((d.pos, d.owner, d.kind, d.team, None)); }
                continue;
            }
            let mut dead = false;
            for _ in 0..n_sub {
                d.spin += sdt * 42.0;
                let grav = if d.kind == 2 { 1.0 } else { 0.0 };
                d.vel.y -= GRAVITY * grav * sdt;
                // Water: bullets and plasma fizzle out; discs and grenades
                // slow hard but still explode on contact or fuse.
                if crate::water::at(map, &self.staged_water, d.pos).is_some() {
                    if d.kind == 1 || d.kind == 3 {
                        dead = true;
                        break;
                    }
                    d.vel *= (-crate::water::PROJECTILE_DRAG * sdt).exp();
                }
                let next = d.pos + d.vel * sdt;
                let mut hit = obstacle_hit(map, &self.pillars, d.pos, next, if d.kind==3 {0.45} else {SHOT_RADIUS});
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
                let mut equipment_victim=None;
                for (idx,(obj,state)) in crate::equipment::definitions(map).iter().zip(&self.equipment).enumerate() {
                    if obj.team as usize==d.team.idx() || state.health<=0. {continue;}
                    if let Some(t)=segment_sphere(d.pos,next,obj.pos(),obj.radius+if d.kind==3 {0.45} else {0.}) {
                        if hit.is_none_or(|old|t<old) {hit=Some(t);equipment_victim=Some(idx);}
                    }
                }
                for (idx, pl) in self.players.iter().enumerate() {
                    if !pl.alive || pl.team == d.team {
                        continue;
                    }
                    let c = pl.pos + Vec3::Y * 0.9;
                    if let Some(t) = segment_sphere(d.pos, next, c, PLAYER_RADIUS + 0.45) {
                        if hit.is_none_or(|old| t < old) {
                            hit = Some(t);
                            victim = Some(idx);
                            equipment_victim=None;
                        }
                    }
                }
                if let Some(t) = hit {
                    let contact = d.pos.lerp(next, t);
                    if d.kind == 2 {
                        // Brief launch safety lets close surfaces bounce the
                        // shell. Armed shells detonate on their next contact.
                        if d.life <= 1.65 {
                            explode.push((contact, d.owner, d.kind, d.team, victim));
                            dead = true;
                            break;
                        }
                        let normal = if let Some(idx) = victim {
                            (contact - (self.players[idx].pos + Vec3::Y * 0.9)).normalize_or_zero()
                        } else if let Some(idx)=equipment_victim {
                            (contact-crate::equipment::definitions(map)[idx].pos()).normalize_or_zero()
                        } else { grenade_contact_normal(map, &self.pillars, contact, far, d.vel) };
                        let inward = d.vel.dot(normal);
                        if inward < 0.0 { d.vel -= normal * inward * 1.5; }
                        d.vel *= 0.82;
                        d.pos = contact + normal * 0.03;
                        continue;
                    }
                    if d.kind == 1 {
                        if let Some(idx)=equipment_victim {equipment_hits.push(idx);}
                        if let Some(idx) = victim { bullet_hits.push((idx, d.owner)); }
                    } else { explode.push((contact, d.owner, d.kind, d.team, victim)); }
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
        let defs=crate::equipment::definitions(self.map);
        for idx in equipment_hits {
            if self.equipment[idx].damage(&defs[idx],8.,true) {self.equipment_destroyed(&defs[idx]);}
        }
        for (idx, owner) in bullet_hits {
            if !self.players[idx].alive { continue; }
            self.players[idx].health -= 8.0;
            if let Some(p) = self.players.get_mut(owner) { p.hits += 1; }
            if owner == self.player_id { self.hitmarker = 1.0; self.push_event("hit"); }
            if idx == self.player_id { self.damage_flash = 0.35; self.push_event("pain"); }
            if self.players[idx].health <= 0.0 {
                self.kill(idx, Some(owner), if owner < self.players.len() { "Chaingun" } else { "Bullet turret" });
                if owner == self.player_id { self.kills += 1; self.msg("FRAG", 1.1); }
            }
        }
        for (pos, owner, kind, team, direct) in explode {
            self.explode_direct(pos, owner, kind, team, direct);
        }
    }

    #[cfg(test)]
    fn explode(&mut self, pos: Vec3, owner: usize, kind: u8, team: Team) {
        self.explode_direct(pos,owner,kind,team,None);
    }

    fn explode_direct(&mut self, pos: Vec3, owner: usize, kind: u8, team: Team, direct:Option<usize>) {
        self.blast_serial += 1;
        let profile=crate::combat::player_weapon_blast(kind);
        let max_r = profile.map_or(if kind==3 {6.} else {9.},|p|p.radius);
        self.explosions.push(Explosion {
            pos,
            age: 0.0,
            max_r,
            kind,
        });
        self.spatial_sounds.push(("boom", pos));
        let dmg_core = if kind == 0 { 52.0 } else if kind==3 {43.} else { 68.0 };
        let mut wrecked=Vec::new();
        for (d,s) in crate::equipment::definitions(self.map).iter().zip(&mut self.equipment) {
            if d.team as usize==team.idx() || s.health<=0. {continue;}
            let delta=d.pos()-pos;let distance=(delta.length()-d.radius).max(0.);
            if distance>max_r {continue;}
            let end=d.pos()-delta.normalize_or_zero()*d.radius.min(delta.length());
            if !splash_reaches(self.map,&self.pillars,pos,end,d) {continue;}
            let damage=profile.map_or((dmg_core+12.)*(1.-distance/max_r),|p|p.damage(distance));
            if s.damage(d,damage,false) {wrecked.push(d.clone());}
        }
        for d in wrecked {self.equipment_destroyed(&d);}
        let mut killed_by_player = false;
        let pid = self.player_id;
        for i in 0..self.players.len() {
            if !self.players[i].alive {
                continue;
            }
            let origin_distance = self.players[i].pos.distance(pos);
            // Damage reaches the body, not just a point at the player's feet.
            // A confirmed impact touches its victim: full splash once, never
            // an additional direct-damage bonus. Other targets still fall off.
            let d = if profile.is_some() {
                if direct==Some(i) {0.} else {player_blast_distance(pos,self.players[i].pos)}
            } else {origin_distance};
            if d > max_r {
                continue;
            }
            // Impulse falls to zero at the blast edge, independently of T2's
            // nonzero edge damage. Use body contact distance, not the feet.
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
            let dmg = if let Some(profile)=profile {profile.damage(d)*mul}
                else if kind==3 && direct==Some(i) {55.}
                else {(12.0 * fall + dmg_core * fall * fall) * mul};
            self.players[i].health -= dmg;
            // Kick from the blast, not from the chest, so a disc at your feet throws you up.
            let away = (body - pos).try_normalize().unwrap_or(Vec3::Y);
            let kick = profile.map_or(if kind==3 {3.5} else {6.},|p|p.impulse/player_mass(&self.players[i]));
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
                self.kill(i, Some(owner), match kind { 0 => "Disc launcher", 2 => "Grenade launcher", 3 => "Plasma turret", _ => "Explosion" });
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

    fn kill(&mut self, i: usize, killer: Option<usize>, weapon: &str) {
        if !self.players[i].alive {
            return;
        }
        self.players[i].alive = false;
        let killer = killer.filter(|&k| k < self.players.len());
        let entry = crate::feed::Entry::Frag {
            killer: killer.map(|k| self.display_name(k)).unwrap_or_else(|| if weapon == "Fall" { "Environment".into() } else { "Turret".into() }),
            victim: self.display_name(i), weapon: weapon.into(),
        };
        crate::feed::push(&mut self.feed, entry);
        self.players[i].losses += 1;
        if let Some(k) = killer.filter(|&k| k != i && k < self.players.len()
            && self.players[k].team != self.players[i].team) {
            self.players[k].frags += 1;
        }
        self.players[i].health = 0.0;
        self.players[i].kit_heal = 0.0;
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

    pub fn display_name(&self, i: usize) -> String {
        let p = &self.players[i];
        if !p.name.is_empty() { p.name.clone() }
        else if i == self.player_id { "You".into() }
        else { format!("Bot {}", i + 1) }
    }

    fn respawn(&mut self, i: usize) {
        let ember = self.players[i].team == Team::Ember;
        let (pos, yaw) = match self.pick_spawn(self.players[i].team) {
            Some(spawn) => spawn,
            None => {
                let mut pos = self.stand(ember);
                pos.x += (self.rng() - 0.5) * 8.0;
                pos.z += (self.rng() - 0.5) * 6.0;
                pos.y = crate::terrain::support_on(self.map,pos).0 + 1.2;
                (pos, if ember { std::f32::consts::PI } else { 0.0 })
            }
        };
        let p = &mut self.players[i];
        p.pos = pos;
        p.vel = Vec3::ZERO;
        p.health = 100.0;
        p.energy = ENERGY_MAX;
        p.alive = true;
        p.kits = KITS_PER_LIFE;
        p.kit_heal = 0.0;
        p.carrying = None;
        p.yaw = yaw;
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
        let y = crate::terrain::support_on(self.map,pos).0 + 0.4;
        let f = &mut self.flags[team.idx()];
        f.carrier = None;
        f.pos = Vec3::new(pos.x, y, pos.z);
        f.drop_timer = 14.0;
        // Flag text is announced by the client from flag state (flag_announce.rs).
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
                let y = crate::terrain::support_on(self.map,pos+Vec3::Y*PLAYER_RADIUS).0 + 0.4;
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
                }
            }

            // Grab enemy flag.
            if self.players[i].carrying.is_none() && self.flags[enemy].carrier.is_none() {
                if pos.distance(self.flags[enemy].pos) < 2.6 {
                    self.flags[enemy].carrier = Some(i);
                    self.flags[enemy].drop_timer = 0.0;
                    self.players[i].carrying = Some(self.flags[enemy].team);
                    self.push_event("flag");
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
        self.push_event(if team == self.players[self.player_id].team { "capture_win" } else { "capture_loss" });
        self.trauma = 0.55;
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

    /// A generator's destruction is a visible, audible blast that hurts no one.
    /// It travels as an ordinary explosion (serial plus snapshot), so clients
    /// replaying a snapshot never see it twice.
    fn equipment_destroyed(&mut self, d:&crate::equipment::Definition) {
        if d.kind!=crate::equipment::Kind::Generator {return;}
        self.blast_serial+=1;
        self.explosions.push(Explosion {pos:d.pos()+Vec3::Y*2.,age:0.,max_r:GENERATOR_BLAST_RADIUS,kind:4});
        self.spatial_sounds.push(("boom",d.pos()));
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
                crate::terrain::overview_height(self.map).max(self.pillars.iter()
                    .map(|p|self.ground(p.x,p.z)+p.h+28.).fold(f32::NEG_INFINITY,f32::max)),
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
        let anchor = p.pos + Vec3::Y * EYE;
        let desired = anchor + off + Vec3::Y * bob + self.net_camera_offset;
        // Visual reconciliation and shake must not move the near plane through
        // a wall even when the authoritative collision body is safely inside.
        let eye = crate::map_pack::on(self.map)
            .and_then(|pack|pack.sweep(anchor,desired,0.35))
            .map_or(desired,|(t,_)|anchor.lerp(desired,(t-0.001).max(0.)));
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
        let kits = p.map(|x| x.kits).unwrap_or(0);
        let kit_heal = p.map(|x| x.kit_heal).unwrap_or(0.0);
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
        let msg = self.equipment_prompt().unwrap_or_else(||self.message.clone()).replace('"', "");
        format!(
            "{{\"health\":{:.1},\"energy\":{:.1},\"speed\":{:.1},\"yaw\":{:.4},\"pitch\":{:.4},\"px\":{:.2},\"py\":{:.2},\"pz\":{:.2},\"ember\":{},\"glacier\":{},\"time\":{:.1},\"state\":{},\"weapon\":{},\"flag\":{},\"ownFlag\":{},\"hit\":{:.2},\"flash\":{:.2},\"msg\":\"{}\",\"kills\":{},\"deaths\":{},\"winner\":{},\"team\":{},\"onGround\":{},\"ski\":{},\"jet\":{},\"alive\":{},\"cd\":{:.2},\"events\":\"{}\",\"blips\":\"{}\",\"mapSize\":{:.0},\"kits\":{},\"kitHeal\":{:.1}}}",
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
            self.map_size(),
            kits,
            kit_heal
        )
    }
}

#[cfg(test)]
mod controls {
    #[test]
    fn walking_fov_is_fixed_and_skiing_curve_is_gentle() {
        for speed in [0.,0.1,1.,5.,10.,15.,20.] {assert_eq!(super::camera_fov(speed),76.);}
        let mut last=76.;
        for speed in 21..=200 {
            let fov=super::camera_fov(speed as f32);
            assert!(fov>=last && fov<=88.);
            assert!(fov-last<0.2);
            last=fov;
        }
    }
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
            let home = crate::terrain::spawn_on(map,true);
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
            let home = crate::terrain::spawn_on(map,true);
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
    fn t2_splash_retains_edge_damage_but_stops_outside_radius() {
        let mut world = solo_airborne();
        let blast = world.players[0].pos + Vec3::X * (crate::combat::DISC.radius + PLAYER_RADIUS - 0.01);
        world.explode(blast, usize::MAX, 0, Team::Glacier);
        let damage = 100.0 - world.players[0].health;
        assert!(damage > 9. && damage < 9.3, "T2 edge damage, got {damage}");
        let health=world.players[0].health;
        world.explode(blast+Vec3::X*0.02,usize::MAX,0,Team::Glacier);
        assert_eq!(world.players[0].health,health);
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
    if let Some((t,_))=crate::map_pack::on(map).and_then(|p|p.sweep(start,end,radius)) {
        hit=Some(hit.map_or(t,|old|old.min(t)));
    }
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
    if let Some(pack)=crate::map_pack::on(map) {return move_in_imported_map(map,pack,p,ski_held,dt);}
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

fn contact_normal(map:MapId,pos:Vec3)->Vec3 {
    if let Some(pack)=crate::map_pack::on(map) {
        if let Some((h,n))=pack.floor(pos-Vec3::Y*PLAYER_RADIUS) {
            if h>=crate::terrain::height_on(map,pos.x,pos.z) || pack.hole(pos.x,pos.z) {return n;}
        }
    }
    ski_normal(map,pos.x,pos.z)
}

fn move_in_imported_map(map:MapId,pack:&crate::map_pack::MapPack,p:&mut Player,ski_held:bool,dt:f32)->f32 {
    let mut impact=0.0_f32;
    let mut remaining=dt;
    // Continuous sweeps, with up to six sliding contacts per simulation tick.
    for _ in 0..6 {
        let start=p.pos;let end=start+p.vel*remaining;
        let mut hit=crate::terrain::segment_hit(map,start,end,PLAYER_RADIUS)
            .map(|t|(t,crate::terrain::surface_on(map,start.lerp(end,t).x,start.lerp(end,t).z).1));
        // Overlapping spheres cover the full body, including the eye and a
        // head/near-plane margin. The old stack ended below the camera.
        if let Some(h)=pack.body_sweep(start,end) {
            if hit.is_none_or(|old|h.0<old.0) {hit=Some(h);}
        }
        let Some((t,n))=hit else {p.pos=end;break;};
        p.pos=start.lerp(end,t)+n*0.001;
        let vn=p.vel.dot(n);
        if vn<0.0 {impact=impact.max(-vn);p.vel-=n*vn;}
        if n.y>0.25 {p.on_ground=true;p.skiing=ski_held&&!p.jetting;}
        remaining*=1.0-t;
        if remaining<0.00001 {break;}
    }
    impact
}

pub(crate) fn gravity_for_speed(speed: f32) -> f32 {
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

#[cfg(test)]
mod equipment_tests {
    use super::*;
    use crate::equipment::{self,Kind};
    #[test]
    fn entire_menu_orbit_clears_terrain_and_solid_scenery() {
        for map in [MapId::Valley, MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone,
            MapId::SnowblindClone, MapId::DesertOfDeathClone] {
            let mut w=World::new(); w.set_map(map);
            // Render-only masts and spires can stand above the solid scenery.
            let render_top=crate::map_pack::on(map).map_or(f32::NEG_INFINITY,|pack| {
                let bytes=pack.asset("vertices.bin").expect("vertices");
                bytes.chunks_exact(48).map(|v|f32::from_le_bytes(v[4..8].try_into().unwrap()))
                    .fold(f32::NEG_INFINITY,f32::max)
            });
            for step in 0..720 {
                w.flyby=step as f32 * std::f32::consts::TAU / 720. / 0.18;
                let (eye,dir,_)=w.camera();
                assert!(eye.is_finite() && dir.is_normalized());
                assert!(eye.y >= w.ground(eye.x,eye.z)+27.9, "{map:?} orbit meets terrain");
                if let Some(pack)=crate::map_pack::on(map) { assert!(eye.y >= pack.highest_solid()+27.9, "{map:?} orbit meets solid"); }
                assert!(eye.y >= render_top+20.0, "{map:?} orbit {} passes render geometry topping {render_top}", eye.y);
            }
        }
    }
    #[test]
    fn player_blasts_push_outward_and_lift_with_distance_falloff() {
        for kind in [0,2] {
            let profile=crate::combat::player_weapon_blast(kind).unwrap();
            let mut w=world(); w.players.truncate(1);
            let pos=Vec3::new(1000.,300.,1000.);
            w.players[0].pos=pos; w.players[0].vel=Vec3::ZERO;
            w.players[0].on_ground=true; w.players[0].skiing=true;
            w.explode(pos-Vec3::Y*PLAYER_RADIUS,0,kind,Team::Ember);
            assert!((w.players[0].vel.y-profile.impulse/90.).abs()<0.001);
            assert!(!w.players[0].on_ground && !w.players[0].skiing);
            w.players[0].vel=Vec3::ZERO; w.players[0].health=100.;
            w.explode(pos-Vec3::X*(profile.radius*0.5+PLAYER_RADIUS),0,kind,Team::Ember);
            assert!((w.players[0].vel.length()-profile.impulse/90.*0.5).abs()<0.001);
            assert!(w.players[0].vel.x>0.);
        }
    }
    fn world()->World {
        let mut w=World::new();w.set_map(MapId::Raindance);w.start_rift(true);w
    }
    #[test]
    fn disc_and_grenade_impacts_apply_one_blast_and_kill_in_two() {
        for kind in [0,2] {
            let profile=crate::combat::player_weapon_blast(kind).unwrap();
            let mut w=world();w.players.truncate(1);
            w.players[0].pos=Vec3::new(1000.,300.,1000.);
            w.players[0].team=Team::Glacier;w.players[0].health=100.;
            let mut nearby=w.players[0].clone();nearby.pos.z+=4.;w.players.push(nearby);
            let mut distant=w.players[0].clone();distant.pos.z+=16.;w.players.push(distant);
            for shot in 0..2 {
                w.discs.push(Disc {pos:w.players[0].pos+Vec3::new(-5.,0.9,0.),vel:Vec3::X*80.,
                    owner:MAX_PLAYERS,team:Team::Ember,kind,life:1.5,spin:0.});
                for _ in 0..8 {w.step_discs(STEP);}
                assert!(w.discs.is_empty());
                if shot==0 {
                    assert!((w.players[0].health-(100.-profile.max_damage)).abs()<0.001,
                        "kind {kind}: {:?}",w.players[0].health);
                    assert!(w.players[0].alive);
                    assert_eq!(w.explosions.len(),1);
                    assert_eq!(w.explosions[0].max_r,profile.radius);
                    let expected=profile.damage(player_blast_distance(w.explosions[0].pos,w.players[1].pos));
                    assert!((w.players[1].health-(100.-expected)).abs()<0.001);
                    assert!(expected>0. && expected<profile.max_damage);
                    assert_eq!(w.players[2].health,100.);
                }
            }
            assert!(!w.players[0].alive,"two direct hits must kill for kind {kind}");
        }
    }
    #[test]
    fn grenade_has_wider_splash_and_cover_blocks_both_weapons() {
        for kind in [0,2] {
            let mut w=world();w.players.truncate(1);w.players[0].team=Team::Glacier;
            w.players[0].pos=Vec3::new(1012.,300.,1000.);
            w.explode(Vec3::new(1000.,300.7,1000.),MAX_PLAYERS,kind,Team::Ember);
            if kind==0 {assert_eq!(w.players[0].health,100.);}
            else {assert!(w.players[0].health<80. && w.players[0].health>70.);}
            w.players[0].health=100.;w.players[0].pos=Vec3::new(829.,110.,1400.);
            w.explode(Vec3::new(832.5,110.7,1400.),MAX_PLAYERS,kind,Team::Ember);
            assert_eq!(w.players[0].health,100.,"base wall must shield kind {kind}");
        }
    }
    #[test]
    fn explosive_self_damage_stays_reduced_and_team_damage_stays_off() {
        for kind in [0,2] {
            let mut w=world();w.players.truncate(1);w.players[0].pos=Vec3::new(1000.,300.,1000.);
            let mut enemy=w.players[0].clone();enemy.team=Team::Glacier;w.players.push(enemy);
            w.players.push(w.players[0].clone());
            w.explode(w.players[0].pos+Vec3::Y*0.7,0,kind,Team::Ember);
            let peak=crate::combat::player_weapon_blast(kind).unwrap().max_damage;
            assert!((w.players[0].health-(100.-peak*0.4)).abs()<0.001);
            assert!((w.players[1].health-(100.-peak)).abs()<0.001);
            assert_eq!(w.players[2].health,100.);
            assert_eq!(w.players[2].vel,Vec3::ZERO);
        }
    }
    #[test]
    fn equipment_uses_the_same_explosive_curve() {
        for kind in [0,2] {
            let mut w=world();let defs=equipment::definitions(w.map);
            let i=defs.iter().position(|d|d.kind==Kind::Sensor && d.team==0).unwrap();
            let d=&defs[i];let blast=d.pos()+Vec3::Y*(d.radius+2.);
            let profile=crate::combat::player_weapon_blast(kind).unwrap();
            w.explode(blast,MAX_PLAYERS,kind,Team::Glacier);
            // The sensor's generator shield soaks the blast before the hull.
            assert!((w.equipment[i].shield-(d.max_shield()-profile.damage(2.))).abs()<0.001);
            assert_eq!(w.equipment[i].health,d.max_health());
        }
    }
    #[test]
    fn full_head_stops_at_ceiling_even_at_high_speed() {
        let mut w=world();
        let pack=crate::map_pack::on(w.map).unwrap();
        let p=&mut w.players[0];
        p.pos=Vec3::new(822.,115.,1400.);p.vel=Vec3::Y*150.;
        move_in_imported_map(w.map,pack,p,false,0.1);
        assert!(p.pos.y>117. && p.pos.y+EYE+PLAYER_RADIUS<=119.401,"head {:?}",p.pos);
        assert!(p.vel.y.abs()<0.001);
        // Keep pressing into the ceiling without creeping through it.
        for _ in 0..60 {p.vel=Vec3::Y*10.;move_in_imported_map(w.map,pack,p,false,STEP);}
        assert!(p.pos.y+EYE+PLAYER_RADIUS<=119.401);
    }
    #[test]
    fn camera_offsets_cannot_cross_ceiling_or_wall() {
        let mut w=world();w.trauma=1.;
        w.players[0].pos=Vec3::new(822.,117.,1400.);
        w.net_camera_offset=Vec3::Y*8.;
        let (eye,_,_)=w.camera();
        assert!(eye.y<119.1 && eye.y>118.5,"ceiling camera {eye:?}");
        w.players[0].pos=Vec3::new(828.,110.,1400.);
        w.net_camera_offset=Vec3::X*10.;
        let (eye,_,_)=w.camera();
        assert!(eye.x<830. && eye.x>828.,"wall camera {eye:?}");
    }
    #[test]
    fn plasma_direct_hits_kill_in_two_with_splash_and_small_push() {
        let mut w=world();w.players.truncate(1);
        w.players[0].pos=Vec3::new(1000.,300.,1000.);
        w.players[0].team=Team::Glacier;w.players[0].health=100.;w.players[0].vel=Vec3::ZERO;
        let mut nearby=w.players[0].clone();nearby.pos.z+=3.;w.players.push(nearby);
        let mut distant=w.players[0].clone();distant.pos.z+=10.;w.players.push(distant);
        for shot in 0..2 {
            w.discs.push(Disc {pos:w.players[0].pos+Vec3::new(-5.,0.9,0.),vel:Vec3::X*80.,
                owner:MAX_PLAYERS,team:Team::Ember,kind:3,life:3.,spin:0.});
            for _ in 0..8 {w.step_discs(STEP);}
            assert!(w.discs.is_empty());
            if shot==0 {
                assert_eq!(w.players[0].health,45.);assert!(w.players[0].alive);
                assert!(w.players[0].vel.length()>0. && w.players[0].vel.length()<=3.5);
                assert!(w.players[1].health>45. && w.players[1].health<100.);
                assert_eq!(w.players[2].health,100.);
            }
        }
        assert!(!w.players[0].alive);
    }
    #[test]
    fn plasma_splash_respects_base_walls_and_friendly_teams() {
        let mut w=world();w.players.truncate(1);
        w.players[0].pos=Vec3::new(829.,110.,1400.);w.players[0].team=Team::Glacier;
        w.explode(Vec3::new(832.5,110.7,1400.),MAX_PLAYERS,3,Team::Ember);
        assert_eq!(w.players[0].health,100.);
        w.players[0].pos=Vec3::new(1000.,300.,1000.);w.players[0].team=Team::Ember;
        w.explode(w.players[0].pos,MAX_PLAYERS,3,Team::Ember);
        assert_eq!(w.players[0].health,100.);
    }
    #[test]
    fn base_turrets_fire_plasma_and_towers_keep_bullets() {
        let mut w=world();let defs=equipment::definitions(w.map);
        assert_eq!(defs.iter().filter(|d|d.kind==Kind::Turret && d.weapon==equipment::TurretWeapon::Plasma).count(),4);
        assert_eq!(defs.iter().filter(|d|d.kind==Kind::Turret && d.weapon==equipment::TurretWeapon::Bullet).count(),2);
        let i=defs.iter().position(|d|d.weapon==equipment::TurretWeapon::Plasma).unwrap();
        w.players[0].team=Team::Glacier;w.players[0].pos=defs[i].pos()+Vec3::Y*30.;
        w.players[0].vel=Vec3::X*25.;
        w.step_equipment(STEP);
        assert!(w.discs.iter().any(|d|d.kind==3 && d.owner==MAX_PLAYERS));
        assert!(w.equipment[i].aim.x>0.1,"plasma turret did not lead sideways motion");
        assert!(w.equipment[i].cooldown>1.);
    }
    #[test]
    fn inventory_requires_use_team_distance_and_power() {
        let mut w=world();let defs=equipment::definitions(w.map);
        let Some(i)=defs.iter().position(|d|d.kind==Kind::Inventory && d.team==0) else {return;};
        w.players[0].pos=defs[i].pos();w.players[0].health=25.;w.players[0].energy=10.;
        w.step_equipment(STEP);assert_eq!(w.players[0].health,25.);
        w.input.interact=true;w.step_equipment(STEP);assert!(w.players[0].health>25.);
        w.players[0].team=Team::Glacier;let before=w.players[0].health;
        w.step_equipment(STEP);assert_eq!(w.players[0].health,before);
        w.players[0].team=Team::Ember;w.players[0].pos+=Vec3::X*30.;
        w.step_equipment(STEP);assert_eq!(w.players[0].health,before);
        w.players[0].pos=defs[i].pos();
        for (d,s) in defs.iter().zip(&mut w.equipment) {if d.kind==Kind::Generator && d.team==0 {s.health=0.;}}
        w.step_equipment(STEP);assert_eq!(w.players[0].health,before);assert!(!w.equipment[i].powered);
    }
    #[test]
    fn generators_take_bullet_damage_and_can_be_repaired() {
        let mut w=world();let defs=equipment::definitions(w.map);
        let Some(i)=defs.iter().position(|d|d.kind==Kind::Generator && d.team==0) else {return;};
        let pos=defs[i].pos();w.players[0].pos=pos+Vec3::X*12.;w.players[0].team=Team::Glacier;
        w.discs.push(Disc {pos:pos+Vec3::X*9.,vel:-Vec3::X*BOLT_SPEED,owner:0,team:Team::Glacier,kind:1,life:1.,spin:0.});
        w.step_discs(STEP);assert!(w.equipment[i].health<defs[i].max_health());
        w.equipment[i].health=0.;w.players[0].team=Team::Ember;
        w.players[0].pos=pos+Vec3::X*3.5;w.input.interact=true;
        let energy=w.players[0].energy;
        w.step_equipment(STEP);assert!(w.equipment[i].health>0.);assert!(!w.equipment[i].powered,"offline until half repaired");
        assert!(w.players[0].energy<energy);
        for _ in 0..600 {w.players[0].energy=ENERGY_MAX;w.step_equipment(STEP);if w.equipment[i].powered {break;}}
        assert!(w.equipment[i].powered && w.equipment[i].health>=defs[i].max_health()*equipment::ONLINE_FRACTION);
    }
    #[test]
    fn equipment_snapshots_and_empty_server_reset() {
        let mut m=Match::new(MapId::Raindance);let Some(s)=m.join(31,"tester") else {panic!()};
        if m.world.equipment.is_empty(){return;}
        m.world.equipment[0].health=12.;m.world.equipment[0].shield=7.5;let snap=m.snapshot();
        let mut client=world();assert!(client.apply_snapshot(&snap,31));assert_eq!(client.equipment[0].health,12.);
        assert_eq!(client.equipment[0].shield,7.5,"shield state reaches clients");
        m.leave(s);assert_eq!(m.world.equipment[0].health,equipment::definitions(m.world.map)[0].max_health());
    }
    #[test]
    fn shielded_turrets_fall_only_after_the_generator_or_a_sustained_attack() {
        let mut w=world();let defs=equipment::definitions(w.map);
        let Some(i)=defs.iter().position(|d|d.kind==Kind::Turret && d.team==0) else {return;};
        let d=&defs[i];assert!(w.equipment[i].shield>0.,"fixed turrets start shielded");
        let blast=d.pos()+Vec3::Y*(d.radius+0.5);
        w.explode(blast,MAX_PLAYERS,0,Team::Glacier);
        assert_eq!(w.equipment[i].health,d.max_health(),"one disc does not reach the hull");
        for (g,s) in defs.iter().zip(&mut w.equipment) {if g.kind==Kind::Generator && g.team==0 {s.health=0.;}}
        w.step_equipment(STEP);
        assert!(!w.equipment[i].powered);assert_eq!(w.equipment[i].shield,0.,"no generator, no shield");
        for _ in 0..40 {w.step_equipment(STEP);}
        assert_eq!(w.equipment[i].shield,0.,"shield cannot regenerate without power");
        let before=w.equipment[i].health;w.explode(blast,MAX_PLAYERS,0,Team::Glacier);
        assert!(w.equipment[i].health<before,"the bare hull takes the blast");
    }
    #[test]
    fn original_base_walk_routes_enter_service_hall_and_reach_roof() {
        if equipment::definitions(MapId::Raindance).is_empty(){return;}
        for (x,roof) in [(800.,false),(822.,true)] {
            let mut w=world();w.players[0].team=Team::Glacier;
            w.players[0].pos=Vec3::new(x,112.+PLAYER_RADIUS,1340.);
            w.players[0].yaw=std::f32::consts::PI;w.players[0].on_ground=true;
            w.input.move_z=1.;
            for _ in 0..255 {w.physics_step();}
            let p=&w.players[0];
            assert!(p.alive && p.pos.is_finite(),"route must stay playable");
            assert!(p.pos.z>1374.,"route blocked at {:?}",p.pos);
            if roof {assert!(p.pos.y>120.,"roof route: {:?}",p.pos);}
            else {assert!(p.pos.y<110. && p.pos.y>101.,"lower hall: {:?}",p.pos);}
        }
    }
    #[test]
    fn powered_turrets_engage_enemies_and_stop_when_power_is_lost() {
        let mut w=world();let defs=equipment::definitions(w.map);
        let Some(i)=defs.iter().rposition(|d|d.kind==Kind::Turret && d.team==0) else {return;};
        w.players[0].team=Team::Glacier;
        // In front of the turret, inside its field of fire.
        let f=defs[i].facing.map_or(Vec3::X,|f|Vec3::new(f[0],0.,f[1]).normalize());
        w.players[0].pos=defs[i].pos()+f*45.-Vec3::Y*0.8;
        w.step_equipment(STEP);
        assert!(w.discs.iter().any(|d|d.owner==MAX_PLAYERS && d.team==Team::Ember));
        w.discs.clear();
        for (d,s) in defs.iter().zip(&mut w.equipment) {if d.kind==Kind::Generator && d.team==0 {s.health=0.;}}
        for _ in 0..60 {w.step_equipment(STEP);}
        assert!(!w.discs.iter().any(|d|d.team==Team::Ember));
    }
}

/// Visual size of a generator's destruction blast (kind 4). Cosmetic only.
pub const GENERATOR_BLAST_RADIUS: f32 = 11.;

/// Splash reaches equipment when nothing but the equipment's own mount lies
/// between the blast and the object. The collision mesh does not tag which
/// solid belongs to which object, so a hit inside the object's footprint column
/// (its mount, pedestal or housing) counts as reaching it. Walls farther out
/// still block the blast.
fn splash_reaches(map:MapId,pillars:&[Pillar],blast:Vec3,end:Vec3,d:&crate::equipment::Definition)->bool {
    let Some(t)=obstacle_hit(map,pillars,blast,end,0.) else {return true};
    if t>=0.995 {return true;}
    let hit=blast.lerp(end,t);let c=d.pos();
    let horizontal=Vec3::new(hit.x-c.x,0.,hit.z-c.z).length();
    horizontal<=d.radius+EQUIPMENT_MOUNT_MARGIN && hit.y<=c.y+d.radius && hit.y>=c.y-EQUIPMENT_MOUNT_DEPTH
}
/// How far an equipment mount extends beyond the object's hit radius, and how
/// far below its centre, for `splash_reaches`.
const EQUIPMENT_MOUNT_MARGIN: f32 = 1.0;
const EQUIPMENT_MOUNT_DEPTH: f32 = 6.0;

fn player_blast_distance(blast:Vec3,player:Vec3)->f32 {
    let spine=Vec3::new(player.x,blast.y.clamp(player.y,player.y+EYE),player.z);
    (blast.distance(spine)-PLAYER_RADIUS).max(0.)
}

fn landing_damage(impact_speed: f32) -> f32 {
    (((impact_speed - 18.0).max(0.0) * 1.4).min(22.0) * 0.5).floor()
}

fn grenade_contact_normal(map: MapId, pillars: &[Pillar], p: Vec3, far: f32, incoming:Vec3) -> Vec3 {
    if let Some(pack)=crate::map_pack::on(map) {
        let direction=incoming.normalize_or_zero();
        if let Some((_,n))=pack.sweep(p-direction*0.3,p+direction*0.3,0.12) {return n;}
    }
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

pub(crate) fn jet_falloff(speed: f32) -> f32 {
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
    let n = contact_normal(map,p.pos);
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
        kits: KITS_PER_LIFE,
        kit_heal: 0.0,
        bot_role: role,
        bot_goal: pos,
        bot_think: 0.0,
        bot_target: None,
        bot_seen: pos,
        bot_memory: 0.0,
        bot_profile: crate::bot_nav::BotProfile::default(),
        bot_path: Vec::new(),
        bot_path_i: 0,
        bot_path_goal: pos,
        bot_replan: 0.0,
        bot_stuck: 0.0,
        bot_best: f32::MAX,
        bot_link: None,
        bot_saving: false,
        bot_seen_for: 0.0,
        bot_aim: Vec2::ZERO,
        bot_kit: false,
        bot_avoid: Vec::new(),
        bot_fly: false,
        coyote: 0.0,
    }
}

#[cfg(test)]
mod spawn_point_tests {
    use super::*;

    #[test]
    fn tower_spawn_points_land_on_the_floor_and_walking_brakes() {
        // Regression: a spawn centre only 0.2 m above a 1 m slab started the
        // support ray inside the slab, so the player never counted as grounded
        // and coasted without ground braking (the "slick" spawn).
        for map in [MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone, MapId::SnowblindClone, MapId::DesertOfDeathClone] {
        let mut world = World::new();
        world.set_map(map);
        world.start_match(true);
        world.players.truncate(1);
        world.player_id = 0;
        for ember in [true, false] {
            let points = crate::terrain::spawn_points_on(map, ember);
            assert!(points.len() >= 6);
            for (pos, yaw) in points {
                let floor = crate::terrain::support_on(map, pos).0;
                assert!((pos.y - floor - 1.2).abs() < 0.01, "spawn {pos:?} support {floor}");
                let p = &mut world.players[0];
                p.pos = pos; p.vel = Vec3::ZERO; p.yaw = yaw; p.alive = true; p.health = 100.0;
                p.on_ground = true; p.skiing = false; p.jetting = false;
                world.input = Input::default();
                for _ in 0..30 { world.step_players(STEP); }
                let p = &world.players[0];
                assert!(p.on_ground, "{map:?} not grounded at {pos:?}");
                assert!((p.pos.y - floor - PLAYER_RADIUS).abs() < 0.05, "sunk or floating at {pos:?}: {}", p.pos.y);
                // Walk forward for half a second, release, and stop like flat ground.
                let start = p.pos;
                world.input.move_z = 1.0;
                for _ in 0..30 { world.step_players(STEP); }
                world.input.move_z = 0.0;
                let released = world.players[0].pos;
                // Spawns face open floor, so the half-second walk stays on the
                // spawn's own floor and must brake like flat ground.
                assert!((crate::terrain::support_on(map, released).0 - floor).abs() < 0.3
                    && (released.y - floor - PLAYER_RADIUS).abs() < 0.05,
                    "{map:?} walk from {start:?} left the spawn floor (faces an edge or wall)");
                for _ in 0..90 { world.step_players(STEP); }
                let p = &world.players[0];
                let coast = Vec2::new(p.pos.x - released.x, p.pos.z - released.z).length();
                assert!(p.on_ground && coast < 2.2, "{map:?} slides {coast} m after release from {start:?}");
                assert!(Vec2::new(p.vel.x, p.vel.z).length() < 0.05);
            }
        }
        }
    }

    /// A skier holding ski, with no steering, enters Frostline's ice cavern
    /// from either trench and coasts out through the far trench: no snag on a
    /// wall, seam or mouth, no fall through the floor, and the dip keeps
    /// momentum. Uses the real movement code on the embedded pack.
    #[test]
    fn frostline_cavern_skis_through_mouth_to_mouth() {
        let map = MapId::SnowblindClone;
        for (z0, dir, speed) in [(950.0_f32, 1.0_f32, 30.0_f32), (1098.0, -1.0, 30.0), (950.0, 1.0, 16.0)] {
            let mut world = World::new();
            world.set_map(map);
            world.start_match(true);
            world.players.truncate(1);
            world.player_id = 0;
            let floor = crate::terrain::support_on(map, Vec3::new(1024.0, 226.0, z0)).0;
            let p = &mut world.players[0];
            p.pos = Vec3::new(1024.0, floor + PLAYER_RADIUS, z0);
            p.vel = Vec3::new(0.0, 0.0, dir * speed);
            p.yaw = if dir > 0.0 { std::f32::consts::PI } else { 0.0 };
            p.alive = true; p.health = 100.0; p.on_ground = true; p.skiing = true; p.jetting = false;
            world.input = Input::default();
            world.input.jump = true;
            let (mut min_speed, mut exit_speed) = (f32::MAX, None);
            for _ in 0..(60 * 12) {
                world.step_players(STEP);
                let p = &world.players[0];
                let d = p.pos.z - 1024.0;
                assert!(p.pos.y > 210.0, "fell through at {:?}", p.pos);
                if d.abs() < 48.0 {
                    assert!((p.pos.x - 1024.0).abs() < 11.4, "drifted into the wall at {:?}", p.pos);
                    min_speed = min_speed.min(Vec2::new(p.vel.x, p.vel.z).length());
                }
                if d * dir > 72.0 { exit_speed = Some(Vec2::new(p.vel.x, p.vel.z).length()); break; }
            }
            let exit = exit_speed.unwrap_or_else(|| panic!("skier from z {z0} at {speed} m/s never left the far trench: {:?}", world.players[0].pos));
            assert!(min_speed > 8.0, "skier from z {z0} slowed to {min_speed} m/s inside");
            assert!(exit > speed * 0.6, "skier from z {z0} left at {exit} m/s after entering at {speed}");
        }
    }

    /// Dustreach's sewer: a skier holding ski, with no steering, drops into
    /// the red leg at its mouth and coasts the whole 368 m leg to the cross
    /// hall under the Sun Gate: no snag on a wall, seam or rib, no fall
    /// through the floor, and the rolling floor keeps momentum. The blue leg
    /// is the red one rotated 180 degrees. Real movement code, embedded pack.
    #[test]
    fn dustreach_sewer_leg_skis_mouth_to_hall() {
        let map = MapId::DesertOfDeathClone;
        for speed in [30.0_f32, 16.0] {
            let mut world = World::new();
            world.set_map(map);
            world.start_match(true);
            world.players.truncate(1);
            world.player_id = 0;
            let pack = crate::map_pack::on(map).unwrap();
            let floor = pack.floor(Vec3::new(1076.0, 130.0, 652.0)).expect("leg floor").0;
            let p = &mut world.players[0];
            p.pos = Vec3::new(1076.0, floor + PLAYER_RADIUS, 652.0);
            p.vel = Vec3::new(0.0, 0.0, speed);
            p.yaw = std::f32::consts::PI;
            p.alive = true; p.health = 100.0; p.on_ground = true; p.skiing = true; p.jetting = false;
            world.input = Input::default();
            world.input.jump = true;
            let (mut min_speed, mut arrived) = (f32::MAX, None);
            for _ in 0..(60 * 60) {
                world.step_players(STEP);
                let p = &world.players[0];
                assert!(p.pos.y > 100.0, "fell through at {:?}", p.pos);
                assert!((p.pos.x - 1076.0).abs() < 3.5, "drifted into the wall at {:?}", p.pos);
                min_speed = min_speed.min(Vec2::new(p.vel.x, p.vel.z).length());
                if p.pos.z > 1010.0 { arrived = Some(Vec2::new(p.vel.x, p.vel.z).length()); break; }
            }
            let at_hall = arrived.unwrap_or_else(|| panic!("skier at {speed} m/s never reached the hall: {:?}", world.players[0].pos));
            eprintln!("sewer leg ski: in {speed} m/s, slowest {min_speed:.1}, at the hall {at_hall:.1}");
            assert!(min_speed > 5.0, "skier entering at {speed} m/s slowed to {min_speed} m/s");
        }
    }

    /// Every opening can be left by jet with the real movement code: from the
    /// floor of each drop shaft (midfield and flank, both teams) and from each
    /// mouth's portal, a player holding jet rises past the lip, steers out
    /// over the ground and lands standing outside the cut cell.
    #[test]
    fn dustreach_sewer_shafts_and_mouths_jet_out() {
        let map = MapId::DesertOfDeathClone;
        let pack = crate::map_pack::on(map).unwrap();
        // (red start x, z, cell x0, x1, z0, z1, outside target x, z)
        let red: [(f32, f32, f32, f32, f32, f32, f32, f32); 3] = [
            (1076.0, 940.0, 1072.0, 1080.0, 936.0, 944.0, 1090.0, 940.0),   // midfield shaft
            (1156.0, 684.0, 1152.0, 1160.0, 680.0, 688.0, 1170.0, 684.0),   // flank shaft
            (1076.0, 646.0, 1072.0, 1080.0, 584.0, 648.0, 1090.0, 640.0)];  // red mouth, out over the trench's east wall
        for &(sx, sz, x0, x1, z0, z1, tx, tz) in &red {
            for blue in [false, true] {
                let m = |x: f32, z: f32| if blue { (2048.0 - x, 2048.0 - z) } else { (x, z) };
                let (sx, sz) = m(sx, sz); let (tx, tz) = m(tx, tz);
                let ((ax, az), (bx, bz)) = (m(x0, z0), m(x1, z1));
                let (cx0, cx1, cz0, cz1) = (ax.min(bx), ax.max(bx), az.min(bz), az.max(bz));
                let mut world = World::new();
                world.set_map(map);
                world.start_match(true);
                world.players.truncate(1);
                world.player_id = 0;
                let lip = surface_on_edge_max(map, cx0, cx1, cz0, cz1);
                let floor = pack.floor(Vec3::new(sx, lip - 1.0, sz)).expect("opening floor").0;
                {
                    let p = &mut world.players[0];
                    p.pos = Vec3::new(sx, floor + PLAYER_RADIUS, sz);
                    p.vel = Vec3::ZERO; p.energy = ENERGY_MAX;
                    p.alive = true; p.health = 100.0; p.on_ground = true; p.skiing = false; p.jetting = false;
                    p.yaw = (-(tx - sx)).atan2(-(tz - sz));
                }
                world.input = Input::default();
                let mut out = false;
                for _ in 0..(60 * 10) {
                    let p = &world.players[0];
                    let above = p.pos.y > lip + 2.0;
                    world.input.jet = !above || p.vel.y < 0.0 && p.pos.y < lip + 1.0;
                    world.input.move_z = if above { 1.0 } else { 0.0 };
                    world.step_players(STEP);
                    let p = &world.players[0];
                    let inside = p.pos.x > cx0 && p.pos.x < cx1 && p.pos.z > cz0 && p.pos.z < cz1;
                    if !inside && p.on_ground { out = true; break; }
                }
                let p = &world.players[0];
                assert!(out, "{} opening at ({sx}, {sz}): no way out by jet, ended at {:?}", if blue { "blue" } else { "red" }, p.pos);
            }
        }
    }

    fn surface_on_edge_max(map: MapId, x0: f32, x1: f32, z0: f32, z1: f32) -> f32 {
        let mut top = f32::MIN;
        for k in 0..=16 {
            let t = k as f32 / 16.0;
            for (x, z) in [(x0 + (x1 - x0) * t, z0), (x0 + (x1 - x0) * t, z1), (x0, z0 + (z1 - z0) * t), (x1, z0 + (z1 - z0) * t)] {
                top = top.max(crate::terrain::surface_on(map, x, z).0);
            }
        }
        top
    }

    /// Evidence, not a rule: crossing Dustreach from the red mouth's top to
    /// the blue mouth's top, over the dunes (through the Sun Gate) versus
    /// through the sewer, with the same simple ski-and-jet driver. Run with
    /// `cargo test -p peakrunner-core --lib dustreach_surface_versus_sewer -- --ignored --nocapture`.
    #[test]
    #[ignore = "route timing probe"]
    fn dustreach_surface_versus_sewer_crossing_time() {
        let map = MapId::DesertOfDeathClone;
        let pack = crate::map_pack::on(map).unwrap();
        let at = |x: f32, z: f32, below: f32| {
            let y = pack.floor(Vec3::new(x, below, z)).map(|f| f.0).unwrap_or_else(|| crate::terrain::surface_on(map, x, z).0);
            Vec3::new(x, y, z)
        };
        let start = at(1096.0, 590.0, 400.0);
        let finish = at(952.0, 1458.0, 400.0);
        let hall = pack.floor(Vec3::new(1024.0, 140.0, 1024.0)).unwrap().0;
        let surface = vec![start, finish];
        let sewer = vec![start, at(1076.0, 592.0, 400.0), at(1076.0, 652.0, 140.0), at(1076.0, 1020.0, hall + 3.0),
            at(972.0, 1028.0, hall + 3.0), at(972.0, 1396.0, 140.0), at(972.0, 1456.0, 400.0), finish];
        let run = |route: &[Vec3]| -> Option<f32> {
            let mut world = World::new();
            world.set_map(map);
            world.start_match(true);
            world.players.truncate(1);
            world.player_id = 0;
            {
                let p = &mut world.players[0];
                p.pos = route[0] + Vec3::Y * PLAYER_RADIUS;
                p.vel = Vec3::ZERO; p.energy = ENERGY_MAX;
                p.alive = true; p.health = 100.0; p.on_ground = true;
            }
            world.input = Input::default();
            let mut k = 1;
            for tick in 0..(60 * 240) {
                let p = &world.players[0];
                let target = route[k];
                let d = Vec2::new(target.x - p.pos.x, target.z - p.pos.z);
                if d.length() < if k + 1 == route.len() { 5.0 } else { 3.0 } {
                    if k + 1 == route.len() { return Some(tick as f32 * STEP); }
                    k += 1;
                    continue;
                }
                let speed = Vec2::new(p.vel.x, p.vel.z).length();
                let climb = target.y > p.pos.y + 2.0;
                world.players[0].yaw = (-d.x).atan2(-d.y);
                world.input.move_z = 1.0;
                world.input.jump = true;
                world.input.jet = climb || speed < 18.0;
                world.step_players(STEP);
            }
            None
        };
        let (a, b) = (run(&surface), run(&sewer));
        eprintln!("Dustreach crossing, red mouth top to blue mouth top: surface {a:?} s, sewer {b:?} s");
        assert!(a.is_some() && b.is_some(), "both routes must finish");
        assert!(a.unwrap() < b.unwrap(), "the surface should stay the faster crossing");
    }

    /// Old Holler's (key `raindance`) bishop flag tower, both teams. Using only
    /// inputs (facing, W, jet) and the real movement code, a player gets from
    /// the roof into the chamber through each of the four doors and over the
    /// mitre through the slit, standing on the chamber floor each time; and
    /// leaves again the same way.
    #[test]
    fn old_holler_flag_tower_entries_and_exits_are_flyable() {
        const FLOOR: f32 = 17.6;
        const TZ: f32 = 20.0;
        let map = MapId::Raindance;
        let h = std::f32::consts::FRAC_1_SQRT_2;
        // The slit's clear lane for a standing body: 9 m out and 1.5 m in along
        // its back-left facing, 0.85 m along the slit, feet 16.5 m above the roof.
        let (sx, sz, lx, lz) = (-h, h, 0.85 * h, 0.85 * h);
        let slit_out = (sx * 9. + lx, 25.1, TZ + sz * 9. + lz);
        let slit_in = (sx * 1.5 + lx, 25.1, TZ + sz * 1.5 + lz);
        // (name, start on the roof, waypoints in, waypoints out); each waypoint
        // is (local x, y above the base origin, local z, jet). Each door is
        // entered from a hover over the ledge round the collar (r 5.8..8.2),
        // straight in: nothing stands behind the doors.
        type Route = (&'static str, (f32, f32, f32), Vec<(f32, f32, f32, bool)>, Vec<(f32, f32, f32, bool)>);
        let routes: [Route; 5] = [
            ("front", (0., 8.6, 8.),
                vec![(0., FLOOR + 1., 13.3, true), (0., FLOOR, 16.2, false), (0., FLOOR, 20., false)],
                vec![(0., FLOOR, 16.2, false), (0., FLOOR, 13.4, false), (0., 8.6, 9.5, false)]),
            // East: walk up the tower ramp from the roof half to its landing
            // and in the east door; out the same way, down to the roof.
            ("east", (29.0, 8.6, 22.2),
                vec![(11.5, FLOOR, 22.2, false), (8.5, FLOOR, 20., false), (3.8, FLOOR, 20., false),
                     (0., FLOOR, 20., false)],
                vec![(3.8, FLOOR, 20., false), (8.5, FLOOR, 20., false), (11.5, FLOOR, 22.2, false),
                     (29.0, 8.6, 22.2, false)]),
            ("back", (8., 8.6, 26.),
                vec![(0., FLOOR + 2.5, 30., true), (0., FLOOR + 1., 26.6, true), (0., FLOOR, 23.8, false),
                     (0., FLOOR, 20., false)],
                vec![(0., FLOOR, 23.8, false), (0., FLOOR, 26.6, false), (7.5, FLOOR + 1., 27.5, true),
                     (8., 8.6, 26., false)]),
            // West: jet from the deck up past the landing's front edge onto
            // the ledge and in the west door; out over the landing and down.
            ("west", (-14., 8.6, 10.),
                vec![(-9.5, FLOOR + 2.5, 14.5, true), (-7.5, FLOOR + 1., 20., true), (-3.8, FLOOR, 20., false),
                     (0., FLOOR, 20., false)],
                vec![(-3.8, FLOOR, 20., false), (-7.5, FLOOR, 20., false), (-10.5, FLOOR + 1., 20., true),
                     (-16., 8.6, 12., false)]),
            ("slit", (-13., 8.6, 26.),
                vec![(slit_out.0, slit_out.1, slit_out.2, true), (slit_in.0, slit_in.1, slit_in.2, true),
                     (slit_in.0, FLOOR, slit_in.2, false)],
                vec![(slit_in.0, slit_in.1, slit_in.2, true), (slit_out.0, slit_out.1, slit_out.2, true),
                     (-16., 11.6, 26., true), (-16., 8.6, 26., false)]),
        ];
        for team in [0u8, 1] {
            let to_world = |x: f32, y: f32, z: f32| if team == 0 {
                Vec3::new(1160. - x, 112. + y, 480. - z) } else { Vec3::new(800. + x, 112. + y, 1400. + z) };
            let axis = to_world(0., FLOOR, TZ);
            let floor = axis.y;
            for (name, start, way, out) in &routes {
                // In: from the roof to the chamber floor.
                let mut world = World::new();
                world.set_map(map);
                world.start_match(true);
                world.players.truncate(1);
                world.player_id = 0;
                let p = &mut world.players[0];
                p.pos = to_world(start.0, start.1, start.2) + Vec3::Y * PLAYER_RADIUS;
                p.vel = Vec3::ZERO; p.alive = true; p.health = 100.; p.energy = ENERGY_MAX;
                p.on_ground = true; p.skiing = false; p.jetting = false;
                let wps: Vec<(Vec3, bool)> = way.iter().map(|w| (to_world(w.0, w.1, w.2), w.3)).collect();
                fly(&mut world, &wps, 60 * 20).unwrap_or_else(|e| panic!("team {team} {name} in: {e}"));
                let p = &world.players[0];
                assert!(p.on_ground && (p.pos.y - floor - PLAYER_RADIUS).abs() < 0.05,
                    "team {team} {name}: not standing on the chamber floor at {:?}", p.pos);
                assert!(Vec2::new(p.pos.x - axis.x, p.pos.z - axis.z).length() < 5.2,
                    "team {team} {name}: ended outside the chamber at {:?}", p.pos);
                // Out the same way, back onto the roof beyond the plinth.
                let back: Vec<(Vec3, bool)> = out.iter().map(|w| (to_world(w.0, w.1, w.2), w.3)).collect();
                world.players[0].energy = ENERGY_MAX;
                fly(&mut world, &back, 60 * 20).unwrap_or_else(|e| panic!("team {team} {name} out: {e}"));
                let p = &world.players[0];
                assert!(Vec2::new(p.pos.x - axis.x, p.pos.z - axis.z).length() > 7.6,
                    "team {team} {name}: did not leave the tower, at {:?}", p.pos);
            }
        }
    }

    /// Old Holler's four flag-chamber doors line up in opposite pairs through
    /// the flag. An enemy flying straight through at 20, 30 or 40 m/s, jetting
    /// only to hold height, grabs the flag without touching a wall and leaves
    /// through the opposite door at nearly full speed, both ways and on both
    /// teams' towers.
    #[test]
    fn old_holler_flag_chamber_fly_through_grabs_the_flag_at_speed() {
        const FLOOR: f32 = 17.6;
        const TZ: f32 = 20.0;
        for team in [0u8, 1] {
            let to_world = |x: f32, y: f32, z: f32| if team == 0 {
                Vec3::new(1160. - x, 112. + y, 480. - z) } else { Vec3::new(800. + x, 112. + y, 1400. + z) };
            // (name, local start, local direction): front to back and east to west.
            let passes = [("front-back", (0., TZ - 15.)), ("back-front", (0., TZ + 15.)),
                          ("east-west", (15., TZ)), ("west-east", (-15., TZ))];
            for (name, (x0, z0)) in passes {
                for speed in [20.0f32, 30., 40.] {
                    let mut world = World::new();
                    world.set_map(MapId::Raindance);
                    world.start_match(true);
                    world.players.truncate(1);
                    world.player_id = 0;
                    let fi = team as usize;
                    let home = world.flags[fi].home;
                    let start = to_world(x0, FLOOR + 1.9, z0);
                    let through = to_world(-x0, FLOOR + 1.9, 2. * TZ - z0);
                    let dir = (through - start).normalize();
                    let p = &mut world.players[0];
                    p.team = if team == 0 { Team::Glacier } else { Team::Ember };
                    p.carrying = None;
                    p.pos = start; p.vel = dir * speed; p.alive = true; p.health = 100.;
                    p.energy = ENERGY_MAX; p.on_ground = false; p.skiing = false; p.jetting = false;
                    p.yaw = (-dir.x).atan2(-dir.z);
                    let hold = start.y;
                    let mut slowest = speed;
                    let mut crossed = false;
                    for _ in 0..240 {
                        let (pos, vel) = (world.players[0].pos, world.players[0].vel);
                        if (pos - start).dot(dir) > 30. { crossed = true; break; }
                        let mut input = Input::default();
                        input.jet = pos.y + vel.y * vel.y.abs() / 40. < hold;
                        world.input = input;
                        world.step_players(STEP);
                        world.step_flags(STEP);
                        slowest = slowest.min(Vec2::new(world.players[0].vel.x, world.players[0].vel.z).length());
                    }
                    let p = &world.players[0];
                    assert!(crossed, "team {team} {name} at {speed}: stuck at {:?}", p.pos);
                    assert_eq!(world.flags[fi].carrier, Some(0), "team {team} {name} at {speed}: flag not grabbed (home {home:?})");
                    assert!(slowest > 0.9 * speed, "team {team} {name} at {speed}: slowed to {slowest}, hit something");
                }
            }
        }
    }

    /// Steer the one player through waypoints with inputs only (facing, W,
    /// jet). A waypoint with `jet` holds its height with the jet and is
    /// reached within 0.8 m across and 1.2 m of height; one without is reached
    /// on foot, or by falling onto it, within 0.8 m across and 0.6 m of height.
    fn fly(world: &mut World, wps: &[(Vec3, bool)], ticks: usize) -> Result<(), String> {
        let mut k = 0;
        for _ in 0..ticks {
            let (target, jet) = wps[k];
            let (pos, vel, ground) = { let p = &world.players[0]; (p.pos, p.vel, p.on_ground) };
            if !pos.is_finite() || !world.players[0].alive { return Err(format!("lost the player at {pos:?}")); }
            let to = Vec3::new(target.x - pos.x, 0., target.z - pos.z);
            let dist = to.length();
            let reached = dist < 0.8 && if jet { (pos.y - PLAYER_RADIUS - target.y).abs() < 1.2 }
                else { (pos.y - PLAYER_RADIUS - target.y).abs() < 0.6 };
            if reached {
                k += 1;
                if k == wps.len() {
                    for _ in 0..60 { world.input = Input::default(); world.step_players(STEP); }
                    return Ok(());
                }
                continue;
            }
            let mut input = Input::default();
            // Steer by velocity error: want a speed that can still stop in the
            // remaining distance, straight at the target; face the difference
            // between that and the current velocity and push along it. This
            // cancels sideways drift, so the player does not orbit a waypoint.
            let brake = if ground { 20. } else { 6. };
            let wanted = if dist > 1e-3 { to / dist * (2. * brake * dist).sqrt().min(if ground { 6. } else { 4. }) } else { Vec3::ZERO };
            let error = wanted - Vec3::new(vel.x, 0., vel.z);
            if error.length() > 0.3 {
                world.players[0].yaw = (-error.x).atan2(-error.z);
                input.move_z = 1.;
            }
            // Hold height by the apex a coast would reach (gravity 20 m/s/s).
            let apex = pos.y - PLAYER_RADIUS + vel.y * vel.y.abs() / 40.;
            input.jet = jet && apex < target.y;
            world.input = input;
            world.step_players(STEP);
        }
        let p = &world.players[0];
        Err(format!("stuck before waypoint {k} {:?} at {:?} vel {:?}", wps[k].0, p.pos, p.vel))
    }

    #[test]
    fn server_respawn_picks_varied_points_for_the_right_team() {
        for map in [MapId::BroadsideClone, MapId::Raindance] {
        let mut world = World::new();
        world.set_map(map);
        world.start_match(true);
        for team in [Team::Ember, Team::Glacier] {
            let points = crate::terrain::spawn_points_on(map, team == Team::Ember);
            let i = world.players.iter().position(|p| p.team == team).unwrap();
            let mut used = std::collections::BTreeSet::new();
            let mut last = None;
            for _ in 0..60 {
                world.respawn(i);
                let p = &world.players[i];
                let k = points.iter().position(|(pos, _)| *pos == p.pos).expect("respawned on an authored point");
                assert_eq!(p.yaw, points[k].1);
                assert_ne!(Some(k), last, "same point twice in a row");
                last = Some(k);
                used.insert(k);
            }
            assert!(used.len() >= points.len() - 1, "{map:?} {team:?} used only {used:?}");
        }
        }
        // Maps without spawn_points keep the legacy single-spawn path.
        assert!(crate::terrain::spawn_points_on(MapId::Valley, false).is_empty());
    }
}

#[cfg(test)]
mod line_of_sight_tests {
    use super::*;
    use crate::equipment::{self,Kind};

    fn world(map:MapId)->World {
        let mut w=World::new();w.set_map(map);w.start_rift(true);w.players.truncate(1);
        w.players[0].team=Team::Glacier;w.players[0].alive=true;w.players[0].vel=Vec3::ZERO;w
    }
    fn spawns(map:MapId)->Vec<Vec3> {
        let m=&crate::map_pack::on(map).unwrap().manifest;
        let mut v:Vec<Vec3>=m.spawn_points.iter().flatten().map(|p|Vec3::new(p[0],p[1],p[2])).collect();
        v.extend(m.spawns.iter().map(|p|Vec3::from_array(*p)));v
    }
    fn fired_by(w:&World,turret:usize)->bool {w.discs.iter().any(|d|d.owner==MAX_PLAYERS && d.spin==turret as f32)}

    /// Doorways and windows are open: a turret may see this far past an
    /// opening's inner face, and no further into a room.
    const DOOR_DEPTH:f32=3.;
    /// Whether turret `d` would engage an enemy standing at `pos`: field of
    /// fire, sensor-boosted range and a clear barrel-to-chest line, exactly as
    /// `step_equipment` decides it.
    fn engages(map:MapId,d:&equipment::Definition,pos:Vec3)->bool {
        let Some(profile)=equipment::profile(d.kind,d.weapon) else {return false};
        if !d.in_arc(pos) {return false;}
        let enemy=equipment::Candidate {index:0,team:1-d.team,pos,vel:Vec3::ZERO};
        equipment::acquire_target(d.pos(),d.radius,d.team,&profile,true,[enemy],
            |a,b|obstacle_hit(map,&[],a,b,0.).is_none()).is_some()
    }
    /// Whether `p` lies within DOOR_DEPTH (horizontally) of any opening's
    /// inner face, given as world-space (x, z) segments.
    fn in_doorway(p:Vec3,openings:&[(Vec2,Vec2)])->bool {
        let q=Vec2::new(p.x,p.z);
        openings.iter().any(|&(a,b)| {
            let ab=b-a;let t=((q-a).dot(ab)/ab.length_squared().max(1e-9)).clamp(0.,1.);
            q.distance(a+ab*t)<DOOR_DEPTH
        })
    }

    /// Every Ember turret, against a Glacier player at each Ember spawn it
    /// cannot see: no track, no shot; a bolt aimed straight at the hidden
    /// player dies in the wall and does no damage.
    #[test]
    fn tower_turrets_do_not_engage_or_hit_players_behind_walls() {
        let map=MapId::BroadsideClone;
        let pack=crate::map_pack::on(map).unwrap();
        let defs=equipment::definitions(map);
        let mut hidden=0;
        for (i,d) in defs.iter().enumerate().filter(|(_,d)|d.kind==Kind::Turret && d.team==0) {
            for spot in spawns(map) {
                let target=spot+Vec3::Y*0.8;let delta=target-d.pos();
                if delta.length()>80. {continue;}
                let muzzle=d.pos()+delta.normalize()*(d.radius+0.6);
                let Some((t,_))=pack.sweep(muzzle,target,0.) else {continue};
                assert!(t<0.99);
                hidden+=1;
                let mut w=world(map);w.players[0].pos=spot;
                w.step_equipment(STEP);
                assert_eq!(w.equipment[i].contacts,0,"{} tracked a player behind a wall at {spot}",d.id);
                assert!(!fired_by(&w,i),"{} fired at a player behind a wall at {spot}",d.id);
                w.discs.push(Disc {pos:muzzle,vel:delta.normalize()*BOLT_SPEED,team:Team::Ember,owner:MAX_PLAYERS,life:1.,kind:1,spin:i as f32});
                for _ in 0..30 {w.step_discs(STEP);}
                assert!(w.discs.is_empty());
                assert_eq!(w.players[0].health,100.,"bolt from {} passed a wall to {spot}",d.id);
            }
        }
        assert!(hidden>=4,"too few hidden spawn cases ({hidden}) to mean anything");
    }

    /// Pod turrets must not engage anyone inside the rooms they guard.
    /// Standable points in the tower (all levels), the rear tunnels and both
    /// rear rooms are sampled on a 1 m grid; no turret may engage one more
    /// than DOOR_DEPTH past an open front opening, tunnel mouth or Level 3
    /// window.
    #[test]
    fn pod_turrets_cannot_see_into_tower_rooms() {
        let map=MapId::BroadsideClone;
        let pack=crate::map_pack::on(map).unwrap();
        let info=crate::terrain::info(map);
        let defs=equipment::definitions(map);
        // Local base frame: red faces +z (yaw 180), blue faces -z (yaw 0).
        let to_world=|team:u8,lx:f32,y:f32,lz:f32| if team==0 {
            Vec3::new(info.ember.x-lx,info.ember.y+y,info.ember.z-lz)
        } else {Vec3::new(info.glacier.x+lx,info.glacier.y+y,info.glacier.z+lz)};
        let flat=|team:u8,lx:f32,lz:f32| {let p=to_world(team,lx,0.,lz);Vec2::new(p.x,p.z)};
        // Inner faces of the open openings (local x0, z0, x1, z1): Level 1 front
        // door and 6 m bridge doors, the 8 m rear tunnel mouths, Level 3
        // window bands (the side bands also cover the Level 2 side doors).
        let inner=11.6;
        let local:[(f32,f32,f32,f32);9]=[(-12.,-inner,-6.,-inner),(-3.,-inner,3.,-inner),(6.,-inner,12.,-inner),
            (-12.,inner,-4.,inner),(4.,inner,12.,inner),
            (-9.,-inner,9.,-inner),(-9.,inner,9.,inner),(-inner,-9.,-inner,9.),(inner,-9.,inner,9.)];
        let openings:Vec<(Vec2,Vec2)>=[0u8,1].iter().flat_map(|&t|local.iter().map(move |&(x0,z0,x1,z1)|(t,x0,z0,x1,z1)))
            .map(|(t,x0,z0,x1,z1)|(flat(t,x0,z0),flat(t,x1,z1))).collect();
        // (x0,x1,z0,z1,floor levels to probe from)
        let rooms:[(f32,f32,f32,f32,&[f32]);7]=[
            (-11.5,11.5,-11.5,11.5,&[0.,8.,17.]),   // tower: atrium floor, balconies
            (-11.5,-4.5,12.,26.,&[0.]),(4.5,11.5,12.,26.,&[0.]),   // tunnels
            (-12.5,-3.5,26.5,35.5,&[0.]),(3.5,12.5,26.5,35.5,&[0.]),   // armory, ship room
            (-8.,8.,-8.,8.,&[-8.]),   // keel-level generator room
            (8.6,13.3,-2.,2.,&[-8.])];   // keel hatch passage
        let (mut sampled,mut seen)=(0,Vec::new());
        for (i,d) in defs.iter().enumerate().filter(|(_,d)|d.kind==Kind::Turret) {
            for (r,&(x0,x1,z0,z1,levels)) in rooms.iter().enumerate() {
                let mut lx=x0; while lx<=x1 { let mut lz=z0; while lz<=z1 {
                    for &level in levels {
                        let probe=to_world(d.team,lx,level+5.5,lz);
                        let Some((fy,_))=pack.floor(probe) else {continue};
                        if fy<probe.y-6.9 {continue;}
                        let pos=Vec3::new(probe.x,fy+1.2,probe.z);
                        if pack.body_sweep(pos,pos+Vec3::Y*0.01).is_some() {continue;}
                        sampled+=1;
                        if !in_doorway(pos,&openings) && engages(map,d,pos) {seen.push((r,i,lx,fy-info.ember.y,lz));}
                    }
                lz+=1.;} lx+=1.;}
            }
        }
        assert!(sampled>2000,"too few interior samples ({sampled})");
        let by_room:Vec<usize>=(0..7).map(|r|seen.iter().filter(|s|s.0==r).count()).collect();
        assert!(seen.is_empty(),"{} interior points engaged by pod turrets (tower, left tunnel, right tunnel, armory, ship, keel room, hatch passage: {by_room:?}), e.g. {:?}",seen.len(),&seen[..seen.len().min(12)]);
    }

    /// Cairnhold's sentry, roof and battery turrets, through the server's
    /// rule (field of fire, sensor-boosted range, line of sight), must not
    /// engage a player standing anywhere inside the bunker, the trench, the
    /// guard hut, the flag tower's interior floors, the vault or the sally
    /// port, beyond DOOR_DEPTH of the open front and exit-house doors.
    /// Tolerance: the columns under the tower's open roof hatches.
    #[test]
    fn cairnhold_turrets_cannot_see_into_base_rooms() {
        let map=MapId::StonehengeClone;
        let pack=crate::map_pack::on(map).unwrap();
        let info=crate::terrain::info(map);
        let defs=equipment::definitions(map);
        let to_world=|team:u8,lx:f32,y:f32,lz:f32| if team==0 {
            Vec3::new(info.ember.x-lx,info.ember.y+y,info.ember.z-lz)
        } else {Vec3::new(info.glacier.x+lx,info.glacier.y+y,info.glacier.z+lz)};
        let trench=|_x:f32,z:f32| 22.*((z-20.)/56.).clamp(0.,1.);
        let level=|y:f32| move |_x:f32,_z:f32| y;
        // Sally port floors: vault door -7.5 -> 3 over x 17..45; south leg 3 -> 8 over z -20..-36.
        let east=|x:f32,_z:f32| -7.5+10.5*((x-17.)/28.).clamp(0.,1.);
        let south=|_x:f32,z:f32| 3.+5.*((-20.-z)/16.).clamp(0.,1.);
        // (x0,x1,z0,z1,floor(x,z),skip open columns: tower hatches and tunnel skylights)
        let rooms:[(f32,f32,f32,f32,&dyn Fn(f32,f32)->f32,bool);14]=[
            (-15.,15.,-18.6,19.,&level(0.),false),      // bunker: hall, inventory and back rooms
            (-14.6,-11.6,-12.4,3.6,&level(7.),false),     // hall gallery, west side
            (11.6,14.6,-12.4,3.6,&level(7.),false),       // hall gallery, east side
            (-10.8,10.8,0.4,3.6,&level(7.),false),        // hall gallery, back
            (1.6,6.4,20.6,75.4,&trench,false),            // covered trench
            (-6.6,14.6,77.4,98.6,&level(22.),true),       // guard hut
            (3.4,14.6,77.4,94.6,&level(29.),true),        // tower floor over the hut
            (3.4,14.6,92.4,94.9,&level(35.),false),       // tower landing
            (3.8,10.6,-9.2,-0.8,&level(-7.5),false),      // vault (generator room)
            (3.8,9.8,4.,11.4,&level(-9.7),false),         // generator well
            (11.8,14.6,4.6,11.4,&level(-7.5),false),      // walkway to the tunnel door
            (16.6,86.6,5.4,10.6,&east,true),              // sally port, east leg
            (81.4,86.6,-35.4,4.,&south,false),            // sally port, south leg
            (81.4,86.6,-42.6,-36.2,&level(8.),false)];    // exit house
        let open_column=|lx:f32,lz:f32| ((9.5..=15.2).contains(&lx) && (80.0..=94.0).contains(&lz))   // tower hatches
            || ((47.5..=52.5).contains(&lx) || (63.5..=68.5).contains(&lx)) && (4.0..=12.0).contains(&lz);  // skylights
        let flat=|team:u8,lx:f32,lz:f32| {let p=to_world(team,lx,0.,lz);Vec2::new(p.x,p.z)};
        // Open doorways: the 6 m front door, the gallery doorways in both hall
        // side walls, and the exit house door.
        let openings:Vec<(Vec2,Vec2)>=[0u8,1].iter().flat_map(|&t|[(flat(t,-3.,-19.2),flat(t,3.,-19.2)),
            (flat(t,-15.2,-7.),flat(t,-15.2,-1.)),(flat(t,15.2,-7.),flat(t,15.2,-1.)),
            (flat(t,87.2,-43.),flat(t,87.2,-37.4))]).collect();
        let (mut sampled,mut seen)=(0,Vec::new());
        for d in defs.iter().filter(|d|matches!(d.kind,Kind::Turret)) {
            {   let team=d.team;   // attackers inside the turret's own base
                for (r,&(x0,x1,z0,z1,floor,hatch)) in rooms.iter().enumerate() {
                    let mut lx=x0; while lx<=x1 { let mut lz=z0; while lz<=z1 {
                        if !(hatch && open_column(lx,lz)) {
                            let y=floor(lx,lz);
                            let probe=to_world(team,lx,y+1.5,lz);
                            if let Some((fy,_))=pack.floor(probe) {
                                let pos=Vec3::new(probe.x,fy+1.2,probe.z);
                                if (fy-(info.ember.y+y)).abs()<1.5 && pack.body_sweep(pos,pos+Vec3::Y*0.01).is_none() {
                                    sampled+=1;
                                    if !in_doorway(pos,&openings) && engages(map,d,pos) {
                                        seen.push((d.id.clone(),r,lx,y,lz,pos));
                                    }
                                }
                            }
                        }
                    lz+=1.;} lx+=1.;}
                }
            }
        }
        assert!(sampled>3000,"too few interior samples ({sampled})");
        let by_room:Vec<usize>=(0..14).map(|r|seen.iter().filter(|s|s.1==r).count()).collect();
        assert!(seen.is_empty(),"{} interior points visible to turrets (bunker, 3 galleries, trench, hut, tower floor, landing, vault, well, walkway, east, south, exit: {by_room:?}), e.g. {:?}",
            seen.len(),&seen[..seen.len().min(8)]);
    }

    /// Same rule for Old Holler's (key `raindance`) sunken halls and the
    /// generator basements beneath them. The roof turrets sit beside the ramp
    /// openings and the atrium; nothing may show them a hall floor, basement
    /// or service-passage point (nor may the lookout towers).
    #[test]
    fn raindance_turrets_cannot_see_into_the_halls() {
        let map=MapId::Raindance;
        let pack=crate::map_pack::on(map).unwrap();
        let defs=equipment::definitions(map);
        let to_world=|team:u8,lx:f32,y:f32,lz:f32| if team==0 {
            Vec3::new(1160.-lx,112.+y,480.-lz)} else {Vec3::new(800.+lx,112.+y,1400.+lz)};
        // (x0, x1, z0, z1, floor above the base origin): hall, basement, passage,
        // and the whole of the bishop tower's flag chamber (axis at local z 20,
        // inner radius 5.2, kept to 4.7 for a body). Its four doors (front -Z,
        // east +X, back +Z, west -X; 2.7 m wide at the inner face) are open;
        // DOOR_DEPTH past their inner faces is allowed.
        let rooms:&[(f32,f32,f32,f32,f32)]=&[(-29.,29.,-25.,25.,-10.),(-10.5,10.5,-5.5,23.5,-19.2),(-4.5,-1.5,25.3,31.3,-19.2),
            (-4.8,4.8,15.2,24.8,17.6)];
        let flat=|team:u8,lx:f32,lz:f32| {let p=to_world(team,lx,0.,lz);Vec2::new(p.x,p.z)};
        let openings:Vec<(Vec2,Vec2)>=[0u8,1].iter().flat_map(|&t|[(flat(t,-1.35,14.8),flat(t,1.35,14.8)),
            (flat(t,5.2,18.65),flat(t,5.2,21.35)),(flat(t,-1.35,25.2),flat(t,1.35,25.2)),
            (flat(t,-5.2,18.65),flat(t,-5.2,21.35))]).collect();
        let (mut sampled,mut chamber,mut seen)=(0,0,Vec::new());
        for d in defs.iter().filter(|d|matches!(d.kind,Kind::Turret)) {
            for team in [0u8,1] { for &(x0,x1,z0,z1,level) in rooms {
                let mut lx=x0; while lx<=x1 { let mut lz=z0; while lz<=z1 {
                    if level>17. && Vec2::new(lx,lz-20.).length()>4.7 {lz+=1.5;continue;}
                    let probe=to_world(team,lx,level+1.5,lz);
                    if let Some((fy,_))=pack.floor(probe) {
                        let pos=Vec3::new(probe.x,fy+1.2,probe.z);
                        if (fy-(112.+level)).abs()<0.05 && pack.body_sweep(pos,pos+Vec3::Y*0.01).is_none() {
                            sampled+=1; if level>17. {chamber+=1;}
                            if !in_doorway(pos,&openings) && engages(map,d,pos) {
                                seen.push((d.id.clone(),team,lx,lz,pos));
                            }
                        }
                    }
                lz+=1.5;} lx+=1.5;}
            }}
        }
        assert!(sampled>3500,"too few hall and basement samples ({sampled})");
        assert!(chamber>100,"too few flag-chamber samples ({chamber})");
        assert!(seen.is_empty(),"{} hall or basement points visible to turrets, e.g. {:?}",seen.len(),&seen[..seen.len().min(8)]);
    }

    /// Same rule for Frostline and Dustreach, over the rooms their Python
    /// suites check (local base coordinates; team 0 is yawed 180 degrees).
    /// Doors and windows are open; DOOR_DEPTH past their inner faces is
    /// allowed. Baffled vestibules (the storehouse, the outpost, the
    /// basement's tunnel door) lie outside these boxes.
    #[test]
    fn frostline_and_dustreach_turrets_cannot_see_into_base_rooms() {
        // (x0, x1, z0, z1, floor above the base origin)
        let frost:&[(f32,f32,f32,f32,f32)]=&[
            (-9.8,9.8,-12.8,5.4,0.),       // station hall, between the two ramps
            (-12.8,12.8,7.2,12.8,0.),      // rear hall
            (-12.8,12.8,-12.8,12.8,7.5),   // command deck (flag level)
            (-6.8,4.2,-6.8,6.8,-7.5),      // basement generator room, west of its baffle
            (4.8,6.8,1.,6.8,-7.5),         // basement, north of the baffle
            (8.6,15.4,-6.6,-1.4,-7.5),     // tunnel to the service shed
            (98.1,109.9,-77.9,-69.,20.)];  // relay outpost behind its baffle
        let dust:&[(f32,f32,f32,f32,f32)]=&[
            (-12.6,12.6,-14.6,2.6,5.),     // keep hall
            (-12.6,12.6,-15.6,5.6,-4.),    // cistern (generator room) under the keep, 8 m tall
            (-14.6,-9.4,7.6,21.6,-4.),     // service tunnel, north leg
            (-23.4,-16.6,16.4,21.6,-4.),   // service tunnel, west leg
            (-30.6,-25.4,16.4,18.4,-4.),   // watch tower base room, south of its exit ramp
            (-26.8,-25.4,18.4,26.,-4.),    // tower room beside the exit ramp
            (32.2,46.6,3.4,17.8,0.),       // storehouse hall floor, behind its baffles
            (32.2,33.2,3.4,20.6,5.),       // storehouse mezzanine, behind the gate baffle
            (34.2,37.,15.4,20.6,5.)];      // storehouse landing at the ramp's head
        for (map,rooms,min) in [(MapId::SnowblindClone,frost,1500),(MapId::DesertOfDeathClone,dust,1500)] {
            let pack=crate::map_pack::on(map).unwrap();
            let info=crate::terrain::info(map);
            let to_world=|team:u8,lx:f32,y:f32,lz:f32| if team==0 {
                Vec3::new(info.ember.x-lx,info.ember.y+y,info.ember.z-lz)
            } else {Vec3::new(info.glacier.x+lx,info.glacier.y+y,info.glacier.z+lz)};
            // Frostline has no floor over its two ramp openings, the
            // two-level void in front of the flag, or its basement stair's
            // opening in the hall.
            let skip=|lx:f32,y:f32,lz:f32| map==MapId::SnowblindClone && (
                (y==7.5 && ((-13.4..=-10.4).contains(&lx) || (10.4..=13.4).contains(&lx)) && (-10.0..=-2.6).contains(&lz))
                || (y==7.5 && (-5.3..=5.3).contains(&lx) && (-4.3..=6.3).contains(&lz))
                || (y==0. && (-7.7..=-4.1).contains(&lx) && (-7.4..=0.3).contains(&lz)));
            let mut targets=Vec::new();
            for team in [0u8,1] {
                for &(x0,x1,z0,z1,y) in rooms {
                    let mut lx=x0; while lx<=x1+0.01 { let mut lz=z0; while lz<=z1+0.01 {
                        if !skip(lx,y,lz) {
                            let probe=to_world(team,lx,y+1.5,lz);
                            if let Some((fy,_))=pack.floor(probe) {
                                let pos=Vec3::new(probe.x,fy+1.2,probe.z);
                                if (fy-(info.ember.y+y)).abs()<1.5 && pack.body_sweep(pos,pos+Vec3::Y*0.01).is_none() {
                                    targets.push(pos);
                                }
                            }
                        }
                    lz+=1.;} lx+=1.;}
                }
            }
            assert!(targets.len()>min,"{map:?}: too few interior samples ({})",targets.len());
            // Inner faces of the open doors and windows, local (x0, z0, x1, z1).
            let local:&[(f32,f32,f32,f32)]=if map==MapId::SnowblindClone {&[
                (-3.,-13.4,3.,-13.4),(-12.,13.4,-6.,13.4),(13.4,6.8,13.4,12.8),
                (-11.,-13.4,-4.,-13.4),(4.,-13.4,11.,-13.4),(-11.,13.4,-4.,13.4),(4.,13.4,11.,13.4),
                (-13.4,-10.,-13.4,-3.),(-13.4,3.,-13.4,10.),(13.4,-10.,13.4,-3.),(13.4,3.,13.4,10.)]
            } else {&[(-3.,-15.2,3.,-15.2),(-3.,3.2,3.,3.2),(-30.5,30.5,-26.5,30.5)]};
            let flat=|team:u8,lx:f32,lz:f32| {let p=to_world(team,lx,0.,lz);Vec2::new(p.x,p.z)};
            let openings:Vec<(Vec2,Vec2)>=[0u8,1].iter().flat_map(|&t|local.iter().map(move |&(x0,z0,x1,z1)|(t,x0,z0,x1,z1)))
                .map(|(t,x0,z0,x1,z1)|(flat(t,x0,z0),flat(t,x1,z1))).collect();
            let mut seen=Vec::new();
            for d in equipment::definitions(map).iter().filter(|d|d.kind==Kind::Turret) {
                for &pos in &targets {
                    if !in_doorway(pos,&openings) && engages(map,d,pos) {seen.push((d.id.clone(),pos));}
                }
            }
            assert!(seen.is_empty(),"{map:?}: {} interior points visible to turrets, e.g. {:?}",seen.len(),&seen[..seen.len().min(8)]);
        }
    }

    /// Dustreach's new routes are walkable end to end with the full player
    /// body (engine body sweep, 0.6 m above each floor or ramp), for both
    /// teams: hall -> stair -> cistern -> tunnel -> tower room -> exit ramp ->
    /// back door, and courtyard -> gate -> bridge -> storehouse -> its ramp and
    /// doors. Local base coordinates; team 0 is yawed 180 degrees.
    #[test]
    fn dustreach_underground_and_storehouse_routes_are_walkable() {
        let map=MapId::DesertOfDeathClone;
        let pack=crate::map_pack::on(map).unwrap();
        let info=crate::terrain::info(map);
        let stair=|z:f32| -4.0+9.0*((z+14.2)/16.2).clamp(0.,1.);           // hall 5 at z 2, cistern -4 at z -14.2
        let tramp=|z:f32| -4.0+3.9*((z-18.5)/8.0).clamp(0.,1.);           // tower room -4 to landing -0.1
        let sramp=|z:f32| 5.0*((z-4.8)/10.0).clamp(0.,1.);                // storehouse hall 0 to mezzanine 5
        let routes:Vec<(&str,Vec<(f32,f32,f32)>)>=vec![
            ("hall to tower exit",vec![(4.,5.,2.6),(11.25,5.,2.6),(11.25,stair(1.9),1.9),(11.25,stair(-14.1),-14.1),
                (11.25,-4.,-15.4),(8.2,-4.,-15.4),(-3.,-4.,-15.4),(-10.5,-4.,-15.4),(-11.,-4.,-10.),(-11.,-4.,5.),(-11.1,-4.,6.6),
                (-12.,-4.,8.),(-12.,-4.,19.),(-20.,-4.,19.),(-26.,-4.,17.5),(-29.2,-4.,17.5),(-29.2,tramp(18.6),18.6),
                (-29.2,tramp(26.4),26.4),(-28.5,-0.1,28.5),(-28.5,-0.1,32.3),(-23.,-0.1,32.3),(-20.,-0.1,32.3),
                (-18.,-0.1,36.)]),
            ("bridge onto the mezzanine and down",vec![(15.,5.,15.5),(20.6,5.,15.5),(24.6,5.,15.5),(28.4,5.,15.5),
                (29.9,5.,15.5),(29.9,5.,20.6),(33.,5.,20.6),(35.7,5.,18.),(35.7,sramp(14.7),14.7),(35.7,sramp(4.9),4.9),
                (35.7,0.,3.6),(40.,0.,3.6),(40.,0.,14.5),(46.4,0.,14.5),(46.4,0.,20.3),(41.5,0.,20.3),(41.5,0.,22.5),
                (41.5,-0.1,25.)]),
            ("storehouse ground west door",vec![(25.,-0.1,7.5),(28.4,0.,7.5),(29.9,0.,7.5),(29.9,0.,13.),(33.,0.,13.),
                (33.,0.,16.),(38.,0.,16.),(40.,0.,5.)]),
        ];
        for team in [0u8,1] {
            let world=|(x,y,z):(f32,f32,f32)| if team==0 {
                Vec3::new(info.ember.x-x,info.ember.y+y+0.6,info.ember.z-z)
            } else {Vec3::new(info.glacier.x+x,info.glacier.y+y+0.6,info.glacier.z+z)};
            for (name,points) in &routes {
                for pair in points.windows(2) {
                    let (a,b)=(world(pair[0]),world(pair[1]));
                    assert!(pack.body_sweep(a,b).is_none(),"team {team} {name}: blocked between {:?} and {:?}",pair[0],pair[1]);
                }
            }
        }
    }

    /// Cairnhold's generator route is walkable end to end with the full body
    /// (engine body sweep, 0.6 m above each floor or ramp), for both teams:
    /// hall -> stair -> vault -> sally port (east, then south) -> exit house ->
    /// round its vestibule -> out onto the battery bench.
    #[test]
    fn cairnhold_vault_and_sally_port_route_is_walkable() {
        let map=MapId::StonehengeClone;
        let pack=crate::map_pack::on(map).unwrap();
        let info=crate::terrain::info(map);
        // Vault floor -7.5 (6.5 m under the hall slab); stair head z -9.5, foot z 4.5.
        let stair=|z:f32| -7.5*((z+9.5)/14.).clamp(0.,1.);
        let east=|x:f32| -7.5+10.5*((x-17.)/28.).clamp(0.,1.);
        let south=|z:f32| 3.+5.*((-20.-z)/16.).clamp(0.,1.);
        let route=[(13.2,0.,-13.),(13.2,0.,-9.5),(13.2,stair(-5.),-5.),(13.2,stair(1.4),1.4),(13.2,stair(3.6),3.6),(13.2,-7.5,5.6),
            (13.2,-7.5,8.),(15.6,-7.5,8.),(17.,-7.5,8.),(29.,east(29.),8.),(45.,3.,8.),(60.,3.,8.),(84.,3.,8.),(84.,3.,4.),(84.,3.,-20.),
            (84.,south(-28.),-28.),(84.,8.,-36.),(82.8,8.,-38.),(82.8,8.,-42.5),(86.3,8.,-42.5),(86.3,8.,-41.3),
            (89.,8.,-41.3),(94.,8.,-46.)];
        for team in [0u8,1] {
            let world=|(x,y,z):(f32,f32,f32)| if team==0 {
                Vec3::new(info.ember.x-x,info.ember.y+y+0.6,info.ember.z-z)
            } else {Vec3::new(info.glacier.x+x,info.glacier.y+y+0.6,info.glacier.z+z)};
            for pair in route.windows(2) {
                let (a,b)=(world(pair[0]),world(pair[1]));
                assert!(pack.body_sweep(a,b).is_none(),"team {team}: blocked between {:?} and {:?}",pair[0],pair[1]);
            }
            // The stair head is the only way down from the hall: the rest of
            // the hall floor is solid over the vault.
            for (x,z) in [(5.,-6.),(9.,0.),(5.,6.)] {
                let p=world((x,0.,z));
                let (fy,_)=pack.floor(p).expect("hall or inventory floor");
                assert!((fy-(p.y-0.6)).abs()<0.01,"team {team}: floor at ({x},{z}) is {fy}");
            }
        }
    }

    /// The front openings are open straight in: a body walks from outside
    /// each opening across Level 1 with nothing in the way, and from the left
    /// bridge door on to the L1->L2 ramp foot.
    #[test]
    fn tower_entries_walk_straight_in() {
        let map=MapId::BroadsideClone;
        let pack=crate::map_pack::on(map).unwrap();
        let info=crate::terrain::info(map);
        for (base,s) in [(info.ember,-1.),(info.glacier,1.)] {
            let at=|lx:f32,lz:f32| {
                let p=Vec3::new(base.x+s*lx,base.y+3.,base.z+s*lz);
                let (fy,_)=pack.floor(p).expect("floor under route point");
                Vec3::new(p.x,fy+1.2,p.z)
            };
            let routes:[&[(f32,f32)];3]=[
                &[(-9.,-14.),(-9.,-6.),(-9.,-2.)],
                &[(0.,-11.2),(0.,-6.)],
                &[(9.,-14.),(9.,-6.)]];
            for route in routes {
                for w in route.windows(2) {
                    let (a,b)=(at(w[0].0,w[0].1),at(w[1].0,w[1].1));
                    // Rise with the ramp in short steps, as movement would.
                    let steps=((b-a).length()/0.25).ceil() as usize;
                    for k in 0..steps {
                        let p0=a.lerp(b,k as f32/steps as f32);let p1=a.lerp(b,(k+1) as f32/steps as f32);
                        let lift=Vec3::Y*0.3;
                        assert!(pack.body_sweep(p0+lift,p1+lift).is_none(),"route blocked between {p0} and {p1} ({:?} -> {:?})",w[0],w[1]);
                    }
                }
            }
        }
    }

    #[test]
    fn tower_turrets_still_engage_a_player_on_their_bridge() {
        let map=MapId::BroadsideClone;
        let pack=crate::map_pack::on(map).unwrap();
        let defs=equipment::definitions(map);
        let flag=Vec3::from_array(pack.manifest.flags[0]);
        for (i,d) in defs.iter().enumerate().filter(|(_,d)|d.kind==Kind::Turret && d.team==0) {
            // Halfway along the bridge toward the tower, standing on its deck.
            let probe=d.pos()+Vec3::new(0.,3.,(flag.z-d.pos().z).signum()*9.);
            let (floor,_)=pack.floor(probe).expect("bridge deck below the probe");
            let mut w=world(map);w.players[0].pos=Vec3::new(probe.x,floor+1.2,probe.z);
            w.step_equipment(STEP);
            assert_eq!(w.equipment[i].contacts,1,"{} ignored a player on its bridge",d.id);
            assert!(fired_by(&w,i));
        }
    }

    /// The viewmodel muzzle reaches ~1.5 m ahead of the eye, past a thin
    /// wall. A shot must start on the shooter's side and stop there.
    #[test]
    fn point_blank_shots_never_spawn_past_a_wall() {
        for map in [MapId::BroadsideClone,MapId::StonehengeClone] {
            let pack=crate::map_pack::on(map).unwrap();
            let mut cases=0;
            for spot in spawns(map) {
                for step in 0..72 {
                    let yaw=step as f32*std::f32::consts::TAU/72.;
                    let fwd=look_dir(yaw,0.);
                    // Walk the real body into the wall, as movement would.
                    let Some((t,_))=pack.body_sweep(spot,spot+fwd*6.) else {continue};
                    let stand=spot+fwd*(t*6.-0.01);
                    let eye=stand+Vec3::Y*EYE;
                    let raw=muzzle_origin(eye,fwd,0,camera_fov(0.));
                    if obstacle_hit(map,&[],eye,raw,SHOT_RADIUS).is_none() {continue;}
                    let mut w=world(map);w.players[0].pos=stand;w.players[0].yaw=yaw;w.players[0].pitch=0.;w.players[0].weapon=0;
                    w.shoot(0);
                    let disc=w.discs.last().unwrap().pos;
                    assert!(obstacle_hit(map,&[],eye,disc,0.).is_none(),"{map:?}: shot spawned past a wall at {stand} yaw {yaw}: eye {eye} raw {raw} disc {disc} pos {} rawhit {:?} dischit {:?} pillars {}",w.players[0].pos,obstacle_hit(map,&w.pillars,eye,raw,SHOT_RADIUS),obstacle_hit(map,&[],eye,disc,0.),w.pillars.len());
                    cases+=1;
                }
                if cases>=6 {break;}
            }
            assert!(cases>0,"{map:?}: no wall-hugging case found; the test did not exercise the muzzle clamp");
        }
    }
}

#[cfg(test)]
mod targeting_equivalence {
    use super::*;
    use crate::equipment::{self,Kind,Definition};

    /// Verbatim copy of the inline targeting that `step_equipment` used before
    /// `equipment::acquire_target`: (index, aim target, aim, fire line clear).
    fn legacy(map:MapId, d:&Definition, sensed:bool, players:&[(u8,Vec3,Vec3)])->Option<(usize,Vec3,Vec3,bool)> {
        let range=if d.kind==Kind::Sensor {260.} else if sensed {150.} else {80.};
        let target=players.iter().enumerate().filter(|(_,p)|p.0 as usize!=d.team as usize)
            .filter_map(|(i,p)| {
                let target=p.1+Vec3::Y*0.8;let delta=target-d.pos();
                if delta.length()>range {return None;}
                let start=d.pos()+delta.normalize_or_zero()*(d.radius+0.6);
                obstacle_hit(map,&[],start,target,0.).is_none().then_some((delta.length(),i,target,p.2))
            }).min_by(|a,b|a.0.total_cmp(&b.0));
        let (_,i,target,velocity)=target?;
        let plasma=d.weapon==equipment::TurretWeapon::Plasma;
        let speed=if plasma {80.} else {BOLT_SPEED};
        let life=if plasma {3.} else {1.};
        let aim_target=if d.kind==Kind::Turret && plasma {
            equipment::intercept_time(target-d.pos(),velocity,speed,d.radius+0.6,life)
                .map_or(target,|t|target+velocity*t)
        } else {target};
        let aim=(aim_target-d.pos()).normalize_or_zero();
        let muzzle=d.pos()+aim*(d.radius+0.6);
        Some((i,aim_target,aim,obstacle_hit(map,&[],muzzle,aim_target,0.).is_none()))
    }

    fn shared(map:MapId, d:&Definition, sensed:bool, players:&[(u8,Vec3,Vec3)])->Option<(usize,Vec3,Vec3,bool)> {
        let profile=equipment::profile(d.kind,d.weapon)?;
        let clear=|a:Vec3,b:Vec3|obstacle_hit(map,&[],a,b,0.).is_none();
        let candidates=players.iter().enumerate()
            .map(|(index,p)|equipment::Candidate {index,team:p.0,pos:p.1,vel:p.2});
        let hit=equipment::acquire_target(d.pos(),d.radius,d.team,&profile,sensed,candidates,clear)?;
        Some((hit.index,hit.aim_point,hit.aim,clear(hit.muzzle,hit.aim_point)))
    }

    #[test]
    fn shared_targeting_matches_the_previous_inline_rule() {
        let mut seed=0x9e37_79b9u32;
        let mut rnd=move || {seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;(seed as f32/u32::MAX as f32)*2.-1.};
        for map in [MapId::BroadsideClone,MapId::StonehengeClone,MapId::Raindance] {
            let defs=equipment::definitions(map);
            let (mut scenarios,mut acquired,mut leading,mut sensed_only)=(0,0,0,0);
            for d in defs.iter().filter(|d|matches!(d.kind,Kind::Sensor|Kind::Turret)) {
                for _ in 0..400 {
                    let enemy=1-d.team.min(1);
                    let mut players=Vec::new();
                    for k in 0..3 {
                        let r=[40.,120.,280.][k%3]*rnd().abs();
                        let pos=d.pos()+Vec3::new(rnd()*r,rnd()*30.,rnd()*r);
                        let vel=Vec3::new(rnd()*40.,rnd()*10.,rnd()*40.);
                        players.push((if k==1 {d.team} else {enemy},pos,vel));
                    }
                    for sensed in [false,true] {
                        let old=legacy(map,d,sensed,&players);
                        let new=shared(map,d,sensed,&players);
                        assert_eq!(old,new,"{map:?} {} sensed={sensed} players={players:?}",d.id);
                        scenarios+=1;
                        if let Some(o)=old {
                            acquired+=1;
                            if o.1!=players[o.0].1+Vec3::Y*0.8 {leading+=1;}
                            if sensed && legacy(map,d,false,&players).is_none() {sensed_only+=1;}
                        }
                    }
                }
            }
            assert!(acquired>0 && sensed_only>0,"{map:?}: {scenarios} scenarios, {acquired} acquired, {sensed_only} sensed-only");
            if matches!(map,MapId::StonehengeClone|MapId::Raindance) {assert!(leading>0,"{map:?}: no plasma intercept case exercised");}
        }
    }
}

/// Collision-budget evidence, not a correctness check. Times whole sim ticks
/// (a full offline match with bots firing) and raw map-pack queries around
/// each base, so a per-base collision budget can be set from numbers. Run with
/// `cargo test --release -p peakrunner-core --lib collision_budget_timing -- --ignored --nocapture`.
#[cfg(test)]
mod collision_budget_timing {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore = "timing probe; run in release"]
    fn collision_budget_timing() {
        for map in [MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone, MapId::SnowblindClone, MapId::DesertOfDeathClone] {
            let pack = crate::map_pack::on(map).unwrap();
            let info = crate::terrain::info(map);
            let mut world = World::new();
            world.set_map(map);
            world.start_match(true);
            let dt = 1.0 / 60.0;
            for _ in 0..120 { world.tick(dt); }
            let (mut total, mut worst) = (0.0f64, 0.0f64);
            let ticks = 3600;
            for i in 0..ticks {
                world.input.move_z = 1.0; world.input.fire = i % 30 < 15; world.input.jet = i % 240 < 90;
                let t = Instant::now();
                world.tick(dt);
                let e = t.elapsed().as_secs_f64();
                total += e; worst = worst.max(e);
            }
            // Raw queries in a 120 m box round each base: short movement
            // sweeps, body sweeps, floor probes and 80 m sight rays.
            let mut seed = 0x2545F491u32;
            let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; seed as f32 / u32::MAX as f32 };
            let n = 200_000;
            let t = Instant::now();
            let mut hits = 0;
            for i in 0..n {
                let home = if i % 2 == 0 { info.ember } else { info.glacier };
                let p = home + Vec3::new((rnd() - 0.5) * 120.0, rnd() * 25.0 - 5.0, (rnd() - 0.5) * 120.0);
                let d = Vec3::new(rnd() - 0.5, (rnd() - 0.5) * 0.3, rnd() - 0.5).normalize_or_zero();
                match i % 4 {
                    0 => hits += pack.sweep(p, p + d * 1.5, 0.52).is_some() as usize,
                    1 => hits += pack.body_sweep(p, p + d * 1.5).is_some() as usize,
                    2 => hits += pack.floor(p).is_some() as usize,
                    _ => hits += pack.sweep(p, p + d * 80.0, 0.0).is_some() as usize,
                }
            }
            let per_query = t.elapsed().as_secs_f64() / n as f64;
            println!("{map:?}: {} collision tris, tick avg {:.1} us worst {:.1} us over {ticks} ticks ({} players), query avg {:.2} us ({hits} hits)",
                pack.triangle_count(), total / ticks as f64 * 1e6, worst * 1e6, world.players.len(), per_query * 1e6);
        }
    }
}


#[cfg(test)]
mod shield_and_kit_tests {
    use super::*;
    use crate::equipment::{self,Kind};
    const MAPS:[MapId;5]=[MapId::Raindance,MapId::BroadsideClone,MapId::StonehengeClone,MapId::SnowblindClone,MapId::DesertOfDeathClone];

    fn world(map:MapId)->World {
        let mut w=World::new();w.set_map(map);w.start_rift(true);w.players.truncate(1);
        w.players[0].team=Team::Glacier;w.players[0].pos=Vec3::new(1.,-500.,1.);w
    }
    fn first(map:MapId,kind:Kind)->usize {
        equipment::definitions(map).iter().position(|d|d.team==0 && d.kind==kind).unwrap()
    }

    #[test]
    fn floor_grenades_and_discs_reach_shielded_equipment_past_their_mounts() {
        for map in MAPS {
            for (i,d) in equipment::definitions(map).iter().enumerate()
                .filter(|(_,d)|d.team==0 && matches!(d.kind,Kind::Turret|Kind::Sensor)) {
                let c=d.pos();let mut tried=0;
                for k in 0..8 {
                    let a=k as f32/8.*std::f32::consts::TAU;
                    let probe=Vec3::new(c.x+a.cos()*3.,c.y+4.,c.z+a.sin()*3.);
                    let Some((fy,_))=crate::map_pack::on(map).and_then(|pk|pk.floor(probe)) else {continue};
                    // Only open floor below the object: a deck over it rightly blocks.
                    if fy>=c.y || c.y-fy>6. {continue;}
                    tried+=1;
                    let mut w=world(map);let before=w.equipment[i].shield;
                    w.explode(Vec3::new(probe.x,fy+0.1,probe.z),MAX_PLAYERS,2,Team::Glacier);
                    assert!(w.equipment[i].shield<before,"{map:?} {}: floor grenade at {a:.2} rad missed",d.id);
                }
                let _=tried;
                let mut w=world(map);let from=c+Vec3::new(0.,3.,25.);
                if obstacle_hit(map,&[],from,c,0.).is_some_and(|t|t<0.9) {continue;}
                let before=w.equipment[i].shield;
                w.discs.push(Disc{pos:from,vel:(c-from).normalize()*DISC_SPEED,team:Team::Glacier,owner:MAX_PLAYERS,life:5.,kind:0,spin:0.});
                for _ in 0..60 {w.step_discs(STEP);}
                assert!(w.equipment[i].shield<before,"{map:?} {}: an aimed disc missed",d.id);
            }
        }
    }

    #[test]
    fn walls_beyond_the_mount_still_stop_splash() {
        let mut blocked=0;
        for map in MAPS {
            for (i,d) in equipment::definitions(map).iter().enumerate().filter(|(_,d)|d.team==0 && d.kind==Kind::Turret) {
                let c=d.pos();
                for k in 0..16 {
                    let a=k as f32/16.*std::f32::consts::TAU;
                    for r in [5.,8.,11.] {
                        let probe=Vec3::new(c.x+a.cos()*r,c.y+3.,c.z+a.sin()*r);
                        let Some((fy,_))=crate::map_pack::on(map).and_then(|pk|pk.floor(probe)) else {continue};
                        let blast=Vec3::new(probe.x,fy+0.1,probe.z);
                        let delta=c-blast;let end=c-delta.normalize_or_zero()*d.radius.min(delta.length());
                        let Some(t)=obstacle_hit(map,&[],blast,end,0.) else {continue};
                        let hit=blast.lerp(end,t);
                        if Vec3::new(hit.x-c.x,0.,hit.z-c.z).length()<=d.radius+EQUIPMENT_MOUNT_MARGIN+0.3 {continue;}
                        let mut w=world(map);let (sh,hp)=(w.equipment[i].shield,w.equipment[i].health);
                        w.explode(blast,MAX_PLAYERS,2,Team::Glacier);
                        assert_eq!((w.equipment[i].shield,w.equipment[i].health),(sh,hp),"{map:?} {}: splash crossed a wall",d.id);
                        blocked+=1;
                    }
                }
            }
        }
        assert!(blocked>20,"the wall check must actually sample walls ({blocked})");
    }

    /// Every attacker fires at max rate with the best possible result: every
    /// explosive lands point blank for its full damage, every bullet hits.
    /// Returns the seconds until damage first gets through the shield to the
    /// hull, or None if the shield holds for `secs`.
    fn sustained(map:MapId,kind:Kind,weapon:u8,attackers:usize,secs:f32)->Option<f32> {
        let mut w=world(map);let i=first(map,kind);let d=equipment::definitions(map)[i].clone();
        let blast=crate::combat::player_weapon_blast(weapon).map(|b|b.max_damage);
        let reload=weapon_reload(weapon);
        let mut next:Vec<f32>=(0..attackers).map(|k|k as f32*reload/attackers as f32).collect();
        let mut t=0.;
        while t<secs {
            for n in &mut next {
                while *n<=t {
                    // Isolated from splash on neighbours such as the generator.
                    match blast {Some(dmg)=>{w.equipment[i].damage(&d,dmg,false);}
                        None=>{w.equipment[i].damage(&d,8.,true);}}
                    *n+=reload;
                }
            }
            w.step_equipment(STEP);
            assert!(w.equipment[i].powered,"the attacked object keeps its generator");
            // Broken means damage gets past the shield to the hull.
            if w.equipment[i].health<d.max_health() {return Some(t);}
            t+=STEP;
        }
        None
    }

    #[test]
    fn one_attacker_with_any_weapon_never_breaks_a_powered_shield() {
        for kind in [Kind::Turret,Kind::Sensor] {
            for weapon in [0u8,1,2] {
                assert_eq!(sustained(MapId::Raindance,kind,weapon,1,60.),None,"{kind:?} broke under solo weapon {weapon}");
            }
        }
    }

    #[test]
    fn focused_explosive_fire_breaks_shields_and_regen_keeps_running() {
        for kind in [Kind::Turret,Kind::Sensor] {
            for weapon in [0u8,2] {
                let t=sustained(MapId::Raindance,kind,weapon,2,30.);
                assert!(t.is_some_and(|t|t<20.),"{kind:?} held against two attackers with weapon {weapon}: {t:?}");
            }
            assert_eq!(sustained(MapId::Raindance,kind,1,2,60.),None,"two chainguns alone do not break it");
            assert!(sustained(MapId::Raindance,kind,1,3,60.).is_some(),"three chainguns do");
        }
    }

    #[test]
    #[ignore]
    fn print_time_to_break_table() {
        for kind in [Kind::Turret,Kind::Sensor] {
            for (weapon,name) in [(0u8,"disc"),(2,"grenade"),(1,"chaingun")] {
                let row:Vec<String>=(1..=3).map(|n|sustained(MapId::Raindance,kind,weapon,n,120.)
                    .map_or("never".into(),|t|format!("{t:.1}s"))).collect();
                println!("{kind:?} {name}: 1p {} · 2p {} · 3p {}",row[0],row[1],row[2]);
            }
        }
    }

    #[test]
    fn destroying_a_generator_explodes_once_harmlessly_and_replays_nothing() {
        let mut w=world(MapId::Raindance);let g=first(MapId::Raindance,Kind::Generator);
        let d=equipment::definitions(w.map)[g].clone();
        w.players[0].team=Team::Ember;w.players[0].pos=d.pos()+Vec3::X*(d.radius+1.);w.players[0].health=100.;
        w.equipment[g].health=1.;let serial=w.blast_serial;
        w.equipment[g].damage(&d,0.5,true);
        assert!(w.explosions.iter().all(|e|e.kind!=4),"no blast before the hull is gone");
        let hits=|w:&World|w.explosions.iter().filter(|e|e.kind==4).count();
        // Destroy it through the normal bullet path.
        w.discs.push(Disc{pos:d.pos()+Vec3::Z*6.,vel:-Vec3::Z*BOLT_SPEED,team:Team::Glacier,owner:MAX_PLAYERS,life:1.,kind:1,spin:0.});
        for _ in 0..4 {w.step_discs(STEP);}
        assert!(w.equipment[g].health<=0. && w.equipment[g].offline);
        assert_eq!((hits(&w),w.blast_serial),(1,serial+1),"one generator blast");
        assert_eq!(w.players[0].health,100.,"the blast hurts no one");
        // A wreck cannot be destroyed again.
        w.explode(d.pos()+Vec3::X*(d.radius+0.1),MAX_PLAYERS,0,Team::Glacier);
        assert_eq!(w.explosions.iter().filter(|e|e.kind==4).count(),1);
        // Snapshot replay leaves the serial and the blast list as they were.
        let mut m=Match::new(MapId::Raindance);m.world.equipment=w.equipment.clone();
        m.world.explosions=w.explosions.clone();m.world.blast_serial=w.blast_serial;
        let snap=m.snapshot();let mut client=World::new();client.set_map(MapId::Raindance);
        for _ in 0..2 {client.apply_snapshot(&snap,0);}
        assert_eq!((client.blast_serial,client.explosions.iter().filter(|e|e.kind==4).count()),(w.blast_serial,1));
    }

    #[test]
    fn a_wrecked_generator_needs_half_hull_before_power_returns() {
        let mut w=world(MapId::Raindance);let defs=equipment::definitions(w.map);
        let g=first(MapId::Raindance,Kind::Generator);let t=first(MapId::Raindance,Kind::Turret);
        w.equipment[g].damage(&defs[g],10000.,false);w.step_equipment(STEP);
        assert!(!w.equipment[t].powered && w.equipment[t].shield==0.);
        w.equipment[g].repair(&defs[g],defs[g].max_health()*0.4);w.step_equipment(STEP);
        assert!(!w.equipment[t].powered,"40% is not enough");
        w.equipment[g].repair(&defs[g],defs[g].max_health()*0.1);w.step_equipment(STEP);
        assert!(w.equipment[t].powered && w.equipment[t].shield>0.,"50% brings the circuit and shields back");
    }

    #[test]
    fn repair_kits_heal_once_per_life_and_refill_at_inventory() {
        let mut w=world(MapId::Raindance);
        w.players[0].team=Team::Ember;w.players[0].pos=Vec3::new(1000.,300.,1000.);
        assert_eq!(w.players[0].kits,KITS_PER_LIFE);
        w.players[0].health=100.;w.input.kit=true;w.step_kits(STEP);
        assert_eq!(w.players[0].kits,1,"a full-armor player keeps the kit");
        w.players[0].health=30.;w.step_kits(STEP);
        assert_eq!(w.players[0].kits,0);assert!(w.players[0].kit_heal>0.);
        let mut t=STEP;
        while t<1.0 {w.step_kits(STEP);t+=STEP;}
        w.players[0].health-=10.;// taking damage does not cancel the heal
        while t<KIT_SECONDS+0.2 {w.step_kits(STEP);t+=STEP;}
        assert!((w.players[0].health-80.).abs()<0.6,"60 armor over two seconds, minus the hit: {}",w.players[0].health);
        assert_eq!(w.players[0].kit_heal,0.);
        let before=w.players[0].health;w.step_kits(STEP);
        assert_eq!(w.players[0].health,before,"no kit, no heal");
        // Refill at a powered inventory station.
        let defs=equipment::definitions(w.map);let s=first(MapId::Raindance,Kind::Inventory);
        let mut p=w.players[0].clone();p.team=Team::Ember;p.health=100.;p.pos=defs[s].pos();
        equipment::service(&defs[s],&mut w.equipment[s],&mut p,true,STEP);
        assert_eq!(p.kits,KITS_PER_LIFE);
        // Death cancels an active heal; respawn restores the kit.
        w.players[0].kits=1;w.players[0].health=50.;w.step_kits(STEP);
        w.kill(0,None,"Fall");assert_eq!(w.players[0].kit_heal,0.);
        w.players[0].kits=0;w.respawn(0);assert_eq!(w.players[0].kits,KITS_PER_LIFE);
    }

    #[test]
    fn prediction_never_spends_kits_and_network_intent_is_validated() {
        let mut m=Match::new(MapId::Raindance);let Some(a)=m.join(3,"medic") else {panic!()};
        m.world.players[a].health=40.;
        let mut commands=vec![None;m.world.players.len()];
        for seq in 1..=((KIT_SECONDS/STEP) as u64+6) {
            commands[a]=Some(Command{seq,kit:true,..Default::default()});m.step(&commands);
        }
        let p=&m.world.players[a];
        assert!(p.kits==0 && p.health>95.,"the server spends one kit and heals: {} hp, {} kits",p.health,p.kits);
        // A client predicting the same intent never touches its kit count or armor.
        let mut client=World::new();client.set_map(MapId::Raindance);
        assert!(client.apply_snapshot(&m.snapshot(),3));
        let me=client.player_id;client.players[me].kits=1;client.players[me].health=40.;
        client.predict_command(Command{seq:999,kit:true,..Default::default()});
        assert_eq!((client.players[me].kits,client.players[me].health),(1,40.));
    }
}

#[cfg(test)]
mod bot_line_of_sight {
    use super::*;

    const MAPS: [MapId; 5] = [MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone,
        MapId::SnowblindClone, MapId::DesertOfDeathClone];

    fn visible_enemy(w: &World, i: usize) -> bool {
        let p = &w.players[i];
        let eye = p.pos + Vec3::Y * EYE;
        w.players.iter().any(|o| o.alive && o.team != p.team && o.pos.distance(p.pos) < 95.0
            && obstacle_hit(w.map, &w.pillars, eye, o.pos + Vec3::Y * crate::equipment::CHEST_HEIGHT, 0.).is_none())
    }

    /// Bot-only match on `map`: (bot shots, shots fired with no enemy in sight, kills).
    fn soak(map: MapId, seconds: f32) -> (u32, u32, u32) {
        let mut w = World::new();
        w.set_map(map);
        w.start_match(true);
        w.players[0].is_bot = true;
        let (mut shots, mut blind) = (0, 0);
        for _ in 0..(seconds / STEP) as u32 {
            let before: Vec<u32> = w.players.iter().map(|p| p.shots).collect();
            let seen: Vec<bool> = (0..w.players.len()).map(|i| visible_enemy(&w, i)).collect();
            w.tick(STEP);
            for i in 0..w.players.len() {
                let fired = w.players[i].shots.saturating_sub(before[i]);
                shots += fired;
                if !seen[i] { blind += fired; }
            }
        }
        (shots, blind, w.players.iter().map(|p| p.frags).sum())
    }

    /// A match with just one Ember bot at `a` and one Glacier player at `b`.
    fn duel(map: MapId, a: Vec3, b: Vec3, b_is_bot: bool) -> World {
        let mut w = World::new();
        w.set_map(map);
        w.start_match(true);
        let face = (-(b - a).x).atan2(-(b - a).z);
        w.players = vec![make_player(Team::Ember, true, a, face, BotRole::Hunter),
            make_player(Team::Glacier, b_is_bot, b, face + std::f32::consts::PI, BotRole::Hunter)];
        w.player_id = 1;
        w
    }

    fn stand(map: MapId, x: f32, z: f32) -> Vec3 {
        Vec3::new(x, crate::terrain::height_on(map, x, z) + 1.2, z)
    }

    fn clear(map: MapId, from: Vec3, to: Vec3) -> bool {
        obstacle_hit(map, &crate::terrain::pillars_on(map), from + Vec3::Y * EYE, to + Vec3::Y * crate::equipment::CHEST_HEIGHT, 0.).is_none()
    }

    /// A spawn point inside a room, plus a standable spot outside within 60 m
    /// that the room's walls hide from it.
    fn hidden_pair(map: MapId) -> (Vec3, Vec3) {
        let pack = crate::map_pack::on(map).expect("embedded map");
        for (spawn, _) in crate::terrain::spawn_points_on(map, true) {
            for r in [25.0f32, 35.0, 45.0, 55.0] {
                for k in 0..24 {
                    let t = k as f32 / 24.0 * std::f32::consts::TAU;
                    let (x, z) = (spawn.x + r * t.cos(), spawn.z + r * t.sin());
                    let Some((fy, _)) = pack.floor(Vec3::new(x, spawn.y + 30.0, z)) else { continue };
                    let p = Vec3::new(x, fy + 1.2, z);
                    if pack.body_sweep(p, p + Vec3::Y * 0.01).is_none() && !clear(map, spawn, p) && !clear(map, p, spawn) {
                        return (spawn, p);
                    }
                }
            }
        }
        panic!("{map:?}: no hidden position found near a spawn");
    }

    #[test]
    fn bots_engage_an_enemy_in_the_open() {
        let map = MapId::Raindance;
        let (a, b) = (600..1500).step_by(40).flat_map(|x| (600..1500).step_by(40).map(move |z| (x as f32, z as f32)))
            .map(|(x, z)| (stand(map, x, z), stand(map, x, z + 40.0)))
            .find(|(a, b)| clear(map, *a, *b) && clear(map, *b, *a))
            .expect("two open spots 40 m apart");
        let mut w = duel(map, a, b, false);
        for _ in 0..(3.0 / STEP) as u32 { w.players[1].pos = b; w.players[1].vel = Vec3::ZERO; w.tick(STEP); }
        assert_eq!(w.players[0].bot_target, Some(1));
        assert!(w.players[0].shots > 0, "a bot with a clear line of sight fires");
    }

    #[test]
    fn bots_do_not_track_or_fire_through_walls() {
        for map in [MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone, MapId::DesertOfDeathClone] {
            let (a, b) = hidden_pair(map);
            let mut w = duel(map, a, b, false);
            for _ in 0..(3.0 / STEP) as u32 {
                w.players[0].pos = a; w.players[0].vel = Vec3::ZERO;
                w.players[1].pos = b; w.players[1].vel = Vec3::ZERO;
                w.tick(STEP);
                assert_eq!(w.players[0].bot_target, None, "{map:?}: bot targeted an enemy behind a wall");
            }
            assert_eq!(w.players[0].shots, 0, "{map:?}: bot fired at an enemy behind a wall");
        }
    }

    #[test]
    fn losing_sight_drops_the_target_and_heads_for_the_last_seen_spot() {
        let map = MapId::DesertOfDeathClone;
        let (hidden_from, hidden) = hidden_pair(map);
        // Find a spot that does see `hidden_from`, then move the enemy behind the wall.
        let pack = crate::map_pack::on(map).unwrap();
        let open = (0..48).find_map(|k| {
            let t = k as f32 / 48.0 * std::f32::consts::TAU;
            let (x, z) = (hidden_from.x + 20.0 * t.cos(), hidden_from.z + 20.0 * t.sin());
            let (fy, _) = pack.floor(Vec3::new(x, hidden_from.y + 10.0, z))?;
            let p = Vec3::new(x, fy + 1.2, z);
            (pack.body_sweep(p, p + Vec3::Y * 0.01).is_none() && clear(map, hidden_from, p)).then_some(p)
        }).expect("a visible spot near the spawn");
        let mut w = duel(map, hidden_from, open, false);
        for _ in 0..30 { w.players[0].pos = hidden_from; w.players[1].pos = open; w.tick(STEP); }
        assert_eq!(w.players[0].bot_target, Some(1));
        let shots = w.players[0].shots;
        w.players[1].pos = hidden;
        for _ in 0..30 { w.players[0].pos = hidden_from; w.players[0].vel = Vec3::ZERO; w.players[1].pos = hidden; w.tick(STEP); }
        assert_eq!(w.players[0].bot_target, None, "target is dropped once sight breaks");
        assert!(w.players[0].bot_memory > 0.0, "the last-seen spot is remembered briefly");
        assert!(w.players[0].bot_seen.distance(open) < 0.5);
        assert_eq!(w.players[0].shots, shots, "no shots at the wall after sight breaks");
    }

    #[test]
    #[ignore = "bot soak; run with --release -- --ignored --nocapture"]
    fn bot_soak_report() {
        for map in MAPS {
            for secs in [60., 180.] { let (shots, blind, kills) = soak(map, secs);
            println!("{map:?} {secs}s: shots {shots}, fired with no enemy in sight {blind}, kills {kills}"); }
        }
    }
}

#[cfg(test)]
mod capture_and_hold_tests {
    use super::*;
    use crate::control::{Definition, Drain};
    use crate::map_catalog::SupportedMode;

    fn defs(map: MapId) -> Vec<Definition> {
        let at = |x: f32, z: f32| [x, crate::terrain::height_on(map, x, z), z];
        vec![Definition { id: "west".into(), name: "West".into(), pos: at(900.0, 1000.0), radius: 12.0, ctf_active: false, drain: None },
             Definition { id: "beacon".into(), name: "Beacon".into(), pos: at(1024.0, 1024.0), radius: 12.0, ctf_active: true,
                drain: Some(Drain { radius: 60.0, rate: crate::control::DEFAULT_DRAIN_RATE }) }]
    }

    fn playing(mode: SupportedMode) -> (Match, usize, usize) {
        let mut m = Match::new(MapId::Raindance);
        m.world.set_mode(mode);
        m.world.set_control_points(defs(MapId::Raindance));
        let a = m.join(1, "Alpha").unwrap();
        let b = m.join(2, "Bravo").unwrap();
        for _ in 0..240 { m.step(&[]); if m.phase == Phase::Playing { break; } }
        assert_eq!(m.phase, Phase::Playing);
        (m, a, b)
    }

    fn hold(m: &mut Match, slot: usize, at: Vec3, seconds: f32) {
        for _ in 0..(seconds / STEP) as usize {
            let p = &mut m.world.players[slot];
            p.pos = at + Vec3::Y * 1.2; p.vel = Vec3::ZERO; p.alive = true;
            m.step(&[]);
        }
    }

    #[test]
    fn capture_and_hold_captures_scores_and_wins() {
        let (mut m, a, b) = playing(SupportedMode::CaptureAndHold);
        m.world.players[b].pos = Vec3::new(100.0, 500.0, 100.0);
        assert!(m.world.points.iter().all(|p| p.active), "all points run in Capture & Hold");
        let west = m.world.points[0].pos;
        hold(&mut m, a, west, 9.8);
        assert_eq!(m.world.points[0].owner, None);
        hold(&mut m, a, west, 0.4);
        let team = m.world.players[a].team.idx() as u8;
        assert_eq!(m.world.points[0].owner, Some(team));
        let before = m.world.score[team as usize];
        hold(&mut m, a, west, 5.0);
        let gained = m.world.score[team as usize] - before;
        assert!((4..=6).contains(&gained), "one held point scores 1/s, got {gained}");
        m.world.score[team as usize] = crate::control::CNH_TARGET - 1;
        hold(&mut m, a, west, 1.1);
        assert_eq!(m.world.state, MatchState::Ended);
        assert_eq!(m.world.score[team as usize], crate::control::CNH_TARGET);
        assert!(m.world.flags.iter().all(|f| f.carrier.is_none()), "flags stay inactive");
    }

    #[test]
    fn ctf_only_runs_points_marked_ctf_active_and_warmup_never_captures() {
        let (mut m, a, _) = playing(SupportedMode::Ctf);
        assert_eq!(m.world.points.iter().map(|p| p.active).collect::<Vec<_>>(), vec![false, true]);
        let (west, beacon) = (m.world.points[0].pos, m.world.points[1].pos);
        hold(&mut m, a, west, 11.0);
        assert_eq!(m.world.points[0].owner, None, "inactive in CTF");
        hold(&mut m, a, beacon, 10.2);
        assert!(m.world.points[1].owner.is_some());
        assert_eq!(m.world.score, [0, 0], "CTF points never score");
        let mut w = Match::new(MapId::Raindance);
        w.world.set_control_points(defs(MapId::Raindance));
        let solo = w.join(1, "Solo").unwrap();
        let beacon = w.world.points[1].pos;
        hold(&mut w, solo, beacon, 11.0);
        assert_eq!(w.phase, Phase::Waiting);
        assert_eq!(w.world.points[1].owner, None, "warmup never captures");
    }

    /// Seconds of continuous jetting from a full tank, and energy regained in
    /// the next four seconds on the ground, with or without an enemy drain field.
    fn jet_budget(drained: bool) -> (f32, f32) {
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        w.start_match(true);
        w.players.truncate(1);
        w.set_control_points(defs(MapId::Raindance));
        let beacon = w.points[1].pos;
        w.points[1].owner = Some(if drained { 1 } else { 0 });
        let high = beacon + Vec3::new(20.0, 300.0, 0.0);
        w.players[0].pos = high; w.players[0].vel = Vec3::ZERO; w.players[0].energy = ENERGY_MAX;
        w.input.jet = true;
        let mut airtime = 0.0;
        for _ in 0..600 {
            w.players[0].pos.y = high.y; w.players[0].vel.y = 0.0;
            w.step_players(STEP);
            if !w.players[0].jetting { break; }
            airtime += STEP;
        }
        w.input.jet = false;
        let start = w.players[0].energy;
        for _ in 0..240 { w.players[0].pos.y = high.y; w.players[0].vel.y = 0.0; w.step_players(STEP); }
        (airtime, w.players[0].energy - start)
    }

    #[test]
    fn drain_field_allows_hops_but_not_sustained_flight() {
        let (free, free_regen) = jet_budget(false);
        let (drained, drained_regen) = jet_budget(true);
        assert!((free - 3.8).abs() < 0.1, "normal tank: {free} s");
        assert!((drained - 2.3).abs() < 0.1, "drained tank: {drained} s");
        assert!((free_regen - 48.0).abs() < 1.0 && (drained_regen - 8.0).abs() < 1.0,
            "regen over 4 s: {free_regen} normal, {drained_regen} drained");
    }

    #[test]
    fn rotation_keeps_mode_and_restart_resets_points() {
        let (mut m, a, _) = playing(SupportedMode::CaptureAndHold);
        let west = m.world.points[0].pos;
        hold(&mut m, a, west, 10.2);
        assert!(m.world.points[0].owner.is_some());
        m.rotate_to(MapId::Raindance);
        assert_eq!(m.world.mode, SupportedMode::CaptureAndHold);
        m.rotate_to_mode(MapId::Raindance, SupportedMode::Ctf);
        assert_eq!(m.world.mode, SupportedMode::Ctf);
        // Back to Old Holler's own three points, none of them CTF-active.
        assert_eq!(m.world.points.len(), 3);
        assert!(m.world.points.iter().all(|p| !p.active && p.owner.is_none()));
    }

    /// Old Holler (key `raindance`) places three Capture & Hold towers: the
    /// Crossing platform beside the bridge at mid-span and two knolls that
    /// mirror each other through the map centre. None runs in CTF.
    #[test]
    fn old_holler_declares_three_capture_and_hold_points() {
        let pack = crate::map_pack::on(MapId::Raindance).expect("embedded");
        let points = &pack.manifest.control_points;
        assert_eq!(points.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["crossing", "west-knoll", "east-knoll"]);
        assert!(points.iter().all(|p| !p.ctf_active && p.drain.is_none() && p.radius == 12.0));
        let (w, e) = (Vec3::from_array(points[1].pos), Vec3::from_array(points[2].pos));
        assert!((w.x + e.x - 1960.0).abs() < 0.01 && (w.z + e.z - 1880.0).abs() < 0.01, "knolls mirror through (980, 940)");
        // Every point's ring is standable floor under its centre.
        for p in points {
            let c = Vec3::from_array(p.pos);
            let floor = crate::terrain::support_on(MapId::Raindance, c + Vec3::new(8.0, 1.2, 0.0)).0;
            assert!((floor - c.y).abs() < 2.1, "{} ring floor {floor} vs {}", p.id, c.y);
        }
        let (mut m, a, _) = {
            let mut m = Match::new(MapId::Raindance);
            m.world.set_mode(SupportedMode::CaptureAndHold);
            let a = m.join(1, "Alpha").unwrap(); let b = m.join(2, "Bravo").unwrap();
            for _ in 0..240 { m.step(&[]); if m.phase == Phase::Playing { break; } }
            (m, a, b)
        };
        assert_eq!(m.world.points.len(), 3);
        assert!(m.world.points.iter().all(|p| p.active), "all three run in Capture & Hold");
        let crossing = m.world.points[0].pos;
        hold(&mut m, a, crossing, 10.2);
        assert!(m.world.points[0].owner.is_some(), "the Crossing platform captures");
    }

    #[test]
    fn tower_complex_holds_three_mirrored_capture_towers() {
        let map = MapId::BroadsideClone;
        let pack = crate::map_pack::on(map).expect("embedded");
        let points = &pack.manifest.control_points;
        assert_eq!(points.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Summit", "Westfall", "Eastfall"]);
        for p in points {
            assert!(!p.ctf_active && p.drain.is_none(), "{} is Capture & Hold only", p.name);
            assert_eq!(p.radius, 12.0);
            let ground = crate::terrain::height_on(map, p.pos[0], p.pos[2]);
            assert!((ground - p.pos[1]).abs() < 0.2, "{} ring centre {:?} off the ground ({ground})", p.name, p.pos);
        }
        let (a, b) = (points[1].pos, points[2].pos);
        assert_eq!((a[0] + b[0], a[2] + b[2]), (2.0 * points[0].pos[0], 2.0 * points[0].pos[2]));
        let mut m = Match::new(map);
        m.world.set_mode(SupportedMode::CaptureAndHold);   // points come from the manifest
        let a = m.join(1, "Alpha").unwrap();
        let b = m.join(2, "Bravo").unwrap();
        for _ in 0..240 { m.step(&[]); if m.phase == Phase::Playing { break; } }
        assert_eq!(m.phase, Phase::Playing);
        assert_eq!(m.world.points.len(), 3);
        assert!(m.world.points.iter().all(|p| p.active));
        m.world.players[b].pos = Vec3::new(100.0, 500.0, 100.0);
        let summit = m.world.points[0].pos;
        hold(&mut m, a, summit, 10.2);
        assert!(m.world.points[0].owner.is_some(), "the Summit ring captures on its plateau");
        m.world.set_mode(SupportedMode::Ctf);
        assert!(m.world.points.iter().all(|p| !p.active), "no CTF-active point on Tower Complex");
    }

    /// Frostline's centre is the beacon: a Capture & Hold point that also runs
    /// in CTF, with a drain field. The West and East Cols are Capture & Hold
    /// only and mirror each other through the map centre.
    #[test]
    fn frostline_beacon_runs_in_ctf_and_drains_only_the_holders_enemies() {
        let map = MapId::SnowblindClone;
        let pack = crate::map_pack::on(map).expect("embedded");
        let points = &pack.manifest.control_points;
        assert_eq!(points.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Beacon", "West Col", "East Col"]);
        assert!(points[0].ctf_active && points[0].drain.is_some());
        assert!(points[1..].iter().all(|p| !p.ctf_active && p.drain.is_none()));
        let (w, e) = (points[1].pos, points[2].pos);
        assert_eq!((w[0] + e[0], w[2] + e[2]), (2048.0, 2048.0));
        for p in points {
            let floor = crate::terrain::support_on(map, Vec3::from_array(p.pos) + Vec3::new(7.0, 1.2, 7.0)).0;
            assert!((floor - p.pos[1]).abs() < 2.1, "{} ring floor {floor} vs {:?}", p.name, p.pos);
        }
        let mut m = Match::new(map);
        let a = m.join(1, "Alpha").unwrap();
        let b = m.join(2, "Bravo").unwrap();
        for _ in 0..240 { m.step(&[]); if m.phase == Phase::Playing { break; } }
        assert_eq!(m.phase, Phase::Playing);
        assert_eq!(m.world.mode, SupportedMode::Ctf);
        assert!(m.world.points[0].active && m.world.points[1..].iter().all(|p| !p.active), "only the beacon runs in CTF");
        m.world.players[b].pos = Vec3::new(100.0, 500.0, 100.0);
        let beacon = m.world.points[0].pos;
        hold(&mut m, a, beacon, 9.0);
        assert!(m.world.points[0].owner.is_none(), "not held after 9 s");
        hold(&mut m, a, beacon, 1.2);
        let owner = m.world.points[0].owner.expect("the beacon captures in about 10 s");
        assert_eq!(owner, m.world.players[a].team.idx() as u8);
        // Drain: the holder's enemies inside 60 m lose energy, the holders do not.
        let near = beacon + Vec3::new(40.0, 30.0, 0.0);
        let enemy = 1 - owner;
        assert_eq!(crate::control::drain_at(&m.world.points, enemy, near), crate::control::DEFAULT_DRAIN_RATE);
        assert_eq!(crate::control::drain_at(&m.world.points, owner, near), 0.0);
        assert_eq!(crate::control::drain_at(&m.world.points, enemy, beacon + Vec3::new(70.0, 0.0, 0.0)), 0.0);
        m.world.set_mode(SupportedMode::CaptureAndHold);
        assert!(m.world.points.iter().all(|p| p.active), "all three run in Capture & Hold");
    }

    /// Dustreach: the Sun Gate is the centre point (its arch spans the flag
    /// lane, so the ring has marker posts rather than a pylon), and the West
    /// and East Wadi towers mirror each other through the map centre. None
    /// runs in CTF.
    #[test]
    fn dustreach_holds_the_sun_gate_and_two_mirrored_wadi_towers() {
        let map = MapId::DesertOfDeathClone;
        let pack = crate::map_pack::on(map).expect("embedded");
        let points = &pack.manifest.control_points;
        assert_eq!(points.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Sun Gate", "West Wadi", "East Wadi"]);
        assert!(points.iter().all(|p| !p.ctf_active && p.drain.is_none() && p.radius == 12.0));
        assert_eq!((points[0].pos[0], points[0].pos[2]), (1024.0, 1024.0));
        let (w, e) = (points[1].pos, points[2].pos);
        assert_eq!((w[0] + e[0], w[2] + e[2]), (2048.0, 2048.0));
        for p in points {
            let floor = crate::terrain::support_on(map, Vec3::from_array(p.pos) + Vec3::new(0.0, 1.2, 8.0)).0;
            assert!((floor - p.pos[1]).abs() < 2.1, "{} ring floor {floor} vs {:?}", p.name, p.pos);
        }
        let mut m = Match::new(map);
        m.world.set_mode(SupportedMode::CaptureAndHold);
        let a = m.join(1, "Alpha").unwrap();
        let b = m.join(2, "Bravo").unwrap();
        for _ in 0..240 { m.step(&[]); if m.phase == Phase::Playing { break; } }
        assert_eq!(m.phase, Phase::Playing);
        assert!(m.world.points.iter().all(|p| p.active));
        m.world.players[b].pos = Vec3::new(100.0, 500.0, 100.0);
        let gate = m.world.points[0].pos;
        hold(&mut m, a, gate, 10.2);
        assert!(m.world.points[0].owner.is_some(), "the Sun Gate ring captures under the arch");
        m.world.set_mode(SupportedMode::Ctf);
        assert!(m.world.points.iter().all(|p| !p.active), "no CTF-active point on Dustreach");
    }

    #[test]
    /// Cairnhold: the Ring's dais is the centre point (the shared tower stands
    /// where the old brazier stone was, and the 12 m capture ring is the
    /// dais), and West and East Cairn mirror each other through the map
    /// centre on levelled plateaus. None runs in CTF.
    fn cairnhold_holds_the_ring_and_two_mirrored_cairns() {
        let map = MapId::StonehengeClone;
        let pack = crate::map_pack::on(map).expect("embedded");
        let points = &pack.manifest.control_points;
        assert_eq!(points.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["The Ring", "West Cairn", "East Cairn"]);
        assert!(points.iter().all(|p| !p.ctf_active && p.drain.is_none() && p.radius == 12.0));
        assert_eq!((points[0].pos[0], points[0].pos[2]), (1024.0, 1024.0));
        let (w, e) = (points[1].pos, points[2].pos);
        assert_eq!((w[0] + e[0], w[2] + e[2]), (2048.0, 2048.0));
        for p in points {
            // Standing ground 8 m out from the pylon, inside the ring.
            let floor = crate::terrain::support_on(map, Vec3::from_array(p.pos) + Vec3::new(0.0, 1.2, 8.0)).0;
            assert!((floor - p.pos[1]).abs() < 1.0, "{} ring floor {floor} vs {:?}", p.name, p.pos);
        }
        let mut m = Match::new(map);
        m.world.set_mode(SupportedMode::CaptureAndHold);
        let a = m.join(1, "Alpha").unwrap();
        let b = m.join(2, "Bravo").unwrap();
        for _ in 0..240 { m.step(&[]); if m.phase == Phase::Playing { break; } }
        assert_eq!(m.phase, Phase::Playing);
        assert!(m.world.points.iter().all(|p| p.active));
        m.world.players[b].pos = Vec3::new(100.0, 500.0, 100.0);
        let ring = m.world.points[0].pos;
        hold(&mut m, a, ring, 10.2);
        assert!(m.world.points[0].owner.is_some(), "the Ring's dais captures");
        let west = m.world.points[1].pos;
        hold(&mut m, a, west, 10.2);
        assert!(m.world.points[1].owner.is_some(), "West Cairn captures on its plateau");
        m.world.set_mode(SupportedMode::Ctf);
        assert!(m.world.points.iter().all(|p| !p.active), "no CTF-active point on Cairnhold");
    }
}

#[cfg(test)]
mod bot_soak_metrics {
    use super::*;

    pub(crate) const MAPS: [MapId; 5] = [MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone,
        MapId::SnowblindClone, MapId::DesertOfDeathClone];

    /// Bot-only match: (seconds until each team first touches the enemy flag,
    /// sight encounters per minute, kills per minute, [mean, worst] tick cost
    /// in microseconds).
    pub(crate) fn soak(map: MapId, seconds: f32) -> ([Option<f32>; 2], f32, f32, [f32; 2]) {
        crate::bot_nav::graph(map);
        let mut w = World::new();
        w.set_map(map);
        w.start_match(true);
        w.players[0].is_bot = true;
        // Offline fills five bots; add two so the cost covers seven bots
        // beside the player's slot (which a bot also drives here).
        for team in [Team::Ember, Team::Glacier] {
            let (pos, yaw) = w.players.iter().find(|p| p.team == team).map(|p| (p.pos, p.yaw)).unwrap();
            w.players.push(make_player(team, true, pos + Vec3::new(2.0, 0.0, 0.0), yaw, BotRole::Offense));
            let i = w.players.len() - 1;
            w.assign_personality(i);
        }
        let mut reach = [None; 2];
        let mut encounters = 0u32;
        let mut had: Vec<bool> = vec![false; w.players.len()];
        let steps = (seconds / STEP) as u32;
        let (mut total, mut worst) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
        for s in 0..steps {
            let clock = std::time::Instant::now();
            w.tick(STEP);
            let spent = clock.elapsed();
            total += spent;
            worst = worst.max(spent);
            let t = s as f32 * STEP;
            for (i, p) in w.players.iter().enumerate() {
                let now = p.bot_target.is_some();
                if now && !had[i] { encounters += 1; }
                had[i] = now;
                let team = p.team.idx();
                if reach[team].is_none() && p.alive
                    && (p.carrying.is_some() || p.pos.distance(w.flags[1 - team].home) < 6.0) {
                    reach[team] = Some(t);
                }
            }
        }
        let kills: u32 = w.players.iter().map(|p| p.frags).sum();
        let min = seconds / 60.0;
        let cost = [total.as_secs_f32() * 1e6 / steps as f32, worst.as_secs_f32() * 1e6];
        (reach, encounters as f32 / min, kills as f32 / min, cost)
    }

    /// A lone bot from each team's spawn, on every map: seconds until it
    /// reaches (touches or takes) the enemy flag, or `None`.
    pub(crate) fn lone_run(map: MapId, ember: bool, limit: f32) -> Option<f32> {
        crate::bot_nav::graph(map);
        let mut w = World::new();
        w.set_map(map);
        w.start_match(ember);
        let team = w.players[0].team;
        let mut bot = make_player(team, true, w.players[0].pos, w.players[0].yaw, BotRole::Offense);
        bot.bot_profile = crate::bot_nav::ARCHETYPES[1];
        w.players = vec![bot];
        let target = w.flags[team.other().idx()].home;
        for s in 0..(limit / STEP) as u32 {
            // Navigation, not survival: base turrets would otherwise end the run.
            w.players[0].health = 100.0;
            w.tick(STEP);
            let p = &w.players[0];
            if p.carrying.is_some() || (p.alive && p.pos.distance(target) < 6.0) { return Some(s as f32 * STEP); }
        }
        None
    }

    /// A lone flyer (the Hawk archetype): (seconds to the enemy flag, share
    /// of the run spent airborne), or `None` if it never got there.
    pub(crate) fn flyer_run(map: MapId, ember: bool, limit: f32) -> Option<(f32, f32)> {
        crate::bot_nav::graph(map);
        let mut w = World::new();
        w.set_map(map);
        w.start_match(ember);
        let team = w.players[0].team;
        let mut bot = make_player(team, true, w.players[0].pos, w.players[0].yaw, BotRole::Offense);
        bot.bot_profile = *crate::bot_nav::ARCHETYPES.iter().find(|a| a.style == crate::bot_nav::MoveStyle::Flyer).unwrap();
        w.players = vec![bot];
        let target = w.flags[team.other().idx()].home;
        let mut air = 0u32;
        for s in 0..(limit / STEP) as u32 {
            w.players[0].health = 100.0;
            w.tick(STEP);
            let p = &w.players[0];
            if !p.on_ground { air += 1; }
            if p.carrying.is_some() || (p.alive && p.pos.distance(target) < 6.0) {
                return Some((s as f32 * STEP, air as f32 / (s + 1) as f32));
            }
        }
        None
    }

    #[test]
    fn flyers_cross_every_map_mostly_airborne() {
        let mut missed = Vec::new();
        for map in MAPS {
            for ember in [true, false] {
                let r = flyer_run(map, ember, 240.0);
                println!("{map:?} {} flyer: {r:?}", if ember { "ember" } else { "glacier" });
                match r {
                    Some((_, air)) if air >= 0.4 => {}
                    _ => missed.push((map, ember, r)),
                }
            }
        }
        assert!(missed.is_empty(), "flyers that never arrived or stayed grounded: {missed:?}");
    }

    #[test]
    fn a_lone_bot_reaches_the_enemy_flag_on_every_map() {
        let mut missed = Vec::new();
        for map in MAPS {
            for ember in [true, false] {
                let t = lone_run(map, ember, 240.0);
                println!("{map:?} {}: {t:?}", if ember { "ember" } else { "glacier" });
                if t.is_none() { missed.push((map, ember)); }
            }
        }
        assert!(missed.is_empty(), "never reached the enemy flag: {missed:?}");
    }

    #[test]
    #[ignore = "bot navigation soak; run with --release -- --ignored --nocapture"]
    fn bot_navigation_soak_report() {
        for map in MAPS {
            let (reach, enc, kills, cost) = soak(map, 180.0);
            println!("{map:?}: flag reach ember {:?} glacier {:?}, encounters/min {enc:.1}, kills/min {kills:.2}, tick {:.0} us mean {:.0} us worst ({} players)",
                reach[0].map(|t| t.round()), reach[1].map(|t| t.round()), cost[0], cost[1], 8);
            // (seven bots plus the player's slot, also bot-driven)
        }
    }
}

#[cfg(test)]
mod water_tests {
    use super::*;
    use crate::water::Volume;

    /// A flat-ish spot on the Valley fixture, with a lone local player on it.
    fn solo_on_ground() -> (World, Vec3) {
        let mut world = World::new();
        world.set_map(MapId::Raindance);
        world.start_match(true);
        world.players.truncate(1);
        let map = world.map;
        assert_eq!(map, MapId::Raindance);
        let mut spot = Vec3::new(512.0, 0.0, 512.0);
        let ground = |x: f32, z: f32, from: f32| crate::terrain::support_on(map, Vec3::new(x, from, z)).0;
        'find: for x in (300..1700).step_by(20) {
            for z in (300..1700).step_by(20) {
                let (x, z) = (x as f32, z as f32);
                let h0 = ground(x, z, 2000.0);
                let flat = (-6..=6).step_by(2).all(|a| (-6..=6).step_by(2).all(|b| {
                    let (sx, sz) = (x + a as f32 * 4.0, z + b as f32 * 4.0);
                    (ground(sx, sz, h0 + 3.0) - h0).abs() < 0.8 && (ground(sx, sz, 2000.0) - h0).abs() < 0.8
                }));
                if flat { spot = Vec3::new(x, h0, z); break 'find; }
            }
        }
        assert!(spot.y != 0.0, "found a flat spot");
        world.player_id = 0;
        world.network_inputs.clear();
        let p = &mut world.players[0];
        p.is_bot = false;
        p.remote = false;
        p.alive = true;
        p.health = 100.0;
        p.energy = ENERGY_MAX;
        p.pos = spot + Vec3::Y * PLAYER_RADIUS;
        p.vel = Vec3::ZERO;
        p.yaw = std::f32::consts::FRAC_PI_2;
        p.on_ground = true;
        (world, spot)
    }

    fn pool(surface: f32, at: Vec3) -> Volume {
        Volume {surface, rect: Some([at.x - 300.0, at.z - 300.0, at.x + 300.0, at.z + 300.0]), ..Default::default()}
    }

    /// Horizontal speed after `secs`, entering at `speed` along +x.
    fn coast(depth: Option<f32>, speed: f32, ski: bool, secs: f32) -> f32 {
        let (mut world, spot) = solo_on_ground();
        if let Some(d) = depth { world.staged_water.push(pool(spot.y + d, spot)); }
        world.players[0].vel = Vec3::new(speed, 0.0, 0.0);
        world.input.jump = ski;
        world.input.jump_prev = ski;
        for _ in 0..(secs / STEP).round() as usize { world.step_players(STEP); }
        let v = world.players[0].vel;
        Vec2::new(v.x, v.z).length()
    }

    #[test]
    fn water_drag_grows_with_depth_and_brakes_skiers() {
        // Ankle, waist and swim depth in metres above the bed.
        let depths = [("dry", None), ("ankle", Some(0.3)), ("waist", Some(1.1)), ("swim", Some(3.0))];
        let mut ski_after_1 = Vec::new();
        for (name, d) in depths {
            let (s1, s3) = (coast(d, 30.0, true, 1.0), coast(d, 30.0, true, 3.0));
            let (w1, w3) = (coast(d, 8.0, false, 1.0), coast(d, 8.0, false, 3.0));
            println!("{name}: ski 30 m/s -> {s1:.1} after 1 s, {s3:.1} after 3 s; coast 8 m/s -> {w1:.2}, {w3:.2}");
            ski_after_1.push(s1);
        }
        assert!(ski_after_1[1] < ski_after_1[0] * 0.8, "ankle water brakes a skier: {ski_after_1:?}");
        assert!(ski_after_1[2] < ski_after_1[1] && ski_after_1[3] < ski_after_1[2], "{ski_after_1:?}");
        assert!(ski_after_1[3] < 8.0, "swimming kills ski speed: {ski_after_1:?}");
    }

    #[test]
    fn wading_slows_walking() {
        let walk = |depth: Option<f32>| {
            let (mut world, spot) = solo_on_ground();
            if let Some(d) = depth { world.staged_water.push(pool(spot.y + d, spot)); }
            world.input.move_z = 1.0;
            for _ in 0..120 { world.step_players(STEP); }
            let v = world.players[0].vel;
            Vec2::new(v.x, v.z).length()
        };
        let (dry, waist) = (walk(None), walk(Some(1.1)));
        println!("walk top speed dry {dry:.2}, waist-deep {waist:.2}");
        assert!((dry - WALK_MAX).abs() < 0.8 && waist < dry * 0.8 && waist > 3.0, "{dry} {waist}");
    }

    #[test]
    fn a_swimmer_floats_and_can_jet_out() {
        let (mut world, spot) = solo_on_ground();
        let surface = spot.y + 6.0;
        world.staged_water.push(pool(surface, spot));
        for _ in 0..600 { world.step_players(STEP); }
        let (wet, _) = crate::water::immersion(world.map, &world.staged_water, world.players[0].pos);
        assert!(wet > 0.5 && wet < 1.0, "floats partly submerged, not on the bed: {wet}");
        let eye = world.players[0].pos.y + crate::terrain::EYE;
        assert!((eye - surface).abs() < 1.0, "eye near the surface: {eye} vs {surface}");
        world.players[0].energy = ENERGY_MAX;
        world.input.jet = true;
        let mut t = 0.0;
        while crate::water::immersion(world.map, &world.staged_water, world.players[0].pos).0 > 0.0 {
            world.step_players(STEP);
            t += STEP;
            assert!(t < 3.0, "could not jet out");
        }
        println!("jet out of deep water in {t:.2} s, energy left {:.1}", world.players[0].energy);
    }

    #[test]
    fn dry_movement_is_unchanged_by_distant_water() {
        let run = |water: bool| {
            let (mut world, spot) = solo_on_ground();
            if water { world.staged_water.push(pool(spot.y + 5.0, spot + Vec3::new(1400.0, 0.0, 1400.0))); }
            let mut trace = Vec::new();
            for tick in 0..900 {
                world.input.move_z = if tick % 200 < 150 { 1.0 } else { 0.0 };
                world.input.jump = tick % 300 > 150;
                world.input.jet = tick % 240 > 200;
                world.step_players(STEP);
                let p = &world.players[0];
                trace.push((p.pos, p.vel, p.energy.to_bits()));
            }
            trace
        };
        assert_eq!(run(false), run(true));
    }

    #[test]
    fn prediction_matches_the_server_in_water() {
        let run = |predicting: bool| {
            let (mut world, spot) = solo_on_ground();
            world.staged_water.push(Volume {flow: [2.0, -1.0], ..pool(spot.y + 1.4, spot)});
            world.predicting = predicting;
            world.players[0].vel = Vec3::new(25.0, 0.0, 3.0);
            world.input.jump = true;
            for tick in 0..300 {
                world.input.jet = tick > 200;
                world.step_players(STEP);
            }
            let p = &world.players[0];
            (p.pos, p.vel, p.energy.to_bits())
        };
        assert_eq!(run(true), run(false));
    }

    #[test]
    fn projectiles_slow_or_fizzle_under_water() {
        let (mut world, spot) = solo_on_ground();
        world.staged_water.push(pool(spot.y + 40.0, spot));
        let start = spot + Vec3::new(0.0, 20.0, 0.0);
        world.discs.push(Disc {pos: start, vel: Vec3::new(0.0, 0.0, -95.0), team: Team::Ember, owner: 0, life: 3.0, kind: 0, spin: 0.0});
        world.discs.push(Disc {pos: start, vel: Vec3::new(0.0, 0.0, -420.0), team: Team::Ember, owner: 0, life: 1.0, kind: 1, spin: 0.0});
        world.step_discs(STEP);
        assert_eq!(world.discs.len(), 1, "the bullet fizzles");
        for _ in 0..30 { world.step_discs(STEP); }
        let speed = world.discs.first().map_or(0.0, |d| d.vel.length());
        assert!(speed < 95.0 * 0.15, "the disc is dragged down: {speed}");
    }

    /// Every map's declared water: a skier coming off the bank at 20 m/s is
    /// braked hard, floats instead of sinking, and can jet back out.
    #[test]
    fn every_maps_water_slows_skiers_who_can_jet_out() {
        let maps = [MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone,
                    MapId::SnowblindClone, MapId::DesertOfDeathClone];
        for map in maps {
            let volumes = crate::water::volumes(map);
            assert!(!volumes.is_empty(), "{map:?} declares water");
            for (i, v) in volumes.iter().enumerate() {
                // Centre and the direction a skier comes in from.
                let ground = |q: Vec2| crate::terrain::support_on(map, Vec3::new(q.x, 2000.0, q.y)).0;
                let (centre, dir) = match (&v.rect, &v.polygon) {
                    // A long channel: come in across it above its deepest point.
                    (Some(r), _) => {
                        let z = (r[1] + r[3]) / 2.0;
                        let x = (0..100).map(|k| 200.0 + k as f32 * 16.0)
                            .min_by(|a, b| ground(Vec2::new(*a, z)).total_cmp(&ground(Vec2::new(*b, z)))).unwrap();
                        (Vec2::new(x, z), Vec2::new(0.0, 1.0))
                    }
                    (_, Some(p)) => {
                        let n = p.len() as f32;
                        (Vec2::new(p.iter().map(|q| q[0]).sum::<f32>() / n, p.iter().map(|q| q[1]).sum::<f32>() / n),
                         Vec2::new(1.0, 0.0))
                    }
                    _ => unreachable!(),
                };
                let mut shore = centre;
                while ground(shore) < v.surface + 0.1 { shore += dir; }
                let start = shore + dir * 6.0;
                let mut world = World::new();
                world.set_map(map);
                world.start_match(true);
                world.players.truncate(1);
                world.player_id = 0;
                world.network_inputs.clear();
                let p = &mut world.players[0];
                p.is_bot = false; p.remote = false; p.alive = true; p.health = 100.0; p.energy = ENERGY_MAX;
                p.pos = Vec3::new(start.x, ground(start) + PLAYER_RADIUS, start.y);
                p.vel = Vec3::new(-dir.x * 20.0, 0.0, -dir.y * 20.0);
                p.on_ground = true;
                world.input.jump = true;
                world.input.jump_prev = true;
                let mut wettest: f32 = 0.0;
                for _ in 0..(2.5 / STEP) as usize {
                    world.step_players(STEP);
                    wettest = wettest.max(crate::water::immersion(map, &[], world.players[0].pos).0);
                }
                let v2 = world.players[0].vel;
                let speed = Vec2::new(v2.x, v2.z).length();
                assert!(wettest > 0.4, "{map:?} volume {i}: the skier reached deep water ({wettest:.2})");
                assert!(speed < 8.0, "{map:?} volume {i}: water braked the skier to {speed:.1} m/s");
                for _ in 0..(2.0 / STEP) as usize { world.step_players(STEP); }
                let (wet, _) = crate::water::immersion(map, &[], world.players[0].pos);
                assert!(wet < 1.0, "{map:?} volume {i}: floats, not sunk ({wet:.2})");
                world.input.jump = false;
                world.players[0].energy = ENERGY_MAX;
                world.input.jet = true;
                let mut t = 0.0;
                while crate::water::immersion(map, &[], world.players[0].pos).0 > 0.0 {
                    world.step_players(STEP);
                    t += STEP;
                    assert!(t < 3.0, "{map:?} volume {i}: could not jet out");
                }
                println!("{map:?} volume {i}: deepest immersion {wettest:.2}, speed {speed:.1} m/s after 2.5 s, jet out {t:.2} s");
            }
        }
    }
}
