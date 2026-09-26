//! Zlib-compress the embedded map payloads into OUT_DIR so the binary carries them
//! compressed. `map.json` stays raw; `map_pack` inflates the rest and checks
//! every payload against the manifest's SHA-256 of the uncompressed bytes.
use std::path::Path;

const MAPS: [&str; 9] = ["raindance", "tower-complex", "cairnhold", "frostline", "dustreach", "longfield", "highgoal", "ozarktic-blast", "reefbreak"];
const PAYLOADS: [&str; 8] = ["vertices.bin", "collision.bin", "height.bin", "weights.rgba", "textures.rgba", "ambient.f32", "shade.rg", "props.bin"];
/// Payloads a pack may omit; the manifest decides whether they are read.
const OPTIONAL: [&str; 1] = ["props.bin"];

fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    let external = std::env::var_os("CARGO_FEATURE_EXTERNAL_MAP").is_some();
    println!("cargo:rerun-if-changed=build.rs");
    for map in MAPS {
        // Launcher-managed builds read Raindance from disk, never embedded.
        if external && map == "raindance" { continue; }
        let dir = Path::new(&out).join("maps").join(map);
        std::fs::create_dir_all(&dir).unwrap();
        for name in PAYLOADS {
            let source = format!("../../assets/maps/{map}/{name}");
            println!("cargo:rerun-if-changed={source}");
            let raw = match std::fs::read(&source) {
                Ok(raw) => raw,
                Err(e) if OPTIONAL.contains(&name) && e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(e) => panic!("{source}: {e}"),
            };
            let packed = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 9);
            std::fs::write(dir.join(format!("{name}.zlib")), packed).unwrap();
        }
    }
}
