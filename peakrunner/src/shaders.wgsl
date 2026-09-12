struct WorldUniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
    normal: mat3x3<f32>,
    cam: vec3<f32>,
    emit: f32,
    sun: vec3<f32>,
    mode: f32,
    fog: vec3<f32>,
    pad0: f32,
    color: vec3<f32>,
    pad1: f32,
}

struct SkyUniforms {
    inv_vp: mat4x4<f32>,
    cam: vec3<f32>,
    pad0: f32,
    sun: vec3<f32>,
    pad1: f32,
}

struct EmitUniforms {
    mvp: mat4x4<f32>,
    color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> world: WorldUniforms;
@group(1) @binding(0) var<uniform> sky: SkyUniforms;
@group(2) @binding(0) var<uniform> emit_u: EmitUniforms;
@group(3) @binding(0) var scene_tex: texture_2d<f32>;
@group(3) @binding(1) var scene_samp: sampler;

struct LitIn {
    @location(0) pos: vec3<f32>,
    @location(1) n: vec3<f32>,
}

struct LitOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) n: vec3<f32>,
}

@vertex
fn vs_world(in: LitIn) -> LitOut {
    let wp = world.model * vec4<f32>(in.pos, 1.0);
    var out: LitOut;
    out.world_pos = wp.xyz;
    out.n = world.normal * in.n;
    out.clip = world.mvp * vec4<f32>(in.pos, 1.0);
    return out;
}

@fragment
fn fs_world(in: LitOut) -> @location(0) vec4<f32> {
    let n = normalize(in.n);
    let ndl = max(dot(n, world.sun), 0.0);
    var albedo = world.color;
    if (world.mode > 0.5) {
        let slope = 1.0 - n.y;
        let rock = vec3<f32>(0.27, 0.25, 0.23);
        let snow = vec3<f32>(0.88, 0.91, 0.95);
        let ice = vec3<f32>(0.42, 0.55, 0.64);
        let snow_amt = smoothstep(0.42, 0.12, slope) * smoothstep(8.0, 20.0, in.world_pos.y);
        let ice_amt = smoothstep(14.0, 3.0, in.world_pos.y);
        albedo = mix(mix(rock, ice, ice_amt * 0.55), snow, snow_amt);
        let grain = fract(sin(dot(in.world_pos.xz, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        albedo = albedo * (0.92 + 0.08 * grain);
    }
    var col = albedo * (0.22 + 0.78 * ndl);
    col = col + albedo * vec3<f32>(0.12, 0.16, 0.22) * max(n.y, 0.0);
    col = col + world.color * world.emit;
    let dist = length(world.cam - in.world_pos);
    let fog = clamp(1.0 - exp(-dist * 0.0072), 0.0, 0.92);
    col = mix(col, world.fog, fog);
    return vec4<f32>(col, 1.0);
}

struct FullOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> FullOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = positions[i];
    var out: FullOut;
    out.clip = vec4<f32>(p, 0.999, 1.0);
    out.ndc = p;
    return out;
}

@fragment
fn fs_sky(in: FullOut) -> @location(0) vec4<f32> {
    let far = sky.inv_vp * vec4<f32>(in.ndc, 1.0, 1.0);
    let dir = normalize(far.xyz / far.w - sky.cam);
    let h = dir.y;
    let zenith = vec3<f32>(0.05, 0.08, 0.14);
    let horizon = vec3<f32>(0.62, 0.48, 0.38);
    let ground = vec3<f32>(0.18, 0.20, 0.24);
    var col = mix(horizon, zenith, smoothstep(-0.02, 0.55, h));
    col = mix(ground, col, smoothstep(-0.18, 0.04, h));
    let sun_d = max(dot(dir, sky.sun), 0.0);
    col = col + vec3<f32>(1.0, 0.86, 0.62) * pow(sun_d, 180.0) * 1.6;
    col = col + vec3<f32>(1.0, 0.55, 0.28) * pow(sun_d, 6.0) * 0.18;
    return vec4<f32>(col, 1.0);
}

@vertex
fn vs_emit(in: LitIn) -> @builtin(position) vec4<f32> {
    let _keep = in.n.x;
    return emit_u.mvp * vec4<f32>(in.pos + vec3<f32>(_keep * 0.0, 0.0, 0.0), 1.0);
}

@fragment
fn fs_emit() -> @location(0) vec4<f32> {
    return emit_u.color;
}

fn blit_uv(ndc: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
}

@fragment
fn fs_blit(in: FullOut) -> @location(0) vec4<f32> {
    let c = textureSample(scene_tex, scene_samp, blit_uv(in.ndc));
    return vec4<f32>(c.rgb, 1.0);
}

@fragment
fn fs_blit_srgb(in: FullOut) -> @location(0) vec4<f32> {
    let c = textureSample(scene_tex, scene_samp, blit_uv(in.ndc)).rgb;
    return vec4<f32>(pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2)), 1.0);
}
