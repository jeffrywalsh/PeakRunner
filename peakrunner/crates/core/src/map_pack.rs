//! Explicit, local-only source-game map packs. No script execution or downloads.
//! The same immutable, checked triangle data serves client and dedicated server.
use glam::Vec3;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, HashMap}, path::{Path, PathBuf}, sync::OnceLock};

#[derive(Deserialize)]
pub struct Manifest {
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
}

pub struct MapPack {
    pub manifest: Manifest,
    pub root: PathBuf,
    pub fingerprint: String,
    triangles: Vec<[Vec3; 3]>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
    holes: Vec<bool>,
    pub heights: Vec<u8>,
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
        Some(MapPack::from_assets(Path::new(""), &|name,_|builtin_asset(name).map(|b|b.to_vec()))
            .expect("Built-in original map failed validation"))
    }).as_ref()
}

pub fn on(map: crate::terrain::MapId) -> Option<&'static MapPack> {
    (map == crate::terrain::MapId::Raindance).then(active).flatten()
}

impl MapPack {
    pub fn load(root: &Path) -> Result<Self, String> {
        let read = |name: &str, max: u64| -> Result<Vec<u8>, String> {
            let path = root.join(name);
            let size = path.metadata().map_err(|e| format!("{name}: {e}"))?.len();
            if size > max { return Err(format!("{name} exceeds size limit")); }
            std::fs::read(path).map_err(|e| e.to_string())
        };
        Self::from_assets(root,&read)
    }

    fn from_assets(root:&Path,read:&dyn Fn(&str,u64)->Result<Vec<u8>,String>)->Result<Self,String> {
        let json = read("map.json", 8_000_000)?;
        let manifest: Manifest = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
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
        if manifest.flags.iter().chain(manifest.spawns.iter()).flatten().any(|v| !v.is_finite() || v.abs()>10000.0) {
            return Err("Invalid map positions".into());
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
        Ok(Self {manifest,root:root.into(),fingerprint:format!("{:x}",Sha256::digest(&json)),triangles,buckets,holes,heights})
    }

    pub fn hole(&self,x:f32,z:f32)->bool {
        let x=(x/8.0).floor().clamp(0.0,255.0) as usize;
        let z=(z/8.0).floor().clamp(0.0,255.0) as usize;
        self.holes[z*256+x]
    }

    pub fn asset(&self,name:&str)->Result<Vec<u8>,String> {
        // Renderer/audio reopen only listed fixed assets. Recheck the digest so
        // editing a pack after startup cannot split visible and physical worlds.
        if name.contains(['/', '\\']) || !self.manifest.files.contains_key(name) {return Err("Unknown map asset".into());}
        if self.root.as_os_str().is_empty() {return builtin_asset(name).map(|b|b.to_vec());}
        let path=self.root.join(name);
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
fn builtin_asset(_name:&str)->Result<&'static [u8],String> {
    Err("This launcher-managed build requires its external map pack. Start it through PeakRunner Launcher.".into())
}

#[cfg(not(feature="external-map"))]
fn builtin_asset(name:&str)->Result<&'static [u8],String> {
    Ok(match name {
        "map.json"=>include_bytes!("../../../assets/maps/raindance/map.json"),
        "vertices.bin"=>include_bytes!("../../../assets/maps/raindance/vertices.bin"),
        "collision.bin"=>include_bytes!("../../../assets/maps/raindance/collision.bin"),
        "height.bin"=>include_bytes!("../../../assets/maps/raindance/height.bin"),
        "weights.rgba"=>include_bytes!("../../../assets/maps/raindance/weights.rgba"),
        "textures.rgba"=>include_bytes!("../../../assets/maps/raindance/textures.rgba"),
        "ambient.f32"=>include_bytes!("../../../assets/maps/raindance/ambient.f32"),
        _=>return Err("Unknown built-in map asset".into()),
    })
}

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
            assert!(!pack.manifest.holes.is_empty());
            assert!(pack.triangles.len()>1000);
            for flag in pack.manifest.flags {
                let flag=Vec3::from_array(flag);
                let (floor,_)=pack.floor(flag+Vec3::Y*0.5).expect("flag platform");
                assert!((flag.y-floor).abs()<3.0,"flag {flag:?} floor {floor}");
            }
            for ember in [true,false] {
                let spawn=crate::terrain::spawn_on(crate::terrain::MapId::Raindance,ember);
                assert!(spawn.is_finite());
                assert!(pack.sweep(spawn,spawn+Vec3::Y*80.0,0.52).is_none(),"spawn must not be inside a structure");
            }
            for &hole in &pack.manifest.holes {
                let x=(hole%256) as f32*8.0+4.0;let z=(hole/256) as f32*8.0+4.0;
                let y=crate::terrain::height_on(crate::terrain::MapId::Raindance,x,z);
                assert!(crate::terrain::segment_hit(crate::terrain::MapId::Raindance,
                    Vec3::new(x,y+2.0,z),Vec3::new(x,y-2.0,z),0.0).is_none(),"entrance terrain cutout");
            }
            assert!(pack.asset("../map.json").is_err());
        }
    }
}
