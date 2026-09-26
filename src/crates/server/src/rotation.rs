//! Server-only policy. Clients never submit rotation entries.
use peakrunner_core::{map_catalog::SupportedMode, terrain::MapId};
use serde::Deserialize;

/// One rotation entry: a map in a mode, or a playlist that expands to every
/// supported map: `{"playlist":"ctf_cnh"}` (every CTF map in CTF, then every
/// Capture & Hold map in Capture & Hold), `{"playlist":"football"}`, or a single
/// mode (`"ctf"`, `"capture_and_hold"`).
#[derive(Deserialize)]
#[serde(untagged)]
enum Entry {
    Map(MapEntry),
    Playlist(PlaylistEntry),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapEntry { map: String, mode: SupportedMode }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlaylistEntry { playlist: String }

/// A playlist's (map, mode) pairs, from the maps that support each mode.
fn playlist(name: &str) -> Result<Vec<(MapId, SupportedMode)>, String> {
    use peakrunner_core::map_catalog::maps_for;
    let modes: &[SupportedMode] = match name {
        "ctf_cnh" => &[SupportedMode::Ctf, SupportedMode::CaptureAndHold],
        "ctf" => &[SupportedMode::Ctf],
        "capture_and_hold" => &[SupportedMode::CaptureAndHold],
        "football" => &[SupportedMode::Football],
        "team_deathmatch" => &[SupportedMode::TeamDeathmatch],
        _ => return Err(format!("unknown playlist {name}; use ctf_cnh, ctf, capture_and_hold, football or team_deathmatch")),
    };
    Ok(modes.iter().flat_map(|&mode| maps_for(mode).into_iter().map(move |map| (map, mode))).collect())
}

#[derive(Clone)]
pub struct Rotation { maps: Vec<(MapId, SupportedMode)>, cursor: usize }

impl Rotation {
    pub fn single(map: MapId) -> Self { Self { maps: vec![(map, SupportedMode::Ctf)], cursor: 0 } }
    pub fn parse(json: &str) -> Result<Self, String> {
        if json.len() > 8192 { return Err("rotation exceeds 8 KiB".into()); }
        let entries: Vec<Entry> = serde_json::from_str(json).map_err(|e| format!("invalid rotation: {e}"))?;
        if entries.is_empty() || entries.len() > 32 { return Err("rotation requires 1–32 entries".into()); }
        let mut maps = Vec::new();
        for entry in entries {
            match entry {
                Entry::Playlist(p) => maps.extend(playlist(&p.playlist)?),
                Entry::Map(e) => maps.push(Self::entry(e)?),
            }
        }
        if maps.is_empty() || maps.len() > 64 { return Err("rotation must expand to 1–64 matches".into()); }
        Ok(Self { maps, cursor: 0 })
    }
    fn entry(e: MapEntry) -> Result<(MapId, SupportedMode), String> {
        {
            let id = MapId::parse(&e.map).ok_or_else(|| unknown_map(&e.map))?;
            if !cfg!(test) && id == MapId::Valley { return Err(format!("Valley is retired; {AVAILABLE}")); }
            match e.mode {
                SupportedMode::Ctf => {
                    if !cfg!(test) && !peakrunner_core::map_catalog::supports(id, SupportedMode::Ctf) {
                        return Err(format!("{} is a football stadium; use \"mode\":\"football\"", e.map));
                    }
                }
                SupportedMode::Football => {
                    if !peakrunner_core::sim::football::has_field(id) {
                        return Err(format!("{} has no football field; football needs a stadium map", e.map));
                    }
                }
                // Free-for-all is implemented but not offered yet.
                SupportedMode::Deathmatch => return Err("deathmatch (free-for-all) is not offered yet; use team_deathmatch".into()),
                // Team Deathmatch runs on any map named explicitly, stadiums too;
                // its playlist leaves the stadiums out.
                SupportedMode::TeamDeathmatch => {}
                SupportedMode::CaptureAndHold => {
                    let points = peakrunner_core::map_pack::on(id).map_or(0, |p| p.manifest.control_points.len());
                    if points < 2 {
                        return Err(format!("{} has {points} control point(s); capture_and_hold needs at least 2", e.map));
                    }
                }
            }
            Ok((id, e.mode))
        }
    }
    pub fn current(&self) -> MapId { self.maps[self.cursor].0 }
    pub fn current_mode(&self) -> SupportedMode { self.maps[self.cursor].1 }
    pub fn advance(&mut self) -> MapId { self.cursor = (self.cursor + 1) % self.maps.len(); self.current() }
    pub fn reset(&mut self) -> MapId { self.cursor = 0; self.current() }
}

pub(crate) const AVAILABLE: &str =
    "use raindance, broadside-clone (Tower Complex), stonehenge-clone (Cairnhold), snowblind-clone (Frostline), desert-of-death-clone (Dustreach), ozarktic-blast, reefbreak, longfield or highgoal (football)";

/// Removed maps get a specific startup error, so an old config fails loudly.
pub(crate) fn unknown_map(key: &str) -> String {
    if key.eq_ignore_ascii_case("skybreak-bastions") {
        format!("Skybreak Bastions was removed; {AVAILABLE}")
    } else {
        format!("map is not installed: {key}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_rotation_wraps_and_resets() {
        let mut r = Rotation::parse(r#"[{"map":"raindance","mode":"ctf"},{"map":"valley","mode":"ctf"}]"#).unwrap();
        assert_eq!(r.current(), MapId::Raindance);
        assert_eq!(r.advance(), MapId::Valley);
        assert_eq!(r.advance(), MapId::Raindance);
        r.advance(); assert_eq!(r.reset(), MapId::Raindance);
    }
    #[test]
    fn invalid_policy_is_rejected_not_replaced() {
        for json in ["[]", "{}", r#"[{"map":"missing","mode":"ctf"}]"#,
            r#"[{"map":"valley","mode":"gungame"}]"#,
            r#"[{"map":"valley","mode":"ctf","script":"x"}]"#] {
            assert!(Rotation::parse(json).is_err(), "{json}");
        }
        let removed = Rotation::parse(r#"[{"map":"raindance","mode":"ctf"},{"map":"skybreak-bastions","mode":"ctf"}]"#).err().unwrap();
        assert!(removed.contains("Skybreak Bastions was removed"), "{removed}");
        // Every former clone slot holds an embedded original; none needs a pack.
        for key in ["broadside-clone", "stonehenge-clone", "snowblind-clone", "desert-of-death-clone"] {
            assert!(Rotation::parse(&format!(r#"[{{"map":"{key}","mode":"ctf"}}]"#)).is_ok(), "{key}");
        }
        // Capture & Hold needs a map with at least two control points: Old
        // Holler (key raindance) has three, the Valley fixture has none.
        assert!(Rotation::parse(r#"[{"map":"raindance","mode":"capture_and_hold"}]"#).is_ok());
        // Tower Complex (key broadside-clone) has three Capture & Hold towers.
        assert!(Rotation::parse(r#"[{"map":"broadside-clone","mode":"capture_and_hold"}]"#).is_ok());
        // Football needs a stadium map with a declared field.
        let e = Rotation::parse(r#"[{"map":"snowblind-clone","mode":"football"}]"#).err().unwrap();
        assert!(e.contains("no football field"), "{e}");
        assert!(Rotation::parse(r#"[{"map":"longfield","mode":"football"}]"#).is_ok());
        assert!(Rotation::parse(r#"[{"map":"highgoal","mode":"football"}]"#).is_ok());
        // Cairnhold (key stonehenge-clone): the Ring and two flank cairns.
        assert!(Rotation::parse(r#"[{"map":"stonehenge-clone","mode":"capture_and_hold"}]"#).is_ok());
        let cnh = Rotation::parse(r#"[{"map":"valley","mode":"capture_and_hold"}]"#).err().unwrap();
        assert!(cnh.contains("capture_and_hold needs at least 2"), "{cnh}");
        assert!(Rotation::parse(&" ".repeat(8193)).is_err());
        assert!(Rotation::parse(&format!("[{}]", vec![r#"{"map":"valley","mode":"ctf"}"#;33].join(","))).is_err());
    }

    #[test]
    fn the_deployed_vps_rotations_parse() {
        let compose = include_str!("../../../deploy/vps/compose.yaml");
        let rotations: Vec<&str> = compose.lines().filter_map(|l| l.trim().strip_prefix("PEAKRUNNER_MATCH_ROTATION: '"))
            .map(|r| r.trim_end_matches('\'')).collect();
        assert_eq!(rotations.len(), 2, "main and football servers");
        for r in rotations { let parsed = Rotation::parse(r).unwrap_or_else(|e| panic!("{r}: {e}")); assert!(!parsed.maps.is_empty()); }
    }

    #[test]
    fn playlists_expand_to_every_supported_map() {
        let r = Rotation::parse(r#"[{"playlist":"ctf_cnh"}]"#).unwrap();
        let ctf = peakrunner_core::map_catalog::maps_for(SupportedMode::Ctf);
        let cnh = peakrunner_core::map_catalog::maps_for(SupportedMode::CaptureAndHold);
        assert_eq!(r.maps.len(), ctf.len() + cnh.len());
        assert!(r.maps.iter().take(ctf.len()).all(|(m, mode)| *mode == SupportedMode::Ctf && ctf.contains(m)));
        assert!(r.maps.iter().skip(ctf.len()).all(|(_, mode)| *mode == SupportedMode::CaptureAndHold));
        let f = Rotation::parse(r#"[{"playlist":"football"}]"#).unwrap();
        assert_eq!(f.maps, vec![(MapId::Longfield, SupportedMode::Football), (MapId::Highgoal, SupportedMode::Football)]);
        // Playlists and single entries mix.
        let mixed = Rotation::parse(r#"[{"map":"raindance","mode":"ctf"},{"playlist":"football"}]"#).unwrap();
        assert_eq!(mixed.maps.len(), 3);
        // Team Deathmatch: every map but the stadiums by default; a stadium
        // only when named. Free-for-all isn't offered yet.
        let dm = Rotation::parse(r#"[{"playlist":"team_deathmatch"}]"#).unwrap();
        assert_eq!(dm.maps.len(), peakrunner_core::terrain::maps().len() - 2);
        assert!(dm.maps.iter().all(|(m, mode)| *mode == SupportedMode::TeamDeathmatch
            && ![MapId::Valley, MapId::Longfield, MapId::Highgoal].contains(m)));
        assert!(Rotation::parse(r#"[{"map":"longfield","mode":"team_deathmatch"}]"#).is_ok());
        assert!(Rotation::parse(r#"[{"map":"ozarktic-blast","mode":"deathmatch"}]"#).is_err());
        assert!(Rotation::parse(r#"[{"playlist":"deathmatch"}]"#).is_err());
        assert!(Rotation::parse(r#"[{"playlist":"gungame"}]"#).is_err());
        assert!(Rotation::parse(r#"[{"playlist":"football","extra":1}]"#).is_err());
    }
}
