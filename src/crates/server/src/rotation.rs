//! Server-only policy. Clients never submit rotation entries.
use peakrunner_core::{map_catalog::SupportedMode, terrain::MapId};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry { map: String, mode: SupportedMode }

#[derive(Clone)]
pub struct Rotation { maps: Vec<(MapId, SupportedMode)>, cursor: usize }

impl Rotation {
    pub fn single(map: MapId) -> Self { Self { maps: vec![(map, SupportedMode::Ctf)], cursor: 0 } }
    pub fn parse(json: &str) -> Result<Self, String> {
        if json.len() > 8192 { return Err("rotation exceeds 8 KiB".into()); }
        let entries: Vec<Entry> = serde_json::from_str(json).map_err(|e| format!("invalid rotation: {e}"))?;
        if entries.is_empty() || entries.len() > 32 { return Err("rotation requires 1–32 entries".into()); }
        let maps = entries.into_iter().map(|e| {
            let id = MapId::parse(&e.map).ok_or_else(|| unknown_map(&e.map))?;
            if !cfg!(test) && id == MapId::Valley { return Err(format!("Valley is retired; {AVAILABLE}")); }
            match e.mode {
                SupportedMode::Ctf => {}
                SupportedMode::CaptureAndHold => {
                    let points = peakrunner_core::map_pack::on(id).map_or(0, |p| p.manifest.control_points.len());
                    if points < 2 {
                        return Err(format!("{} has {points} control point(s); capture_and_hold needs at least 2", e.map));
                    }
                }
            }
            Ok((id, e.mode))
        }).collect::<Result<Vec<_>, _>>()?;
        Ok(Self { maps, cursor: 0 })
    }
    pub fn current(&self) -> MapId { self.maps[self.cursor].0 }
    pub fn current_mode(&self) -> SupportedMode { self.maps[self.cursor].1 }
    pub fn advance(&mut self) -> MapId { self.cursor = (self.cursor + 1) % self.maps.len(); self.current() }
    pub fn reset(&mut self) -> MapId { self.cursor = 0; self.current() }
}

pub(crate) const AVAILABLE: &str =
    "use raindance, broadside-clone (Tower Complex), stonehenge-clone (Cairnhold), snowblind-clone (Frostline) or desert-of-death-clone (Dustreach)";

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
            r#"[{"map":"valley","mode":"deathmatch"}]"#,
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
        let cnh = Rotation::parse(r#"[{"map":"valley","mode":"capture_and_hold"}]"#).err().unwrap();
        assert!(cnh.contains("capture_and_hold needs at least 2"), "{cnh}");
        assert!(Rotation::parse(&" ".repeat(8193)).is_err());
        assert!(Rotation::parse(&format!("[{}]", vec![r#"{"map":"valley","mode":"ctf"}"#;33].join(","))).is_err());
    }
}
