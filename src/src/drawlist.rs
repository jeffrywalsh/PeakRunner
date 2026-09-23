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
    Armor = 5,
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

/// QA-only: override the render camera from `QA_FLYCAM=x,y,z,yaw_deg,pitch_deg,fov_deg`,
/// a free camera decoupled from the player entity's position/physics, used only by
/// capture tooling (see AGENTS.md). Absent the env var, rendering is unaffected.
fn qa_flycam_override() -> Option<(Vec3, Vec3, f32)> {
    let raw = std::env::var("QA_FLYCAM").ok()?;
    let v: Vec<f32> = raw.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if v.len() != 6 {
        return None;
    }
    let (x, y, z, yaw, pitch, fov) = (v[0], v[1], v[2], v[3].to_radians(), v[4].to_radians(), v[5]);
    let cp = pitch.cos();
    let dir = Vec3::new(-yaw.sin() * cp, pitch.sin(), -yaw.cos() * cp);
    Some((Vec3::new(x, y, z), dir, fov))
}

pub fn build_frame(world: &World, aspect: f32, dt: f32) -> DrawFrame {
    let (eye, dir, fov) = qa_flycam_override().unwrap_or_else(|| world.camera());
    let view = Mat4::look_to_rh(eye, dir, Vec3::Y);
    let size = crate::terrain::info(world.map).size;
    let far = (size * 1.05).max(480.0);
    let proj_gl = Mat4::perspective_rh(fov.to_radians(), aspect.max(0.1), 0.14, far);
    let proj = clip_correct(proj_gl);
    let vp = proj * view;
    let inv_vp = vp.inverse();
    let sun = Vec3::new(-0.35, 0.78, -0.42).normalize();
    let fog = if let Some(pack) = peakrunner_core::map_pack::on(world.map) {Vec3::from_array(pack.fog_color())} else {Vec3::new(0.55, 0.46, 0.38)};

    let mut lit = Vec::new();
    let mut emit = Vec::new();

    let imported=peakrunner_core::map_pack::on(world.map).is_some();
    if !imported {lit.push(LitDraw {
        mesh: MeshId::Terrain,
        model: Mat4::IDENTITY,
        color: Vec3::ONE,
        emit: 0.0,
        mode: if world.map != MapId::Valley { 2.0 } else { 1.0 },
    });}

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

    if !imported {
        push_base(&mut lit, world, true);
        push_base(&mut lit, world, false);
    }

    for f in &world.flags {
        let color = if f.team == Team::Ember {
            Vec3::new(0.89, 0.29, 0.20)
        } else {
            Vec3::new(0.24, 0.78, 0.88)
        };
        let metal = Vec3::new(0.15, 0.16, 0.18);
        lit.push(LitDraw {
            mesh: MeshId::Cube,
            model: Mat4::from_translation(f.pos + Vec3::Y * 1.75)
                * Mat4::from_scale(Vec3::new(0.12, 3.5, 0.12)),
            color: metal, emit: 0.0, mode: 0.0,
        });
        // Finial and a collar where the cloth's hoist meets the pole.
        lit.push(LitDraw {
            mesh: MeshId::Sphere,
            model: Mat4::from_translation(f.pos + Vec3::Y * 3.58) * Mat4::from_scale(Vec3::splat(0.17)),
            color: Vec3::new(0.78, 0.62, 0.30), emit: 0.15, mode: 0.0,
        });
        lit.push(LitDraw {
            mesh: MeshId::Cube,
            model: Mat4::from_translation(f.pos + Vec3::Y * 3.32) * Mat4::from_scale(Vec3::new(0.2, 0.1, 0.2)),
            color: metal, emit: 0.0, mode: 0.0,
        });
        // The cloth keeps facing the viewer so it reads from range, and waves
        // in strips whose ripple grows toward the free edge.
        let mut to = eye - f.pos;
        to.y = 0.0;
        let to = to.normalize_or_zero();
        let right = to.cross(Vec3::Y).normalize_or_zero();
        const STRIPS: usize = 6;
        let (width, height) = (1.5, 1.1);
        let strip = width / STRIPS as f32;
        for i in 0..STRIPS {
            let u = (i as f32 + 0.5) / STRIPS as f32;
            let phase = world.time * 4.2 - u * 5.0 + f.team.idx() as f32;
            let sway = u * 0.22 * phase.sin();
            let droop = u * u * 0.12;
            let shade = 0.86 + 0.14 * (phase + 1.2).cos();
            let center = f.pos + Vec3::Y * (2.75 - droop) + right * (0.06 + u * width) + to * sway;
            lit.push(LitDraw {
                mesh: MeshId::Cube,
                model: Mat4::from_cols(
                    (right * strip * 1.04).extend(0.0),
                    (Vec3::Y * height).extend(0.0),
                    (to * 0.06).extend(0.0),
                    center.extend(1.0),
                ),
                color: color * shade,
                emit: 0.35,
                mode: 0.0,
            });
        }
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
        push_runner(&mut lit, p, world.time, eye.distance_squared(p.pos) < 75.0 * 75.0);
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
            // No spherical halo: the spinning disc owns the silhouette.
            // A fine blue wake stays behind it. Limit it to the flight age so a
            // new shot never draws a trail behind its launch point.
            let trail_time = (5.0 - d.life).clamp(0.0, 0.08).min(6.0/d.vel.length().max(1.0));
            let forward = d.vel.normalize_or_zero();
            let mut right = forward.cross(Vec3::Y).normalize_or_zero();
            if right.length_squared() < 0.01 { right = Vec3::X; }
            let up = right.cross(forward);
            for k in 1..=8 {
                if trail_time <= 0.0 { break; }
                let t = k as f32 / 8.0;
                let radius = 0.055 * (1.0 - t * 0.7);
                let center = d.pos - d.vel * (trail_time * (k as f32 - 0.5) / 8.0);
                let length = d.vel.length() * trail_time / 8.0;
                emit.push(EmitDraw {
                    mesh: MeshId::Sphere,
                    model: Mat4::from_cols((right * radius).extend(0.0), (up * radius).extend(0.0),
                        (-forward * length * 0.55).extend(0.0), center.extend(1.0)),
                    color: [0.45, 0.78, 1.0, 0.24 * (1.0 - t * 0.85)],
                });
            }
        } else if d.kind == 3 {
            lit.push(LitDraw {
                mesh: MeshId::Sphere,
                model: Mat4::from_translation(d.pos) * Mat4::from_scale(Vec3::splat(0.45)),
                color: Vec3::new(0.75, 1.0, 0.55), emit: 1.0, mode: 0.0,
            });
            emit.push(EmitDraw {
                mesh: MeshId::Sphere,
                model: Mat4::from_translation(d.pos) * Mat4::from_scale(Vec3::splat(0.65)),
                color: [0.3, 1.0, 0.12, 0.5],
            });
            for k in 1..=5 {
                let t=k as f32/5.;
                let center=d.pos-d.vel*(3.-d.life).clamp(0.,0.035)*t;
                emit.push(EmitDraw {
                    mesh: MeshId::Sphere,
                    model: Mat4::from_translation(center)*Mat4::from_scale(Vec3::splat(0.4*(1.-t*0.75))),
                    color: [0.35,1.0,0.1,0.3*(1.-t*0.8)],
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
            flame_burst(&mut emit,e.pos,e.age,true);
            emit.push(EmitDraw {
                mesh: MeshId::Disc,
                model: Mat4::from_translation(e.pos + Vec3::Y * 0.2)
                    * Mat4::from_scale(Vec3::new(ring, 0.12, ring)),
                color: [0.12, 0.55, 1.0, a * 0.18],
            });
        } else if e.kind == 2 {
            flame_burst(&mut emit,e.pos,e.age,false);
        } else {
            let r = e.max_r * (0.2 + t * 0.9);
            emit.push(EmitDraw {
                mesh: MeshId::Sphere,
                model: Mat4::from_translation(e.pos) * Mat4::from_scale(Vec3::splat(r)),
                color: if e.kind==3 {[0.35,1.0,0.12,a*0.6]} else {[1.0, 0.55, 0.18, a]},
            });
        }
    }

    for (d,s) in peakrunner_core::equipment::definitions(world.map).iter().zip(&world.equipment) {
        let color=if s.health<=0. {Vec3::new(0.45,0.08,0.03)}
            else if !s.powered {Vec3::new(0.65,0.34,0.04)} else {Vec3::new(0.16,0.9,0.6)};
        let lamp_height=match d.kind {
            peakrunner_core::equipment::Kind::Repair=>-0.4,
            peakrunner_core::equipment::Kind::Inventory=>3.0,
            peakrunner_core::equipment::Kind::Generator=>3.2,
            peakrunner_core::equipment::Kind::Sensor=>2.0,
            peakrunner_core::equipment::Kind::Turret=>0.7,
        };
        lit.push(LitDraw {mesh:MeshId::Cube,
            model:Mat4::from_translation(d.pos()+Vec3::Y*lamp_height)*Mat4::from_scale(Vec3::new(0.24,0.12,0.24)),
            color,emit:if s.powered {1.0} else {0.1},mode:0.0});
        if d.kind==peakrunner_core::equipment::Kind::Turret {
            push_turret_head(&mut lit,&mut emit,d,s);
        }
    }
    let viewmodel = viewmodel_draws(world);

    let fog_density = if imported {0.0012} else {0.0072 * (256.0 / size)};
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
fn flame_burst(emit:&mut Vec<EmitDraw>,pos:Vec3,age:f32,blue:bool) {
    let t=(age/0.55).clamp(0.,1.);let fade=(1.-t).powi(2);
    let extent=if blue {2.2} else {3.2};
    // Separate soft-edged tongues of fire, not one large explosion sphere.
    // The existing negative-alpha shader mode softens each lobe's silhouette.
    for i in 0..7 {
        let angle=i as f32*2.399+pos.x*0.13+pos.z*0.07;
        let axis=Vec3::new(angle.cos(),0.25+(i%3) as f32*0.35,angle.sin()).normalize();
        let center=pos+axis*extent*(0.15+t*1.2)+Vec3::Y*t*t*2.;
        let size=(0.55+(i%3) as f32*0.14)*(0.6+(t*5.).min(1.))*(1.-t*0.6);
        let model=Mat4::from_translation(center)*Mat4::from_rotation_z(angle*0.2)
            *Mat4::from_scale(Vec3::new(size,size*(1.5+t),size));
        emit.push(EmitDraw {mesh:MeshId::Sphere,model,
            color:if blue {[0.04,0.22,1.,-fade*0.65]} else {[1.,0.07,0.005,-fade*0.7]}});
        emit.push(EmitDraw {mesh:MeshId::Sphere,model:model*Mat4::from_scale(Vec3::splat(0.58)),
            color:if blue {[0.35,0.85,1.,-fade*0.9]} else {[1.,0.75,0.04,-fade*0.95]}});
    }
    emit.push(EmitDraw {mesh:MeshId::Sphere,
        model:Mat4::from_translation(pos)*Mat4::from_scale(Vec3::splat(0.65+t)),
        color:if blue {[0.7,0.95,1.,-fade]} else {[1.,0.95,0.4,-fade]}});
    for i in 0..5 {
        let angle=i as f32*2.399;
        let direction=Vec3::new(angle.cos(),0.4+(i%2) as f32*0.5,angle.sin()).normalize();
        let center=pos+direction*(0.3+t*extent*3.)-Vec3::Y*t*t;
        emit.push(EmitDraw {mesh:MeshId::Sphere,
            model:Mat4::from_translation(center)*Mat4::from_scale(Vec3::new(0.045,0.16,0.045)),
            color:if blue {[0.3,0.75,1.,fade]} else {[1.,0.4,0.02,fade]}});
    }
}

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

/// Original modular armor. Local -Z is forward, +Y is up; render-only posing
/// never changes the authoritative capsule, aim or movement.
fn push_runner(lit: &mut Vec<LitDraw>, p: &crate::sim::Player, time: f32, detailed: bool) {
    let team = if p.team == Team::Ember { Vec3::new(0.74,0.20,0.12) }
        else { Vec3::new(0.12,0.51,0.65) };
    let alloy = Vec3::new(0.48,0.55,0.61);
    let suit = Vec3::new(0.065,0.085,0.11);
    let trim = Vec3::new(0.18,0.23,0.29);
    let light = Vec3::new(0.32,0.88,1.0);
    let base = Mat4::from_translation(p.pos) * Mat4::from_rotation_y(p.yaw);
    let speed = Vec3::new(p.vel.x,0.,p.vel.z).length();
    let stride = if p.on_ground && !p.skiing {
        (time * 9.0 + p.net_id as f32 * 0.7).sin() * (speed / 8.).min(1.) * 0.23
    } else { 0.0 };
    let crouch = if p.skiing { 0.13 } else if p.jetting { 0.06 } else { 0.0 };
    let torso = base * Mat4::from_translation(Vec3::new(0.,-crouch,0.))
        * Mat4::from_rotation_x(if p.skiing { -0.09 } else { 0.0 });
    let mut part = |root: Mat4, pos: Vec3, size: Vec3, color: Vec3, glow: f32| {
        lit.push(LitDraw { mesh:MeshId::Armor,
            model:root * Mat4::from_translation(pos) * Mat4::from_scale(size),
            color, emit:glow, mode:0.0 });
    };
    // Narrow waist, broad breastplate and a sealed, recessed visor.
    part(torso,Vec3::new(0.,0.92,0.),Vec3::new(0.38,0.31,0.29),suit,0.);
    part(torso,Vec3::new(0.,1.22,0.),Vec3::new(0.60,0.49,0.35),team,0.);
    part(torso,Vec3::new(0.,1.63,0.),Vec3::new(0.37,0.37,0.37),alloy,0.);
    part(torso,Vec3::new(0.,1.65,-0.189),Vec3::new(0.30,0.115,0.04),suit,0.);
    part(torso,Vec3::new(0.,1.66,-0.235),Vec3::new(0.25,0.044,0.022),light,0.65);
    if !detailed {
        for side in [-1.,1.] {
            part(base,Vec3::new(side*0.21,0.42,0.),Vec3::new(0.23,0.76,0.27),alloy,0.);
            part(torso,Vec3::new(side*0.39,1.18,0.),Vec3::new(0.25,0.48,0.32),team,0.);
        }
        part(torso,Vec3::new(0.,1.17,0.30),Vec3::new(0.62,0.53,0.28),trim,0.);
        return;
    }
    for side in [-1.,1.] {
        // Connected two-piece legs with a planted sole, or a tucked flight pose.
        let hip=Vec3::new(side*0.19,0.85-crouch,0.);
        let knee=Vec3::new(side*0.21,0.46-crouch*0.5,-0.06+side*stride);
        let ankle=Vec3::new(side*0.23,0.14,if p.jetting {0.13} else {side*stride*0.65});
        for (a,b,width,color) in [(hip,knee,0.235,team),(knee,ankle,0.20,alloy)] {
            let axis=b-a;
            let root=base * Mat4::from_translation((a+b)*0.5)
                * Mat4::from_quat(glam::Quat::from_rotation_arc(Vec3::Y,axis.normalize()));
            part(root,Vec3::ZERO,Vec3::new(width,axis.length()+0.04,0.23),color,0.);
        }
        part(base,ankle+Vec3::new(0.,-0.07,-0.06),Vec3::new(0.25,0.14,0.39),trim,0.);
        part(torso,Vec3::new(side*0.39,1.31,0.),Vec3::new(0.27,0.27,0.37),team,0.);
        part(torso,Vec3::new(side*0.40,1.07,-0.035),Vec3::new(0.19,0.30,0.21),trim,0.);
        // Forearms carried forward in a braced weapon stance.
        part(torso * Mat4::from_rotation_x(-0.65),Vec3::new(side*0.36,0.89,0.42),
            Vec3::new(0.20,0.29,0.23),alloy,0.);
        part(torso,Vec3::new(side*0.21,1.17,0.30),Vec3::new(0.24,0.53,0.28),trim,0.);
        if detailed {
            part(torso,Vec3::new(side*0.21,1.40,0.32),Vec3::new(0.18,0.11,0.25),alloy,0.);
            part(torso,Vec3::new(side*0.21,0.91,0.31),Vec3::new(0.15,0.065,0.19),
                light,if p.jetting {1.6} else {0.15});
            part(torso,Vec3::new(side*0.15,1.28,-0.185),Vec3::new(0.23,0.22,0.075),alloy,0.);
            part(torso,Vec3::new(side*0.40,1.35,-0.196),Vec3::new(0.14,0.045,0.025),alloy,0.);
            part(base,knee+Vec3::new(0.,0.,-0.125),Vec3::new(0.19,0.16,0.06),trim,0.);
        }
    }
    if detailed {
        part(torso,Vec3::new(0.,1.45,0.),Vec3::new(0.42,0.085,0.40),trim,0.);
        part(torso,Vec3::new(0.,1.20,-0.218),Vec3::new(0.065,0.09,0.02),light,0.5);
        part(torso,Vec3::new(0.,0.91,-0.17),Vec3::new(0.35,0.08,0.065),alloy,0.);
        part(torso,Vec3::new(0.,1.54,-0.19),Vec3::new(0.22,0.075,0.06),trim,0.);
    }
    // Compact third-person weapon silhouette (not a second first-person model).
    part(torso,Vec3::new(0.29,0.98,-0.39),Vec3::new(0.22,0.18,0.53),suit,0.);
}

#[cfg(test)]
mod character_tests {
    use super::*;
    #[test]
    fn armor_poses_are_finite_and_distant_models_are_bounded() {
        let mut world=World::new(); world.start_match(true);
        let mut p=world.players[0].clone();
        p.pos=Vec3::ZERO;
        for (ski,jet,ground) in [(false,false,true),(true,false,true),(false,true,false)] {
            p.skiing=ski; p.jetting=jet; p.on_ground=ground;
            p.vel=Vec3::new(9.,0.,3.);
            for time in [0.,0.2,0.8] {
                let mut near=Vec::new(); let mut far=Vec::new();
                push_runner(&mut near,&p,time,true);
                push_runner(&mut far,&p,time,false);
                assert!(near.len()<=40 && far.len()<=10 && far.len()<near.len());
                for draw in near.iter().chain(&far) {
                    assert!(draw.model.is_finite() && draw.model.determinant()>0.);
                    assert!(draw.color.is_finite());
                }
            }
        }
    }
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

/// Runtime turret head: a housing that yaws with the aim, a cradle that also
/// pitches, and barrels by weapon type. The static mount is baked into the map.
fn push_turret_head(lit:&mut Vec<LitDraw>,emit:&mut Vec<EmitDraw>,d:&peakrunner_core::equipment::Definition,s:&peakrunner_core::equipment::State) {
    let aim=if s.aim.length_squared()>1e-6 {s.aim.normalize()} else {Vec3::NEG_Z};
    let flat=Vec3::new(aim.x,0.,aim.z).normalize_or(Vec3::NEG_Z);
    let yaw=glam::Quat::from_rotation_arc(Vec3::NEG_Z,flat);
    let full=glam::Quat::from_rotation_arc(Vec3::NEG_Z,aim);
    let base=Mat4::from_translation(d.pos());
    let dark=Vec3::new(0.16,0.19,0.22);let steel=Vec3::new(0.30,0.33,0.36);
    let accent=if d.team==0 {Vec3::new(0.78,0.26,0.18)} else {Vec3::new(0.22,0.66,0.78)};
    let part=|lit:&mut Vec<LitDraw>,q:glam::Quat,mesh,at:Vec3,size:Vec3,color:Vec3,glow:f32| lit.push(LitDraw {
        mesh,model:base*Mat4::from_quat(q)*Mat4::from_translation(at)*Mat4::from_scale(size),color,emit:glow,mode:0.});
    // Yaw housing with team stripes.
    part(lit,yaw,MeshId::Bevel,Vec3::new(0.,-0.25,0.2),Vec3::new(2.3,0.9,2.3),steel,0.);
    for x in [-1.16,1.16] {part(lit,yaw,MeshId::Cube,Vec3::new(x,-0.25,0.2),Vec3::new(0.04,0.22,1.9),accent,0.25);}
    // Pitch cradle cheeks and receiver.
    for x in [-0.95,0.95] {part(lit,full,MeshId::Bevel,Vec3::new(x,0.25,0.),Vec3::new(0.32,1.0,1.5),dark,0.);}
    part(lit,full,MeshId::Bevel,Vec3::new(0.,0.25,-0.1),Vec3::new(1.5,0.8,1.7),steel,0.);
    if d.weapon==peakrunner_core::equipment::TurretWeapon::Plasma {
        part(lit,full,MeshId::Cube,Vec3::new(0.,0.25,-1.9),Vec3::new(0.62,0.62,2.4),dark,0.);
        for z in [-1.2,-2.0,-2.8] {
            part(lit,full,MeshId::Cube,Vec3::new(0.,0.25,z),Vec3::new(0.9,0.9,0.14),Vec3::new(0.35,1.0,0.3),if s.powered {0.9} else {0.05});
        }
        if s.powered {
            emit.push(EmitDraw {mesh:MeshId::Sphere,
                model:Mat4::from_translation(d.pos()+aim*3.2)*Mat4::from_scale(Vec3::splat(0.38)),
                color:[0.35,1.,0.12,0.6]});
        }
    } else {
        for x in [-0.42,0.42] {
            part(lit,full,MeshId::Cube,Vec3::new(x,0.25,-2.0),Vec3::new(0.26,0.26,2.8),dark,0.);
            part(lit,full,MeshId::Cube,Vec3::new(x,0.25,-3.35),Vec3::new(0.4,0.4,0.36),steel,0.);
        }
    }
}
