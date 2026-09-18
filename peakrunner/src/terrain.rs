use glam::Vec3;

pub const MAP: f32 = 256.0;
pub const RES: usize = 128;
pub const PLAYER_RADIUS: f32 = 0.52;
pub const EYE: f32 = 1.52;

pub const EMBER_HOME: Vec3 = Vec3::new(128.0, 0.0, 24.0);
pub const GLACIER_HOME: Vec3 = Vec3::new(128.0, 0.0, 232.0);

#[inline]
fn hash(ix: i32, iz: i32) -> f32 {
    let mut n = ix.wrapping_mul(374_761_393) ^ iz.wrapping_mul(668_265_263);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    (n as u32 as f32) * (1.0 / 4_294_967_295.0)
}

fn noise(x: f32, z: f32) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let fx = x - ix as f32;
    let fz = z - iz as f32;
    let u = fx * fx * (3.0 - 2.0 * fx);
    let v = fz * fz * (3.0 - 2.0 * fz);
    let a = hash(ix, iz);
    let b = hash(ix + 1, iz);
    let c = hash(ix, iz + 1);
    let d = hash(ix + 1, iz + 1);
    (a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v) * 2.0 - 1.0
}

fn fbm(x: f32, z: f32) -> f32 {
    noise(x, z) * 0.62 + noise(x * 2.17, z * 2.17) * 0.28 + noise(x * 5.3, z * 5.3) * 0.1
}

fn dist2_seg(px: f32, pz: f32, ax: f32, az: f32, bx: f32, bz: f32) -> (f32, f32) {
    let abx = bx - ax;
    let abz = bz - az;
    let len2 = abx * abx + abz * abz + 1e-6;
    let t = ((px - ax) * abx + (pz - az) * abz) / len2;
    let t = t.clamp(0.0, 1.0);
    let qx = ax + abx * t;
    let qz = az + abz * t;
    let dx = px - qx;
    let dz = pz - qz;
    (dx * dx + dz * dz, t)
}

fn spine(x: f32, z: f32, ax: f32, az: f32, bx: f32, bz: f32, width: f32, height: f32) -> f32 {
    let (d2, t) = dist2_seg(x, z, ax, az, bx, bz);
    let w = (-d2 / (width * width)).exp();
    let taper = (t * (1.0 - t) * 4.0).clamp(0.0, 1.0);
    w * height * (0.35 + 0.65 * taper)
}

fn flatten(h: f32, x: f32, z: f32, hx: f32, hz: f32, plat: f32, inner: f32, outer: f32) -> f32 {
    let d = ((x - hx).hypot(z - hz)).max(0.0);
    if d >= outer {
        return h;
    }
    let t = ((outer - d) / (outer - inner)).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    h + (plat - h) * t
}

pub fn height(x: f32, z: f32) -> f32 {
    let nx = (x / MAP).clamp(0.0, 1.0);
    let nz = (z / MAP).clamp(0.0, 1.0);
    let zt = nz * 2.0 - 1.0;
    let xt = nx * 2.0 - 1.0;

    // Twin ridges at the bases, valley through midfield.
    let mut h = 6.0 + 30.0 * zt * zt;
    // Bowl the sides so you stay in the arena.
    h += 26.0 * xt.abs().powf(5.4);
    // Midfield mesa — a fight over the high ground.
    let mx = (nx - 0.5) * 9.0;
    let mz = (nz - 0.5) * 8.4;
    h += 10.5 * (-(mx * mx + mz * mz)).exp();
    // Off-center knoll.
    let kx = (nx - 0.68) * 11.0;
    let kz = (nz - 0.46) * 11.0;
    h += 6.5 * (-(kx * kx + kz * kz)).exp();

    // Classic ski lines: two spines off each base toward mid.
    h += spine(x, z, 108.0, 28.0, 96.0, 118.0, 9.0, 7.5);
    h += spine(x, z, 148.0, 28.0, 162.0, 122.0, 9.0, 7.2);
    h += spine(x, z, 108.0, 228.0, 94.0, 138.0, 9.0, 7.4);
    h += spine(x, z, 148.0, 228.0, 164.0, 136.0, 9.0, 7.1);
    h += spine(x, z, 70.0, 80.0, 70.0, 176.0, 7.5, 5.0);
    h += spine(x, z, 186.0, 80.0, 186.0, 176.0, 7.5, 5.0);

    h += fbm(x * 0.028, z * 0.028) * 4.2;
    h += fbm(x * 0.09 + 20.0, z * 0.09) * 1.35;

    let ember_plat = 28.4;
    let glac_plat = 28.4;
    h = flatten(h, x, z, EMBER_HOME.x, EMBER_HOME.z, ember_plat, 8.0, 16.0);
    h = flatten(h, x, z, GLACIER_HOME.x, GLACIER_HOME.z, glac_plat, 8.0, 16.0);

    h.max(1.2)
}

pub struct Pillar {
    pub x: f32,
    pub z: f32,
    pub r: f32,
    pub h: f32,
}

pub fn pillars() -> Vec<Pillar> {
    let mut out = Vec::new();
    for i in 0..16i32 {
        let x = 28.0 + hash(i * 3, 17) * (MAP - 56.0);
        let z = 48.0 + hash(i * 5, 9) * (MAP - 96.0);
        let de = (x - EMBER_HOME.x).hypot(z - EMBER_HOME.z);
        let dg = (x - GLACIER_HOME.x).hypot(z - GLACIER_HOME.z);
        if de < 22.0 || dg < 22.0 {
            continue;
        }
        out.push(Pillar {
            x,
            z,
            r: 1.3 + hash(i, 2) * 2.4,
            h: 3.8 + hash(i, 11) * 10.0,
        });
    }
    out
}

/// Which ground the match is on. Valley is the small rift. Raindance is the
/// Tribes 2 terrain: 256 samples, 8 m apart, heights in meters as raw/32.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapId {
    Valley,
    Raindance,
}

#[derive(Clone, Copy)]
pub struct MapInfo {
    pub id: MapId,
    pub name: &'static str,
    pub note: &'static str,
    pub size: f32,
    pub ember: Vec3,
    pub glacier: Vec3,
    pub res: usize,
}

const RAIN_N: usize = 256;
const RAIN_STEP: f32 = 8.0;
/// 255 steps of 8 m. The Tribes file stores 256 corners.
const RAIN_SIZE: f32 = 2040.0;

static RAIN: &[u8] = include_bytes!("../assets/raindance.h16");

pub fn maps() -> [MapInfo; 2] {
    [
        MapInfo {
            id: MapId::Valley,
            name: "Valley",
            note: "256 m. The small rift.",
            size: MAP,
            ember: EMBER_HOME,
            glacier: GLACIER_HOME,
            res: RES,
        },
        MapInfo {
            id: MapId::Raindance,
            name: "Raindance",
            note: "2 km. Ravine between the bases, from the Tribes 2 terrain.",
            size: RAIN_SIZE,
            // Flag stands, shifted from Tribes coordinates (origin -1024,-1024).
            // Ember is the low-Z stand so the existing facing still looks across.
            ember: Vec3::new(1083.51, 0.0, 469.52),
            glacier: Vec3::new(741.18, 0.0, 1248.84),
            res: RAIN_N,
        },
    ]
}

pub fn info(id: MapId) -> MapInfo {
    maps().into_iter().find(|m| m.id == id).unwrap_or(maps()[0])
}

pub fn height_on(id: MapId, x: f32, z: f32) -> f32 {
    match id {
        MapId::Valley => height(x, z),
        MapId::Raindance => height_rain(x, z),
    }
}

pub fn normal_on(id: MapId, x: f32, z: f32) -> Vec3 {
    let e = if info(id).size > 1000.0 { 4.0 } else { 0.7 };
    let hl = height_on(id, x - e, z);
    let hr = height_on(id, x + e, z);
    let hd = height_on(id, x, z - e);
    let hu = height_on(id, x, z + e);
    Vec3::new(hl - hr, 2.0 * e, hd - hu).normalize_or_zero()
}

pub fn spawn_on(id: MapId, ember: bool) -> Vec3 {
    let home = if ember { info(id).ember } else { info(id).glacier };
    Vec3::new(home.x, height_on(id, home.x, home.z) + 1.2, home.z)
}

pub fn pillars_on(id: MapId) -> Vec<Pillar> {
    if id == MapId::Valley {
        return pillars();
    }
    // Midfield towers from the Raindance mission, in this map's coordinates.
    let marks = [(1086.2, 1274.7, 14.0), (698.9, 739.1, 16.0), (1035.0, 926.5, 12.0)];
    marks
        .into_iter()
        .map(|(x, z, h)| Pillar { x, z, r: 2.2, h })
        .collect()
}

pub fn sample_mesh_of(id: MapId) -> (Vec<f32>, Vec<u16>) {
    let spec = info(id);
    let res = spec.res;
    let mut verts = Vec::with_capacity(res * res * 6);
    for iz in 0..res {
        for ix in 0..res {
            let x = ix as f32 / (res - 1) as f32 * spec.size;
            let z = iz as f32 / (res - 1) as f32 * spec.size;
            let y = height_on(id, x, z);
            let n = normal_on(id, x, z);
            verts.extend_from_slice(&[x, y, z, n.x, n.y, n.z]);
        }
    }
    let mut idx = Vec::with_capacity((res - 1) * (res - 1) * 6);
    for iz in 0..res - 1 {
        for ix in 0..res - 1 {
            let i = (iz * res + ix) as u16;
            let r = res as u16;
            idx.extend_from_slice(&[i, i + r, i + 1, i + 1, i + r, i + r + 1]);
        }
    }
    (verts, idx)
}

fn height_rain(x: f32, z: f32) -> f32 {
    let n = RAIN_N;
    let fx = (x / RAIN_STEP).clamp(0.0, (n - 1) as f32);
    let fz = (z / RAIN_STEP).clamp(0.0, (n - 1) as f32);
    let x0 = fx.floor() as usize;
    let z0 = fz.floor() as usize;
    let x1 = (x0 + 1).min(n - 1);
    let z1 = (z0 + 1).min(n - 1);
    let tx = fx - x0 as f32;
    let tz = fz - z0 as f32;
    let h = |ix: usize, iz: usize| {
        let o = (iz * n + ix) * 2;
        u16::from_le_bytes([RAIN[o], RAIN[o + 1]]) as f32 / 32.0
    };
    let a = h(x0, z0) + (h(x1, z0) - h(x0, z0)) * tx;
    let b = h(x0, z1) + (h(x1, z1) - h(x0, z1)) * tx;
    a + (b - a) * tz
}

#[cfg(test)]
mod map_tests {
    use super::*;

    #[test]
    fn raindance_is_a_two_kilometer_ravine() {
        let m = info(MapId::Raindance);
        assert!(m.size > 1800.0, "size {}", m.size);
        let apart = (m.glacier - m.ember).length();
        assert!(apart > 700.0, "bases {apart:.0} m apart");
        let mut low = f32::MAX;
        let mut high = 0.0_f32;
        for i in 0..24 {
            let t = i as f32 / 23.0;
            let p = m.ember + (m.glacier - m.ember) * t;
            let h = height_on(MapId::Raindance, p.x, p.z);
            low = low.min(h);
            high = high.max(h);
        }
        assert!(
            high - low > 25.0,
            "the route between bases should drop into the ravine, span {:.1}",
            high - low
        );
    }
}
