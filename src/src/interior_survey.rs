//! Runs inside the game binary, on the map pack that binary loaded.
//! Reference mode must be pointed at the Broadside app's own map files.
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::time::Instant;

use peakrunner_core::interior_survey::{self, ProbeSample, StandSample};
use peakrunner_core::terrain::MapId;
use serde::Serialize;

#[derive(Serialize)]
struct FlagLocal {
    right: f32,
    forward: f32,
    up: f32,
}

#[derive(Serialize)]
struct BaseReport {
    name: String,
    origin: [f32; 3],
    flag: Option<FlagLocal>,
    probes: Vec<ProbeSample>,
    stands: Vec<StandSample>,
}

#[derive(Serialize)]
struct Report {
    method: &'static str,
    map: String,
    pack: String,
    fingerprint: String,
    step: f32,
    player_radius: f32,
    eye: f32,
    stand_note: &'static str,
    bases: Vec<BaseReport>,
}

pub fn run(path: &Path, which: &str) -> Result<(), String> {
    let (map, label) = match which {
        "skybreak" => (MapId::Skybreak, "skybreak"),
        "reference" | "broadside" => (MapId::Raindance, "reference"),
        other => return Err(format!("PEAKRUNNER_SURVEY_MAP must be reference or skybreak, not {other}")),
    };
    let pack = peakrunner_core::map_pack::on(map).ok_or("that map has no collision pack")?;
    if map == MapId::Raindance && !pack.manifest.private_reference {
        return Err("Reference survey needs PEAKRUNNER_MAP_PACK set to PeakRunner-Broadside-Reference.app/Contents/Resources/map. Refusing to survey Raindance.".into());
    }
    let frames = interior_survey::frames_for(pack, map)?;
    let names: Vec<String> = if pack.manifest.reference_bases.is_empty() {
        vec!["ember".into(), "glacier".into()]
    } else {
        pack.manifest.reference_bases.iter().map(|base| base.name.clone()).collect()
    };
    if names.len() != frames.len() {
        return Err("base names and frames do not match".into());
    }
    let step: f32 = std::env::var("PEAKRUNNER_SURVEY_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let started = Instant::now();
    let mut bases = Vec::new();
    for (frame, name) in frames.iter().zip(names) {
        let flag = interior_survey::nearest_flag_local(pack, frame);
        if let Some(flag) = flag {
            let horizontal = (flag.x * flag.x + (flag.y + 12.0) * (flag.y + 12.0)).sqrt();
            eprintln!("{name} flag local right {:.2} forward {:.2} up {:.2}", flag.x, flag.y, flag.z);
            if horizontal > 1.5 || !(12.0..18.0).contains(&flag.z) {
                return Err(format!("{name} flag local ({:.2}, {:.2}, {:.2}) is not the front room. The base frame is wrong; not writing a survey.", flag.x, flag.y, flag.z));
            }
        }
        let probes: Vec<_> = interior_survey::AUDIT_PROBES.iter()
            .map(|(probe, at)| interior_survey::probe_at(pack, frame, probe, *at))
            .collect();
        for probe in &probes {
            let rays = &probe.rays;
            eprintln!(
                "{name} {} left {:?} right {:?} back {:?} front {:?} floor {:?} ceiling {:?} body_forward {:?}",
                probe.name, rays.left, rays.right, rays.back, rays.front, rays.floor, rays.ceiling, probe.body_forward
            );
        }
        let sample_started = Instant::now();
        let stands = interior_survey::standing_samples(pack, frame, step);
        let fits = stands.iter().filter(|stand| stand.fits).count();
        eprintln!("{name} {} stands, {} fit, in {:.1}s", stands.len(), fits, sample_started.elapsed().as_secs_f32());
        bases.push(BaseReport {
            name,
            origin: frame.origin.to_array(),
            flag: flag.map(|flag| FlagLocal { right: flag.x, forward: flag.y, up: flag.z }),
            probes,
            stands,
        });
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let pack_source = std::env::var("PEAKRUNNER_MAP_PACK").unwrap_or_else(|_| "embedded".into());
    let report = Report {
        method: "MapPack::sweep radius 0 from the standing center, the private-reference overlay ray. Fit requires that ray to clear one player radius horizontally and 1.6 m overhead. body_forward uses the walking four-sphere sweep.",
        map: label.into(),
        pack: pack_source,
        fingerprint: pack.fingerprint.clone(),
        step,
        player_radius: peakrunner_core::terrain::PLAYER_RADIUS,
        eye: peakrunner_core::terrain::EYE,
        stand_note: "right, forward and floor are metres in the base frame. Clearances are capped at 80 m when the ray misses.",
        bases,
    };
    let file = File::create(path).map_err(|e| e.to_string())?;
    serde_json::to_writer(BufWriter::new(file), &report).map_err(|e| e.to_string())?;
    eprintln!("wrote {} in {:.1}s", path.display(), started.elapsed().as_secs_f32());
    Ok(())
}
