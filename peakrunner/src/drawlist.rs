use glam::{Mat3, Mat4, Vec3};

use crate::sim::{MatchState, Team, World};
use crate::terrain::{self, height, MAP};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MeshId {
    Terrain = 0,
    Cube = 1,
    Sphere = 2,
    Disc = 3,
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
    let proj_gl = Mat4::perspective_rh(fov.to_radians(), aspect.max(0.1), 0.14, 480.0);
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
        mode: 1.0,
    });

    for p in &world.pillars {
        let y = height(p.x, p.z);
        lit.push(LitDraw {
            mesh: MeshId::Cube,
            model: Mat4::from_translation(Vec3::new(p.x, y + p.h * 0.5, p.z))
                * Mat4::from_scale(Vec3::new(p.r * 1.6, p.h, p.r * 1.6)),
            color: Vec3::new(0.32, 0.30, 0.28),
            emit: 0.0,
            mode: 0.0,
        });
    }

    push_base(&mut lit, true);
    push_base(&mut lit, false);

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
        let color = if d.kind == 0 {
            Vec3::new(1.0, 0.55, 0.18)
        } else {
            Vec3::new(0.7, 0.95, 1.0)
        };
        let f = d.vel.normalize_or_zero();
        let r = if f.length_squared() < 0.01 {
            Vec3::X
        } else {
            f.cross(Vec3::Y).normalize_or_zero()
        };
        let u = r.cross(f).normalize_or_zero();
        let scale = if d.kind == 0 { 0.55 } else { 0.18 };
        let model = Mat4::from_cols(
            (r * scale).extend(0.0),
            (u * 0.08).extend(0.0),
            (f * scale).extend(0.0),
            d.pos.extend(1.0),
        );
        lit.push(LitDraw {
            mesh: MeshId::Disc,
            model,
            color,
            emit: 0.9,
            mode: 0.0,
        });
        let glow = if d.kind == 0 { 0.35 } else { 0.16 };
        emit.push(EmitDraw {
            mesh: MeshId::Sphere,
            model: Mat4::from_translation(d.pos) * Mat4::from_scale(Vec3::splat(glow)),
            color: [color.x, color.y, color.z, 0.45],
        });
    }

    for e in &world.explosions {
        let t = (e.age / 0.55).clamp(0.0, 1.0);
        let r = e.max_r * (0.2 + t * 0.9);
        let a = (1.0 - t) * 0.7;
        emit.push(EmitDraw {
            mesh: MeshId::Sphere,
            model: Mat4::from_translation(e.pos) * Mat4::from_scale(Vec3::splat(r)),
            color: [1.0, 0.55, 0.18, a],
        });
    }

    let viewmodel = viewmodel_draws(world);

    let _ = MAP;
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
        vm_proj: proj,
        dt,
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

fn push_base(lit: &mut Vec<LitDraw>, ember: bool) {
    let home = if ember {
        terrain::EMBER_HOME
    } else {
        terrain::GLACIER_HOME
    };
    let y = height(home.x, home.z);
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
    let recoil = (p.cooldown / if p.weapon == 0 { 1.05 } else { 0.16 }).clamp(0.0, 1.0);
    let kick = recoil * recoil;
    let sway = (world.time * 1.4).sin() * 0.012;
    let base = Mat4::from_translation(Vec3::new(
        0.32 + sway,
        -0.28 - kick * 0.05,
        -0.62 + kick * 0.08,
    )) * Mat4::from_rotation_y(0.18)
        * Mat4::from_rotation_x(-0.08 + kick * 0.12);
    let mut draws = vec![
        LitDraw {
            mesh: MeshId::Cube,
            model: base * Mat4::from_scale(Vec3::new(0.18, 0.16, 0.55)),
            color: Vec3::new(0.16, 0.17, 0.19),
            emit: 0.0,
            mode: 0.0,
        },
        LitDraw {
            mesh: MeshId::Cube,
            model: base
                * Mat4::from_translation(Vec3::new(0.0, 0.02, -0.38))
                * Mat4::from_scale(Vec3::new(0.07, 0.07, 0.42)),
            color: Vec3::new(0.12, 0.12, 0.13),
            emit: 0.0,
            mode: 0.0,
        },
    ];
    if p.weapon == 0 {
        draws.push(LitDraw {
            mesh: MeshId::Disc,
            model: base
                * Mat4::from_translation(Vec3::new(0.0, 0.08, -0.12))
                * Mat4::from_rotation_x(1.2)
                * Mat4::from_scale(Vec3::new(0.16, 0.16, 0.03)),
            color: Vec3::new(1.0, 0.5, 0.15),
            emit: 0.6,
            mode: 0.0,
        });
    } else {
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
            * Mat4::from_translation(Vec3::new(0.0, 0.12, -0.05))
            * Mat4::from_scale(Vec3::new(0.03, 0.06, 0.08)),
        color: Vec3::new(0.85, 0.25, 0.16),
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
