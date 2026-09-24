//! Articulated third-person armor. Rigid parts per bone, posed procedurally
//! from state every client already has in its snapshot (velocity, ground,
//! ski, jet, aim, weapon cooldown, carried flag, death timer). Render-only:
//! the authoritative capsule, hitbox and movement never read any of this.
//! Local frame: origin at the feet, -Z forward, +Y up.

use crate::drawlist::{EmitDraw, LitDraw, MeshId};
use crate::sim::{weapon_reload, Player, Team};
use glam::{Mat4, Quat, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Respawn delay the sim starts the death timer at; the collapse pose plays
/// over the first part of it.
const RESPAWN_DELAY: f32 = 3.4;
/// Bodies stay visible this long after death, then vanish until respawn.
const CORPSE_TIME: f32 = 2.6;
/// Stride cadence in radians per second; amplitude, not rate, follows speed
/// so the phase never jumps when speed changes.
const CADENCE: f32 = 9.0;

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
}

impl PoseInput {
    pub fn from_player(p: &Player, time: f32) -> Self {
        let shot_age = if p.cooldown > 0.0 { weapon_reload(p.weapon) - p.cooldown } else { 9.0 };
        let dead_for = if p.alive { None } else { Some((RESPAWN_DELAY - p.respawn).max(0.0)) };
        PoseInput { vel: p.vel, yaw: p.yaw, pitch: p.pitch, on_ground: p.on_ground,
            skiing: p.skiing, jetting: p.jetting, weapon: p.weapon, shot_age, time,
            seed: p.net_id as f32 * 0.7, dead_for }
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
    pub shoulders: [Vec3; 2],
    pub elbows: [Vec3; 2],
    pub hands: [Vec3; 2],
}

fn rot_x(a: f32) -> Mat4 { Mat4::from_rotation_x(a) }

/// Pure, deterministic pose from the inputs.
pub fn pose(i: &PoseInput) -> Pose {
    let local_vel = Mat4::from_rotation_y(-i.yaw).transform_vector3(i.vel);
    let speed = Vec3::new(i.vel.x, 0.0, i.vel.z).length();
    let grounded_run = i.on_ground && !i.skiing;
    let amp = if grounded_run { (speed / 8.0).min(1.0) } else { 0.0 };
    let phase = i.time * CADENCE + i.seed;
    let breathe = (i.time * 1.7 + i.seed).sin();

    // Body posture per state.
    let (crouch, lean, roll) = if i.skiing {
        // Deep ski tuck, leaning into the turn from sideways velocity.
        (0.24, 0.32, (-local_vel.x / 25.0).clamp(-1.0, 1.0) * 0.32)
    } else if i.jetting {
        (0.06, 0.22, 0.0)
    } else if !i.on_ground {
        (0.02, 0.08 + (-i.vel.y / 40.0).clamp(-0.1, 0.25), 0.0)
    } else {
        (0.015 * breathe * 0.5 + amp * 0.03 * (phase * 2.0).sin().abs(), amp * 0.12, 0.0)
    };

    // Death: tip over backwards around the feet and settle.
    let body = match i.dead_for {
        Some(t) => {
            let k = (t / 0.55).min(1.0);
            let fall = k * k * (3.0 - 2.0 * k);
            Mat4::from_translation(Vec3::new(0.0, 0.18 * fall, 0.0))
                * rot_x(-1.42 * fall) * Mat4::from_rotation_z(0.25 * fall)
        }
        None => Mat4::from_rotation_z(roll),
    };

    let pelvis = Vec3::new(0.0, 0.95 - crouch, 0.0);
    let chest = Mat4::from_translation(pelvis) * rot_x(-lean)
        * Mat4::from_scale(Vec3::new(1.0, 1.0 + 0.008 * breathe, 1.0));
    let neck = chest.transform_point3(Vec3::new(0.0, 0.58, 0.0));
    let look = i.pitch.clamp(-1.1, 1.1);
    let head = Mat4::from_translation(neck) * rot_x(lean * 0.6 + look * 0.55);

    // Legs: two-bone chains driven by hip and knee angles about X.
    // Positive hip swings the leg forward (-Z); knee bend folds the shin back.
    let mut hips = [Vec3::ZERO; 2];
    let mut knees = [Vec3::ZERO; 2];
    let mut ankles = [Vec3::ZERO; 2];
    for (s, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let swing = (phase + if side < 0.0 { 0.0 } else { PI }).sin();
        let (hip_a, knee_a, splay) = if i.dead_for.is_some() {
            (0.25 + side * 0.1, 0.35, 0.10)
        } else if i.skiing {
            // Feet together, knees bent and slightly offset for balance.
            (0.62 + side * 0.06, 1.05, 0.02)
        } else if i.jetting {
            // Legs trail behind in flight.
            (-0.45 + side * 0.08, 0.75, 0.05)
        } else if !i.on_ground {
            // Airborne: one knee up, one leg down.
            (0.35 + side * 0.30, 0.55 - side * 0.2, 0.06)
        } else {
            let lift = (phase + if side < 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 }).sin().max(0.0);
            (swing * 0.55 * amp, 0.12 + lift * 1.0 * amp, 0.03)
        };
        let hip = pelvis + Vec3::new(side * 0.17, -0.05, 0.0);
        let thigh_dir = (rot_x(hip_a) * Mat4::from_rotation_z(side * splay)).transform_vector3(Vec3::NEG_Y);
        let knee = hip + thigh_dir * 0.44;
        let shin_dir = rot_x(hip_a - knee_a).transform_vector3(Vec3::NEG_Y);
        let ankle = knee + shin_dir * 0.44;
        hips[s] = hip; knees[s] = knee; ankles[s] = ankle;
    }

    // Aim frame: shoulders follow the chest, the weapon follows yaw/pitch.
    let shoulders = [
        chest.transform_point3(Vec3::new(-0.30, 0.47, 0.0)),
        chest.transform_point3(Vec3::new(0.30, 0.47, 0.0)),
    ];
    let (back, rise, _) = crate::drawlist::recoil(i.weapon, i.shot_age, i.time);
    let run_bob = amp * 0.03 * (phase * 2.0).sin();
    let aim = Mat4::from_translation(chest.transform_point3(Vec3::new(0.14, 0.30 + run_bob, 0.0)))
        * rot_x(look * 0.9 + rise * 2.0)
        * Mat4::from_translation(Vec3::new(0.0, 0.0, -0.36 + back * 2.5));
    // Hands: right on the grip, left on the fore-grip.
    let hands = [aim.transform_point3(Vec3::new(-0.06, -0.02, -0.34)),
        aim.transform_point3(Vec3::new(0.0, -0.09, 0.06))];
    // Elbows bow down and out between shoulder and hand.
    let mut elbows = [Vec3::ZERO; 2];
    for s in 0..2 {
        let side = if s == 0 { -1.0 } else { 1.0 };
        let mid = (shoulders[s] + hands[s]) * 0.5;
        let reach = shoulders[s].distance(hands[s]);
        let sag = (0.64 - reach).max(0.0) * 0.8 + 0.06;
        elbows[s] = mid + Vec3::new(side * sag * 0.7, -sag, 0.05);
    }
    Pose { body, pelvis, chest, head, aim, hips, knees, ankles, shoulders, elbows, hands }
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
pub fn push_player(lit: &mut Vec<LitDraw>, emit: &mut Vec<EmitDraw>, p: &Player, time: f32, detailed: bool) {
    let input = PoseInput::from_player(p, time);
    if matches!(input.dead_for, Some(t) if t > CORPSE_TIME) { return; }
    let pose = pose(&input);
    let root = Mat4::from_translation(p.pos) * Mat4::from_rotation_y(p.yaw) * pose.body;
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

    // Torso: pelvis, abdomen, breastplate, back plate, collar; head and visor.
    let c = pose.chest;
    part(c * at(Vec3::new(0.0, 0.02, 0.0), Vec3::new(0.40, 0.22, 0.28)), MeshId::Armor, suit, 0.0);
    part(c * at(Vec3::new(0.0, 0.34, 0.0), Vec3::new(0.60, 0.46, 0.36)), MeshId::Armor, team, 0.0);
    part(pose.head * at(Vec3::new(0.0, 0.12, 0.0), Vec3::new(0.34, 0.36, 0.36)), MeshId::Armor, alloy, 0.0);
    part(pose.head * at(Vec3::new(0.0, 0.13, -0.17), Vec3::new(0.27, 0.10, 0.05)), MeshId::Bevel, suit, 0.0);
    part(pose.head * at(Vec3::new(0.0, 0.135, -0.198), Vec3::new(0.22, 0.04, 0.02)), MeshId::Cube, light, visor);

    // Pelvis block joining the thighs to the torso, then the legs.
    part(at(pose.pelvis + Vec3::new(0.0, -0.04, 0.0), Vec3::new(0.38, 0.18, 0.26)), MeshId::Armor, suit, 0.0);
    for s in 0..2 {
        part(seg(pose.hips[s], pose.knees[s], 0.23, 0.24), MeshId::Armor, team, 0.0);
        part(seg(pose.knees[s], pose.ankles[s], 0.19, 0.21), MeshId::Armor, alloy, 0.0);
    }
    // Arms: single segments at the far LOD.
    for s in 0..2 {
        if detailed {
            part(seg(pose.shoulders[s], pose.elbows[s], 0.17, 0.18), MeshId::Armor, trim, 0.0);
            part(seg(pose.elbows[s], pose.hands[s], 0.15, 0.16), MeshId::Armor, alloy, 0.0);
        } else {
            part(seg(pose.shoulders[s], pose.hands[s], 0.17, 0.18), MeshId::Armor, trim, 0.0);
        }
    }
    // Jetpack on the back.
    let pack = c * Mat4::from_translation(Vec3::new(0.0, 0.30, 0.27));
    part(pack * Mat4::from_scale(Vec3::new(0.46, 0.52, 0.22)), MeshId::Armor, trim, 0.0);

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
        push_held_weapon(lit, root * pose.aim, p.weapon, team, &input, false);
        return;
    }

    // Near-LOD detail: pauldrons, knee and elbow caps, boots, gloves, belt,
    // chest light, pack nozzles and fins.
    for s in 0..2 {
        let side = if s == 0 { -1.0 } else { 1.0 };
        part(c * at(Vec3::new(side * 0.36, 0.52, 0.0), Vec3::new(0.26, 0.20, 0.34)), MeshId::Armor, team, 0.0);
        part(at(pose.knees[s] + Vec3::new(0.0, 0.0, -0.11), Vec3::new(0.17, 0.15, 0.07)), MeshId::Bevel, trim, 0.0);
        part(at(pose.elbows[s], Vec3::splat(0.13)), MeshId::Sphere, suit, 0.0);
        let foot = pose.ankles[s] + Vec3::new(0.0, -0.06, -0.07);
        part(at(foot, Vec3::new(0.21, 0.13, 0.36)), MeshId::Armor, suit, 0.0);
        part(at(foot + Vec3::new(0.0, -0.05, 0.0), Vec3::new(0.22, 0.03, 0.38)), MeshId::Cube, trim, 0.0);
        part(at(pose.hands[s], Vec3::new(0.11, 0.10, 0.12)), MeshId::Bevel, suit, 0.0);
        // Pack nozzles with a glowing throat while jetting.
        let nozzle = pack * Mat4::from_translation(Vec3::new(side * 0.13, -0.33, 0.02));
        part(nozzle * Mat4::from_scale(Vec3::new(0.10, 0.12, 0.10)), MeshId::Disc, suit, 0.0);
        part(nozzle * Mat4::from_translation(Vec3::new(0.0, -0.065, 0.0)) * Mat4::from_scale(Vec3::new(0.07, 0.01, 0.07)),
            MeshId::Disc, light, if p.jetting { 1.8 } else { 0.1 });
        part(pack * at(Vec3::new(side * 0.25, 0.10, 0.02), Vec3::new(0.04, 0.34, 0.16)), MeshId::Bevel, team, 0.0);
    }
    part(c * at(Vec3::new(0.0, 0.10, -0.02), Vec3::new(0.44, 0.07, 0.31)), MeshId::Cube, alloy, 0.0);
    part(c * at(Vec3::new(0.0, 0.36, -0.19), Vec3::new(0.07, 0.10, 0.02)), MeshId::Cube, light, visor * 0.7);
    part(c * at(Vec3::new(0.0, 0.58, 0.0), Vec3::new(0.30, 0.08, 0.30)), MeshId::Bevel, trim, 0.0);

    push_held_weapon(lit, root * pose.aim, p.weapon, team, &input, true);

    // Jet flame from both nozzles.
    if p.jetting && !dead {
        let col = if p.team == Team::Ember { [1.0, 0.42, 0.12, 0.55] } else { [0.3, 0.85, 1.0, 0.55] };
        for side in [-1.0f32, 1.0] {
            let flick = 1.0 + 0.18 * (time * 43.0 + side * 3.0 + input.seed).sin();
            let base = root * pack * Mat4::from_translation(Vec3::new(side * 0.13, -0.62, 0.05));
            emit.push(EmitDraw { mesh: MeshId::Sphere,
                model: base * Mat4::from_scale(Vec3::new(0.12, 0.42 * flick, 0.12)), color: col });
            emit.push(EmitDraw { mesh: MeshId::Sphere,
                model: base * Mat4::from_translation(Vec3::new(0.0, 0.2, 0.0)) * Mat4::from_scale(Vec3::new(0.07, 0.2, 0.07)),
                color: [1.0, 0.95, 0.85, 0.7] });
        }
    }

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
    push_player(&mut lit, &mut emit, p, time, detailed);
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
            jetting: false, weapon: 0, shot_age: 9.0, time, seed: 1.4, dead_for: None }
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
    fn running_swings_legs_opposite_and_skiing_tucks() {
        let mut i = input(0.1);
        let run = pose(&i);
        assert!((run.ankles[0].z - run.ankles[1].z).abs() > 0.05, "legs should be apart mid-stride");
        i.skiing = true;
        let ski = pose(&i);
        assert!(ski.pelvis.y < run.pelvis.y - 0.15, "ski tuck lowers the pelvis");
        assert!((ski.ankles[0].z - ski.ankles[1].z).abs() < 0.1, "ski feet stay together");
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
                assert!(near <= 60, "near LOD {near}");
                assert!(far <= 18, "far LOD {far}");
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
}

