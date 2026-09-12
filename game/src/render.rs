use glam::{Mat4, Vec3};
use js_sys::{Float32Array, Uint16Array};
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram,
    WebGlUniformLocation, WebGlVertexArrayObject,
};

use crate::sim::{MatchState, Team, World};
use crate::terrain::{height, MAP};

const VS_WORLD: &str = r#"#version 300 es
layout(location=0) in vec3 a_pos;
layout(location=1) in vec3 a_n;
uniform mat4 u_mvp;
uniform mat4 u_model;
out vec3 v_w;
out vec3 v_n;
void main(){
  vec4 w = u_model * vec4(a_pos,1.0);
  v_w = w.xyz;
  v_n = mat3(u_model) * a_n;
  gl_Position = u_mvp * vec4(a_pos,1.0);
}
"#;

const FS_WORLD: &str = r#"#version 300 es
precision highp float;
in vec3 v_w;
in vec3 v_n;
uniform vec3 u_cam;
uniform vec3 u_sun;
uniform vec3 u_fog;
uniform vec3 u_color;
uniform float u_emit;
uniform float u_mode;
out vec4 frag;
void main(){
  vec3 n = normalize(v_n);
  float ndl = max(dot(n, u_sun), 0.0);
  vec3 albedo = u_color;
  if(u_mode > 0.5){
    float slope = 1.0 - n.y;
    vec3 rock = vec3(0.27, 0.25, 0.23);
    vec3 snow = vec3(0.88, 0.91, 0.95);
    vec3 ice  = vec3(0.42, 0.55, 0.64);
    float snowAmt = smoothstep(0.42, 0.12, slope) * smoothstep(8.0, 20.0, v_w.y);
    float iceAmt  = smoothstep(14.0, 3.0, v_w.y);
    albedo = mix(mix(rock, ice, iceAmt * 0.55), snow, snowAmt);
    albedo *= 0.92 + 0.08 * fract(sin(dot(v_w.xz, vec2(12.9898,78.233))) * 43758.5453);
  }
  vec3 col = albedo * (0.22 + 0.78 * ndl);
  col += albedo * vec3(0.12, 0.16, 0.22) * max(n.y, 0.0);
  col += u_color * u_emit;
  float dist = length(u_cam - v_w);
  float fog = 1.0 - exp(-dist * 0.0072);
  col = mix(col, u_fog, clamp(fog, 0.0, 0.92));
  frag = vec4(col, 1.0);
}
"#;

const VS_SKY: &str = r#"#version 300 es
layout(location=0) in vec2 a_pos;
out vec2 v_uv;
void main(){
  v_uv = a_pos;
  gl_Position = vec4(a_pos, 0.999, 1.0);
}
"#;

const FS_SKY: &str = r#"#version 300 es
precision highp float;
in vec2 v_uv;
uniform mat4 u_inv;
uniform vec3 u_cam;
uniform vec3 u_sun;
out vec4 frag;
void main(){
  vec4 far = u_inv * vec4(v_uv, 1.0, 1.0);
  vec3 dir = normalize(far.xyz / far.w - u_cam);
  float h = dir.y;
  vec3 zenith = vec3(0.05, 0.08, 0.14);
  vec3 horizon = vec3(0.62, 0.48, 0.38);
  vec3 ground = vec3(0.18, 0.20, 0.24);
  vec3 col = mix(horizon, zenith, smoothstep(-0.02, 0.55, h));
  col = mix(ground, col, smoothstep(-0.18, 0.04, h));
  float sun = pow(max(dot(dir, u_sun), 0.0), 180.0);
  col += vec3(1.0, 0.86, 0.62) * sun * 1.6;
  col += vec3(1.0, 0.55, 0.28) * pow(max(dot(dir, u_sun), 0.0), 6.0) * 0.18;
  frag = vec4(col, 1.0);
}
"#;

const VS_EMIT: &str = r#"#version 300 es
layout(location=0) in vec3 a_pos;
uniform mat4 u_mvp;
void main(){ gl_Position = u_mvp * vec4(a_pos,1.0); }
"#;

const FS_EMIT: &str = r#"#version 300 es
precision highp float;
uniform vec4 u_color;
out vec4 frag;
void main(){ frag = u_color; }
"#;

struct Mesh {
    vao: WebGlVertexArrayObject,
    _vbo: WebGlBuffer,
    _ibo: Option<WebGlBuffer>,
    count: i32,
}

struct Prog {
    p: WebGlProgram,
    u_mvp: Option<WebGlUniformLocation>,
    u_model: Option<WebGlUniformLocation>,
    u_cam: Option<WebGlUniformLocation>,
    u_sun: Option<WebGlUniformLocation>,
    u_fog: Option<WebGlUniformLocation>,
    u_color: Option<WebGlUniformLocation>,
    u_emit: Option<WebGlUniformLocation>,
    u_mode: Option<WebGlUniformLocation>,
    u_inv: Option<WebGlUniformLocation>,
}

pub struct Renderer {
    gl: GL,
    world: Prog,
    sky: Prog,
    emit: Prog,
    terrain: Mesh,
    cube: Mesh,
    sphere: Mesh,
    disc: Mesh,
    quad: Mesh,
    width: u32,
    height: u32,
    snow: Vec<Vec3>,
}

fn shader(gl: &GL, ty: u32, src: &str) -> Result<web_sys::WebGlShader, String> {
    let s = gl.create_shader(ty).ok_or("shader")?;
    gl.shader_source(&s, src);
    gl.compile_shader(&s);
    if !gl
        .get_shader_parameter(&s, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        return Err(gl.get_shader_info_log(&s).unwrap_or_else(|| "compile".into()));
    }
    Ok(s)
}

fn program(gl: &GL, vs: &str, fs: &str) -> Result<WebGlProgram, String> {
    let p = gl.create_program().ok_or("program")?;
    let v = shader(gl, GL::VERTEX_SHADER, vs)?;
    let f = shader(gl, GL::FRAGMENT_SHADER, fs)?;
    gl.attach_shader(&p, &v);
    gl.attach_shader(&p, &f);
    gl.link_program(&p);
    if !gl
        .get_program_parameter(&p, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        return Err(gl.get_program_info_log(&p).unwrap_or_else(|| "link".into()));
    }
    Ok(p)
}

fn loc(gl: &GL, p: &WebGlProgram, name: &str) -> Option<WebGlUniformLocation> {
    gl.get_uniform_location(p, name)
}

fn make_prog(gl: &GL, vs: &str, fs: &str) -> Result<Prog, String> {
    let p = program(gl, vs, fs)?;
    Ok(Prog {
        u_mvp: loc(gl, &p, "u_mvp"),
        u_model: loc(gl, &p, "u_model"),
        u_cam: loc(gl, &p, "u_cam"),
        u_sun: loc(gl, &p, "u_sun"),
        u_fog: loc(gl, &p, "u_fog"),
        u_color: loc(gl, &p, "u_color"),
        u_emit: loc(gl, &p, "u_emit"),
        u_mode: loc(gl, &p, "u_mode"),
        u_inv: loc(gl, &p, "u_inv"),
        p,
    })
}

fn upload_mesh(gl: &GL, verts: &[f32], idx: Option<&[u16]>, stride: i32) -> Result<Mesh, String> {
    let vao = gl.create_vertex_array().ok_or("vao")?;
    gl.bind_vertex_array(Some(&vao));
    let vbo = gl.create_buffer().ok_or("vbo")?;
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vbo));
    let va = Float32Array::new_with_length(verts.len() as u32);
    va.copy_from(verts);
    gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &va, GL::STATIC_DRAW);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_with_i32(0, 3, GL::FLOAT, false, stride, 0);
    if stride >= 24 {
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_with_i32(1, 3, GL::FLOAT, false, stride, 12);
    }
    let (ibo, count) = if let Some(ix) = idx {
        let b = gl.create_buffer().ok_or("ibo")?;
        gl.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, Some(&b));
        let ia = Uint16Array::new_with_length(ix.len() as u32);
        ia.copy_from(ix);
        gl.buffer_data_with_array_buffer_view(GL::ELEMENT_ARRAY_BUFFER, &ia, GL::STATIC_DRAW);
        (Some(b), ix.len() as i32)
    } else {
        (None, (verts.len() as i32) / (stride / 4))
    };
    gl.bind_vertex_array(None);
    Ok(Mesh {
        vao,
        _vbo: vbo,
        _ibo: ibo,
        count,
    })
}

fn cube_mesh() -> (Vec<f32>, Vec<u16>) {
    let faces: [([f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5], [0.0, 0.0, 1.0]),
        ([0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5], [0.0, 0.0, -1.0]),
        ([-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5], [-1.0, 0.0, 0.0]),
        ([0.5, -0.5, 0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5], [1.0, 0.0, 0.0]),
        ([-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5], [0.0, 1.0, 0.0]),
        ([-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5], [0.0, -1.0, 0.0]),
    ];
    let mut v = Vec::new();
    let mut i = Vec::new();
    for (fi, (a, b, c, d, n)) in faces.iter().enumerate() {
        let base = (fi * 4) as u16;
        for p in [a, b, c, d] {
            v.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2]]);
        }
        i.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (v, i)
}

fn sphere_mesh(lat: u32, lon: u32) -> (Vec<f32>, Vec<u16>) {
    let mut v = Vec::new();
    let mut idx = Vec::new();
    for y in 0..=lat {
        let p = y as f32 / lat as f32;
        let th = p * std::f32::consts::PI;
        let sy = th.cos();
        let r = th.sin();
        for x in 0..=lon {
            let q = x as f32 / lon as f32;
            let ph = q * std::f32::consts::TAU;
            let px = r * ph.cos();
            let pz = r * ph.sin();
            v.extend_from_slice(&[px, sy, pz, px, sy, pz]);
        }
    }
    let stride = lon + 1;
    for y in 0..lat {
        for x in 0..lon {
            let i = (y * stride + x) as u16;
            let s = stride as u16;
            idx.extend_from_slice(&[i, i + s, i + 1, i + 1, i + s, i + s + 1]);
        }
    }
    (v, idx)
}

fn disc_mesh() -> (Vec<f32>, Vec<u16>) {
    let n = 24u16;
    let mut v = vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        v.extend_from_slice(&[a.cos(), 0.0, a.sin(), 0.0, 1.0, 0.0]);
    }
    let mut idx = Vec::new();
    for i in 0..n {
        let a = 1 + i;
        let b = 1 + (i + 1) % n;
        idx.extend_from_slice(&[0, a, b]);
    }
    // underside
    let base = (n + 1) as usize;
    v.extend_from_slice(&[0.0, -0.08, 0.0, 0.0, -1.0, 0.0]);
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        v.extend_from_slice(&[a.cos(), -0.08, a.sin(), 0.0, -1.0, 0.0]);
    }
    let b0 = base as u16;
    for i in 0..n {
        let a = b0 + 1 + i;
        let c = b0 + 1 + (i + 1) % n;
        idx.extend_from_slice(&[b0, c, a]);
    }
    (v, idx)
}

fn sky_quad() -> (Vec<f32>, Vec<u16>) {
    (
        vec![-1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0],
        vec![0, 1, 2, 0, 2, 3],
    )
}

impl Renderer {
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Self, String> {
        let gl: GL = canvas
            .get_context("webgl2")
            .map_err(|_| "ctx")?
            .ok_or("webgl2 unavailable")?
            .dyn_into()
            .map_err(|_| "cast")?;
        gl.enable(GL::DEPTH_TEST);
        gl.enable(GL::CULL_FACE);
        gl.depth_func(GL::LEQUAL);
        gl.clear_color(0.07, 0.10, 0.16, 1.0);

        let world = make_prog(&gl, VS_WORLD, FS_WORLD)?;
        let sky = make_prog(&gl, VS_SKY, FS_SKY)?;
        let emit = make_prog(&gl, VS_EMIT, FS_EMIT)?;

        let (tv, ti) = crate::terrain::sample_mesh();
        let terrain = upload_mesh(&gl, &tv, Some(&ti), 24)?;
        let (cv, ci) = cube_mesh();
        let cube = upload_mesh(&gl, &cv, Some(&ci), 24)?;
        let (sv, si) = sphere_mesh(10, 16);
        let sphere = upload_mesh(&gl, &sv, Some(&si), 24)?;
        let (dv, di) = disc_mesh();
        let disc = upload_mesh(&gl, &dv, Some(&di), 24)?;
        let (qv, qi) = sky_quad();
        // sky uses 2-float verts packed as xyz with z=0; attribute 0 is vec3 so we upload as 12-byte
        let quad = upload_mesh(&gl, &qv, Some(&qi), 12)?;

        let mut snow = Vec::new();
        let mut rng = 0x91A2u32;
        for _ in 0..90 {
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = (rng >> 8) as f32 / 16777216.0;
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let y = (rng >> 8) as f32 / 16777216.0;
            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let z = (rng >> 8) as f32 / 16777216.0;
            snow.push(Vec3::new(x * 40.0 - 20.0, y * 22.0, z * 40.0 - 20.0));
        }

        Ok(Self {
            gl,
            world,
            sky,
            emit,
            terrain,
            cube,
            sphere,
            disc,
            quad,
            width: 1,
            height: 1,
            snow,
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.width = w;
        self.height = h;
        self.gl.viewport(0, 0, w as i32, h as i32);
    }

    fn u_mat(gl: &GL, loc: &Option<WebGlUniformLocation>, m: &Mat4) {
        if let Some(l) = loc {
            gl.uniform_matrix4fv_with_f32_array(Some(l), false, &m.to_cols_array());
        }
    }
    fn u_vec3(gl: &GL, loc: &Option<WebGlUniformLocation>, v: Vec3) {
        if let Some(l) = loc {
            gl.uniform3f(Some(l), v.x, v.y, v.z);
        }
    }

    fn draw_mesh(&self, mesh: &Mesh, indexed: bool) {
        self.gl.bind_vertex_array(Some(&mesh.vao));
        if indexed {
            self.gl
                .draw_elements_with_i32(GL::TRIANGLES, mesh.count, GL::UNSIGNED_SHORT, 0);
        } else {
            self.gl.draw_arrays(GL::TRIANGLES, 0, mesh.count);
        }
    }

    fn draw_lit(&self, pv: &Mat4, model: Mat4, color: Vec3, emit: f32, mode: f32, mesh: &Mesh) {
        let mvp = *pv * model;
        self.gl.use_program(Some(&self.world.p));
        Self::u_mat(&self.gl, &self.world.u_mvp, &mvp);
        Self::u_mat(&self.gl, &self.world.u_model, &model);
        Self::u_vec3(&self.gl, &self.world.u_color, color);
        if let Some(l) = &self.world.u_emit {
            self.gl.uniform1f(Some(l), emit);
        }
        if let Some(l) = &self.world.u_mode {
            self.gl.uniform1f(Some(l), mode);
        }
        self.draw_mesh(mesh, true);
    }

    fn draw_emit(&self, pv: &Mat4, model: Mat4, color: [f32; 4], mesh: &Mesh) {
        let mvp = *pv * model;
        self.gl.use_program(Some(&self.emit.p));
        Self::u_mat(&self.gl, &self.emit.u_mvp, &mvp);
        if let Some(l) = &self.emit.u_color {
            self.gl.uniform4f(Some(l), color[0], color[1], color[2], color[3]);
        }
        self.draw_mesh(mesh, true);
    }

    pub fn draw(&mut self, world: &World) {
        let gl = &self.gl;
        let aspect = self.width.max(1) as f32 / self.height.max(1) as f32;
        let (eye, dir, fov) = world.camera();
        let view = Mat4::look_to_rh(eye, dir, Vec3::Y);
        let proj = Mat4::perspective_rh(fov.to_radians(), aspect, 0.14, 480.0);
        let pv = proj * view;
        let sun = Vec3::new(-0.35, 0.78, -0.42).normalize();
        let fog = Vec3::new(0.55, 0.46, 0.38);

        gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);

        // Sky
        gl.disable(GL::DEPTH_TEST);
        gl.use_program(Some(&self.sky.p));
        let inv = pv.inverse();
        Self::u_mat(gl, &self.sky.u_inv, &inv);
        Self::u_vec3(gl, &self.sky.u_cam, eye);
        Self::u_vec3(gl, &self.sky.u_sun, sun);
        gl.bind_vertex_array(Some(&self.quad.vao));
        gl.draw_elements_with_i32(GL::TRIANGLES, self.quad.count, GL::UNSIGNED_SHORT, 0);
        gl.enable(GL::DEPTH_TEST);

        // Terrain
        gl.use_program(Some(&self.world.p));
        Self::u_vec3(gl, &self.world.u_cam, eye);
        Self::u_vec3(gl, &self.world.u_sun, sun);
        Self::u_vec3(gl, &self.world.u_fog, fog);
        self.draw_lit(&pv, Mat4::IDENTITY, Vec3::ONE, 0.0, 1.0, &self.terrain);

        // Pillars
        for p in &world.pillars {
            let y = height(p.x, p.z);
            let model = Mat4::from_translation(Vec3::new(p.x, y + p.h * 0.5, p.z))
                * Mat4::from_scale(Vec3::new(p.r * 1.6, p.h, p.r * 1.6));
            self.draw_lit(&pv, model, Vec3::new(0.32, 0.30, 0.28), 0.0, 0.0, &self.cube);
        }

        // Bases
        self.draw_base(&pv, true);
        self.draw_base(&pv, false);

        // Flags
        for f in &world.flags {
            let color = if f.team == Team::Ember {
                Vec3::new(0.89, 0.29, 0.20)
            } else {
                Vec3::new(0.24, 0.78, 0.88)
            };
            let pole = Mat4::from_translation(f.pos + Vec3::Y * 1.7)
                * Mat4::from_scale(Vec3::new(0.12, 3.4, 0.12));
            self.draw_lit(&pv, pole, Vec3::new(0.15, 0.16, 0.18), 0.0, 0.0, &self.cube);
            let to = {
                let mut d = eye - f.pos;
                d.y = 0.0;
                d.normalize_or_zero()
            };
            let right = to.cross(Vec3::Y).normalize_or_zero();
            let banner = Mat4::from_cols(
                (right * 1.4).extend(0.0),
                (Vec3::Y * 1.1).extend(0.0),
                (to * 0.08).extend(0.0),
                (f.pos + Vec3::Y * 2.7 + right * 0.7).extend(1.0),
            );
            self.draw_lit(&pv, banner, color, 0.35, 0.0, &self.cube);
        }

        // Players
        for (i, p) in world.players.iter().enumerate() {
            if !p.alive {
                continue;
            }
            if i == world.player_id && world.state != MatchState::Flyby {
                if p.jetting {
                    self.draw_jet(&pv, p.pos, p.team);
                }
                continue;
            }
            let body_c = if p.team == Team::Ember {
                Vec3::new(0.72, 0.18, 0.14)
            } else {
                Vec3::new(0.16, 0.58, 0.70)
            };
            let yaw = p.yaw;
            let rot = Mat4::from_rotation_y(yaw);
            let body = Mat4::from_translation(p.pos + Vec3::Y * 0.85)
                * rot
                * Mat4::from_scale(Vec3::new(0.72, 1.15, 0.58));
            self.draw_lit(&pv, body, body_c, 0.05, 0.0, &self.cube);
            let head = Mat4::from_translation(p.pos + Vec3::Y * 1.62)
                * rot
                * Mat4::from_scale(Vec3::new(0.42, 0.34, 0.42));
            self.draw_lit(&pv, head, Vec3::new(0.12, 0.13, 0.15), 0.0, 0.0, &self.cube);
            if p.carrying.is_some() {
                let c = if p.team == Team::Ember {
                    Vec3::new(0.24, 0.78, 0.88)
                } else {
                    Vec3::new(0.89, 0.29, 0.20)
                };
                let fl = Mat4::from_translation(p.pos + Vec3::Y * 2.25)
                    * Mat4::from_scale(Vec3::new(0.35, 0.7, 0.08));
                self.draw_lit(&pv, fl, c, 0.4, 0.0, &self.cube);
            }
            if p.jetting {
                self.draw_jet(&pv, p.pos, p.team);
            }
        }

        // Discs
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
            self.draw_lit(&pv, model, color, 0.9, 0.0, &self.disc);
            let glow = Mat4::from_translation(d.pos) * Mat4::from_scale(Vec3::splat(if d.kind == 0 { 0.35 } else { 0.16 }));
            gl.enable(GL::BLEND);
            gl.blend_func(GL::SRC_ALPHA, GL::ONE);
            gl.depth_mask(false);
            self.draw_emit(
                &pv,
                glow,
                [color.x, color.y, color.z, 0.45],
                &self.sphere,
            );
            gl.depth_mask(true);
            gl.disable(GL::BLEND);
        }

        // Explosions
        gl.enable(GL::BLEND);
        gl.blend_func(GL::SRC_ALPHA, GL::ONE);
        gl.depth_mask(false);
        for e in &world.explosions {
            let t = (e.age / 0.55).clamp(0.0, 1.0);
            let r = e.max_r * (0.2 + t * 0.9);
            let a = (1.0 - t) * 0.7;
            let model = Mat4::from_translation(e.pos) * Mat4::from_scale(Vec3::splat(r));
            self.draw_emit(&pv, model, [1.0, 0.55, 0.18, a], &self.sphere);
        }
        gl.depth_mask(true);
        gl.disable(GL::BLEND);

        // Snow
        let eye_s = eye;
        for s in &mut self.snow {
            s.y -= 4.2 * 0.016;
            s.x += 0.4 * 0.016;
            let w = eye_s + *s;
            if w.y < height(w.x, w.z) + 0.4 || (w - eye_s).length() > 24.0 {
                *s = Vec3::new(
                    ((s.x + 17.0) % 40.0) - 20.0,
                    10.0 + ((s.z.abs() * 3.0) % 8.0),
                    ((s.z + 11.0) % 40.0) - 20.0,
                );
            }
        }
        let flakes: Vec<Vec3> = self.snow.iter().map(|s| eye + *s).collect();
        for w in flakes {
            let model = Mat4::from_translation(w) * Mat4::from_scale(Vec3::splat(0.045));
            self.draw_lit(&pv, model, Vec3::splat(0.92), 0.15, 0.0, &self.cube);
        }

        // Viewmodel
        if world.state != MatchState::Flyby {
            if let Some(p) = world.players.get(world.player_id) {
                if p.alive {
                    self.draw_viewmodel(&proj, p.weapon, p.cooldown, world.time);
                }
            }
        }

        gl.bind_vertex_array(None);
    }

    fn draw_jet(&self, pv: &Mat4, pos: Vec3, team: Team) {
        let c = if team == Team::Ember {
            [1.0, 0.42, 0.12, 0.55]
        } else {
            [0.3, 0.85, 1.0, 0.55]
        };
        self.gl.enable(GL::BLEND);
        self.gl.blend_func(GL::SRC_ALPHA, GL::ONE);
        self.gl.depth_mask(false);
        let model = Mat4::from_translation(pos + Vec3::new(0.0, 0.1, 0.0))
            * Mat4::from_scale(Vec3::new(0.28, 0.7, 0.28));
        self.draw_emit(pv, model, c, &self.sphere);
        self.gl.depth_mask(true);
        self.gl.disable(GL::BLEND);
    }

    fn draw_base(&self, pv: &Mat4, ember: bool) {
        let home = if ember {
            crate::terrain::EMBER_HOME
        } else {
            crate::terrain::GLACIER_HOME
        };
        let y = height(home.x, home.z);
        let accent = if ember {
            Vec3::new(0.78, 0.22, 0.16)
        } else {
            Vec3::new(0.18, 0.62, 0.74)
        };
        let steel = Vec3::new(0.18, 0.20, 0.23);
        let pad = Mat4::from_translation(Vec3::new(home.x, y + 0.18, home.z))
            * Mat4::from_scale(Vec3::new(16.0, 0.4, 16.0));
        self.draw_lit(pv, pad, steel, 0.0, 0.0, &self.cube);
        let tower = Mat4::from_translation(Vec3::new(home.x + 6.5, y + 5.5, home.z + if ember { 4.0 } else { -4.0 }))
            * Mat4::from_scale(Vec3::new(2.4, 11.0, 2.4));
        self.draw_lit(pv, tower, steel, 0.0, 0.0, &self.cube);
        let cap = Mat4::from_translation(Vec3::new(home.x + 6.5, y + 11.2, home.z + if ember { 4.0 } else { -4.0 }))
            * Mat4::from_scale(Vec3::new(3.0, 0.5, 3.0));
        self.draw_lit(pv, cap, accent, 0.25, 0.0, &self.cube);
        for k in 0..3 {
            let a = k as f32 * 2.1;
            let bx = home.x + a.cos() * 7.5;
            let bz = home.z + a.sin() * 7.5;
            let gen = Mat4::from_translation(Vec3::new(bx, y + 1.1, bz))
                * Mat4::from_scale(Vec3::new(1.6, 2.2, 1.6));
            self.draw_lit(pv, gen, steel * 1.15, 0.0, 0.0, &self.cube);
        }
        let _ = MAP;
    }

    fn draw_viewmodel(&self, proj: &Mat4, weapon: u8, cd: f32, time: f32) {
        let gl = &self.gl;
        gl.clear(GL::DEPTH_BUFFER_BIT);
        let recoil = (cd / if weapon == 0 { 1.05 } else { 0.16 }).clamp(0.0, 1.0);
        let kick = recoil * recoil;
        let sway = (time * 1.4).sin() * 0.012;
        let base = Mat4::from_translation(Vec3::new(0.32 + sway, -0.28 - kick * 0.05, -0.62 + kick * 0.08))
            * Mat4::from_rotation_y(0.18)
            * Mat4::from_rotation_x(-0.08 + kick * 0.12);
        // Identity view: model is already in view space, so mvp = proj * model
        let body_c = Vec3::new(0.16, 0.17, 0.19);
        let body = base * Mat4::from_scale(Vec3::new(0.18, 0.16, 0.55));
        self.draw_vm(proj, body, body_c, 0.0);
        let barrel = base
            * Mat4::from_translation(Vec3::new(0.0, 0.02, -0.38))
            * Mat4::from_scale(Vec3::new(0.07, 0.07, 0.42));
        self.draw_vm(proj, barrel, Vec3::new(0.12, 0.12, 0.13), 0.0);
        if weapon == 0 {
            let disc = base
                * Mat4::from_translation(Vec3::new(0.0, 0.08, -0.12))
                * Mat4::from_rotation_x(1.2)
                * Mat4::from_scale(Vec3::new(0.16, 0.16, 0.03));
            self.draw_vm(proj, disc, Vec3::new(1.0, 0.5, 0.15), 0.6);
        } else {
            let mag = base
                * Mat4::from_translation(Vec3::new(0.0, -0.12, 0.05))
                * Mat4::from_scale(Vec3::new(0.08, 0.18, 0.14));
            self.draw_vm(proj, mag, Vec3::new(0.22, 0.23, 0.25), 0.0);
        }
        let sight = base
            * Mat4::from_translation(Vec3::new(0.0, 0.12, -0.05))
            * Mat4::from_scale(Vec3::new(0.03, 0.06, 0.08));
        self.draw_vm(proj, sight, Vec3::new(0.85, 0.25, 0.16), 0.3);
    }

    fn draw_vm(&self, proj: &Mat4, model: Mat4, color: Vec3, emit: f32) {
        self.gl.use_program(Some(&self.world.p));
        let mvp = *proj * model;
        Self::u_mat(&self.gl, &self.world.u_mvp, &mvp);
        Self::u_mat(&self.gl, &self.world.u_model, &model);
        Self::u_vec3(&self.gl, &self.world.u_cam, Vec3::ZERO);
        Self::u_vec3(&self.gl, &self.world.u_sun, Vec3::new(0.2, 0.8, 0.5).normalize());
        Self::u_vec3(&self.gl, &self.world.u_fog, Vec3::ZERO);
        Self::u_vec3(&self.gl, &self.world.u_color, color);
        if let Some(l) = &self.world.u_emit {
            self.gl.uniform1f(Some(l), emit);
        }
        if let Some(l) = &self.world.u_mode {
            self.gl.uniform1f(Some(l), 0.0);
        }
        // Fog would wash the gun — set density via fog color 0. Already fog mixes with black at dist 0 = no fog.
        self.draw_mesh(&self.cube, true);
    }
}
