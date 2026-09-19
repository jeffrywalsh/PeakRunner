//! Server-only policy. Clients never submit rotation entries.
use peakrunner_core::{map_catalog::SupportedMode, terrain::MapId};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry { map: String, mode: SupportedMode }

#[derive(Clone)]
pub struct Rotation { maps: Vec<MapId>, cursor: usize }

impl Rotation {
    pub fn single(map: MapId) -> Self { Self { maps: vec![map], cursor: 0 } }
    pub fn parse(json: &str) -> Result<Self, String> {
        if json.len() > 8192 { return Err("rotation exceeds 8 KiB".into()); }
        let entries: Vec<Entry> = serde_json::from_str(json).map_err(|e| format!("invalid rotation: {e}"))?;
        if entries.is_empty() || entries.len() > 32 { return Err("rotation requires 1–32 entries".into()); }
        let maps = entries.into_iter().map(|e| {
            match e.mode { SupportedMode::Ctf => {} }
            let id = MapId::parse(&e.map).ok_or_else(|| format!("map is not installed: {}", e.map))?;
            if !cfg!(test) && id == MapId::Valley { return Err("Valley is retired; use raindance or skybreak-bastions".to_string()); }
            Ok(id)
        }).collect::<Result<Vec<_>, _>>()?;
        Ok(Self { maps, cursor: 0 })
    }
    pub fn current(&self) -> MapId { self.maps[self.cursor] }
    pub fn advance(&mut self) -> MapId { self.cursor = (self.cursor + 1) % self.maps.len(); self.current() }
    pub fn reset(&mut self) -> MapId { self.cursor = 0; self.current() }
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
            assert!(Rotation::parse(json).is_err());
        }
        assert!(Rotation::parse(&" ".repeat(8193)).is_err());
        assert!(Rotation::parse(&format!("[{}]", vec![r#"{"map":"valley","mode":"ctf"}"#;33].join(","))).is_err());
    }
}
