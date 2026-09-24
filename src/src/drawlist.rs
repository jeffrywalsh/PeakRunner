use glam::{Mat3, Mat4, Vec3};

use crate::effects::{Effects, ViewAnim};
use crate::sim::{
    MatchState, Team, World, DISC_RELOAD, VM_ANCHOR_BOLT, VM_ANCHOR_DISC, VM_MUZZLE_DISC,
    VM_TURN_X, VM_TURN_Y, VM_DISC_TURN_X, VM_DISC_TURN_Y, weapon_reload,
};

/// Additive effect draws per frame. World effects are pushed first, so a
/// storm of particles is what gets trimmed, never flags or jets.
pub const MAX_EMIT_DRAWS: usize = 1200;
/// Alpha-blended smoke/dust/scorch draws per frame; the farthest are dropped.
pub const MAX_SMOKE_DRAWS: usize = 220;
/// Players closer than this get the full-detail articulated model.
pub const PLAYER_DETAIL_RANGE: f32 = 80.0;
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
    /// Alpha-blended, fogged effects that can darken the scene, back to front.
    pub smoke: Vec<EmitDraw>,
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
/// The camera the frame is rendered from, so screen overlays line up with it.
pub(crate) fn view_camera(world: &World) -> (Vec3, Vec3, f32) {
    qa_flycam_override().unwrap_or_else(|| world.camera())
}

/// The QA fly-camera's position, when `QA_FLYCAM` is set.
pub(crate) fn qa_flycam_eye() -> Option<Vec3> {
    qa_flycam_override().map(|(p, _, _)| p)
}

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

/// Stateless frame (captures, tests): effects start empty and animation rests.
#[cfg(test)]
pub fn build_frame(world: &World, aspect: f32, dt: f32) -> DrawFrame {
    build_frame_with(world, aspect, dt, &mut Effects::new())
}

pub fn build_frame_with(world: &World, aspect: f32, dt: f32, fx: &mut Effects) -> DrawFrame {
    let (eye, dir, fov) = view_camera(world);
    fx.update(world, eye, dt);
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
    let mut smoke = Vec::new();

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

    // Capture & Hold has no flags in play.
    let ctf = world.mode == peakrunner_core::map_catalog::SupportedMode::Ctf;
    for f in world.flags.iter().filter(|_| ctf) {
        // A carried flag rides on its carrier's back (player_model).
        if f.carrier.is_some() { continue; }
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
        if i == world.player_id && world.state != MatchState::Flyby {
            if p.alive && p.jetting {
                push_jet(&mut emit, p.pos, p.team);
            }
            continue;
        }
        // Articulated armor; dead players collapse briefly, then vanish.
        crate::player_model::push_player(&mut lit, &mut emit, p, world.time,
            eye.distance_squared(p.pos) < PLAYER_DETAIL_RANGE * PLAYER_DETAIL_RANGE, fx.players.motion(p.net_id));
    }

    for d in &world.discs {
        match d.kind {
            0 => push_disc_round(&mut lit, &mut emit, d),
            2 => push_grenade_round(&mut lit, &mut emit, d, world.time),
            3 => push_plasma(&mut lit, &mut emit, d, world.time),
            _ => push_tracer(&mut lit, &mut emit, d, world.time),
        }
    }

    // Persist actual sampled positions, rather than extrapolating backward:
    // smoke follows the arc and stays behind when a grenade bounces. It is
    // alpha-blended now, so the trail darkens snow and sand instead of vanishing.
    for puff in &world.smoke {
        let t = (puff.age / 0.5).clamp(0.0, 1.0);
        let near_fade = ((puff.pos.distance(eye) - 0.8) / 1.2).clamp(0.0, 1.0);
        smoke.push(EmitDraw {
            mesh: MeshId::Sphere,
            model: Mat4::from_translation(puff.pos)
                * Mat4::from_scale(Vec3::splat(0.08 + t * 0.26)),
            color: [0.60, 0.60, 0.58, 0.34 * (1.0 - t) * near_fade],
        });
    }
    for e in &world.explosions {
        push_blast(&mut emit, e, eye);
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
        if d.kind==peakrunner_core::equipment::Kind::Generator && (s.health<=0. || s.offline) {
            push_generator_wreck(&mut emit,&mut smoke,d.pos(),d.radius,world.time,s.health<=0.);
        }
    }
    fx.draw(&mut lit, &mut emit, &mut smoke, eye);
    // Back to front for alpha blending; past the cap the farthest go first.
    let depth = |d: &EmitDraw| d.model.w_axis.truncate().distance_squared(eye);
    smoke.sort_unstable_by(|a, b| depth(b).total_cmp(&depth(a)));
    if smoke.len() > MAX_SMOKE_DRAWS {
        let excess = smoke.len() - MAX_SMOKE_DRAWS;
        smoke.drain(..excess);
    }
    emit.truncate(MAX_EMIT_DRAWS);
    let viewmodel = viewmodel_draws(world, &fx.anim);

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
        smoke,
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
/// A wrecked generator smoulders: low smoke curling off its shell and
/// flickering sparks spitting from its sides. While it is being repaired back
/// past half hull the smoke thins and the sparks stop. Everything hugs the
/// outside of the casing (hit radius) and stays low, so it reads in basements;
/// positions are a deterministic function of time.
fn push_generator_wreck(emit:&mut Vec<EmitDraw>,smoke:&mut Vec<EmitDraw>,base:Vec3,radius:f32,time:f32,destroyed:bool) {
    let puffs=if destroyed {10} else {5};
    for k in 0..puffs {
        let phase=(time*0.4+k as f32/puffs as f32).fract();
        let a=k as f32*2.39996+time*0.15;
        let out=radius*0.8+phase*1.4;
        let pos=base+Vec3::new(a.cos()*out,0.6+phase*3.0,a.sin()*out);
        // Alpha-blended: the dark smoke actually darkens the room now.
        smoke.push(EmitDraw {mesh:MeshId::Sphere,
            model:Mat4::from_translation(pos)*Mat4::from_scale(Vec3::splat(0.3+phase*0.8)),
            color:[0.08,0.075,0.07,0.55*(1.-phase)*phase.min(0.15)/0.15]});
    }
    if !destroyed {return;}
    for k in 0..18 {
        let seed=k as f32*12.9898+(time*14.).floor()*78.233;
        let flick=(seed.sin()*43758.547).fract();
        if flick<0.4 {continue;}
        let a=(seed*0.37).sin()*std::f32::consts::TAU;
        // Sparks spit outward and fall; each lives a fraction of a flicker.
        let fall=(time*14.).fract();
        let pos=base+Vec3::new(a.cos()*(radius+0.2+fall*0.6),0.5+flick*3.2-fall*0.8,a.sin()*(radius+0.2+fall*0.6));
        emit.push(EmitDraw {mesh:MeshId::Sphere,
            model:Mat4::from_translation(pos)*Mat4::from_scale(Vec3::splat(0.045+flick*0.04)),
            color:[1.0,0.62+flick*0.3,0.18,1.0]});
    }
}

/// Stateless blast layers from the authoritative record: a white flash, then
/// flame lobes and a shockwave ring. Smoke, debris, sparks and scorch marks
/// come from `effects` so they can outlive the 0.55 s record.
fn push_blast(emit: &mut Vec<EmitDraw>, e: &crate::sim::Explosion, eye: Vec3) {
    let t = (e.age / 0.42).clamp(0.0, 1.0);
    let a = (1.0 - t) * 0.85;
    let flash = (1.0 - e.age / 0.08).max(0.0);
    if flash > 0.0 {
        let r = e.max_r * if e.kind == 4 { 0.45 } else { 0.28 } * (1.25 - flash * 0.25);
        emit.push(EmitDraw { mesh: MeshId::Sphere,
            model: Mat4::from_translation(e.pos) * Mat4::from_scale(Vec3::splat(r)),
            color: [1.0, 0.97, 0.9, flash * 0.9] });
    }
    // Far away only the flash reads anyway; skip the multi-draw layers.
    if e.pos.distance(eye) > crate::effects::DETAIL_RANGE { return; }
    let ring = |emit: &mut Vec<EmitDraw>, at: Vec3, r: f32, color: [f32; 4]| emit.push(EmitDraw {
        mesh: MeshId::Disc,
        model: Mat4::from_translation(at) * Mat4::from_scale(Vec3::new(r.max(0.35), 0.12, r.max(0.35))),
        color });
    match e.kind {
        0 => {
            flame_burst(emit, e.pos, e.age, true);
            ring(emit, e.pos + Vec3::Y * 0.2, 0.35 + e.max_r * t, [0.12, 0.55, 1.0, a * 0.18]);
        }
        2 => {
            flame_burst(emit, e.pos, e.age, false);
            shock_ring(emit, e.pos + Vec3::Y * 0.3, 0.6 + e.max_r * 1.1 * t, 0.18, [1.0, 0.62, 0.3, a * 0.5], t);
        }
        3 => {
            let r = e.max_r * (0.2 + t * 0.7);
            emit.push(EmitDraw { mesh: MeshId::Sphere,
                model: Mat4::from_translation(e.pos) * Mat4::from_scale(Vec3::splat(r)),
                color: [0.35, 1.0, 0.12, -a * 0.7] });
            ring(emit, e.pos, 0.3 + e.max_r * 0.9 * t, [0.35, 1.0, 0.2, a * 0.2]);
        }
        4 => {
            // Generator: rolling flame tongues around a hot core and a wide
            // ground shockwave, instead of one orange bubble. Cosmetic only.
            for (dx, dz) in [(0.0, 0.0), (1.6, 0.4), (-1.3, 1.1), (0.4, -1.5)] {
                flame_burst(emit, e.pos + Vec3::new(dx, 0.6 + t * 2.5, dz), e.age, false);
            }
            // A short-lived hot core, then the flames carry the blast.
            let core = (1.0 - e.age / 0.2).max(0.0);
            if core > 0.0 {
                emit.push(EmitDraw { mesh: MeshId::Sphere,
                    model: Mat4::from_translation(e.pos) * Mat4::from_scale(Vec3::splat(e.max_r * (0.12 + 0.12 * (1.0 - core)))),
                    color: [1.0, 0.8, 0.5, core * 0.8] });
            }
            shock_ring(emit, e.pos - Vec3::Y * 1.6, 1.0 + e.max_r * 1.5 * t, 0.3, [1.0, 0.6, 0.28, a * 0.55], t);
        }
        _ => {
            let r = e.max_r * (0.2 + t * 0.9);
            emit.push(EmitDraw { mesh: MeshId::Sphere,
                model: Mat4::from_translation(e.pos) * Mat4::from_scale(Vec3::splat(r)),
                color: [1.0, 0.55, 0.18, a] });
        }
    }
}

/// Expanding shockwave drawn as a ring of soft segments, not a filled disc.
fn shock_ring(emit: &mut Vec<EmitDraw>, center: Vec3, radius: f32, thickness: f32, color: [f32; 4], t: f32) {
    if t > 0.7 || color[3] <= 0.01 { return; }
    const SEGMENTS: usize = 16;
    let arc = std::f32::consts::TAU * radius / SEGMENTS as f32;
    for k in 0..SEGMENTS {
        let a = k as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let at = center + Vec3::new(a.cos(), 0.0, a.sin()) * radius;
        emit.push(EmitDraw { mesh: MeshId::Sphere,
            model: Mat4::from_translation(at) * Mat4::from_rotation_y(-a)
                * Mat4::from_scale(Vec3::new(thickness, thickness * 0.6, arc * 0.6)),
            color: [color[0], color[1], color[2], -color[3]] });
    }
}

/// Disc in flight: a bright rim and a dark hub so the spin reads, two rim
/// studs that sweep round, and a longer ribbon wake.
fn push_disc_round(lit: &mut Vec<LitDraw>, emit: &mut Vec<EmitDraw>, d: &crate::sim::Disc) {
    let model = spinning_disc(d.pos, d.vel, d.spin, 0.42, 0.06);
    lit.push(LitDraw { mesh: MeshId::Disc, model, color: Vec3::new(0.10, 0.74, 1.0), emit: 1.1, mode: 0.0 });
    lit.push(LitDraw { mesh: MeshId::Disc, model: model * Mat4::from_scale(Vec3::new(0.6, 1.5, 0.6)),
        color: Vec3::new(0.03, 0.06, 0.09), emit: 0.0, mode: 0.0 });
    for k in [0.0, std::f32::consts::PI] {
        lit.push(LitDraw { mesh: MeshId::Cube,
            model: model * Mat4::from_rotation_y(k) * Mat4::from_translation(Vec3::new(0.84, 0.0, 0.0))
                * Mat4::from_scale(Vec3::new(0.18, 1.7, 0.18)),
            color: Vec3::new(0.85, 0.97, 1.0), emit: 1.3, mode: 0.0 });
    }
    // Limit the wake to the flight age so a new shot never trails its launch.
    let trail_time = (5.0 - d.life).clamp(0.0, 0.14).min(9.0 / d.vel.length().max(1.0));
    if trail_time <= 0.0 { return; }
    let forward = d.vel.normalize_or_zero();
    let mut right = forward.cross(Vec3::Y).normalize_or_zero();
    if right.length_squared() < 0.01 { right = Vec3::X; }
    let up = right.cross(forward);
    const SEGMENTS: usize = 12;
    for k in 1..=SEGMENTS {
        let t = k as f32 / SEGMENTS as f32;
        let radius = 0.07 * (1.0 - t * 0.75);
        let center = d.pos - d.vel * (trail_time * (k as f32 - 0.5) / SEGMENTS as f32);
        let length = d.vel.length() * trail_time / SEGMENTS as f32;
        emit.push(EmitDraw { mesh: MeshId::Sphere,
            model: Mat4::from_cols((right * radius * 1.8).extend(0.0), (up * radius * 0.5).extend(0.0),
                (-forward * length * 0.55).extend(0.0), center.extend(1.0)),
            color: [0.45, 0.8, 1.0, 0.3 * (1.0 - t * 0.85)] });
    }
}

/// Stubby grenade round whose fuse light blinks faster as detonation nears.
fn push_grenade_round(lit: &mut Vec<LitDraw>, emit: &mut Vec<EmitDraw>, d: &crate::sim::Disc, time: f32) {
    let spin = Mat4::from_translation(d.pos) * Mat4::from_rotation_y(d.spin) * Mat4::from_rotation_x(d.spin * 0.6);
    lit.push(LitDraw { mesh: MeshId::Sphere, model: spin * Mat4::from_scale(Vec3::new(0.17, 0.15, 0.17)),
        color: Vec3::new(0.26, 0.31, 0.15), emit: 0.05, mode: 0.0 });
    lit.push(LitDraw { mesh: MeshId::Cube, model: spin * Mat4::from_scale(Vec3::new(0.24, 0.045, 0.24)),
        color: Vec3::new(0.95, 0.62, 0.12), emit: 0.35, mode: 0.0 });
    let rate = 3.0 + 14.0 * (1.0 - (d.life / 2.0).clamp(0.0, 1.0));
    let on = (time * rate).fract() < 0.5 || d.life < 0.25;
    let fuse = d.pos + Vec3::Y * 0.15;
    lit.push(LitDraw { mesh: MeshId::Sphere, model: Mat4::from_translation(fuse) * Mat4::from_scale(Vec3::splat(0.045)),
        color: Vec3::new(1.0, 0.28, 0.08), emit: if on { 1.6 } else { 0.08 }, mode: 0.0 });
    if on {
        emit.push(EmitDraw { mesh: MeshId::Sphere, model: Mat4::from_translation(fuse) * Mat4::from_scale(Vec3::splat(0.11)),
            color: [1.0, 0.32, 0.08, 0.55] });
    }
}

/// Turret plasma: a flickering hot core inside two halo shells, and a trail.
fn push_plasma(lit: &mut Vec<LitDraw>, emit: &mut Vec<EmitDraw>, d: &crate::sim::Disc, time: f32) {
    let flicker = 0.88 + 0.12 * (time * 37.0 + d.pos.x * 3.1).sin();
    lit.push(LitDraw { mesh: MeshId::Sphere, model: Mat4::from_translation(d.pos) * Mat4::from_scale(Vec3::splat(0.32 * flicker)),
        color: Vec3::new(0.85, 1.0, 0.7), emit: 1.3, mode: 0.0 });
    emit.push(EmitDraw { mesh: MeshId::Sphere, model: Mat4::from_translation(d.pos) * Mat4::from_scale(Vec3::splat(0.55)),
        color: [0.35, 1.0, 0.15, 0.55] });
    emit.push(EmitDraw { mesh: MeshId::Sphere, model: Mat4::from_translation(d.pos) * Mat4::from_scale(Vec3::splat(0.95 * flicker)),
        color: [0.2, 0.9, 0.1, -0.35] });
    for k in 1..=7 {
        let t = k as f32 / 7.0;
        let center = d.pos - d.vel * (3.0 - d.life).clamp(0.0, 0.06) * t;
        emit.push(EmitDraw { mesh: MeshId::Sphere,
            model: Mat4::from_translation(center) * Mat4::from_scale(Vec3::splat(0.42 * (1.0 - t * 0.75))),
            color: [0.35, 1.0, 0.1, 0.3 * (1.0 - t * 0.8)] });
    }
}

/// Chaingun round. Every third round of a burst is a long bright tracer with
/// a soft glow; the rest are faint streaks, which gives the stream a rhythm.
fn push_tracer(lit: &mut Vec<LitDraw>, emit: &mut Vec<EmitDraw>, d: &crate::sim::Disc, time: f32) {
    let f = d.vel.normalize_or_zero();
    let mut r = f.cross(Vec3::Y).normalize_or_zero();
    if r.length_squared() < 0.01 { r = Vec3::X; }
    let u = r.cross(f).normalize_or_zero();
    // time + life is constant over a round's flight, so it names the round.
    let bright = (((time + d.life) / 0.075).round() as i64).rem_euclid(3) == 0;
    let (cap, max_len) = if bright { (0.012, 4.5) } else { (0.006, 2.0) };
    let length = (d.vel.length() * (1.2 - d.life).clamp(0.0, cap)).min(max_len);
    if length > 0.01 {
        let streak = |w: f32, color: [f32; 4]| EmitDraw { mesh: MeshId::Sphere,
            model: Mat4::from_cols((r * w).extend(0.0), (u * w).extend(0.0),
                (f * length * 0.5).extend(0.0), (d.pos - f * length * 0.5).extend(1.0)),
            color };
        if bright {
            emit.push(streak(0.035, [1.0, 0.82, 0.4, 0.9]));
            emit.push(streak(0.09, [1.0, 0.55, 0.15, 0.22]));
        } else {
            emit.push(streak(0.022, [1.0, 0.68, 0.22, 0.4]));
        }
    }
    lit.push(LitDraw {
        mesh: MeshId::Cube,
        model: Mat4::from_cols((r * 0.018).extend(0.0), (u * 0.018).extend(0.0), (f * 0.30).extend(0.0), d.pos.extend(1.0)),
        color: Vec3::new(1.0, 0.78, 0.30),
        emit: if bright { 1.2 } else { 0.8 },
        mode: 0.0,
    });
}

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

fn disc_launcher(base: Mat4, time: f32, cooldown: f32, shot_age: f32, ready_flash: f32, team: Vec3) -> Vec<LitDraw> {
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
    // Charge bar along the top of the left rail, where the player sees it:
    // it fills rear to front over the reload and flashes once when ready.
    let fill = if cooldown > 0.0 { 1.0 - cooldown / DISC_RELOAD } else { 1.0 };
    let pulse = (1.0 - ready_flash / 0.25).max(0.0);
    for k in 0..8 {
        let on = (k as f32 + 0.5) / 8.0 <= fill;
        draws.push(LitDraw { mesh: MeshId::Cube,
            model: base * Mat4::from_translation(Vec3::new(-0.205, 0.104, 0.11 - k as f32 * 0.055))
                * Mat4::from_scale(Vec3::new(0.03, 0.006, 0.04)),
            color: if on { cyan } else { dark }, emit: if on { 0.7 + pulse * 1.3 } else { 0.0 }, mode: 0.0 });
    }
    draws.push(LitDraw { mesh: MeshId::Cube,
        model: base * Mat4::from_translation(Vec3::new(0.152, -0.05, 0.12)) * Mat4::from_scale(Vec3::new(0.004, 0.035, 0.34)),
        color: team, emit: 0.3, mode: 0.0 });
    // Muzzle crown: two short prongs at the rail ends that flare on fire.
    let flare = (-shot_age * 14.0).exp();
    for side in [-1.0, 1.0] {
        draws.push(LitDraw { mesh: MeshId::Bevel,
            model: base * Mat4::from_translation(Vec3::new(side * 0.205, 0.03, -0.835)) * Mat4::from_scale(Vec3::new(0.035, 0.05, 0.09)),
            color: silver, emit: 0.0, mode: 0.0 });
        draws.push(LitDraw { mesh: MeshId::Cube,
            model: base * Mat4::from_translation(Vec3::new(side * 0.205, 0.04, -0.885)) * Mat4::from_scale(Vec3::new(0.026, 0.02, 0.02)),
            color: cyan, emit: 0.25 + flare * 2.5, mode: 0.0 });
    }
    if shot_age < 0.08 {
        let k = 1.0 - shot_age / 0.08;
        draws.push(LitDraw {
            mesh: MeshId::Sphere,
            model: base * Mat4::from_translation(VM_MUZZLE_DISC)
                * Mat4::from_scale(Vec3::new(0.1, 0.035, 0.18) * (0.7 + k * 0.5)),
            color: Vec3::new(0.35, 0.85, 1.0), emit: 0.95, mode: 0.0,
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

fn team_accent(team: Team) -> Vec3 {
    if team == Team::Ember { Vec3::new(0.86, 0.28, 0.18) } else { Vec3::new(0.22, 0.70, 0.86) }
}

/// Per-weapon recoil read from the time since the shot: (push back, pitch
/// up, yaw jitter). Stateless, so snapshot replays and captures are exact.
pub(crate) fn recoil(weapon: u8, age: f32, time: f32) -> (f32, f32, f32) {
    match weapon {
        // Heavy shove: fast back, then a lagging pitch-up that settles.
        0 => { let k = (-age * 9.0).exp(); let rise = age * (-age * 7.0).exp() * 5.5; (0.10 * k, 0.14 * rise + 0.04 * k, 0.0) }
        // Fast, small jitter that keeps buzzing while the trigger cycles.
        1 => { let k = (-age * 40.0).exp(); (0.014 * k, 0.02 * k, 0.008 * (time * 97.0).sin() * k) }
        // Big, slow shove with a long recovery.
        _ => { let k = (-age * 4.5).exp(); let rise = age * (-age * 5.0).exp() * 4.0; (0.09 * k, 0.26 * rise + 0.05 * k, 0.0) }
    }
}

fn viewmodel_draws(world: &World, anim: &ViewAnim) -> Vec<LitDraw> {
    if world.state == MatchState::Flyby {
        return Vec::new();
    }
    let Some(p) = world.players.get(world.player_id) else {
        return Vec::new();
    };
    if !p.alive {
        return Vec::new();
    }
    let (shown, drop) = anim.shown();
    // A weapon on its way down has no live cooldown of its own.
    let cooldown = if shown == p.weapon { p.cooldown } else { 0.0 };
    let shot_age = if cooldown > 0.0 { weapon_reload(shown) - cooldown } else { 9.0 };
    let (back, rise, jitter) = recoil(shown, shot_age, world.time);
    let anchor = if shown == 0 { VM_ANCHOR_DISC } else { VM_ANCHOR_BOLT };
    let (yaw, pitch) = if shown == 0 { (VM_DISC_TURN_Y, VM_DISC_TURN_X) } else { (VM_TURN_Y, VM_TURN_X) };
    let offset = anim.bob_offset(world.time) + Vec3::new(0.0, -back * 0.4 - 0.2 * drop, back + 0.05 * drop);
    let base = Mat4::from_translation(anchor + offset)
        * Mat4::from_rotation_y(yaw + jitter)
        * Mat4::from_rotation_x(pitch + rise - drop * 0.45);
    let team = team_accent(p.team);
    let (frame, mut draws) = match shown {
        0 => {
            // Pivot around the muzzle, not the grip: move the rear toward the
            // right edge without moving the launch point or changing ballistics.
            let angled = base * Mat4::from_translation(VM_MUZZLE_DISC)
                * Mat4::from_rotation_y(0.12)
                * Mat4::from_translation(-VM_MUZZLE_DISC);
            (angled, disc_launcher(angled, world.time, cooldown, shot_age, anim.ready_flash, team))
        }
        1 => (base, chaingun(base, anim, shot_age, team)),
        _ => (base, grenade_launcher(base, anim, cooldown, shot_age, team)),
    };
    push_vm_hands(&mut draws, frame, shown, team, p.team);
    draws
}

/// The support hand: a gloved left hand wrapped round the left side of the
/// barrel or rail, with an armored forearm running in from the lower left.
/// The weapon sits at the lower-right screen edge, so this side faces the
/// player; the firing hand on the grip is always off-screen and not drawn.
fn push_vm_hands(draws: &mut Vec<LitDraw>, frame: Mat4, weapon: u8, accent: Vec3, team: Team) {
    let plate = if team == Team::Ember { Vec3::new(0.74, 0.20, 0.12) } else { Vec3::new(0.12, 0.51, 0.65) };
    let glove = Vec3::new(0.07, 0.085, 0.10);
    let alloy = Vec3::new(0.48, 0.55, 0.61);
    // Left-side grip point in the weapon's frame.
    let hand = match weapon {
        0 => Vec3::new(-0.30, -0.02, -0.30),
        1 => Vec3::new(-0.135, -0.01, -0.22),
        _ => Vec3::new(-0.115, -0.01, -0.34),
    };
    let mut part = |mesh, model: Mat4, color: Vec3, glow: f32| {
        draws.push(LitDraw { mesh, model: frame * model, color, emit: glow, mode: 0.0 });
    };
    // Palm against the weapon, fingers curling over the top, knuckle guard.
    part(MeshId::Bevel, Mat4::from_translation(hand) * Mat4::from_scale(Vec3::new(0.08, 0.12, 0.15)), glove, 0.0);
    part(MeshId::Bevel, Mat4::from_translation(hand + Vec3::new(0.045, 0.075, 0.0))
        * Mat4::from_rotation_z(-0.5) * Mat4::from_scale(Vec3::new(0.10, 0.045, 0.14)), glove, 0.0);
    part(MeshId::Bevel, Mat4::from_translation(hand + Vec3::new(-0.045, 0.01, 0.0))
        * Mat4::from_scale(Vec3::new(0.03, 0.10, 0.13)), alloy, 0.0);
    part(MeshId::Cube, Mat4::from_translation(hand + Vec3::new(-0.062, 0.01, 0.0))
        * Mat4::from_scale(Vec3::new(0.004, 0.05, 0.08)), accent, 0.35);
    // Forearm to an elbow off the lower-left of the screen.
    let wrist = hand + Vec3::new(-0.02, -0.05, 0.10);
    let elbow = hand + Vec3::new(-0.36, -0.36, 0.46);
    let axis = elbow - wrist;
    let arm = Mat4::from_translation((wrist + elbow) * 0.5)
        * Mat4::from_quat(glam::Quat::from_rotation_arc(Vec3::Y, axis.normalize()));
    part(MeshId::Armor, arm * Mat4::from_scale(Vec3::new(0.12, axis.length(), 0.13)), alloy, 0.0);
    part(MeshId::Armor, arm * Mat4::from_translation(Vec3::new(0.0, -axis.length() * 0.15, 0.0))
        * Mat4::from_scale(Vec3::new(0.14, axis.length() * 0.5, 0.15)), plate, 0.0);
    part(MeshId::Cube, arm * Mat4::from_translation(Vec3::new(0.0, axis.length() * 0.38, -0.07))
        * Mat4::from_scale(Vec3::new(0.06, 0.03, 0.02)), Vec3::new(0.32, 0.88, 1.0), 0.6);
}

fn along_z(at: Vec3, radius: f32, length: f32) -> Mat4 {
    Mat4::from_translation(at) * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)
        * Mat4::from_scale(Vec3::new(radius, length, radius))
}

/// Rotary chaingun: six barrels in a shroud ring and a vented heat jacket,
/// a rear motor housing and a side ammo drum. The barrels spin up and coast
/// down; the jacket vents glow with (cosmetic) heat.
fn chaingun(base: Mat4, anim: &ViewAnim, shot_age: f32, team: Vec3) -> Vec<LitDraw> {
    let gun = Vec3::new(0.25, 0.28, 0.31);
    let dark = Vec3::new(0.07, 0.08, 0.10);
    let steel = Vec3::new(0.44, 0.48, 0.52);
    let olive = Vec3::new(0.22, 0.25, 0.19);
    let amber = Vec3::new(1.0, 0.55, 0.16);
    let mut draws = Vec::with_capacity(48);
    let mut put = |mesh, model: Mat4, color: Vec3, glow: f32| draws.push(LitDraw { mesh, model: base * model, color, emit: glow, mode: 0.0 });
    let box_at = |at: Vec3, size: Vec3| Mat4::from_translation(at) * Mat4::from_scale(size);
    // Receiver, carry rail and grip.
    put(MeshId::Bevel, box_at(Vec3::new(0.0, -0.01, 0.07), Vec3::new(0.17, 0.15, 0.32)), gun, 0.0);
    put(MeshId::Cube, box_at(Vec3::new(0.0, 0.092, 0.03), Vec3::new(0.05, 0.022, 0.28)), steel, 0.0);
    put(MeshId::Bevel, Mat4::from_translation(Vec3::new(0.0, -0.13, 0.12)) * Mat4::from_rotation_x(0.25)
        * Mat4::from_scale(Vec3::new(0.07, 0.17, 0.10)), dark, 0.0);
    put(MeshId::Cube, box_at(Vec3::new(0.087, -0.005, 0.07), Vec3::new(0.004, 0.04, 0.26)), team, 0.35);
    // Rear motor housing with a status ring.
    put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, 0.24), 0.085, 0.13), dark, 0.0);
    put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, 0.305), 0.06, 0.012), amber, 0.25 + anim.heat * 0.9);
    // Side drum turning with the feed, brass rounds on its rim.
    let drum = Mat4::from_translation(Vec3::new(-0.155, -0.045, 0.03))
        * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2) * Mat4::from_rotation_y(anim.spin * 0.12);
    put(MeshId::Disc, drum * Mat4::from_scale(Vec3::new(0.09, 0.075, 0.09)), olive, 0.0);
    for k in 0..6 {
        let a = k as f32 * std::f32::consts::TAU / 6.0;
        put(MeshId::Cube, drum * Mat4::from_translation(Vec3::new(a.cos() * 0.07, 0.04, a.sin() * 0.07))
            * Mat4::from_scale(Vec3::splat(0.018)), Vec3::new(0.82, 0.62, 0.26), 0.12);
    }
    put(MeshId::Cube, box_at(Vec3::new(-0.1, -0.03, 0.03), Vec3::new(0.05, 0.03, 0.07)), dark, 0.0);
    // Barrel cluster: hub, clamp rings, spinning barrels, bores.
    put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.29), 0.028, 0.56), dark, 0.0);
    put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.09), 0.098, 0.05), steel, 0.0);
    put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.31), 0.094, 0.025), dark, 0.0);
    put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.53), 0.104, 0.035), steel, 0.0);
    for k in 0..6 {
        let a = anim.spin + k as f32 * std::f32::consts::TAU / 6.0;
        let o = Vec3::new(a.cos(), a.sin(), 0.0) * 0.062;
        put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.30) + o, 0.024, 0.54), Vec3::new(0.32, 0.35, 0.38), 0.0);
        put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.572) + o, 0.017, 0.006), Vec3::splat(0.02), 0.0);
    }
    // Vented heat jacket: fixed slats that glow amber as the gun heats.
    let vent = dark + (amber - dark) * anim.heat;
    for k in 0..6 {
        let a = (k as f32 + 0.5) * std::f32::consts::TAU / 6.0;
        let o = Vec3::new(a.cos(), a.sin(), 0.0) * 0.1;
        put(MeshId::Cube, Mat4::from_translation(Vec3::new(0.0, 0.02, -0.31) + o) * Mat4::from_rotation_z(a)
            * Mat4::from_scale(Vec3::new(0.014, 0.024, 0.34)), vent, anim.heat * 1.1);
    }
    // Star-shaped muzzle flash on each round.
    if shot_age < 0.035 {
        let flash = Vec3::new(1.0, 0.8, 0.35);
        let spin = Mat4::from_translation(Vec3::new(0.0, 0.02, -0.63)) * Mat4::from_rotation_z(shot_age * 900.0);
        put(MeshId::Sphere, spin * Mat4::from_scale(Vec3::new(0.16, 0.03, 0.05)), flash, 1.7);
        put(MeshId::Sphere, spin * Mat4::from_scale(Vec3::new(0.03, 0.16, 0.05)), flash, 1.7);
        put(MeshId::Sphere, spin * Mat4::from_scale(Vec3::new(0.06, 0.06, 0.14)), Vec3::new(1.0, 0.95, 0.75), 1.9);
    }
    draws
}

/// Grenade launcher: a wide bore, a four-chamber revolving cylinder with
/// round lights, a pump grip and a stock. The existing reload timer drives a
/// cylinder step and a pump rack; a fresh round rotates up as it completes.
fn grenade_launcher(base: Mat4, anim: &ViewAnim, cooldown: f32, shot_age: f32, team: Vec3) -> Vec<LitDraw> {
    let gun = Vec3::new(0.23, 0.25, 0.27);
    let dark = Vec3::new(0.07, 0.075, 0.08);
    let steel = Vec3::new(0.44, 0.47, 0.50);
    let olive = Vec3::new(0.30, 0.34, 0.19);
    let amber = Vec3::new(1.0, 0.62, 0.18);
    let reload = weapon_reload(2);
    let progress = if cooldown > 0.0 { 1.0 - cooldown / reload } else { 1.0 };
    // The cylinder turns one step over the middle of the reload.
    let step = ((progress - 0.2) / 0.6).clamp(0.0, 1.0);
    let turn = step * step * (3.0 - 2.0 * step);
    let steps = anim.cylinder as f32 - if cooldown > 0.0 { 1.0 - turn } else { 0.0 };
    let angle = steps * std::f32::consts::FRAC_PI_2;
    let pump = (progress * std::f32::consts::PI).sin() * if cooldown > 0.0 { 0.075 } else { 0.0 };
    let mut draws = Vec::with_capacity(32);
    let mut put = |mesh, model: Mat4, color: Vec3, glow: f32| draws.push(LitDraw { mesh, model: base * model, color, emit: glow, mode: 0.0 });
    let box_at = |at: Vec3, size: Vec3| Mat4::from_translation(at) * Mat4::from_scale(size);
    // Receiver, top strap over the cylinder, stock, grip, team stripe.
    put(MeshId::Bevel, box_at(Vec3::new(0.0, -0.01, 0.12), Vec3::new(0.17, 0.16, 0.24)), gun, 0.0);
    put(MeshId::Cube, box_at(Vec3::new(0.0, 0.07, -0.1), Vec3::new(0.05, 0.022, 0.30)), steel, 0.0);
    put(MeshId::Bevel, box_at(Vec3::new(0.0, -0.04, 0.3), Vec3::new(0.08, 0.13, 0.16)), olive, 0.0);
    put(MeshId::Bevel, Mat4::from_translation(Vec3::new(0.0, -0.14, 0.15)) * Mat4::from_rotation_x(0.25)
        * Mat4::from_scale(Vec3::new(0.07, 0.16, 0.10)), dark, 0.0);
    put(MeshId::Cube, box_at(Vec3::new(0.087, -0.01, 0.12), Vec3::new(0.004, 0.04, 0.2)), team, 0.35);
    // Ready light on the sight.
    put(MeshId::Cube, box_at(Vec3::new(0.0, 0.092, 0.02), Vec3::new(0.03, 0.018, 0.03)),
        if cooldown > 0.0 { dark } else { amber }, if cooldown > 0.0 { 0.0 } else { 0.9 });
    // Wide-bore barrel with a muzzle ring.
    put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.40), 0.075, 0.38), gun, 0.0);
    put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.575), 0.088, 0.035), steel, 0.0);
    put(MeshId::Disc, along_z(Vec3::new(0.0, 0.02, -0.594), 0.058, 0.004), Vec3::splat(0.02), 0.0);
    // Pump grip racks back and forward during the reload.
    put(MeshId::Bevel, box_at(Vec3::new(0.0, -0.075, -0.36 + pump), Vec3::new(0.10, 0.06, 0.16)), Vec3::splat(0.11), 0.0);
    // Revolving cylinder: four chambers, the fired one dark until it cycles.
    let cyl = Mat4::from_translation(Vec3::new(0.0, -0.035, -0.09)) * Mat4::from_rotation_z(angle);
    put(MeshId::Disc, cyl * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2) * Mat4::from_scale(Vec3::new(0.112, 0.17, 0.112)), olive, 0.0);
    // Chamber k is on top after the cylinder has stepped n times when
    // k + n = 0 (mod 4). The chamber that just fired stays dark as it turns
    // away; the fresh round brightens as it rotates up into place.
    let steps_done = anim.cylinder % 4;
    let fired_chamber = (4 - (steps_done + 3) % 4) % 4;
    let arriving = (4 - steps_done) % 4;
    for k in 0..4u32 {
        let a = k as f32 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_2;
        let at = Vec3::new(a.cos() * 0.066, a.sin() * 0.066, 0.0);
        put(MeshId::Disc, cyl * along_z(at, 0.03, 0.172), dark, 0.0);
        let reloading = cooldown > 0.0;
        let glow = if reloading && k == fired_chamber { 0.0 }
            else if reloading && k == arriving { 0.2 + 0.7 * turn } else { 0.8 };
        // The round light sits on the cylinder's outer face over its chamber,
        // so the player sees rounds turn past.
        let out = Vec3::new(a.cos(), a.sin(), 0.0);
        put(MeshId::Cube, cyl * Mat4::from_translation(out * 0.114 + Vec3::new(0.0, 0.0, 0.03)) * Mat4::from_rotation_z(a)
            * Mat4::from_scale(Vec3::new(0.008, 0.03, 0.06)), if glow > 0.0 { amber } else { dark }, glow);
    }
    if shot_age < 0.06 {
        let k = 1.0 - shot_age / 0.06;
        put(MeshId::Sphere, box_at(Vec3::new(0.0, 0.02, -0.66), Vec3::new(0.12, 0.12, 0.2) * (0.6 + k * 0.6)),
            Vec3::new(1.0, 0.6, 0.2), 1.7);
    }
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
