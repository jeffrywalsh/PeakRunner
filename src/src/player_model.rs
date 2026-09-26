//! Articulated third-person armor. Rigid parts per bone, posed procedurally
//! from state every client already has in its snapshot (velocity, ground,
//! ski, jet, aim, weapon cooldown, carried flag, death timer). Render-only:
//! the authoritative capsule, hitbox and movement never read any of this.
//! Local frame: origin at the feet, -Z forward, +Y up.

use crate::drawlist::{EmitDraw, LitDraw, MeshId};
use crate::sim::{weapon_reload, Player, Team};
use glam::{Mat4, Quat, Vec2, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Respawn delay the sim starts the death timer at; the collapse pose plays
/// over the first part of it.
const RESPAWN_DELAY: f32 = 3.4;
/// Bodies stay visible this long after death, then vanish until respawn.
const CORPSE_TIME: f32 = 2.6;
/// Stride cadence in radians per second for untracked poses. Tracked players
/// accumulate their stride phase at `cadence(speed)`, so faster runners take
/// quicker strides without the phase ever jumping.
const CADENCE: f32 = 9.0;

/// Stride rate for a ground speed: a jog at walking pace, near two full
/// strides a second at a sprint or on a ski run.
pub fn cadence(speed: f32) -> f32 { (5.0 + speed * 0.65).clamp(6.0, 13.5) }

/// Everything the pose depends on, lifted out of `Player` so the maths is a
/// pure function of its inputs.
#[derive(Clone, Copy, Debug)]
pub struct PoseInput {
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub skiing: bool,
    pub jetting: bool,
    pub weapon: u8,
    /// Seconds since the last shot (large when idle).
    pub shot_age: f32,
    pub time: f32,
    pub seed: f32,
    /// Seconds since death, if dead.
    pub dead_for: Option<f32>,
    /// Landing squash, 0 to 1 (see `Motion::squash`).
    pub squash: f32,
    /// Weapon lowered for a switch, 0 to 1 (see `Motion::lower`).
    pub lower: f32,
    /// Stride phase in radians; advances with ground speed (see `Motion::stride`).
    pub stride: f32,
    /// What the hands are doing.
    pub hold: Hold,
    /// Fall direction and tilt for a downed body (see `Motion::fall_dir`).
    pub fall_dir: Vec2,
    pub fall_tilt: f32,
}

/// What a player's hands hold, which decides how the arms are driven: gripping
/// a weapon or the ball (the hands reach to fixed grips), or free (the arm
/// chain is posed from joint angles, as later actions such as pointing or
/// dancing will be).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold { Weapon, Ball, Free }

/// Upper arm and forearm lengths (m).
const UPPER_ARM: f32 = 0.32;
const FOREARM: f32 = 0.30;

/// Forward kinematics for one arm: shoulder, elbow and wrist. `swing` raises
/// the arm forward about the shoulder (0 hangs it down, +pi/2 points it ahead),
/// `spread` lifts it out to the side, `bend` folds the elbow forward, and
/// `wrist` pitches the hand. Returns (elbow, wrist position, hand frame), all in
/// the frame `chest` maps into. `side` is -1 for the left arm, +1 for the right.
pub fn arm_chain(chest: Mat4, shoulder: Vec3, side: f32, swing: f32, spread: f32, bend: f32, wrist: f32) -> (Vec3, Vec3, Mat4) {
    let upper = chest * rot_x(swing) * Mat4::from_rotation_z(side * spread);
    let elbow = shoulder + upper.transform_vector3(Vec3::NEG_Y) * UPPER_ARM;
    let fore = upper * rot_x(bend);
    let hand = elbow + fore.transform_vector3(Vec3::NEG_Y) * FOREARM;
    let frame = Mat4::from_translation(hand) * Mat4::from_quat(Quat::from_mat4(&(fore * rot_x(wrist))).normalize());
    (elbow, hand, frame)
}

/// A hand frame at `hand`, its -Y running along the forearm from `elbow`.
fn wrist_frame(elbow: Vec3, hand: Vec3) -> Mat4 {
    Mat4::from_translation(hand) * Mat4::from_quat(Quat::from_rotation_arc(Vec3::NEG_Y, (hand - elbow).normalize_or(Vec3::NEG_Y)))
}

impl PoseInput {
    pub fn from_player(p: &Player, time: f32) -> Self {
        let reload = if p.weapon == crate::sim::rifles::RIFLE_SLOT { crate::sim::rifles::rifle_reload(p) } else { weapon_reload(p.weapon) };
        let shot_age = if p.cooldown > 0.0 { (reload - p.cooldown).max(0.0) } else { 9.0 };
        // A tackled player falls into the collapse pose and stays down.
        let down = peakrunner_core::sim::football::TACKLE_STUN - p.stun;
        let dead_for = if !p.alive { Some((RESPAWN_DELAY - p.respawn).max(0.0)) }
            else if p.stun > 0.0 { Some(down.clamp(0.0, 0.8)) } else { None };
        PoseInput { vel: p.vel, yaw: p.yaw, pitch: p.pitch, on_ground: p.on_ground,
            skiing: p.skiing, jetting: p.jetting, weapon: p.weapon, shot_age, time,
            seed: p.net_id as f32 * 0.7, dead_for, squash: 0.0, lower: 0.0, stride: time * CADENCE, hold: Hold::Weapon,
            fall_dir: Vec2::new(0.0, -1.0), fall_tilt: FALL_TILT }
    }
}

/// Short-lived animation cues derived from how a player's snapshot state
/// changes frame to frame: a hard landing and a weapon switch. Nothing here
/// is sent over the network; every client derives it the same way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Seconds since the last hard landing, and how hard it was (0 to 1).
    pub land_age: f32,
    pub land_strength: f32,
    /// Seconds since the last weapon switch, and the weapon switched from.
    pub switch_age: f32,
    pub switch_from: u8,
    /// Stride phase, advanced only while on the ground (0 = untracked).
    pub stride: f32,
    /// How a downed body falls: the direction in the player's frame (x, z;
    /// -z is forward) and how far it may tip (radians), chosen when the
    /// player went down so the body never lies through a wall or pillar.
    pub fall_dir: Vec2,
    pub fall_tilt: f32,
}

/// A full fall: flat on the ground.
const FALL_TILT: f32 = 1.42;
/// Feet to crown of a fallen body, and the room kept clear past it (m).
const BODY_LENGTH: f32 = 1.9;

impl Default for Motion {
    fn default() -> Self { Motion { land_age: 9.0, land_strength: 0.0, switch_age: 9.0, switch_from: 0, stride: 0.0,
        fall_dir: Vec2::new(0.0, -1.0), fall_tilt: FALL_TILT } }
}

/// Where a body going down should fall: the way it was knocked (its
/// velocity), or the other way if that side has more room, tipping only as
/// far as the room allows. `clear(from, to)` is how far a ray gets before
/// the map blocks it.
fn fall_for(p: &Player, clear: &dyn Fn(Vec3, Vec3) -> f32) -> (Vec2, f32) {
    let flat = Vec3::new(p.vel.x, 0.0, p.vel.z);
    let knock = if flat.length() > 1.0 { flat.normalize() } else { Mat4::from_rotation_y(p.yaw).transform_vector3(Vec3::NEG_Z) };
    let body = p.pos + Vec3::Y * 0.8;
    let reach = BODY_LENGTH + 0.5;
    let room = |d: Vec3| clear(body, body + d * reach);
    let (ahead, behind) = (room(knock), room(-knock));
    let (dir, space) = if behind > ahead + 0.3 { (-knock, behind) } else { (knock, ahead) };
    let tilt = if space >= reach - 0.01 { FALL_TILT } else { ((space - 0.35) / BODY_LENGTH).clamp(0.05, 1.0).asin().min(FALL_TILT) };
    let local = Mat4::from_rotation_y(-p.yaw).transform_vector3(dir);
    (Vec2::new(local.x, local.z).normalize_or(Vec2::new(0.0, -1.0)), tilt)
}

/// A landing's squash and recovery, and a weapon switch's lower and raise.
const SQUASH_TIME: f32 = 0.38;
const SWITCH_TIME: f32 = 0.32;
/// Falls slower than this don't squash; airborne blips shorter than
/// `MIN_AIR` (snapshot jitter over bumps) aren't landings.
const SQUASH_FALL: f32 = 8.0;
const MIN_AIR: f32 = 0.15;

impl Motion {
    /// Squash amount: a quick dip over the first 0.08 s, then a smooth recovery.
    pub fn squash(&self) -> f32 {
        let t = self.land_age;
        if t >= SQUASH_TIME { return 0.0; }
        let dip = (t / 0.08).min(1.0);
        let rise = ((t - 0.08).max(0.0) / (SQUASH_TIME - 0.08)).min(1.0);
        let ease = |k: f32| k * k * (3.0 - 2.0 * k);
        self.land_strength * ease(dip) * (1.0 - ease(rise))
    }
    /// Weapon lowered: down and back up over `SWITCH_TIME`.
    pub fn lower(&self) -> f32 {
        if self.switch_age >= SWITCH_TIME { 0.0 } else { (self.switch_age / SWITCH_TIME * PI).sin() }
    }
    /// The weapon shown: the old one while lowering, the new one on the way up.
    pub fn shown_weapon(&self, current: u8) -> u8 {
        if self.switch_age < SWITCH_TIME * 0.5 { self.switch_from } else { current }
    }
}

struct Track { id: u32, alive: bool, down: bool, grounded: bool, air: f32, fall: f32, weapon: u8, motion: Motion }

/// Per-player animation memory for the client, keyed by `net_id`.
#[derive(Default)]
pub struct AnimTracker { tracks: Vec<Track> }

impl AnimTracker {
    /// Observe this frame's players and age the cues by `dt`. `clear(from,
    /// to)` measures free space in the map, for where fallen bodies lie.
    pub fn update(&mut self, players: &[Player], dt: f32, clear: &dyn Fn(Vec3, Vec3) -> f32) {
        self.tracks.retain(|t| players.iter().any(|p| p.net_id == t.id));
        for p in players {
            let Some(t) = self.tracks.iter_mut().find(|t| t.id == p.net_id) else {
                // First sight: nothing to animate yet.
                self.tracks.push(Track { id: p.net_id, alive: p.alive, down: !p.alive || p.stun > 0.0,
                    grounded: p.on_ground, air: 0.0, fall: 0.0, weapon: p.weapon, motion: Motion::default() });
                continue;
            };
            t.motion.land_age += dt;
            t.motion.switch_age += dt;
            if p.on_ground {
                let speed = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
                t.motion.stride = (t.motion.stride + dt * cadence(speed)) % (TAU * 64.0) + 1e-3;
            }
            let down = !p.alive || p.stun > 0.0;
            if p.alive != t.alive {
                // Death or respawn: reset without a cue (a death picks where
                // the body falls).
                *t = Track { id: p.net_id, alive: p.alive, down, grounded: p.on_ground, air: 0.0, fall: 0.0,
                    weapon: p.weapon, motion: Motion::default() };
                if down { (t.motion.fall_dir, t.motion.fall_tilt) = fall_for(p, clear); }
                continue;
            }
            if down && !t.down {
                // Tackled: fall the way the hit sent you, where there's room.
                (t.motion.fall_dir, t.motion.fall_tilt) = fall_for(p, clear);
            }
            t.down = down;
            if !p.on_ground {
                t.air += dt;
                t.fall = t.fall.max(-p.vel.y);
            } else if !t.grounded {
                if t.air >= MIN_AIR && t.fall > SQUASH_FALL {
                    t.motion.land_age = 0.0;
                    t.motion.land_strength = ((t.fall - SQUASH_FALL) / 22.0).clamp(0.25, 1.0);
                }
                t.air = 0.0;
                t.fall = 0.0;
            }
            t.grounded = p.on_ground;
            if p.alive && p.weapon != t.weapon {
                t.motion.switch_from = t.weapon;
                t.motion.switch_age = 0.0;
                t.weapon = p.weapon;
            }
        }
    }

    pub fn motion(&self, id: u32) -> Motion {
        self.tracks.iter().find(|t| t.id == id).map_or_else(Motion::default, |t| t.motion)
    }
}

/// Joint positions and frames in the local (feet) frame.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    /// Whole-body transform on top of the root: death tip-over, ski lean.
    pub body: Mat4,
    pub pelvis: Vec3,
    pub chest: Mat4,
    pub head: Mat4,
    /// Weapon frame: at the right grip, -Z along the aim, recoil applied.
    pub aim: Mat4,
    pub hips: [Vec3; 2],
    pub knees: [Vec3; 2],
    pub ankles: [Vec3; 2],
    /// Foot frames at the ankles: pitched by the ankle joint (toes down is
    /// positive), -Z along the foot.
    pub feet: [Mat4; 2],
    pub shoulders: [Vec3; 2],
    pub elbows: [Vec3; 2],
    pub hands: [Vec3; 2],
    /// Hand frames at the wrists (-Y along the forearm, -Z the palm's facing).
    pub wrists: [Mat4; 2],
}

fn rot_x(a: f32) -> Mat4 { Mat4::from_rotation_x(a) }

/// Pure, deterministic pose from the inputs.
pub fn pose(i: &PoseInput) -> Pose {
    let local_vel = Mat4::from_rotation_y(-i.yaw).transform_vector3(i.vel);
    let speed = Vec3::new(i.vel.x, 0.0, i.vel.z).length();
    // On the ground, running or skiing, the legs run (as Tribes players did
    // while sliding); in the air, jetting or not, they dangle.
    let airborne = !i.on_ground;
    let amp = if airborne { 0.0 } else if i.skiing { 0.85 } else { (speed / 8.0).min(1.0) };
    let phase = i.stride + i.seed;
    let breathe = (i.time * 1.7 + i.seed).sin();
    // Forward speed in the player's frame (local -Z is forward).
    let ahead = -local_vel.z;

    // Body posture: a runner leans with speed; in the air the torso tips
    // gently into the direction of travel.
    let squash = if i.dead_for.is_some() { 0.0 } else { i.squash };
    let (crouch, lean, roll) = if airborne {
        (0.0, (ahead / 60.0).clamp(-0.08, 0.2) + if i.jetting { 0.05 } else { 0.0 }, 0.0)
    } else {
        let turn = if i.skiing { (-local_vel.x / 30.0).clamp(-1.0, 1.0) * 0.18 } else { 0.0 };
        (0.015 * breathe * 0.5 + amp * 0.04 * (phase * 2.0).sin().abs(), amp * 0.14 + if i.skiing { 0.06 } else { 0.0 }, turn)
    };

    // Death: tip over backwards around the feet and settle.
    let body = match i.dead_for {
        Some(t) => {
            let k = (t / 0.55).min(1.0);
            let fall = k * k * (3.0 - 2.0 * k);
            // Tip toward the chosen side, only as far as there's room.
            let toward = Vec3::new(i.fall_dir.x, 0.0, i.fall_dir.y).normalize_or(Vec3::NEG_Z);
            let axis = Vec3::Y.cross(toward).normalize_or(Vec3::NEG_X);
            Mat4::from_translation(Vec3::new(0.0, 0.18 * fall * i.fall_tilt / FALL_TILT, 0.0))
                * Mat4::from_axis_angle(axis, i.fall_tilt * fall) * Mat4::from_rotation_z(0.25 * fall)
        }
        None => Mat4::from_rotation_z(roll),
    };

    let crouch = crouch + 0.2 * squash;
    let lean = lean + 0.14 * squash;
    let pelvis = Vec3::new(0.0, 0.95 - crouch, 0.0);
    let chest = Mat4::from_translation(pelvis) * rot_x(-lean)
        * Mat4::from_scale(Vec3::new(1.0, 1.0 + 0.008 * breathe, 1.0));
    let neck = chest.transform_point3(Vec3::new(0.0, 0.58, 0.0));
    let look = i.pitch.clamp(-1.1, 1.1);
    let head = Mat4::from_translation(neck) * rot_x(lean * 0.6 + look * 0.55);

    // Legs: three joints, hip and knee about X plus the ankle. Positive hip
    // swings the leg forward (-Z); knee bend folds the shin back; positive
    // foot angle points the toes down.
    let mut hips = [Vec3::ZERO; 2];
    let mut knees = [Vec3::ZERO; 2];
    let mut ankles = [Vec3::ZERO; 2];
    let mut feet = [Mat4::IDENTITY; 2];
    for (s, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let swing = (phase + if side < 0.0 { 0.0 } else { PI }).sin();
        let (hip_a, knee_a, splay, toes) = if i.dead_for.is_some() {
            (0.25 + side * 0.1, 0.35, 0.10, 0.5)
        } else if airborne {
            // Dangling: loose, slightly bent legs that trail with speed and
            // sway a little out of step; toes hang down.
            let sway = (i.time * 1.4 + i.seed + side * 1.3).sin();
            let trail = (ahead / 40.0).clamp(-0.25, 0.45);
            (0.12 - trail + sway * 0.07, 0.38 + side * 0.08 + sway * 0.05, 0.05, 0.75 + sway * 0.06)
        } else {
            // Running: the knee lifts on the forward swing; the foot rolls
            // off the toes behind and lands heel first in front.
            let lift = (phase + if side < 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 }).sin().max(0.0);
            let toes = amp * (0.55 * (-swing).max(0.0) - 0.25 * swing.max(0.0) + 0.35 * lift);
            (swing * 0.6 * amp, 0.12 + lift * 1.1 * amp, 0.03, toes)
        };
        // Squash: thighs forward, knees folded, so the feet stay under the hips.
        let (hip_a, knee_a) = (hip_a + 0.42 * squash, knee_a + 0.84 * squash);
        let hip = pelvis + Vec3::new(side * 0.16, -0.05, 0.0);
        let thigh_dir = (rot_x(hip_a) * Mat4::from_rotation_z(side * splay)).transform_vector3(Vec3::NEG_Y);
        let knee = hip + thigh_dir * 0.44;
        let shin_dir = rot_x(hip_a - knee_a).transform_vector3(Vec3::NEG_Y);
        let ankle = knee + shin_dir * 0.44;
        hips[s] = hip; knees[s] = knee; ankles[s] = ankle;
        feet[s] = Mat4::from_translation(ankle) * rot_x(-toes);
    }

    // Aim frame: shoulders follow the chest, the weapon follows yaw/pitch.
    let shoulders = [
        chest.transform_point3(Vec3::new(-0.30, 0.47, 0.0)),
        chest.transform_point3(Vec3::new(0.30, 0.47, 0.0)),
    ];
    let (back, rise, _) = crate::drawlist::recoil(i.weapon, i.shot_age, i.time);
    let run_bob = amp * 0.03 * (phase * 2.0).sin();
    // A weapon switch dips the gun down and in, then brings the new one up.
    let lower = if i.dead_for.is_some() { 0.0 } else { i.lower };
    let aim = Mat4::from_translation(chest.transform_point3(Vec3::new(0.14, 0.30 + run_bob - 0.12 * lower, 0.08 * lower)))
        * rot_x(look * 0.9 + rise * 2.0 - 1.0 * lower)
        * Mat4::from_translation(Vec3::new(0.0, 0.0, -0.36 + back * 2.5));
    // Arms: three joints each. Gripping hands reach to their grips with
    // the elbows bowing down and out; free arms are posed from joint angles.
    let grips = match i.hold {
        // Right on the grip, left on the fore-grip.
        Hold::Weapon => Some([aim.transform_point3(Vec3::new(-0.06, -0.02, -0.34)),
            aim.transform_point3(Vec3::new(0.0, -0.09, 0.06))]),
        // Both hands round the ball, held out in front of the chest.
        Hold::Ball => Some([chest.transform_point3(Vec3::new(-0.13, 0.28, -0.42)),
            chest.transform_point3(Vec3::new(0.13, 0.28, -0.42))]),
        Hold::Free => None,
    };
    let mut hands = [Vec3::ZERO; 2];
    let mut elbows = [Vec3::ZERO; 2];
    let mut wrists = [Mat4::IDENTITY; 2];
    for s in 0..2 {
        let side = if s == 0 { -1.0 } else { 1.0 };
        match grips {
            Some(grip) => {
                let mid = (shoulders[s] + grip[s]) * 0.5;
                let reach = shoulders[s].distance(grip[s]);
                let sag = (UPPER_ARM + FOREARM - reach).max(0.0) * 0.8 + 0.06;
                elbows[s] = mid + Vec3::new(side * sag * 0.7, -sag, 0.05);
                hands[s] = grip[s];
                wrists[s] = wrist_frame(elbows[s], hands[s]);
            }
            None => {
                // Running: arms swing opposite the legs, elbows bent. In the
                // air: loose, a little out from the body, swaying. Dead: limp.
                let leg = (phase + if side < 0.0 { PI } else { 0.0 }).sin();
                let (swing, spread, bend) = if i.dead_for.is_some() { (0.2, 0.5, 0.3) }
                    else if airborne {
                        let sway = (i.time * 1.2 + i.seed + side).sin();
                        (0.15 + sway * 0.08, 0.28 + sway * 0.05, 0.35)
                    } else { (leg * 0.7 * amp, 0.12, 0.35 + 0.9 * amp) };
                let (elbow, hand, frame) = arm_chain(chest, shoulders[s], side, swing, spread, bend, 0.1);
                elbows[s] = elbow; hands[s] = hand; wrists[s] = frame;
            }
        }
    }
    Pose { body, pelvis, chest, head, aim, hips, knees, ankles, feet, shoulders, elbows, hands, wrists }
}

fn team_color(team: Team) -> Vec3 {
    if team == Team::Ember { Vec3::new(0.74, 0.20, 0.12) } else { Vec3::new(0.12, 0.51, 0.65) }
}

/// Carried-flag cloth colour: the colour of the flag being carried.
fn flag_color(flag: Team) -> Vec3 {
    if flag == Team::Ember { Vec3::new(0.89, 0.29, 0.20) } else { Vec3::new(0.24, 0.78, 0.88) }
}

/// Parts for one player. `detailed` is the near LOD; far players keep the
/// animated silhouette with fewer parts.
/// `hold`: a weapon, the football, or free hands (Football without the ball).
pub fn push_player(lit: &mut Vec<LitDraw>, emit: &mut Vec<EmitDraw>, p: &Player, time: f32, detailed: bool, motion: Motion, hold: Hold) {
    let base = PoseInput::from_player(p, time);
    let stride = if motion.stride > 0.0 { motion.stride } else { base.stride };
    let input = PoseInput { squash: motion.squash(), lower: motion.lower(), stride, hold,
        fall_dir: motion.fall_dir, fall_tilt: motion.fall_tilt, ..base };
    let armed = hold == Hold::Weapon;
    let shown = motion.shown_weapon(p.weapon);
    if matches!(input.dead_for, Some(t) if t > CORPSE_TIME) { return; }
    let pose = pose(&input);
    // Heavy armor is bigger all round: broader, deeper, a little taller.
    let bulk = if p.armor == peakrunner_core::sim::ArmorClass::Heavy && hold != Hold::Free && hold != Hold::Ball {
        Mat4::from_scale(Vec3::new(1.22, 1.1, 1.25)) } else { Mat4::IDENTITY };
    let root = Mat4::from_translation(p.pos) * Mat4::from_rotation_y(p.yaw) * bulk * pose.body;
    let team = team_color(p.team);
    let alloy = Vec3::new(0.48, 0.55, 0.61);
    let suit = Vec3::new(0.065, 0.085, 0.11);
    let trim = Vec3::new(0.18, 0.23, 0.29);
    let light = Vec3::new(0.32, 0.88, 1.0);
    let dead = input.dead_for.is_some();
    let visor = if dead { 0.0 } else { 0.7 };

    let mut part = |m: Mat4, mesh: MeshId, color: Vec3, glow: f32| {
        lit.push(LitDraw { mesh, model: root * m, color, emit: glow, mode: 0.0 });
    };
    let at = |pos: Vec3, size: Vec3| Mat4::from_translation(pos) * Mat4::from_scale(size);
    let seg = |a: Vec3, b: Vec3, w: f32, d: f32| {
        let axis = b - a;
        Mat4::from_translation((a + b) * 0.5)
            * Mat4::from_quat(Quat::from_rotation_arc(Vec3::Y, axis.normalize_or(Vec3::Y)))
            * Mat4::from_scale(Vec3::new(w, axis.length() + 0.05, d))
    };

    // Classic light-armor silhouette: a broad team-coloured chest over a
    // narrow waist, big raised shoulder pads, an angular helmet with a
    // forward faceplate and crest, and a big twin-tube jetpack. The pads and
    // pack are at every LOD: they're what makes a player readable at range.
    let c = pose.chest;
    let side_of = |s: usize| if s == 0 { -1.0f32 } else { 1.0 };
    part(c * at(Vec3::new(0.0, 0.03, 0.0), Vec3::new(0.32, 0.22, 0.24)), MeshId::Armor, suit, 0.0);
    part(c * at(Vec3::new(0.0, 0.36, 0.0), Vec3::new(0.58, 0.40, 0.34)), MeshId::Armor, team, 0.0);
    for s in 0..2 {
        let pad = c * Mat4::from_translation(Vec3::new(side_of(s) * 0.39, 0.57, 0.0)) * Mat4::from_rotation_z(side_of(s) * 0.32);
        part(pad * Mat4::from_scale(Vec3::new(0.30, 0.22, 0.40)), MeshId::Armor, team, 0.0);
    }
    let h = pose.head;
    part(h * at(Vec3::new(0.0, 0.13, 0.01), Vec3::new(0.30, 0.32, 0.34)), MeshId::Armor, alloy, 0.0);
    part(h * Mat4::from_translation(Vec3::new(0.0, 0.07, -0.18)) * rot_x(0.35) * Mat4::from_scale(Vec3::new(0.24, 0.15, 0.10)),
        MeshId::Bevel, suit, 0.0);
    part(h * at(Vec3::new(0.0, 0.16, -0.185), Vec3::new(0.23, 0.045, 0.02)), MeshId::Cube, light, visor);
    part(h * at(Vec3::new(0.0, 0.31, 0.03), Vec3::new(0.05, 0.10, 0.30)), MeshId::Cube, team, 0.0);

    // Pelvis block joining the thighs to the torso, then the legs and boots.
    part(at(pose.pelvis + Vec3::new(0.0, -0.04, 0.0), Vec3::new(0.36, 0.18, 0.25)), MeshId::Armor, suit, 0.0);
    for s in 0..2 {
        part(seg(pose.hips[s], pose.knees[s], 0.21, 0.22), MeshId::Armor, team, 0.0);
        part(seg(pose.knees[s], pose.ankles[s], 0.17, 0.19), MeshId::Armor, alloy, 0.0);
        part(pose.feet[s] * at(Vec3::new(0.0, -0.07, -0.07), Vec3::new(0.22, 0.16, 0.38)), MeshId::Armor, suit, 0.0);
    }
    // Arms: shoulder, elbow and wrist chains; single segments at the far LOD.
    for s in 0..2 {
        if detailed {
            part(seg(pose.shoulders[s], pose.elbows[s], 0.16, 0.17), MeshId::Armor, trim, 0.0);
            part(seg(pose.elbows[s], pose.hands[s], 0.14, 0.15), MeshId::Armor, alloy, 0.0);
        } else {
            part(seg(pose.shoulders[s], pose.hands[s], 0.16, 0.17), MeshId::Armor, trim, 0.0);
        }
    }
    // Jetpack: a tall box with two exhaust tubes below it.
    let pack = c * Mat4::from_translation(Vec3::new(0.0, 0.30, 0.28));
    part(pack * Mat4::from_scale(Vec3::new(0.50, 0.56, 0.26)), MeshId::Armor, trim, 0.0);
    for s in 0..2 {
        part(pack * at(Vec3::new(side_of(s) * 0.15, -0.36, 0.02), Vec3::new(0.16, 0.30, 0.16)), MeshId::Armor, suit, 0.0);
    }

    // Carried flag on the back at every LOD, so carriers read from range:
    // a pole from the pack and a waving cloth, larger than the armor.
    if let (Some(flag), false) = (p.carrying, dead) {
        let pole_base = pack * Mat4::from_translation(Vec3::new(-0.16, 0.2, 0.08));
        part(pole_base * at(Vec3::new(0.0, 0.58, 0.0), Vec3::new(0.04, 1.2, 0.04)), MeshId::Cube, Vec3::splat(0.08), 0.0);
        let cloth = flag_color(flag);
        let drift = Vec3::new(p.vel.x, 0.0, p.vel.z).length().min(40.0) / 40.0;
        let strips = if detailed { 5 } else { 2 };
        for k in 0..strips {
            let u = k as f32 / strips as f32;
            let wave = (time * 7.0 - u * 3.0 + input.seed).sin() * (0.05 + 0.12 * u);
            part(pole_base * Mat4::from_translation(Vec3::new(0.0, 0.88 - u * 0.12 * drift, 0.10 + u * 0.66))
                * Mat4::from_rotation_y(wave) * Mat4::from_scale(Vec3::new(0.025, 0.52, 0.68 / strips as f32 + 0.02)),
                MeshId::Cube, cloth, 0.35);
        }
        if detailed {
            part(pole_base * at(Vec3::new(0.0, 1.20, 0.0), Vec3::splat(0.10)), MeshId::Sphere, Vec3::new(0.92, 0.74, 0.32), 0.3);
        }
    }
    if !detailed {
        if p.repair_beam.is_some() { push_repair_tool(lit, root * pose.aim, time); }
        else { push_held_weapon(lit, root * pose.aim, shown, team, &input, false); }
        return;
    }

    // Near-LOD detail: pad rims, knee caps and shin guards, elbow joints,
    // soles, gloves on the wrists, belt, chest light, pack nozzles and fins.
    for s in 0..2 {
        let side = side_of(s);
        let pad = c * Mat4::from_translation(Vec3::new(side * 0.40, 0.46, 0.0)) * Mat4::from_rotation_z(side * 0.32);
        part(pad * Mat4::from_scale(Vec3::new(0.31, 0.05, 0.41)), MeshId::Cube, trim, 0.0);
        part(at(pose.knees[s] + Vec3::new(0.0, 0.0, -0.11), Vec3::new(0.17, 0.15, 0.07)), MeshId::Bevel, trim, 0.0);
        let shin = (pose.knees[s] + pose.ankles[s]) * 0.5 + Vec3::new(0.0, 0.0, -0.10);
        part(seg(shin + (pose.knees[s] - pose.ankles[s]) * 0.35, shin - (pose.knees[s] - pose.ankles[s]) * 0.3, 0.12, 0.05),
            MeshId::Bevel, team, 0.0);
        part(at(pose.elbows[s], Vec3::splat(0.12)), MeshId::Sphere, suit, 0.0);
        part(pose.feet[s] * at(Vec3::new(0.0, -0.155, -0.07), Vec3::new(0.23, 0.03, 0.40)), MeshId::Cube, trim, 0.0);
        part(pose.wrists[s] * at(Vec3::new(0.0, -0.05, 0.0), Vec3::new(0.11, 0.12, 0.12)), MeshId::Bevel, suit, 0.0);
        // Nozzles at the tube ends, with a glowing throat while jetting.
        let nozzle = pack * Mat4::from_translation(Vec3::new(side * 0.15, -0.52, 0.02));
        part(nozzle * Mat4::from_scale(Vec3::new(0.12, 0.10, 0.12)), MeshId::Disc, trim, 0.0);
        part(nozzle * Mat4::from_translation(Vec3::new(0.0, -0.055, 0.0)) * Mat4::from_scale(Vec3::new(0.09, 0.01, 0.09)),
            MeshId::Disc, light, if p.jetting { 1.8 } else { 0.1 });
        part(pack * at(Vec3::new(side * 0.27, 0.10, 0.02), Vec3::new(0.04, 0.36, 0.18)), MeshId::Bevel, team, 0.0);
    }
    part(c * at(Vec3::new(0.0, 0.10, -0.02), Vec3::new(0.36, 0.07, 0.27)), MeshId::Cube, alloy, 0.0);
    part(c * at(Vec3::new(0.0, 0.40, -0.18), Vec3::new(0.07, 0.10, 0.02)), MeshId::Cube, light, visor * 0.7);
    part(c * at(Vec3::new(0.0, 0.58, 0.0), Vec3::new(0.28, 0.08, 0.28)), MeshId::Bevel, trim, 0.0);

    if p.repair_beam.is_some() && armed { push_repair_tool(lit, root * pose.aim, time); }
    else if armed { push_held_weapon(lit, root * pose.aim, shown, team, &input, true); }

    // Jet flame from both nozzles.
    if p.jetting && !dead {
        let col = if p.team == Team::Ember { [1.0, 0.42, 0.12, 0.55] } else { [0.3, 0.85, 1.0, 0.55] };
        for side in [-1.0f32, 1.0] {
            let flick = 1.0 + 0.18 * (time * 43.0 + side * 3.0 + input.seed).sin();
            let base = root * pack * Mat4::from_translation(Vec3::new(side * 0.15, -0.80, 0.02));
            emit.push(EmitDraw { mesh: MeshId::Sphere,
                model: base * Mat4::from_scale(Vec3::new(0.12, 0.42 * flick, 0.12)), color: col });
            emit.push(EmitDraw { mesh: MeshId::Sphere,
                model: base * Mat4::from_translation(Vec3::new(0.0, 0.2, 0.0)) * Mat4::from_scale(Vec3::new(0.07, 0.2, 0.07)),
                color: [1.0, 0.95, 0.85, 0.7] });
        }
    }

}

/// The repair tool: a compact emitter with twin prongs, a green coil and a
/// glowing canister, held in the aim frame (-Z forward). Shared by the
/// first-person view (`drawlist`) and other players' models.
pub fn repair_tool_parts(frame: Mat4, time: f32) -> Vec<LitDraw> {
    let dark = Vec3::new(0.12, 0.13, 0.15);
    let steel = Vec3::new(0.55, 0.6, 0.64);
    let green = Vec3::new(0.35, 1.0, 0.45);
    let pulse = 0.9 + 0.5 * (time * 11.0).sin().abs();
    let at = |pos: Vec3, size: Vec3| Mat4::from_translation(pos) * Mat4::from_scale(size);
    let mut out = vec![
        LitDraw { mesh: MeshId::Bevel, model: frame * at(Vec3::new(0.0, 0.0, -0.08), Vec3::new(0.11, 0.13, 0.32)), color: dark, emit: 0.0, mode: 0.0 },
        LitDraw { mesh: MeshId::Bevel, model: frame * Mat4::from_translation(Vec3::new(0.0, -0.11, 0.02)) * Mat4::from_rotation_x(0.35)
            * Mat4::from_scale(Vec3::new(0.07, 0.16, 0.08)), color: dark, emit: 0.0, mode: 0.0 },
        LitDraw { mesh: MeshId::Sphere, model: frame * at(Vec3::new(0.0, 0.09, 0.0), Vec3::new(0.06, 0.05, 0.1)), color: green, emit: 0.9 * pulse, mode: 0.0 },
        LitDraw { mesh: MeshId::Disc, model: frame * Mat4::from_translation(Vec3::new(0.0, 0.0, -0.26)) * Mat4::from_rotation_x(FRAC_PI_2)
            * Mat4::from_rotation_y(time * 9.0) * Mat4::from_scale(Vec3::new(0.075, 0.02, 0.075)), color: green, emit: 1.2 * pulse, mode: 0.0 },
    ];
    for s in [-1.0f32, 1.0] {
        out.push(LitDraw { mesh: MeshId::Cube, model: frame * at(Vec3::new(s * 0.04, 0.0, -0.33), Vec3::new(0.022, 0.022, 0.16)),
            color: steel, emit: 0.0, mode: 0.0 });
    }
    out
}

fn push_repair_tool(lit: &mut Vec<LitDraw>, grip: Mat4, time: f32) {
    lit.extend(repair_tool_parts(grip * Mat4::from_scale(Vec3::splat(1.3)), time));
}

/// Third-person weapon in the aim frame, echoing each first-person model.
fn push_held_weapon(lit: &mut Vec<LitDraw>, grip: Mat4, weapon: u8, team: Vec3, input: &PoseInput, detailed: bool) {
    // Slightly oversized so the weapon reads against the armor at range.
    let grip = grip * Mat4::from_scale(Vec3::splat(1.3));
    let mut put = |mesh, model: Mat4, color: Vec3, glow: f32| lit.push(LitDraw { mesh, model: grip * model, color, emit: glow, mode: 0.0 });
    let box_at = |at: Vec3, size: Vec3| Mat4::from_translation(at) * Mat4::from_scale(size);
    let along_z = |at: Vec3, r: f32, len: f32| Mat4::from_translation(at) * Mat4::from_rotation_x(FRAC_PI_2)
        * Mat4::from_scale(Vec3::new(r, len, r));
    let gun = Vec3::new(0.24, 0.27, 0.30);
    let dark = Vec3::new(0.06, 0.07, 0.09);
    let steel = Vec3::new(0.44, 0.48, 0.52);
    let flash = input.shot_age < 0.06;
    if !detailed {
        // Far LOD: body and barrel in the weapon's key colour.
        let body = match weapon { 0 => Vec3::new(0.64, 0.68, 0.72), 1 => gun, _ => Vec3::new(0.30, 0.34, 0.19) };
        put(MeshId::Bevel, box_at(Vec3::new(0.0, 0.0, 0.04), Vec3::new(0.17, 0.15, 0.40)), body, 0.0);
        put(MeshId::Cube, box_at(Vec3::new(0.0, 0.02, -0.30), Vec3::new(0.09, 0.09, 0.34)), steel, 0.0);
        return;
    }
    match weapon {
        0 => {
            let silver = Vec3::new(0.64, 0.68, 0.72);
            let cyan = Vec3::new(0.05, 0.74, 0.95);
            put(MeshId::Bevel, box_at(Vec3::new(0.0, 0.0, 0.02), Vec3::new(0.17, 0.15, 0.44)), silver, 0.0);
            put(MeshId::Bevel, box_at(Vec3::new(0.0, -0.10, 0.10), Vec3::new(0.07, 0.12, 0.10)), dark, 0.0);
            for side in [-1.0, 1.0] {
                put(MeshId::Bevel, box_at(Vec3::new(side * 0.068, 0.055, -0.14), Vec3::new(0.04, 0.06, 0.60)), Vec3::new(0.84, 0.87, 0.9), 0.0);
                put(MeshId::Cube, box_at(Vec3::new(side * 0.052, 0.06, -0.14), Vec3::new(0.006, 0.02, 0.44)), cyan, 0.8);
            }
            if detailed {
                put(MeshId::Cube, box_at(Vec3::new(0.088, -0.02, 0.02), Vec3::new(0.004, 0.035, 0.30)), team, 0.35);
                let charge = (1.0 - input.shot_age / 0.75).clamp(0.0, 1.0);
                if charge < 1.0 {
                    put(MeshId::Disc, Mat4::from_translation(Vec3::new(0.0, 0.07, -0.05)) * Mat4::from_scale(Vec3::new(0.07, 0.01, 0.07)), cyan, 0.6);
                }
            }
            if flash {
                put(MeshId::Sphere, box_at(Vec3::new(0.0, 0.05, -0.48), Vec3::new(0.08, 0.04, 0.12)), Vec3::new(0.35, 0.85, 1.0), 1.4);
            }
        }
        1 => {
            // Six-barrel cluster that spins while firing.
            let spin = if input.shot_age < 0.4 { input.time * 30.0 } else { 0.0 };
            put(MeshId::Bevel, box_at(Vec3::new(0.0, 0.0, 0.08), Vec3::new(0.16, 0.15, 0.30)), gun, 0.0);
            put(MeshId::Disc, along_z(Vec3::new(0.0, 0.01, 0.25), 0.07, 0.10), dark, 0.0);
            put(MeshId::Disc, Mat4::from_translation(Vec3::new(-0.11, -0.04, 0.06)) * Mat4::from_rotation_z(FRAC_PI_2)
                * Mat4::from_scale(Vec3::new(0.08, 0.07, 0.08)), Vec3::new(0.22, 0.25, 0.19), 0.0);
            put(MeshId::Disc, along_z(Vec3::new(0.0, 0.01, -0.10), 0.075, 0.04), steel, 0.0);
            put(MeshId::Disc, along_z(Vec3::new(0.0, 0.01, -0.44), 0.075, 0.03), steel, 0.0);
            let barrels = if detailed { 6 } else { 3 };
            for k in 0..barrels {
                let a = spin + k as f32 * TAU / barrels as f32;
                let o = Vec3::new(a.cos(), a.sin(), 0.0) * 0.045;
                put(MeshId::Disc, along_z(Vec3::new(0.0, 0.01, -0.27) + o, 0.017, 0.44), Vec3::new(0.32, 0.35, 0.38), 0.0);
            }
            put(MeshId::Cube, box_at(Vec3::new(0.082, 0.0, 0.08), Vec3::new(0.006, 0.04, 0.24)), team, 0.35);
            if flash {
                put(MeshId::Sphere, box_at(Vec3::new(0.0, 0.01, -0.54), Vec3::new(0.09, 0.09, 0.05)), Vec3::new(1.0, 0.8, 0.35), 1.6);
            }
        }
        _ => {
            let olive = Vec3::new(0.30, 0.34, 0.19);
            let amber = Vec3::new(1.0, 0.62, 0.18);
            put(MeshId::Bevel, box_at(Vec3::new(0.0, 0.0, 0.10), Vec3::new(0.17, 0.16, 0.26)), gun, 0.0);
            put(MeshId::Bevel, box_at(Vec3::new(0.0, -0.03, 0.27), Vec3::new(0.08, 0.12, 0.14)), olive, 0.0);
            put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.28), 0.07, 0.34), gun, 0.0);
            put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.45), 0.08, 0.03), steel, 0.0);
            put(MeshId::Disc, Mat4::from_translation(Vec3::new(0.0, -0.03, -0.06)) * Mat4::from_rotation_x(FRAC_PI_2)
                * Mat4::from_scale(Vec3::new(0.10, 0.15, 0.10)), olive, 0.0);
            let ready = input.shot_age > weapon_reload(2);
            put(MeshId::Cube, box_at(Vec3::new(0.0, 0.09, 0.02), Vec3::new(0.03, 0.02, 0.03)), amber, if ready { 0.8 } else { 0.05 });
            if detailed {
                put(MeshId::Cube, box_at(Vec3::new(0.087, -0.01, 0.10), Vec3::new(0.004, 0.04, 0.2)), team, 0.35);
            }
            if flash {
                put(MeshId::Sphere, box_at(Vec3::new(0.0, 0.02, -0.52), Vec3::new(0.09, 0.09, 0.14)), Vec3::new(1.0, 0.6, 0.2), 1.6);
            }
        }
    }
}

/// Part counts for tests and budgets.
#[cfg(test)]
pub fn part_count(p: &Player, time: f32, detailed: bool) -> (usize, usize) {
    let (mut lit, mut emit) = (Vec::new(), Vec::new());
    push_player(&mut lit, &mut emit, p, time, detailed, Motion::default(), Hold::Weapon);
    (lit.len(), emit.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::World;

    fn runner() -> Player {
        let mut world = World::new();
        world.start_match(true);
        let mut p = world.players[0].clone();
        p.pos = Vec3::ZERO;
        p.alive = true;
        p
    }

    fn input(time: f32) -> PoseInput {
        PoseInput { vel: Vec3::new(0.0, 0.0, -8.0), yaw: 0.3, pitch: 0.1, on_ground: true, skiing: false,
            jetting: false, weapon: 0, shot_age: 9.0, time, seed: 1.4, dead_for: None, squash: 0.0, lower: 0.0,
            stride: time * CADENCE, hold: Hold::Weapon, fall_dir: Vec2::new(0.0, -1.0), fall_tilt: FALL_TILT }
    }

    #[test]
    fn pose_is_deterministic_and_finite() {
        let a = pose(&input(1.23));
        let b = pose(&input(1.23));
        assert_eq!(a.aim, b.aim);
        assert_eq!(a.knees, b.knees);
        for state in 0..5 {
            let mut i = input(0.7);
            match state { 1 => i.skiing = true, 2 => { i.jetting = true; i.on_ground = false }
                3 => i.on_ground = false, 4 => i.dead_for = Some(0.4), _ => {} }
            let p = pose(&i);
            for v in p.knees.iter().chain(&p.ankles).chain(&p.elbows).chain(&p.hands) {
                assert!(v.is_finite(), "state {state}");
            }
            assert!(p.aim.is_finite() && p.body.is_finite());
        }
    }

    #[test]
    fn bones_keep_their_lengths() {
        for t in [0.0, 0.3, 0.9, 2.2] {
            let p = pose(&input(t));
            for s in 0..2 {
                assert!((p.hips[s].distance(p.knees[s]) - 0.44).abs() < 1e-4);
                assert!((p.knees[s].distance(p.ankles[s]) - 0.44).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn animation_phase_is_continuous() {
        // Small time steps and a speed change mid-stride move joints only a
        // little: amplitude follows speed, the phase never jumps.
        let mut first = input(0.0);
        first.vel = Vec3::new(0.0, 0.0, -2.0);
        let mut prev = pose(&first);
        for step in 1..600 {
            let t = step as f32 / 120.0;
            let mut i = input(t);
            i.vel = Vec3::new(0.0, 0.0, -2.0 - (step as f32 / 60.0));
            let next = pose(&i);
            for s in 0..2 {
                assert!(prev.ankles[s].distance(next.ankles[s]) < 0.12, "jump at {t}");
                assert!(prev.hands[s].distance(next.hands[s]) < 0.05, "hand jump at {t}");
            }
            prev = next;
        }
    }

    #[test]
    fn legs_run_on_the_ground_even_skiing_and_dangle_in_the_air() {
        let mut i = input(0.1);
        let run = pose(&i);
        assert!((run.ankles[0].z - run.ankles[1].z).abs() > 0.05, "legs should be apart mid-stride");
        // Skiing shows the run, as sliding Tribes players did.
        i.skiing = true;
        i.vel = Vec3::new(0.0, 0.0, -30.0);
        let ski = pose(&i);
        assert!((ski.ankles[0].z - ski.ankles[1].z).abs() > 0.05, "a skier's legs run");
        // In the air the legs hang: both feet below the knees, toes down.
        i.on_ground = false; i.skiing = false; i.jetting = true;
        let fly = pose(&i);
        for s in 0..2 {
            assert!(fly.ankles[s].y < fly.knees[s].y - 0.25, "dangling shin");
            let toe = fly.feet[s].transform_vector3(Vec3::NEG_Z);
            assert!(toe.y < -0.4, "toes hang down: {toe}");
        }
        // Free arms swing opposite the legs on the run; gripping hands hold.
        i = input(0.1);
        i.hold = Hold::Free;
        let free = pose(&i);
        assert!((free.hands[0].z - free.hands[1].z).abs() > 0.05, "arms swing");
        i.hold = Hold::Ball;
        let ball = pose(&i);
        assert!(ball.hands[0].distance(ball.hands[1]) < 0.35, "both hands on the ball");
    }

    #[test]
    fn lod_part_counts_are_bounded() {
        let mut p = runner();
        for (ski, jet, ground, carry) in [(false, false, true, false), (true, false, true, true), (false, true, false, false)] {
            p.skiing = ski; p.jetting = jet; p.on_ground = ground;
            p.carrying = if carry { Some(Team::Glacier) } else { None };
            for weapon in 0..3 {
                p.weapon = weapon;
                let (near, near_emit) = part_count(&p, 0.4, true);
                let (far, far_emit) = part_count(&p, 0.4, false);
                assert!(near <= 64, "near LOD {near}");
                assert!(far <= 26, "far LOD {far}");
                assert!(far < near);
                assert!(near_emit <= 4 && far_emit == 0);
            }
        }
    }

    #[test]
    fn corpses_collapse_then_vanish() {
        let mut p = runner();
        p.alive = false;
        p.respawn = RESPAWN_DELAY - 0.3;
        assert!(part_count(&p, 0.0, true).0 > 0);
        p.respawn = RESPAWN_DELAY - CORPSE_TIME - 0.1;
        assert_eq!(part_count(&p, 0.0, true).0, 0);
        let lying = pose(&PoseInput { dead_for: Some(1.0), ..input(0.0) });
        let head_y = lying.body.transform_point3(lying.head.transform_point3(Vec3::ZERO)).y;
        assert!(head_y < 0.8, "a settled corpse lies down, head at {head_y}");
    }

    const DT: f32 = 1.0 / 60.0;

    /// Feed the tracker `frames` of one player's state; count landing cues.
    fn feed(t: &mut AnimTracker, p: &mut Player, frames: usize, on_ground: bool, vy: f32) -> usize {
        let mut landings = 0;
        for _ in 0..frames {
            p.on_ground = on_ground;
            p.vel.y = vy;
            t.update(std::slice::from_ref(p), DT, &|a: Vec3, b: Vec3| a.distance(b));
            if t.motion(p.net_id).land_age == 0.0 { landings += 1; }
        }
        landings
    }

    #[test]
    fn a_hard_landing_squashes_once_and_jitter_does_not() {
        let mut t = AnimTracker::default();
        let mut p = runner();
        feed(&mut t, &mut p, 10, true, 0.0);
        // A one-frame airborne blip over a bump (snapshot jitter): no squash.
        assert_eq!(feed(&mut t, &mut p, 1, false, -2.0) + feed(&mut t, &mut p, 5, true, 0.0), 0);
        // A slow step down: no squash either.
        assert_eq!(feed(&mut t, &mut p, 30, false, -5.0) + feed(&mut t, &mut p, 5, true, 0.0), 0);
        // A real fall: exactly one squash, scaled by impact speed.
        assert_eq!(feed(&mut t, &mut p, 60, false, -24.0) + feed(&mut t, &mut p, 60, true, 0.0), 1);
        let mut t2 = AnimTracker::default();
        let mut q = runner();
        feed(&mut t2, &mut q, 5, true, 0.0);
        feed(&mut t2, &mut q, 60, false, -12.0);
        feed(&mut t2, &mut q, 1, true, 0.0);
        let soft = t2.motion(q.net_id).land_strength;
        feed(&mut t, &mut p, 60, false, -30.0);
        feed(&mut t, &mut p, 1, true, 0.0);
        assert!(t.motion(p.net_id).land_strength > soft, "harder landings squash more");
    }

    #[test]
    fn squash_and_switch_curves_are_continuous_and_settle() {
        let m = |land_age: f32, switch_age: f32| Motion { land_age, land_strength: 1.0, switch_age, switch_from: 1, ..Motion::default() };
        let mut prev = (m(0.0, 0.0).squash(), m(0.0, 0.0).lower());
        let mut peak = 0.0f32;
        // Sampled finely: no step between neighbouring instants (the dip is
        // quick, 0.08 s, but smooth).
        for k in 1..=480 {
            let a = k as f32 / 480.0;
            let now = (m(a, a).squash(), m(a, a).lower());
            assert!((now.0 - prev.0).abs() < 0.05 && (now.1 - prev.1).abs() < 0.05, "jump at {a}: {prev:?} -> {now:?}");
            peak = peak.max(now.0);
            prev = now;
        }
        assert!(peak > 0.9, "a full-strength landing reaches a full squash");
        assert_eq!(m(1.0, 1.0).squash(), 0.0);
        assert_eq!(m(1.0, 1.0).lower(), 0.0);
        // The posed hips sink during the squash.
        let flat = pose(&input(0.0)).pelvis.y;
        let sunk = pose(&PoseInput { squash: 1.0, ..input(0.0) }).pelvis.y;
        assert!(sunk < flat - 0.15);
    }

    #[test]
    fn a_weapon_switch_plays_once_and_swaps_the_model_midway() {
        let mut t = AnimTracker::default();
        let mut p = runner();
        p.weapon = 0;
        t.update(std::slice::from_ref(&p), DT, &|a: Vec3, b: Vec3| a.distance(b));
        // First sight never plays a switch.
        assert!(t.motion(p.net_id).switch_age > SWITCH_TIME);
        p.weapon = 1;
        let mut starts = 0;
        let mut shown = Vec::new();
        for _ in 0..40 {
            t.update(std::slice::from_ref(&p), DT, &|a: Vec3, b: Vec3| a.distance(b));
            let m = t.motion(p.net_id);
            if m.switch_age == 0.0 { starts += 1; }
            shown.push(m.shown_weapon(p.weapon));
        }
        assert_eq!(starts, 1, "one switch, one animation");
        assert_eq!(shown.first(), Some(&0), "the old weapon goes down first");
        assert_eq!(shown.last(), Some(&1), "the new weapon comes up");
        // Respawning with a different weapon is a reset, not a switch.
        p.alive = false;
        t.update(std::slice::from_ref(&p), DT, &|a: Vec3, b: Vec3| a.distance(b));
        p.alive = true;
        p.weapon = 2;
        t.update(std::slice::from_ref(&p), DT, &|a: Vec3, b: Vec3| a.distance(b));
        assert!(t.motion(p.net_id).switch_age > SWITCH_TIME);
    }

    #[test]
    fn a_downed_body_falls_the_way_it_was_knocked_and_never_through_a_wall() {
        let mut p = runner();
        p.yaw = 0.0;
        // Knocked toward +x with open ground: a full fall toward +x.
        p.vel = Vec3::new(12.0, 0.0, 0.0);
        let open = |a: Vec3, b: Vec3| a.distance(b);
        let (dir, tilt) = fall_for(&p, &open);
        assert!(dir.x > 0.9 && (tilt - FALL_TILT).abs() < 1e-5, "{dir} {tilt}");
        // A pillar 0.8 m away on that side, open behind: fall the other way.
        let pillar = |a: Vec3, b: Vec3| if b.x > a.x { 0.8 } else { a.distance(b) };
        let (dir, tilt) = fall_for(&p, &pillar);
        assert!(dir.x < -0.9 && (tilt - FALL_TILT).abs() < 1e-5, "{dir} {tilt}");
        // Boxed in both ways: tip only as far as fits.
        let boxed = |_: Vec3, _: Vec3| 1.0;
        let (_, tilt) = fall_for(&p, &boxed);
        assert!(tilt < 0.5, "{tilt}");
        let mut i = input(0.1);
        i.dead_for = Some(1.0);
        i.fall_dir = Vec2::new(1.0, 0.0);
        i.fall_tilt = tilt;
        let head = pose(&i).body.transform_point3(Vec3::new(0.0, 1.8, 0.0));
        assert!(head.x.abs() < 1.0 - 0.35 + 0.1, "the head stays inside the room: {head}");
    }
}
