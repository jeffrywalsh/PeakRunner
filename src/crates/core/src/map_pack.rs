//! Explicit, local-only source-game map packs. No script execution or downloads.
//! The same immutable, checked triangle data serves client and dedicated server.
use glam::Vec3;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, HashMap}, path::{Path, PathBuf}, sync::OnceLock};

fn default_step()->f32 {8.0}
fn default_water()->bool {true}

#[derive(Deserialize)]
pub struct ReferenceBase {
    pub name: String,
    pub position: [f32;3],
    pub world_to_local: [[f32;3];3],
}

#[derive(Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub private_reference: bool,
    #[serde(default)]
    pub reference_bases: Vec<ReferenceBase>,
    #[serde(default="default_step")]
    pub terrain_step: f32,
    #[serde(default="default_water")]
    pub water_enabled: bool,
    #[serde(default)]
    pub exact_spawns: bool,
    pub version: u32,
    pub flags: [[f32; 3]; 2],
    pub spawns: [[f32; 3]; 2],
    pub holes: Vec<usize>,
    pub texture_count: u32,
    pub terrain_layers: [u32; 4],
    pub sky_layers: Vec<u32>,
    pub water_layer: u32,
    pub ambient_emitters: Vec<[f32; 6]>,
    pub sky: BTreeMap<String, String>,
    pub water: BTreeMap<String, String>,
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub entities: Vec<crate::equipment::Definition>,
    /// Optional per-team spawn list, `[x, y, z, yaw]` with the player's
    /// centre and world yaw. Absent: the single `spawns` entry per team.
    #[serde(default)]
    pub spawn_points: Vec<Vec<[f32; 4]>>,
}

pub struct MapPack {
    embedded: Embedded,
    pub manifest: Manifest,
    pub root: PathBuf,
    pub fingerprint: String,
    triangles: Vec<[Vec3; 3]>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
    holes: Vec<bool>,
    pub heights: Vec<u8>,
}

/// Embedded payloads are zlib-compressed at build time by `crates/core/build.rs`;
/// `map.json` stays raw. `from_assets` checks every inflated payload against the
/// manifest's SHA-256 of the uncompressed bytes, so fingerprints and
/// compatibility are unchanged. Later reopens rely on zlib's Adler-32: the
/// embedded bytes are immutable and inflate deterministically.
fn inflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(bytes, 128_000_000)
        .map_err(|e| format!("Corrupt embedded map asset: {:?}", e.status))
}

macro_rules! embedded_map {
    ($(#[$doc:meta])* $func:ident, $dir:literal, $label:literal) => {
        $(#[$doc])*
        fn $func(name: &str) -> Result<Vec<u8>, String> {
            macro_rules! packed {($file:literal) => {
                inflate(include_bytes!(concat!(env!("OUT_DIR"), "/maps/", $dir, "/", $file, ".zlib")))
            }}
            match name {
                "map.json" => Ok(include_bytes!(concat!("../../../assets/maps/", $dir, "/map.json")).to_vec()),
                "vertices.bin" => packed!("vertices.bin"),
                "collision.bin" => packed!("collision.bin"),
                "height.bin" => packed!("height.bin"),
                "weights.rgba" => packed!("weights.rgba"),
                "textures.rgba" => packed!("textures.rgba"),
                "ambient.f32" => packed!("ambient.f32"),
                _ => Err(concat!("Unknown ", $label, " asset").into()),
            }
        }
    };
}

embedded_map!(
    /// Original Tower Complex, built by `scripts/build-tower-complex.py`. It holds
    /// the `broadside-clone` rotation slot so existing server configs keep working.
    tower_complex_asset, "tower-complex", "Tower Complex");
embedded_map!(
    /// Original Cairnhold, built by `scripts/build-cairnhold.py`. It holds the
    /// `stonehenge-clone` rotation slot so existing server configs keep working.
    cairnhold_asset, "cairnhold", "Cairnhold");
embedded_map!(
    /// Original Frostline, built by `scripts/build-frostline.py`. It holds the
    /// `snowblind-clone` rotation slot so existing server configs keep working.
    frostline_asset, "frostline", "Frostline");
embedded_map!(
    /// Original Dustreach, built by `scripts/build-dustreach.py`. It holds the
    /// `desert-of-death-clone` rotation slot so existing server configs keep working.
    dustreach_asset, "dustreach", "Dustreach");

type Embedded = fn(&str) -> Result<Vec<u8>, String>;

fn embedded_pack(assets: Embedded, label: &str) -> MapPack {
    let mut pack = MapPack::from_assets(Path::new(""), &|n,_|assets(n))
        .unwrap_or_else(|e| panic!("Built-in {label} pack failed validation: {e}"));
    pack.embedded = assets; pack
}

static PACK: OnceLock<Option<MapPack>> = OnceLock::new();

pub fn active() -> Option<&'static MapPack> {
    PACK.get_or_init(|| {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = std::env::var_os("PEAKRUNNER_MAP_PACK") {
            return Some(MapPack::load(Path::new(&path)).unwrap_or_else(|e|
                panic!("Cannot load PEAKRUNNER_MAP_PACK: {e}. Refusing a partial map.")));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let bundled = std::env::current_exe().ok().and_then(|p| p.parent()?.parent().map(|p| p.join("Resources/map")));
            if let Some(path) = bundled {
                if path.join("map.json").is_file() {
                    return Some(MapPack::load(&path).unwrap_or_else(|e| panic!("Invalid original map: {e}")));
                }
            }
        }
        Some(MapPack::from_assets(Path::new(""), &|name,_|builtin_asset(name))
            .expect("Built-in original map failed validation"))
    }).as_ref()
}

pub fn on(map: crate::terrain::MapId) -> Option<&'static MapPack> {
    match map {
        crate::terrain::MapId::Valley => None,
        crate::terrain::MapId::Raindance => active(),
        crate::terrain::MapId::SnowblindClone => {
            static FROST: OnceLock<MapPack> = OnceLock::new();
            Some(FROST.get_or_init(|| embedded_pack(frostline_asset, "Frostline")))
        }
        crate::terrain::MapId::DesertOfDeathClone => {
            static DUST: OnceLock<MapPack> = OnceLock::new();
            Some(DUST.get_or_init(|| embedded_pack(dustreach_asset, "Dustreach")))
        }
        crate::terrain::MapId::StonehengeClone => {
            static CAIRN: OnceLock<MapPack> = OnceLock::new();
            Some(CAIRN.get_or_init(|| embedded_pack(cairnhold_asset, "Cairnhold")))
        }
        crate::terrain::MapId::BroadsideClone => {
            static TOWER: OnceLock<MapPack> = OnceLock::new();
            Some(TOWER.get_or_init(|| embedded_pack(tower_complex_asset, "Tower Complex")))
        }
    }
}

/// Distance-fog colour used by every map before per-map colours existed.
pub const DEFAULT_FOG: [f32; 3] = [0.62, 0.62, 0.62];

fn parse_fog(value: &str) -> Option<[f32; 3]> {
    let v: Vec<f32> = value.split_whitespace().map(|s| s.parse().ok()).collect::<Option<_>>()?;
    let c: [f32; 3] = v.try_into().ok()?;
    c.iter().all(|x| x.is_finite() && (0.0..=1.0).contains(x)).then_some(c)
}

impl MapPack {
    /// Optional manifest `sky.fogColor` ("r g b", 0..1); absent means [`DEFAULT_FOG`].
    pub fn fog_color(&self) -> [f32; 3] {
        self.manifest.sky.get("fogColor").and_then(|v| parse_fog(v)).unwrap_or(DEFAULT_FOG)
    }

    pub fn load(root: &Path) -> Result<Self, String> {
        let read = |name: &str, max: u64| -> Result<Vec<u8>, String> {
            let path = root.join(name);
            let size = match path.metadata() {
                Ok(meta) => meta.len(),
                Err(e) if name == "ambient.f32" && e.kind() == std::io::ErrorKind::NotFound => {
                    // Launcher r1 cannot carry empty files. The manifest hash
                    // below still must prove that this map expects empty audio.
                    return Ok(Vec::new());
                }
                Err(e) => return Err(format!("{name}: {e}")),
            };
            if size > max { return Err(format!("{name} exceeds size limit")); }
            std::fs::read(path).map_err(|e| e.to_string())
        };
        Self::from_assets(root,&read)
    }

    fn from_assets(root:&Path,read:&dyn Fn(&str,u64)->Result<Vec<u8>,String>)->Result<Self,String> {
        let json = read("map.json", 8_000_000)?;
        let manifest: Manifest = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
        if !manifest.terrain_step.is_finite() || !(1.0..=32.0).contains(&manifest.terrain_step)
            || manifest.name.len()>128 || manifest.reference_bases.len()>16
            || manifest.reference_bases.iter().any(|b| b.name.len()>64
                || b.position.iter().chain(b.world_to_local.iter().flatten()).any(|v|!v.is_finite() || v.abs()>10000.)) {
            return Err("Invalid terrain/reference metadata".into());
        }
        if manifest.version != 1 || !(1..=256).contains(&manifest.texture_count) {
            return Err("Unsupported map version/texture count".into());
        }
        if manifest.terrain_layers.iter().chain(manifest.sky_layers.iter()).chain([&manifest.water_layer])
            .any(|&n| n >= manifest.texture_count) || manifest.sky_layers.len() < 6 {
            return Err("Invalid texture layer".into());
        }
        let sky_distance = |key: &str| -> Result<f32, String> {
            let n = manifest.sky.get(key).ok_or("Missing sky distance")?
                .parse::<f32>().map_err(|_| "Invalid sky distance")?;
            if !n.is_finite() || !(0.0..=10000.0).contains(&n) {
                return Err("Invalid sky distance".into());
            }
            Ok(n)
        };
        if sky_distance("visibleDistance")? <= sky_distance("fogDistance")? {
            return Err("Fog must start before visibility ends".into());
        }
        if manifest.sky.get("fogColor").is_some_and(|v| parse_fog(v).is_none()) {
            return Err("Invalid fog colour".into());
        }
        if manifest.flags.iter().chain(manifest.spawns.iter()).flatten().any(|v| !v.is_finite() || v.abs()>10000.0) {
            return Err("Invalid map positions".into());
        }
        if !manifest.spawn_points.is_empty() && (manifest.spawn_points.len()!=2
            || manifest.spawn_points.iter().any(|team| team.is_empty() || team.len()>32
                || team.iter().flatten().any(|v| !v.is_finite() || v.abs()>10000.0))) {
            return Err("Invalid spawn points".into());
        }
        if manifest.ambient_emitters.len()>32 || manifest.ambient_emitters.iter().any(|e|
            e.iter().any(|v|!v.is_finite()) || !(0.0..=1.0).contains(&e[3]) || e[4]<=0.0 || e[5]<=e[4]) {
            return Err("Invalid ambient emitter".into());
        }
        for (key,count) in [("position",3),("scale",3)] {
            let values:Vec<f32>=manifest.water.get(key).ok_or("Missing water bounds")?.split_whitespace()
                .map(|s|s.parse::<f32>()).collect::<Result<_,_>>().map_err(|_|"Invalid water bounds")?;
            if values.len()!=count || values.iter().any(|n|!n.is_finite() || n.abs()>10000.0)
                || (key=="scale" && values.iter().any(|v|*v<=0.0)) {return Err("Invalid water bounds".into());}
        }
        let mut collision = Vec::new();
        let mut heights = Vec::new();
        for name in ["vertices.bin", "collision.bin", "height.bin", "weights.rgba", "textures.rgba", "ambient.f32"] {
            let bytes = read(name, 128_000_000)?;
            let hash = format!("{:x}", Sha256::digest(&bytes));
            if manifest.files.get(name) != Some(&hash) { return Err(format!("Checksum mismatch: {name}")); }
            match name {
                "collision.bin" => collision = bytes,
                "vertices.bin" => {
                    if bytes.len()%144 != 0 {return Err("Invalid vertex stride".into());}
                    for v in bytes.chunks_exact(48) {
                        let f: Vec<f32> = v.chunks_exact(4).map(|x| f32::from_le_bytes(x.try_into().unwrap())).collect();
                        if f.iter().any(|n| !n.is_finite()) || f[..3].iter().any(|n| n.abs()>10000.0)
                            || f[10]<0.0 || f[10]>=manifest.texture_count as f32 || f[11]>=manifest.texture_count as f32
                            || f[11]< -3.0-manifest.texture_count as f32 {
                            return Err("Invalid vertex data".into());
                        }
                    }
                }
                "height.bin" => {
                    if bytes.len()!=256*256*2 {return Err("Invalid heightfield dimensions".into());}
                    heights=bytes;
                }
                "weights.rgba" if bytes.len()!=256*256*4 => return Err("Invalid layer weights".into()),
                "textures.rgba" if bytes.len()!=349524*manifest.texture_count as usize => return Err("Invalid textures".into()),
                "ambient.f32" if bytes.len()%4!=0 || bytes.len()>44100*4*60
                    || bytes.chunks_exact(4).any(|b| {let v=f32::from_le_bytes(b.try_into().unwrap());!v.is_finite() || v.abs()>1.0}) => return Err("Invalid ambience".into()),
                _ => (),
            }
        }
        if collision.len()%36 != 0 || collision.len()>36*500_000 {return Err("Invalid collision data".into());}
        let mut triangles=Vec::new();
        let mut buckets: HashMap<(i32,i32),Vec<usize>>=HashMap::new();
        for bytes in collision.chunks_exact(36) {
            let f: Vec<f32>=bytes.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
            if f.iter().any(|x| !x.is_finite() || x.abs()>10000.0) {return Err("Invalid triangle".into());}
            let tri=[Vec3::from_slice(&f[..3]),Vec3::from_slice(&f[3..6]),Vec3::from_slice(&f[6..])];
            let low=tri[0].min(tri[1]).min(tri[2]);let high=tri[0].max(tri[1]).max(tri[2]);
            if (high-low).max_element()>512.0 {return Err("Oversized collision triangle".into());}
            let id=triangles.len();
            for x in cell(low.x)..=cell(high.x) {for z in cell(low.z)..=cell(high.z) {
                buckets.entry((x,z)).or_default().push(id);
            }}
            triangles.push(tri);
        }
        let mut holes=vec![false;65536];
        for &i in &manifest.holes {
            if i>=holes.len() {return Err("Invalid terrain hole".into());}
            holes[i]=true;
        }
        crate::equipment::validate(&manifest.entities)?;
        Ok(Self {embedded:builtin_asset,manifest,root:root.into(),fingerprint:format!("{:x}",Sha256::digest(&json)),triangles,buckets,holes,heights})
    }

    pub fn triangle_count(&self)->usize {self.triangles.len()}

    pub fn hole(&self,x:f32,z:f32)->bool {
        let x=(x/self.manifest.terrain_step).floor().clamp(0.0,255.0) as usize;
        let z=(z/self.manifest.terrain_step).floor().clamp(0.0,255.0) as usize;
        self.holes[z*256+x]
    }

    pub fn asset(&self,name:&str)->Result<Vec<u8>,String> {
        // Renderer/audio reopen only listed fixed assets. Recheck the digest so
        // editing a pack after startup cannot split visible and physical worlds.
        if name.contains(['/', '\\']) || !self.manifest.files.contains_key(name) {return Err("Unknown map asset".into());}
        if self.root.as_os_str().is_empty() {return (self.embedded)(name);}
        let path=self.root.join(name);
        if name == "ambient.f32" && self.manifest.files[name] == format!("{:x}",Sha256::digest([])) {
            match path.try_exists() {
                Ok(false) => return Ok(Vec::new()),
                Err(e) => return Err(e.to_string()),
                Ok(true) => (),
            }
        }
        if path.metadata().map_err(|e|e.to_string())?.len()>128_000_000 {return Err("Oversized asset".into());}
        let bytes=std::fs::read(path).map_err(|e|e.to_string())?;
        if format!("{:x}",Sha256::digest(&bytes))!=self.manifest.files[name] {return Err(format!("Map asset changed: {name}"));}
        Ok(bytes)
    }

    /// Continuous sphere/triangle sweep; includes face, edge and vertex contacts.
    pub fn sweep(&self,start:Vec3,end:Vec3,radius:f32)->Option<(f32,Vec3)> {
        let low=start.min(end)-Vec3::splat(radius);let high=start.max(end)+Vec3::splat(radius);
        let mut best:Option<(f32,Vec3)>=None;
        for x in cell(low.x)..=cell(high.x) {for z in cell(low.z)..=cell(high.z) {
            if let Some(ids)=self.buckets.get(&(x,z)) {for &id in ids {
                let tri=self.triangles[id];
                if tri.iter().all(|p| p.y<low.y) || tri.iter().all(|p| p.y>high.y) {continue;}
                if let Some(hit)=sweep_triangle(start,end,radius,tri) {
                    if best.is_none_or(|old|hit.0<old.0) {best=Some(hit);}
                }
            }}
        }}
        best
    }

    /// Walking collision's four body spheres. Callers must keep this in step
    /// with movement; a single sphere at the feet is not the player.
    pub fn body_sweep(&self, start: Vec3, end: Vec3) -> Option<(f32, Vec3)> {
        let mut best: Option<(f32, Vec3)> = None;
        for offset in [0.0, 0.5, 1.0, crate::terrain::EYE] {
            let rise = Vec3::Y * offset;
            if let Some(hit) = self.sweep(start + rise, end + rise, crate::terrain::PLAYER_RADIUS) {
                if best.is_none_or(|old| hit.0 < old.0) { best = Some(hit); }
            }
        }
        best
    }

    pub fn floor(&self,pos:Vec3)->Option<(f32,Vec3)> {
        let start=pos+Vec3::Y*0.15;let end=Vec3::new(pos.x,-1000.0,pos.z);
        self.sweep(start,end,0.0).filter(|(_,n)|n.y>0.05).map(|(t,n)|(start.lerp(end,t).y,n))
    }

    /// Used once when constructing a safe overhead menu-camera altitude.
    pub fn highest_solid(&self)->f32 {
        self.triangles.iter().flatten().map(|p|p.y).fold(f32::NEG_INFINITY,f32::max)
    }
}

#[cfg(feature="external-map")]
fn builtin_asset(_name:&str)->Result<Vec<u8>,String> {
    Err("This launcher-managed build requires its external map pack. Start it through PeakRunner Launcher.".into())
}

#[cfg(not(feature="external-map"))]
embedded_map!(
    /// Original Raindance, built by `scripts/build-raindance.py`.
    builtin_asset, "raindance", "built-in map");

fn cell(x:f32)->i32 {(x/32.0).floor() as i32}

fn inside(p:Vec3,t:[Vec3;3],n:Vec3)->bool {
    (0..3).all(|i|(t[(i+1)%3]-t[i]).cross(p-t[i]).dot(n)>=-0.0001)
}

fn root(a:f32,b:f32,c:f32)->Option<f32> {
    if a<1e-12 {return None;}
    let d=b*b-a*c;
    if d<0.0 {return None;}
    let t=(-b-d.sqrt())/a;
    (0.0..=1.0).contains(&t).then_some(t)
}

fn sweep_triangle(s:Vec3,e:Vec3,r:f32,tri:[Vec3;3])->Option<(f32,Vec3)> {
    let v=e-s;let n=(tri[1]-tri[0]).cross(tri[2]-tri[0]).normalize_or_zero();
    if n.length_squared()<0.5 {return None;}
    let mut best=None;
    let mut accept=|t:f32,normal:Vec3| {
        if (0.0..=1.0).contains(&t) && v.dot(normal)< -1e-7 && best.is_none_or(|(old,_)|t<old) {best=Some((t,normal));}
    };
    let d=(s-tri[0]).dot(n);let dv=v.dot(n);
    if dv.abs()>1e-8 {
        for sign in [-1.0,1.0] {
            let t=(sign*r-d)/dv;
            if (0.0..=1.0).contains(&t) && inside(s+v*t-n*(sign*r),tri,n) {accept(t,n*sign);}
        }
    }
    if r>0.0 {
        for i in 0..3 {
            let a=tri[i];let edge=tri[(i+1)%3]-a;let len=edge.length_squared();
            let q=s-a;
            if let Some(t)=root(v.length_squared(),q.dot(v),q.length_squared()-r*r) {
                accept(t,(s+v*t-a).normalize_or_zero());
            }
            if len>1e-10 {
                let qp=q-edge*(q.dot(edge)/len);let vp=v-edge*(v.dot(edge)/len);
                if let Some(t)=root(vp.length_squared(),qp.dot(vp),qp.length_squared()-r*r) {
                    let u=(q+v*t).dot(edge)/len;
                    if (0.0..=1.0).contains(&u) {accept(t,(s+v*t-a-edge*u).normalize_or_zero());}
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    /// Release-mode load timing probe: `cargo test --release -p peakrunner-core
    /// --lib embedded_pack_load_timing -- --ignored --nocapture`.
    #[test]
    #[ignore = "timing probe"]
    fn embedded_pack_load_timing() {
        use super::*;
        use crate::terrain::MapId;
        let t = std::time::Instant::now();
        let rain = active().unwrap();
        println!("raindance load {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        for (map, label) in [(MapId::BroadsideClone, "tower-complex"), (MapId::StonehengeClone, "cairnhold"),
            (MapId::SnowblindClone, "frostline"), (MapId::DesertOfDeathClone, "dustreach")] {
            let t = std::time::Instant::now();
            let pack = on(map).unwrap();
            let load = t.elapsed();
            let t = std::time::Instant::now();
            let textures = pack.asset("textures.rgba").unwrap();
            println!("{label} load {:.1} ms, textures reopen {:.1} ms ({} bytes)",
                load.as_secs_f64() * 1e3, t.elapsed().as_secs_f64() * 1e3, textures.len());
        }
        let t = std::time::Instant::now();
        rain.asset("textures.rgba").unwrap();
        println!("raindance textures reopen {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    }

    #[test]
    fn embedded_payloads_inflate_to_their_manifest_hashes() {
        use super::*;
        use crate::terrain::MapId;
        assert!(inflate(b"not a deflate stream").is_err());
        for map in [MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone, MapId::SnowblindClone, MapId::DesertOfDeathClone] {
            let pack = on(map).unwrap();
            for name in pack.manifest.files.keys() {
                let bytes = pack.asset(name).unwrap();
                assert_eq!(format!("{:x}", Sha256::digest(&bytes)), pack.manifest.files[name], "{map:?} {name}");
            }
        }
    }

    #[test]
    fn fog_colour_parses_validates_and_defaults() {
        use super::*;
        assert_eq!(parse_fog("0.8 0.69 0.52"), Some([0.8, 0.69, 0.52]));
        for bad in ["0.8 0.69", "0.8 0.69 0.52 1", "1.2 0 0", "x 0 0", "NaN 0 0"] {
            assert_eq!(parse_fog(bad), None, "{bad}");
        }
        use crate::terrain::MapId;
        // Maps without fogColor keep the historical grey.
        assert_eq!(on(MapId::Raindance).unwrap().fog_color(), DEFAULT_FOG);
        assert_eq!(on(MapId::BroadsideClone).unwrap().fog_color(), DEFAULT_FOG);
        assert_eq!(on(MapId::DesertOfDeathClone).unwrap().fog_color(), [0.80, 0.69, 0.52]);
        assert_eq!(on(MapId::SnowblindClone).unwrap().fog_color(), [0.84, 0.87, 0.90]);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "external-map")))]
    #[test]
    fn omitted_ambience_requires_the_empty_content_hash() {
        use super::*;
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("peakrunner-empty-audio-{}-{nonce}",std::process::id()));
        std::fs::create_dir(&root).unwrap();
        for name in ["map.json","vertices.bin","collision.bin","height.bin","weights.rgba","textures.rgba"] {
            std::fs::write(root.join(name), builtin_asset(name).unwrap()).unwrap();
        }
        assert!(MapPack::load(&root).is_err(), "missing nonempty audio must fail");
        let mut manifest: serde_json::Value = serde_json::from_slice(&builtin_asset("map.json").unwrap()).unwrap();
        manifest["files"]["ambient.f32"] = format!("{:x}",Sha256::digest([])).into();
        std::fs::write(root.join("map.json"),serde_json::to_vec(&manifest).unwrap()).unwrap();
        let absent = MapPack::load(&root).unwrap();
        assert!(absent.asset("ambient.f32").unwrap().is_empty());
        std::fs::write(root.join("ambient.f32"),[]).unwrap();
        assert_eq!(absent.fingerprint, MapPack::load(&root).unwrap().fingerprint);
        std::fs::write(root.join("ambient.f32"),[1,2,3,4]).unwrap();
        assert!(absent.asset("ambient.f32").is_err());
        assert!(MapPack::load(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    use super::*;

    #[test]
    fn fast_sweeps_stop_at_thin_faces_from_both_sides() {
        let t=[Vec3::new(-10.,0.,-10.),Vec3::new(0.,0.,10.),Vec3::new(10.,0.,-10.)];
        for sign in [-1.,1.] {
            let (time,n)=sweep_triangle(Vec3::Y*20.*sign,-Vec3::Y*20.*sign,0.5,t).unwrap();
            assert!((time-0.4875).abs()<1e-5);assert!(n.y*sign>0.99);
        }
        assert!(sweep_triangle(Vec3::new(50.,20.,0.),Vec3::new(50.,-20.,0.),0.5,t).is_none());
    }
    #[test]
    fn loaded_pack_is_valid_when_requested() {
        if let Some(pack)=active() {
            assert!(pack.triangles.len()>1000);
            for flag in pack.manifest.flags {
                let flag=Vec3::from_array(flag);
                let (floor,_)=pack.floor(flag+Vec3::Y*0.5).expect("flag platform");
                assert!((flag.y-floor).abs()<3.0,"flag {flag:?} floor {floor}");
            }
            for ember in [true,false] {
                let spawn=crate::terrain::spawn_on(crate::terrain::MapId::Raindance,ember);
                assert!(spawn.is_finite());
                let clearance=if pack.manifest.exact_spawns {2.0} else {80.0};
                assert!(pack.sweep(spawn,spawn+Vec3::Y*clearance,0.52).is_none(),"spawn must not be inside a structure");
            }
            for &hole in &pack.manifest.holes {
                let step=pack.manifest.terrain_step;
                let x=(hole%256) as f32*step+step/2.;let z=(hole/256) as f32*step+step/2.;
                let y=crate::terrain::height_on(crate::terrain::MapId::Raindance,x,z);
                assert!(crate::terrain::segment_hit(crate::terrain::MapId::Raindance,
                    Vec3::new(x,y+2.0,z),Vec3::new(x,y-2.0,z),0.0).is_none(),"entrance terrain cutout");
            }
            assert!(pack.asset("../map.json").is_err());
        }
    }
}
