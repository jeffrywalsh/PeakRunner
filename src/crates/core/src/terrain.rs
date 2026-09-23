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
/// Original highland terrain: 256 samples, 8 m apart, heights as raw/32.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum MapId {
    Valley,
    Raindance,
    BroadsideClone,
    StonehengeClone,
    SnowblindClone,
    DesertOfDeathClone,
}

impl MapId {
    /// Stable identity, independent of display labels and filesystem locations.
    pub fn key(self) -> &'static str {
        match self { Self::Valley => "valley", Self::Raindance => "raindance", Self::BroadsideClone => "broadside-clone", Self::StonehengeClone => "stonehenge-clone", Self::SnowblindClone => "snowblind-clone", Self::DesertOfDeathClone => "desert-of-death-clone" }
    }
    pub fn parse(key: &str) -> Option<Self> {
        [Self::Valley, Self::Raindance, Self::BroadsideClone, Self::StonehengeClone, Self::SnowblindClone, Self::DesertOfDeathClone].into_iter().find(|id| id.key().eq_ignore_ascii_case(key))
    }
    pub fn is_private_clone(self) -> bool {
        matches!(self, Self::SnowblindClone | Self::DesertOfDeathClone)
    }
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
/// Surface appearance is independent of the heightfield and collision mesh.
/// New landscapes can supply palettes/scales here without adding grass geometry.
pub fn surface_style(map: MapId) -> crate::grass::SurfaceStyle {
    match map {
        MapId::Raindance | MapId::BroadsideClone | MapId::StonehengeClone | MapId::SnowblindClone | MapId::DesertOfDeathClone => crate::grass::HIGHLAND,
        MapId::Valley => crate::grass::SurfaceStyle {
            cover: [0.66, 0.73, 0.79, 5.0],
            soil: [0.42, 0.55, 0.64, 35.0],
            rock: [0.27, 0.25, 0.23, 0.5],
            rules: [0.12, 0.42, 0.08, 0.5],
        },
    }
}
const RAIN_STEP: f32 = 8.0;
/// 255 steps of 8 m. The Tribes file stores 256 corners.
const RAIN_SIZE: f32 = 2040.0;

// Authored procedural heightfield; no extracted source-game data in the build.
static RAIN: &[u8] = include_bytes!("../../../assets/maps/raindance/height.bin");

fn all_maps() -> [MapInfo; 6] {
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
            note: "2 km. Original rainy highlands, ravine crossing and powered bases.",
            size: RAIN_SIZE,
            // Original base centers; map packs supply exact flag deck positions.
            ember: Vec3::new(1160.0, 0.0, 480.0),
            glacier: Vec3::new(800.0, 0.0, 1400.0),
            res: RAIN_N,
        },
        // Original map in the former Broadside Clone slot; the key stays for rotations.
        MapInfo { id: MapId::BroadsideClone, name: "Tower Complex",
            note: "Floating towers over rolling hills. Turret pods, a central shaft and flags on level two.",
            size: RAIN_SIZE, ember: Vec3::new(1024.,240.,820.), glacier: Vec3::new(1024.,240.,1228.), res: RAIN_N },
        // Original map in the former Stonehenge Clone slot; the key stays for rotations.
        MapInfo { id: MapId::StonehengeClone, name: "Cairnhold",
            note: "Hillside bunkers, trench-linked flag towers and a central ring on rugged ground.",
            size: RAIN_SIZE, ember: Vec3::new(1024.,191.,844.), glacier: Vec3::new(1024.,191.,1204.), res: RAIN_N },
        MapInfo { id: MapId::SnowblindClone, name: "Snowblind Clone",
            note: "Reference snow-bunker layout. Geometry starts here.", size:RAIN_SIZE,
            ember:Vec3::ZERO, glacier:Vec3::ZERO, res:RAIN_N },
        MapInfo { id: MapId::DesertOfDeathClone, name: "Desert of Death Clone",
            note: "Reference desert-ruin layout. Geometry starts here.", size:RAIN_SIZE,
            ember:Vec3::ZERO, glacier:Vec3::ZERO, res:RAIN_N },
    ]
}

pub fn maps() -> Vec<MapInfo> {
    let mut maps=vec![info(MapId::Raindance),info(MapId::BroadsideClone),info(MapId::StonehengeClone)];
    for id in [MapId::SnowblindClone, MapId::DesertOfDeathClone] {
        if crate::map_pack::on(id).is_some() { maps.push(info(id)); }
    }
    maps
}

pub fn info(id: MapId) -> MapInfo {
    let mut result=all_maps().into_iter().find(|m| m.id == id).expect("known map");
    if let Some(pack)=crate::map_pack::on(id) {
        result.size=pack.manifest.terrain_step*255.;
        if pack.manifest.private_reference {
            result.name=&pack.manifest.name;
        }
    }
    result
}

fn source_height(id: MapId, x: f32, z: f32) -> f32 {
    match id {
        MapId::Valley => height(x, z),
        MapId::Raindance | MapId::BroadsideClone | MapId::StonehengeClone | MapId::SnowblindClone | MapId::DesertOfDeathClone => height_pack(id, x, z),
    }
}

/// The exact two triangles emitted by sample_mesh_of, including its diagonal.
/// Shading and ski steering can use smoothed normals; collision cannot use a
/// separate curved heightfield beneath these planar faces.
pub fn surface_on(id: MapId, x: f32, z: f32) -> (f32, Vec3) {
    let spec = info(id);
    let cell = spec.size / (spec.res - 1) as f32;
    let gx = (x / cell).clamp(0.0, (spec.res - 1) as f32);
    let gz = (z / cell).clamp(0.0, (spec.res - 1) as f32);
    let ix = (gx.floor() as usize).min(spec.res - 2);
    let iz = (gz.floor() as usize).min(spec.res - 2);
    let tx = gx - ix as f32;
    let tz = gz - iz as f32;
    let x0 = ix as f32 * cell;
    let z0 = iz as f32 * cell;
    let a = source_height(id, x0, z0);
    let b = source_height(id, x0 + cell, z0);
    let c = source_height(id, x0, z0 + cell);
    let original_diagonal = crate::map_pack::on(id).is_some() && ((ix ^ iz) & 1) == 0;
    let (y, dx, dz) = if original_diagonal {
        let d = source_height(id, x0 + cell, z0 + cell);
        if tx < tz { (a+(d-c)*tx+(c-a)*tz, d-c, c-a) }
        else { (a+(b-a)*tx+(d-b)*tz, b-a, d-b) }
    } else if tx + tz <= 1.0 {
        (a + (b - a) * tx + (c - a) * tz, b - a, c - a)
    } else {
        let d = source_height(id, x0 + cell, z0 + cell);
        (d + (c - d) * (1.0 - tx) + (b - d) * (1.0 - tz), d - c, d - b)
    };
    (y, Vec3::new(-dx / cell, 1.0, -dz / cell).normalize())
}

pub fn height_on(id: MapId, x: f32, z: f32) -> f32 {
    surface_on(id, x, z).0
}

/// Earliest contact along a segment. Grid X, Z and X+Z crossings divide it
/// into intervals contained in one rendered triangle, so the height gap is
/// linear on each interval. This catches ridges even if both endpoints clear.
pub fn segment_hit(id: MapId, start: Vec3, end: Vec3, clearance: f32) -> Option<f32> {
    let spec = info(id);
    let cell = spec.size / (spec.res - 1) as f32;
    let mut cuts = vec![0.0, 1.0];
    for (a, b) in [(start.x, end.x), (start.z, end.z), (start.x + start.z, end.x + end.z), (start.x-start.z,end.x-end.z)] {
        if (b - a).abs() < 1e-6 { continue; }
        let lo = (a.min(b) / cell).floor() as i32 + 1;
        let hi = (a.max(b) / cell).ceil() as i32;
        for k in lo..hi {
            let t = (k as f32 * cell - a) / (b - a);
            if t > 0.0 && t < 1.0 { cuts.push(t); }
        }
    }
    cuts.sort_by(f32::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let gap = |t: f32| {
        let p = start.lerp(end, t);
        p.y - height_on(id, p.x, p.z) - clearance
    };
    for window in cuts.windows(2) {
        let previous_t=window[0];let t=window[1];
        let middle=start.lerp(end,(previous_t+t)*0.5);
        if crate::map_pack::on(id).is_some_and(|p|p.hole(middle.x,middle.z)) {continue;}
        let previous_gap=gap(previous_t);
        if previous_gap < -0.0001 {return Some(previous_t);}
        let next_gap = gap(t);
        if next_gap < -0.0001 || (next_gap <= 0.0 && previous_gap > 0.0001) {
            let fraction = (previous_gap / (previous_gap - next_gap)).clamp(0.0, 1.0);
            return Some(previous_t + (t - previous_t) * fraction);
        }
    }
    None
}

pub fn normal_on(id: MapId, x: f32, z: f32) -> Vec3 {
    let e = if info(id).size > 1000.0 { 4.0 } else { 0.7 };
    let hl = height_on(id, x - e, z);
    let hr = height_on(id, x + e, z);
    let hd = height_on(id, x, z - e);
    let hu = height_on(id, x, z + e);
    Vec3::new(hl - hr, 2.0 * e, hd - hu).normalize_or_zero()
}

/// Authored spawn points for a team, `(centre, world yaw)`. Empty when the
/// pack only has the legacy single spawn.
pub fn spawn_points_on(id: MapId, ember: bool) -> Vec<(Vec3, f32)> {
    crate::map_pack::on(id).and_then(|pack| pack.manifest.spawn_points.get(usize::from(!ember)))
        .map(|team| team.iter().map(|p| (Vec3::new(p[0], p[1], p[2]), p[3])).collect())
        .unwrap_or_default()
}

pub fn spawn_on(id: MapId, ember: bool) -> Vec3 {
    if let Some(pack)=crate::map_pack::on(id) {
        let center=Vec3::from_array(pack.manifest.spawns[usize::from(!ember)]);
        if pack.manifest.exact_spawns { return center; }
        // A SpawnSphere is a search region, not a spawn point. Its center can
        // be inside a wall. Pick a clear outdoor candidate within its 80m radius.
        for radius in [24.0,40.0,60.0,76.0] {
            for i in 0..16 {
                let angle=i as f32*std::f32::consts::TAU/16.0;
                let mut p=center+Vec3::new(angle.cos()*radius,0.0,angle.sin()*radius);
                if pack.hole(p.x,p.z) {continue;}
                p.y=height_on(id,p.x,p.z)+1.2;
                if pack.sweep(p-Vec3::Y*0.3,p+Vec3::Y*100.0,PLAYER_RADIUS).is_none()
                    && [Vec3::X,-Vec3::X,Vec3::Z,-Vec3::Z].into_iter().all(|axis|
                        pack.sweep(p,p+axis*5.0,PLAYER_RADIUS).is_none()) {return p;}
            }
        }
        panic!("Imported map has no clear spawn inside its source spawn sphere");
    }
    let home = if ember { info(id).ember } else { info(id).glacier };
    Vec3::new(home.x, height_on(id, home.x, home.z) + 1.2, home.z)
}

pub fn pillars_on(id: MapId) -> Vec<Pillar> {
    if crate::map_pack::on(id).is_some() {return Vec::new();}
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
            let y = source_height(id, x, z);
            let n = normal_on(id, x, z);
            verts.extend_from_slice(&[x, y, z, n.x, n.y, n.z]);
        }
    }
    let mut idx = Vec::with_capacity((res - 1) * (res - 1) * 6);
    for iz in 0..res - 1 {
        for ix in 0..res - 1 {
            if crate::map_pack::on(id).is_some_and(|p|p.hole(ix as f32*8.0,iz as f32*8.0)) {continue;}
            let i = (iz * res + ix) as u16;
            let r = res as u16;
            if crate::map_pack::on(id).is_some() && ((ix ^ iz)&1)==0 {
                idx.extend_from_slice(&[i,i+r,i+r+1,i,i+r+1,i+1]);
            } else {idx.extend_from_slice(&[i, i + r, i + 1, i + 1, i + r, i + r + 1]);}
        }
    }
    (verts, idx)
}

/// Cached safe altitude for the menu orbit, above terrain and solid scenery.
pub fn overview_height(id:MapId)->f32 {
    // A level orbit above every rendered terrain vertex and solid structure:
    // no dives through hills and no abrupt vertical corrections over roofs.
    // Map packs are immutable for the process lifetime, so cache the scan.
    static VALLEY:std::sync::OnceLock<f32>=std::sync::OnceLock::new();
    static RAIN:std::sync::OnceLock<f32>=std::sync::OnceLock::new();
    static CLONE:std::sync::OnceLock<f32>=std::sync::OnceLock::new();
    static STONE:std::sync::OnceLock<f32>=std::sync::OnceLock::new();
    static SNOW: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    static DESERT: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    let cache=match id {MapId::Valley=>&VALLEY,MapId::Raindance=>&RAIN,MapId::BroadsideClone=>&CLONE,MapId::StonehengeClone=>&STONE,MapId::SnowblindClone=>&SNOW,MapId::DesertOfDeathClone=>&DESERT};
    *cache.get_or_init(|| {
        let map=info(id);let step=map.size/(map.res-1) as f32;
        let mut top=f32::NEG_INFINITY;
        for z in 0..map.res {for x in 0..map.res {
            top=top.max(height_on(id,x as f32*step,z as f32*step));
        }}
        if let Some(pack)=crate::map_pack::on(id) {top=top.max(pack.highest_solid());}
        top+28.
    })
}

/// Highest walkable support below the player's feet, never an overhead roof.
pub fn support_on(id:MapId,pos:Vec3)->(f32,Vec3) {
    let mut ground=surface_on(id,pos.x,pos.z);
    if let Some(pack)=crate::map_pack::on(id) {
        if pack.hole(pos.x,pos.z) {ground=(-1000.0,Vec3::Y);}
        if let Some(floor)=pack.floor(pos-Vec3::Y*PLAYER_RADIUS) {
            if floor.0>ground.0 {ground=floor;}
        }
    }
    ground
}

fn height_pack(id: MapId, x: f32, z: f32) -> f32 {
    let n = RAIN_N;
    let step=crate::map_pack::on(id).map_or(RAIN_STEP,|p|p.manifest.terrain_step);
    let fx = (x / step).clamp(0.0, (n - 1) as f32);
    let fz = (z / step).clamp(0.0, (n - 1) as f32);
    let x0 = fx.floor() as usize;
    let z0 = fz.floor() as usize;
    let x1 = (x0 + 1).min(n - 1);
    let z1 = (z0 + 1).min(n - 1);
    let tx = fx - x0 as f32;
    let tz = fz - z0 as f32;
    let h = |ix: usize, iz: usize| {
        let o = (iz * n + ix) * 2;
        let data=crate::map_pack::on(id).map_or(RAIN,|p|p.heights.as_slice());
        u16::from_le_bytes([data[o], data[o + 1]]) as f32 / 32.0
    };
    let a = h(x0, z0) + (h(x1, z0) - h(x0, z0)) * tx;
    let b = h(x0, z1) + (h(x1, z1) - h(x0, z1)) * tx;
    a + (b - a) * tz
}

#[cfg(test)]
mod map_tests {
    use super::*;

    #[test]
    fn tower_complex_is_embedded_with_floating_decks_and_clear_spawns() {
        let id = MapId::BroadsideClone;
        assert!(!id.is_private_clone());
        assert!(maps().iter().any(|m| m.id == id && m.name == "Tower Complex"));
        let pack = crate::map_pack::on(id).expect("embedded Tower Complex");
        assert!(maps().iter().all(|m| m.id != MapId::Valley));
        assert!(!pack.manifest.private_reference);
        assert_eq!(pack.manifest.name, "Tower Complex");
        assert!(pack.asset("textures.rgba").unwrap().len() > 1_000_000);
        for team in [true, false] {
            let spawn = spawn_on(id, team);
            let floor = support_on(id, spawn).0;
            assert!((spawn.y-floor-1.2).abs() < 0.05, "spawn support {floor} {spawn:?}");
            assert!(pack.sweep(spawn, spawn+Vec3::Y*2., PLAYER_RADIUS).is_none());
            let flag = Vec3::from_array(pack.manifest.flags[usize::from(!team)]);
            // Broadside-like flow: flags roughly 60-120 m over rolling ground.
            let air=flag.y-height_on(id,flag.x,flag.z);
            assert!((60.0..120.0).contains(&air), "flag {air} m over terrain");
            assert!(pack.floor(flag).is_some(), "flag has solid deck");
            assert!(pack.sweep(flag+Vec3::Y,flag+Vec3::Y*80.,0.).is_some(), "flag has solid roof");
        }
        for (team, points) in pack.manifest.spawn_points.iter().enumerate() {
            let flag = Vec3::from_array(pack.manifest.flags[team]);
            assert!(points.len() >= 6);
            for p in points {
                let spawn = Vec3::new(p[0], p[1], p[2]);
                assert!((spawn - flag).length() < 60.0, "team {team} spawn {spawn:?} is not at its own base");
                let floor = pack.floor(spawn).expect("spawn has a deck").0;
                assert!((spawn.y - floor - 1.2).abs() < 0.01);
                assert!(pack.body_sweep(spawn, spawn + Vec3::Y*0.2).is_none(), "spawn {spawn:?} head clearance");
                assert!([Vec3::X,-Vec3::X,Vec3::Z,-Vec3::Z].iter().all(|d| pack.body_sweep(spawn, spawn + *d*0.5).is_none()),
                    "spawn {spawn:?} boxed in");
            }
        }
        for other in [MapId::Raindance] {
            assert_ne!(pack.fingerprint, crate::map_pack::on(other).unwrap().fingerprint);
        }
    }

    #[test]
    fn cairnhold_is_embedded_with_grounded_spawns_and_flag_decks() {
        let id = MapId::StonehengeClone;
        assert!(!id.is_private_clone());
        assert!(maps().iter().any(|m| m.id == id && m.name == "Cairnhold"));
        let pack = crate::map_pack::on(id).expect("embedded Cairnhold");
        assert!(!pack.manifest.private_reference);
        assert_eq!(pack.manifest.name, "Cairnhold");
        assert!(pack.asset("textures.rgba").unwrap().len() > 1_000_000);
        let bases = [Vec3::new(1024.,191.,844.), Vec3::new(1024.,191.,1204.)];
        for team in [true, false] {
            let spawn = spawn_on(id, team);
            let floor = support_on(id, spawn).0;
            assert!((spawn.y-floor-1.2).abs() < 0.05, "spawn support {floor} {spawn:?}");
            assert!(pack.sweep(spawn, spawn+Vec3::Y*2., PLAYER_RADIUS).is_none());
        }
        for (team, flag) in pack.manifest.flags.iter().enumerate() {
            let flag = Vec3::from_array(*flag);
            assert!(pack.floor(flag).is_some(), "team {team} flag has solid deck");
            // Exposed stand: open sky above the flag, on a tower over the knoll.
            assert!(pack.sweep(flag+Vec3::Y,flag+Vec3::Y*60.,0.).is_none(), "flag stand is open");
            assert!(flag.y-height_on(id,flag.x,flag.z) > 8., "flag stand is raised");
        }
        for (team, points) in pack.manifest.spawn_points.iter().enumerate() {
            assert!(points.len() >= 6);
            for p in points {
                let spawn = Vec3::new(p[0], p[1], p[2]);
                // Bunker, flag hut and the two flank structures all count as home.
                let (own, enemy) = ((spawn-bases[team]).length(), (spawn-bases[1-team]).length());
                assert!(own < 150.0 && own < enemy, "team {team} spawn {spawn:?} is not on its own side");
                let floor = pack.floor(spawn).expect("spawn has a floor").0;
                assert!((spawn.y - floor - 1.2).abs() < 0.01);
                assert!(pack.body_sweep(spawn, spawn + Vec3::Y*0.2).is_none(), "spawn {spawn:?} head clearance");
                assert!([Vec3::X,-Vec3::X,Vec3::Z,-Vec3::Z].iter().any(|d| pack.body_sweep(spawn, spawn + *d*0.5).is_none()),
                    "spawn {spawn:?} boxed in");
            }
        }
        for other in [MapId::Raindance, MapId::BroadsideClone] {
            assert_ne!(pack.fingerprint, crate::map_pack::on(other).unwrap().fingerprint);
        }
    }

    #[test]
    fn collision_height_and_normal_match_rendered_triangles() {
        for map in [MapId::Valley, MapId::Raindance] {
            let (vertices, indices) = sample_mesh_of(map);
            let vertex = |i: u16| Vec3::from_slice(&vertices[i as usize * 6..i as usize * 6 + 3]);
            for triangle in indices.chunks_exact(3).step_by(97) {
                let a = vertex(triangle[0]);
                let b = vertex(triangle[1]);
                let c = vertex(triangle[2]);
                let normal = (b - a).cross(c - a).normalize();
                for weights in [[0.2, 0.3, 0.5], [0.75, 0.2, 0.05]] {
                    let p = a * weights[0] + b * weights[1] + c * weights[2];
                    let (y, n) = surface_on(map, p.x, p.z);
                    assert!((y - p.y).abs() < 0.002, "{map:?}: visual y={} collision y={y}", p.y);
                    assert!(n.dot(normal) > 0.999, "collision must use the rendered face normal");
                }
            }
        }
    }

    #[test]
    fn swept_contact_finds_a_ridge_between_clear_endpoints() {
        for map in [MapId::Valley, MapId::Raindance] {
            let spec = info(map);
            let cell = spec.size / (spec.res - 1) as f32;
            let mut found = false;
            'search: for z in (4..spec.res - 4).step_by(3) {
                for x in 4..spec.res - 4 {
                    let x0 = x as f32 * cell;
                    let z0 = z as f32 * cell;
                    let a = height_on(map, x0, z0);
                    let b = height_on(map, x0 + cell, z0);
                    let c = height_on(map, x0 + 2.0 * cell, z0);
                    if b <= (a + c) * 0.5 + 0.5 { continue; }
                    let start = Vec3::new(x0, a + 0.25, z0);
                    let end = Vec3::new(x0 + 2.0 * cell, c + 0.25, z0);
                    let t = segment_hit(map, start, end, 0.0).expect("ridge must stop the segment");
                    assert!(t > 0.0 && t < 0.5);
                    let hit = start.lerp(end, t);
                    assert!((hit.y - height_on(map, hit.x, hit.z)).abs() < 0.002);
                    found = true;
                    break 'search;
                }
            }
            assert!(found, "need a convex ridge on {map:?}");
        }
    }

    #[test]
    fn a_segment_leaving_the_surface_is_not_a_collision() {
        let ground = height_on(MapId::Valley, EMBER_HOME.x, EMBER_HOME.z);
        let start = Vec3::new(EMBER_HOME.x, ground, EMBER_HOME.z);
        assert!(segment_hit(MapId::Valley, start, start + Vec3::Y * 3.0, 0.0).is_none());
    }

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
