use glam::{Mat3, Mat4, Vec3};

use crate::sim::{
    MatchState, Team, World, DISC_RELOAD, VM_ANCHOR_BOLT, VM_ANCHOR_DISC, VM_MUZZLE_DISC,
    VM_TURN_X, VM_TURN_Y, VM_DISC_TURN_X, VM_DISC_TURN_Y, weapon_reload,
};
use crate::terrain::{self, MapId};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MeshId {
    Terrain = 0,
    Cube = 1,
    Sphere = 2,
    Disc = 3,
    Bevel = 4,
}

#[derive(Clone)]
pub struct LitDraw {
    pub mesh: MeshId,
    pub model: Mat4,
    pub color: Vec3,
    pub emit: f32,
    pub mode: f32,
}

#[derive(Clone)]
pub struct EmitDraw {
    pub mesh: MeshId,
    pub model: Mat4,
    pub color: [f32; 4],
}

#[derive(Clone)]
pub struct DrawFrame {
    pub eye: Vec3,
    pub sun: Vec3,
    pub fog: Vec3,
    pub view: Mat4,
    pub proj: Mat4,
    pub inv_vp: Mat4,
    pub lit: Vec<LitDraw>,
    pub emit: Vec<EmitDraw>,
    pub viewmodel: Vec<LitDraw>,
    pub vm_proj: Mat4,
    pub dt: f32,
    pub map: MapId,
    pub fog_density: f32,
    pub time: f32,
}

pub fn clip_correct(proj: Mat4) -> Mat4 {
    let correction = Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
    ]);
    correction * proj
}

pub fn build_frame(world: &World, aspect: f32, dt: f32) -> DrawFrame {
    let (eye, dir, fov) = world.camera();
    let view = Mat4::look_to_rh(eye, dir, Vec3::Y);
    let size = crate::terrain::info(world.map).size;
    let far = (size * 1.05).max(480.0);
    let proj_gl = Mat4::perspective_rh(fov.to_radians(), aspect.max(0.1), 0.14, far);
    let proj = clip_correct(proj_gl);
    let vp = proj * view;
    let inv_vp = vp.inverse();
    let sun = Vec3::new(-0.35, 0.78, -0.42).normalize();
    let fog = Vec3::new(0.55, 0.46, 0.38);

    let mut lit = Vec::new();
    let mut emit = Vec::new();

    lit.push(LitDraw {
        mesh: MeshId::Terrain,
        model: Mat4::IDENTITY,
        color: Vec3::ONE,
        emit: 0.0,
        mode: if world.map == MapId::Raindance { 2.0 } else { 1.0 },
    });

    for p in &world.pillars {
        let y = crate::terrain::height_on(world.map, p.x, p.z);
        lit.push(LitDraw {
            mesh: MeshId::Cube,
            model: Mat4::from_translation(Vec3::new(p.x, y + p.h * 0.5, p.z))
                * Mat4::from_scale(Vec3::new(p.r * 1.6, p.h, p.r * 1.6)),
            color: Vec3::new(0.32, 0.30, 0.28),
            emit: 0.0,
            mode: 0.0,
        });
    }

    push_base(&mut lit, world, true);
    push_base(&mut lit, world, false);

    for f in &world.flags {
        let color = if f.team == Team::Ember {
            Vec3::new(0.89, 0.29, 0.20)
        } else {
            Vec3::new(0.24, 0.78, 0.88)
        };
        lit.push(LitDraw {
            mesh: MeshId::Cube,
            model: Mat4::from_translation(f.pos + Vec3::Y * 1.7)
                * Mat4::from_scale(Vec3::new(0.12, 3.4, 0.12)),
            color: Vec3::new(0.15, 0.16, 0.18),
            emit: 0.0,
            mode: 0.0,
        });
        let mut to = eye - f.pos;
        to.y = 0.0;
        let to = to.normalize_or_zero();
        let right = to.cross(Vec3::Y).normalize_or_zero();
        let banner = Mat4::from_cols(
            (right * 1.4).extend(0.0),
            (Vec3::Y * 1.1).extend(0.0),
            (to * 0.08).extend(0.0),
            (f.pos + Vec3::Y * 2.7 + right * 0.7).extend(1.0),
        );
        lit.push(LitDraw {
            mesh: MeshId::Cube,
            model: banner,
            color,
            emit: 0.35,
            mode: 0.0,
        });
    }

    for (i, p) in world.players.iter().enumerate() {
        if !p.alive {
            continue;
        }
        if i == world.player_id && world.state != MatchState::Flyby {
            if p.jetting {
                push_jet(&mut emit, p.pos, p.team);
            }
            continue;
        }
        let body_c = if p.team == Team::Ember {
            Vec3::new(0.72, 0.18, 0.14)
        } else {
            Vec3::new(0.16, 0.58, 0.70)
        };
        let rot = Mat4::from_rotation_y(p.yaw);
        lit.push(LitDraw {
            mesh: MeshId::Cube,
            model: Mat4::from_translation(p.pos + Vec3::Y * 0.85)
                * rot
                * Mat4::from_scale(Vec3::new(0.72, 1.15, 0.58)),
            color: body_c,
            emit: 0.05,
            mode: 0.0,
        });
        lit.push(LitDraw {
            mesh: MeshId::Cube,
            model: Mat4::from_translation(p.pos + Vec3::Y * 1.62)
                * rot
                * Mat4::from_scale(Vec3::new(0.42, 0.34, 0.42)),
            color: Vec3::new(0.12, 0.13, 0.15),
            emit: 0.0,
            mode: 0.0,
        });
        if p.carrying.is_some() {
            let c = if p.team == Team::Ember {
                Vec3::new(0.24, 0.78, 0.88)
            } else {
                Vec3::new(0.89, 0.29, 0.20)
            };
            lit.push(LitDraw {
                mesh: MeshId::Cube,
                model: Mat4::from_translation(p.pos + Vec3::Y * 2.25)
                    * Mat4::from_scale(Vec3::new(0.35, 0.7, 0.08)),
                color: c,
                emit: 0.4,
                mode: 0.0,
            });
        }
        if p.jetting {
            push_jet(&mut emit, p.pos, p.team);
        }
    }

    for d in &world.discs {
        if d.kind == 0 {
            let model = spinning_disc(d.pos, d.vel, d.spin, 0.42, 0.06);
            lit.push(LitDraw {
                mesh: MeshId::Disc,
                model,
                color: Vec3::new(0.12, 0.78, 1.0),
                emit: 0.8,
                mode: 0.0,
            });
            emit.push(EmitDraw {
                mesh: MeshId::Sphere,
                model: Mat4::from_translation(d.pos) * Mat4::from_scale(Vec3::splat(0.28)),
                color: [0.3, 0.85, 1.0, 0.35],
            });
            // Connected, tapered wake. Limit it to the actual flight age so a
            // new shot never draws a trail behind its launch point.
            let trail_time = (5.0 - d.life).clamp(0.0, 0.04);
            let forward = d.vel.normalize_or_zero();
            let mut right = forward.cross(Vec3::Y).normalize_or_zero();
            if right.length_squared() < 0.01 { right = Vec3::X; }
            let up = right.cross(forward);
            for k in 1..=5 {
                if trail_time <= 0.0 { break; }
                let t = k as f32 / 5.0;
                let radius = 0.10 * (1.0 - t * 0.7);
                let center = d.pos - d.vel * (trail_time * (k as f32 - 0.5) / 5.0);
                let length = d.vel.length() * trail_time / 5.0;
                emit.push(EmitDraw {
                    mesh: MeshId::Sphere,
                    model: Mat4::from_cols((right * radius).extend(0.0), (up * radius).extend(0.0),
                        (-forward * length * 0.55).extend(0.0), center.extend(1.0)),
                    color: [0.35, 0.75, 1.0, 0.24 * (1.0 - t * 0.8)],
                });
            }
        } else if d.kind == 2 {
            lit.push(LitDraw {
                mesh: MeshId::Sphere,
                model: Mat4::from_translation(d.pos) * Mat4::from_scale(Vec3::splat(0.16)),
                color: Vec3::new(0.24, 0.34, 0.12), emit: 0.1, mode: 0.0,
            });
            lit.push(LitDraw {
                mesh: MeshId::Cube,
                model: Mat4::from_translation(d.pos) * Mat4::from_rotation_y(d.spin)
                    * Mat4::from_scale(Vec3::new(0.21, 0.04, 0.21)),
                color: Vec3::new(0.95, 0.62, 0.12), emit: 0.35, mode: 0.0,
            });
        } else {
            let f = d.vel.normalize_or_zero();
            let mut r = f.cross(Vec3::Y).normalize_or_zero();
            if r.length_squared() < 0.01 { r = Vec3::X; }
            let u = r.cross(f).normalize_or_zero();
            let length = (d.vel.length() * (1.2 - d.life).clamp(0.0, 0.006)).min(2.0);
            if length > 0.01 {
                emit.push(EmitDraw {
                    mesh: MeshId::Sphere,
                    model: Mat4::from_cols((r * 0.025).extend(0.0), (u * 0.025).extend(0.0),
                        (f * length * 0.5).extend(0.0), (d.pos - f * length * 0.5).extend(1.0)),
                    color: [1.0, 0.68, 0.22, 0.45],
                });
            }
            lit.push(LitDraw {
                mesh: MeshId::Cube,
                model: Mat4::from_cols(
                    (r * 0.018).extend(0.0),
                    (u * 0.018).extend(0.0),
                    (f * 0.30).extend(0.0),
                    d.pos.extend(1.0),
                ),
                color: Vec3::new(1.0, 0.78, 0.30),
                emit: 0.8,
                mode: 0.0,
            });
        }
    }

    // Persist actual sampled positions, rather than extrapolating backward:
    // smoke follows the arc and stays behind when a grenade bounces.
    for puff in &world.smoke {
        let t = (puff.age / 0.5).clamp(0.0, 1.0);
        let near_fade = ((puff.pos.distance(eye) - 0.8) / 1.2).clamp(0.0, 1.0);
        emit.push(EmitDraw {
            mesh: MeshId::Sphere,
            model: Mat4::from_translation(puff.pos)
                * Mat4::from_scale(Vec3::splat(0.07 + t * 0.20)),
            color: [0.55, 0.57, 0.59, -0.16 * (1.0 - t) * near_fade],
        });
    }
    for e in &world.explosions {
        let t = (e.age / 0.42).clamp(0.0, 1.0);
        let a = (1.0 - t) * 0.85;
        if e.kind == 0 {
            let ring = (0.35 + e.max_r * t).max(0.35);
            emit.push(EmitDraw {
                mesh: MeshId::Sphere,
                model: Mat4::from_translation(e.pos) * Mat4::from_scale(Vec3::splat(0.55 + t * 1.4)),
                color: [1.0, 0.92, 0.75, a],
            });
            emit.push(EmitDraw {
                mesh: MeshId::Disc,
                model: Mat4::from_translation(e.pos + Vec3::Y * 0.2)
                    * Mat4::from_scale(Vec3::new(ring, 0.12, ring)),
                color: [1.0, 0.42, 0.08, a * 0.9],
            });
            emit.push(EmitDraw {
                mesh: MeshId::Sphere,
                model: Mat4::from_translation(e.pos) * Mat4::from_scale(Vec3::splat(e.max_r * (0.25 + 0.75 * t))),
                color: [1.0, 0.35, 0.08, a * 0.28],
            });
        } else {
            let r = e.max_r * (0.2 + t * 0.9);
            emit.push(EmitDraw {
                mesh: MeshId::Sphere,
                model: Mat4::from_translation(e.pos) * Mat4::from_scale(Vec3::splat(r)),
                color: [1.0, 0.55, 0.18, a],
            });
        }
    }

    let viewmodel = viewmodel_draws(world);

    let fog_density = 0.0072 * (256.0 / size);
    DrawFrame {
        eye,
        sun,
        fog,
        view,
        proj,
        inv_vp,
        lit,
        emit,
        viewmodel,
        // A separate lens keeps the gun stable when speed widens the world FOV.
        vm_proj: clip_correct(Mat4::perspective_rh(65.0_f32.to_radians(), aspect.max(0.75), 0.05, 8.0)),
        dt,
        map: world.map,
        fog_density,
        time: world.time,
    }
}

/// Fly edge-first, like the chambered round, with spin around the disc normal.
fn spinning_disc(pos: Vec3, vel: Vec3, spin: f32, radius: f32, thick: f32) -> Mat4 {
    let mut axis = vel.normalize_or_zero();
    if axis.length_squared() < 0.01 {
        axis = Vec3::Z;
    }
    let mut side = axis.cross(Vec3::Y);
    if side.length_squared() < 1e-4 {
        side = Vec3::X;
    }
    side = side.normalize();
    let up = side.cross(axis);
    let (s, c) = spin.sin_cos();
    let r = side * c - axis * s;
    let u = -side * s - axis * c;
    Mat4::from_cols(
        (r * radius).extend(0.0),
        (up * thick).extend(0.0),
        (u * radius).extend(0.0),
        pos.extend(1.0),
    )
}

fn disc_launcher(base: Mat4, time: f32, cooldown: f32) -> Vec<LitDraw> {
    // Original T1 silhouette: long silver split rails, recessed black feed bed,
    // cyan insets and yellow hazard marks. All parts are actual lit geometry.
    let silver = Vec3::new(0.66, 0.70, 0.73);
    let edge = Vec3::new(0.86, 0.89, 0.90);
    let steel = Vec3::new(0.29, 0.33, 0.37);
    let dark = Vec3::new(0.045, 0.065, 0.08);
    let cyan = Vec3::new(0.05, 0.74, 0.95);
    let mut draws = Vec::with_capacity(48);
    let mut part = |mesh, pos, scale, color, emit| {
        draws.push(LitDraw {
            mesh, model: base * Mat4::from_translation(pos) * Mat4::from_scale(scale),
            color, emit, mode: 0.0,
        });
    };
    part(MeshId::Bevel, Vec3::new(0.0, -0.055, 0.12), Vec3::new(0.30, 0.19, 0.42), silver, 0.0);
    part(MeshId::Bevel, Vec3::new(0.0, -0.22, 0.23), Vec3::new(0.13, 0.25, 0.21), dark, 0.0);
    part(MeshId::Bevel, Vec3::new(0.0, -0.04, -0.31), Vec3::new(0.40, 0.07, 0.70), steel, 0.0);
    part(MeshId::Cube, Vec3::new(0.0, 0.003, -0.27), Vec3::new(0.32, 0.012, 0.62), dark, 0.0);
    for side in [-1.0, 1.0] {
        let x = side * 0.205;
        part(MeshId::Bevel, Vec3::new(x, 0.018, -0.27), Vec3::new(0.09, 0.15, 0.98), silver, 0.0);
        part(MeshId::Bevel, Vec3::new(x, 0.088, -0.27), Vec3::new(0.063, 0.018, 0.91), edge, 0.0);
        part(MeshId::Bevel, Vec3::new(x, 0.018, -0.765), Vec3::new(0.085, 0.115, 0.055), steel, 0.0);
        // Interior rail energy strip; the open channel remains dark.
        part(MeshId::Cube, Vec3::new(side * 0.157, 0.035, -0.36), Vec3::new(0.008, 0.025, 0.50), cyan, 0.6);
        for k in 0..5 {
            let z = 0.04 - k as f32 * 0.14;
            part(MeshId::Cube, Vec3::new(x, 0.099, z), Vec3::new(0.047, 0.005, 0.05), dark, 0.0);
            part(MeshId::Bevel, Vec3::new(side * 0.251, 0.008, z), Vec3::new(0.01, 0.065, 0.065), steel, 0.0);
        }
        for k in 0..3 {
            part(MeshId::Cube, Vec3::new(x, 0.102, -0.49 - k as f32 * 0.065),
                Vec3::new(0.049, 0.01, 0.022), cyan, 0.7);
        }
    }
    // A mechanical feed sled opens and returns over the existing reload cycle.
    let cycle = (1.0 - cooldown / DISC_RELOAD).clamp(0.0, 1.0);
    let open = if cooldown > 0.0 { (cycle * std::f32::consts::PI).sin() } else { 0.0 };
    part(MeshId::Bevel, Vec3::new(0.0, 0.065, 0.055 + open * 0.12),
        Vec3::new(0.17, 0.045, 0.19), steel, 0.0);
    part(MeshId::Cube, Vec3::new(0.0, 0.09, 0.08 + open * 0.12),
        Vec3::new(0.1, 0.008, 0.018), cyan, 0.35 + cycle * 0.25);
    for k in 0..4 {
        part(MeshId::Bevel, Vec3::new(-0.253, 0.031, 0.03 - k as f32 * 0.09),
            Vec3::new(0.013, 0.033, 0.041), Vec3::new(0.92, 0.66, 0.08), 0.0);
    }
    let charge = (1.0 - cooldown / 0.75).clamp(0.0, 1.0);
    if charge > 0.0 {
        // Expose the round in the widened tray: it should read as a
        // loaded disc launcher, not two bars concealing a tiny light.
        let disc_base = base * Mat4::from_translation(Vec3::new(0.0, 0.075, 0.12 - charge * 0.65))
            * Mat4::from_rotation_y(time * 18.0);
        draws.push(LitDraw {
            mesh: MeshId::Disc,
            model: disc_base * Mat4::from_scale(Vec3::new(0.155, 0.022, 0.155)),
            color: cyan, emit: 0.55, mode: 0.0,
        });
        draws.push(LitDraw {
            mesh: MeshId::Disc,
            model: disc_base * Mat4::from_translation(Vec3::Y * 0.012)
                * Mat4::from_scale(Vec3::new(0.132, 0.004, 0.132)),
            color: Vec3::new(0.025, 0.23, 0.95), emit: 0.5, mode: 0.0,
        });
    }
    if cooldown > DISC_RELOAD - 0.08 {
        draws.push(LitDraw {
            mesh: MeshId::Sphere,
            model: base * Mat4::from_translation(VM_MUZZLE_DISC)
                * Mat4::from_scale(Vec3::new(0.07, 0.025, 0.12)),
            color: Vec3::new(0.5, 0.9, 1.0), emit: 1.4, mode: 0.0,
        });
    }
    draws
}

fn push_jet(emit: &mut Vec<EmitDraw>, pos: Vec3, team: Team) {
    let c = if team == Team::Ember {
        [1.0, 0.42, 0.12, 0.55]
    } else {
        [0.3, 0.85, 1.0, 0.55]
    };
    emit.push(EmitDraw {
        mesh: MeshId::Sphere,
        model: Mat4::from_translation(pos + Vec3::new(0.0, 0.1, 0.0))
            * Mat4::from_scale(Vec3::new(0.28, 0.7, 0.28)),
        color: c,
    });
}

fn push_base(lit: &mut Vec<LitDraw>, world: &World, ember: bool) {
    let spec = terrain::info(world.map);
    let home = if ember { spec.ember } else { spec.glacier };
    let y = terrain::height_on(world.map, home.x, home.z);
    let accent = if ember {
        Vec3::new(0.78, 0.22, 0.16)
    } else {
        Vec3::new(0.18, 0.62, 0.74)
    };
    let steel = Vec3::new(0.18, 0.20, 0.23);
    lit.push(LitDraw {
        mesh: MeshId::Cube,
        model: Mat4::from_translation(Vec3::new(home.x, y + 0.18, home.z))
            * Mat4::from_scale(Vec3::new(16.0, 0.4, 16.0)),
        color: steel,
        emit: 0.0,
        mode: 0.0,
    });
    let zoff = if ember { 4.0 } else { -4.0 };
    lit.push(LitDraw {
        mesh: MeshId::Cube,
        model: Mat4::from_translation(Vec3::new(home.x + 6.5, y + 5.5, home.z + zoff))
            * Mat4::from_scale(Vec3::new(2.4, 11.0, 2.4)),
        color: steel,
        emit: 0.0,
        mode: 0.0,
    });
    lit.push(LitDraw {
        mesh: MeshId::Cube,
        model: Mat4::from_translation(Vec3::new(home.x + 6.5, y + 11.2, home.z + zoff))
            * Mat4::from_scale(Vec3::new(3.0, 0.5, 3.0)),
        color: accent,
        emit: 0.25,
        mode: 0.0,
    });
    for k in 0..3 {
        let a = k as f32 * 2.1;
        lit.push(LitDraw {
            mesh: MeshId::Cube,
            model: Mat4::from_translation(Vec3::new(
                home.x + a.cos() * 7.5,
                y + 1.1,
                home.z + a.sin() * 7.5,
            )) * Mat4::from_scale(Vec3::new(1.6, 2.2, 1.6)),
            color: steel * 1.15,
            emit: 0.0,
            mode: 0.0,
        });
    }
}

fn viewmodel_draws(world: &World) -> Vec<LitDraw> {
    if world.state == MatchState::Flyby {
        return Vec::new();
    }
    let Some(p) = world.players.get(world.player_id) else {
        return Vec::new();
    };
    if !p.alive {
        return Vec::new();
    }
    let shot_age = weapon_reload(p.weapon) - p.cooldown;
    let kick = if p.cooldown > 0.0 { (-shot_age * 15.0).exp() } else { 0.0 };
    let sway = (world.time * 1.4).sin() * 0.012;
    let anchor = if p.weapon == 0 { VM_ANCHOR_DISC } else { VM_ANCHOR_BOLT };
    let (yaw, pitch) = if p.weapon == 0 { (VM_DISC_TURN_Y, VM_DISC_TURN_X) }
        else { (VM_TURN_Y, VM_TURN_X) };
    let base = Mat4::from_translation(anchor + Vec3::new(sway, -kick * 0.05, kick * 0.08))
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_rotation_x(pitch + kick * 0.12);
    if p.weapon == 0 {
        // Pivot around the muzzle, not the grip: move the rear toward the
        // right edge without moving the launch point or changing ballistics.
        let angled = base * Mat4::from_translation(VM_MUZZLE_DISC)
            * Mat4::from_rotation_y(0.12)
            * Mat4::from_translation(-VM_MUZZLE_DISC);
        return disc_launcher(angled, world.time, p.cooldown);
    }
    let mut draws = vec![
        LitDraw {
            mesh: MeshId::Bevel,
            model: base * Mat4::from_scale(Vec3::new(0.18, 0.16, 0.55)),
            color: Vec3::new(0.34, 0.38, 0.41),
            emit: 0.0,
            mode: 0.0,
        },
    ];
    let barrel_count = if p.weapon == 1 { 6 } else { 1 };
    for k in 0..barrel_count {
        let angle = k as f32 * std::f32::consts::TAU / 6.0
            + if p.cooldown > 0.0 && p.weapon == 1 { world.time * 35.0 } else { 0.0 };
        let radius = if p.weapon == 1 { 0.036 } else { 0.105 };
        let offset = if p.weapon == 1 { Vec3::new(angle.cos(), angle.sin(), 0.0) * 0.075 }
            else { Vec3::ZERO };
        let tube = base * Mat4::from_translation(Vec3::new(0.0, 0.02, -0.38) + offset)
            * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2);
        draws.push(LitDraw { mesh: MeshId::Disc,
            model: tube * Mat4::from_scale(Vec3::new(radius, 0.42, radius)),
            color: Vec3::new(0.30, 0.34, 0.36), emit: 0.0, mode: 0.0 });
        draws.push(LitDraw { mesh: MeshId::Disc,
            model: base * Mat4::from_translation(Vec3::new(0.0, 0.02, -0.593) + offset)
                * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)
                * Mat4::from_scale(Vec3::new(radius * 0.72, 0.004, radius * 0.72)),
            color: Vec3::splat(0.025), emit: 0.0, mode: 0.0 });
    }
    if p.weapon == 2 {
        draws.push(LitDraw { mesh: MeshId::Disc,
            model: base * Mat4::from_translation(Vec3::new(0.0, -0.04, -0.06))
                * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2)
                * Mat4::from_scale(Vec3::new(0.17, 0.30, 0.17)),
            color: Vec3::new(0.37, 0.42, 0.23), emit: 0.0, mode: 0.0 });
    }
    if p.cooldown > 0.0 && shot_age < 0.035 {
        draws.push(LitDraw { mesh: MeshId::Sphere,
            model: base * Mat4::from_translation(Vec3::new(0.0, 0.02, -0.64))
                * Mat4::from_scale(Vec3::new(0.07, 0.07, 0.14)),
            color: Vec3::new(1.0, 0.66, 0.18), emit: 1.3, mode: 0.0 });
    }
    {
        draws.push(LitDraw {
            mesh: MeshId::Cube,
            model: base
                * Mat4::from_translation(Vec3::new(0.0, -0.12, 0.05))
                * Mat4::from_scale(Vec3::new(0.08, 0.18, 0.14)),
            color: Vec3::new(0.22, 0.23, 0.25),
            emit: 0.0,
            mode: 0.0,
        });
    }
    draws.push(LitDraw {
        mesh: MeshId::Cube,
        model: base
            * Mat4::from_translation(Vec3::new(0.0, 0.095, -0.05))
            * Mat4::from_scale(Vec3::new(0.025, 0.025, 0.08)),
        color: Vec3::new(0.95, 0.64, 0.14),
        emit: 0.3,
        mode: 0.0,
    });
    draws
}

pub fn normal_columns(model: Mat4) -> [[f32; 4]; 3] {
    let m = Mat3::from_mat4(model);
    let n = m.inverse().transpose();
    let c = n.to_cols_array();
    [
        [c[0], c[1], c[2], 0.0],
        [c[3], c[4], c[5], 0.0],
        [c[6], c[7], c[8], 0.0],
    ]
}
