//! Portable map identity metadata. Paths and executable scripts are not metadata.
//! This schema is the catalog prerequisite; runtime pack registration is separate.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapDescriptor {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub revision: u32,
    pub modes: Vec<SupportedMode>,
    pub team_spawns: [[f32; 3]; 2],
    pub extent: f32,
    pub resolution: u16,
}

/// Only implemented rules can be advertised. Deathmatch is every player for
/// themselves; Team Deathmatch scores each enemy frag for the team.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportedMode { #[default] Ctf, CaptureAndHold, Football, Deathmatch, TeamDeathmatch }

impl SupportedMode {
    pub const ALL: [SupportedMode; 5] = [SupportedMode::Ctf, SupportedMode::CaptureAndHold, SupportedMode::Football,
        SupportedMode::Deathmatch, SupportedMode::TeamDeathmatch];
    pub fn label(self) -> &'static str {
        match self { SupportedMode::Ctf => "Capture the Flag", SupportedMode::CaptureAndHold => "Capture & Hold", SupportedMode::Football => "Football",
            SupportedMode::Deathmatch => "Deathmatch", SupportedMode::TeamDeathmatch => "Team Deathmatch" }
    }
    pub fn key(self) -> &'static str {
        match self { SupportedMode::Ctf => "ctf", SupportedMode::CaptureAndHold => "capture_and_hold", SupportedMode::Football => "football",
            SupportedMode::Deathmatch => "deathmatch", SupportedMode::TeamDeathmatch => "team_deathmatch" }
    }
    /// Modes the menus and server rotations offer. Free-for-all Deathmatch
    /// is held back for now.
    pub fn offered(self) -> bool { self != SupportedMode::Deathmatch }
    /// Either deathmatch mode: frags score, no flags, random conditions.
    pub fn deathmatch(self) -> bool { matches!(self, SupportedMode::Deathmatch | SupportedMode::TeamDeathmatch) }
}

/// Whether `map` can host `mode`: stadiums (a manifest football field) host
/// Football; other maps host CTF, and Capture & Hold when they place at
/// least two capture points. Team Deathmatch runs on every other map (the
/// stadiums only by an explicit rotation entry). Free-for-all Deathmatch is
/// implemented but not offered yet (`OFFERED`).
pub fn supports(map: crate::terrain::MapId, mode: SupportedMode) -> bool {
    let pack = crate::map_pack::on(map);
    let stadium = pack.is_some_and(|p| p.manifest.football.is_some());
    match mode {
        SupportedMode::Football => stadium,
        SupportedMode::Ctf => !stadium,
        SupportedMode::CaptureAndHold => !stadium && pack.is_some_and(|p| p.manifest.control_points.len() >= 2),
        SupportedMode::Deathmatch | SupportedMode::TeamDeathmatch => !stadium,
    }
}

/// Every listed map that hosts `mode`, in menu order.
pub fn maps_for(mode: SupportedMode) -> Vec<crate::terrain::MapId> {
    crate::terrain::maps().into_iter().map(|m| m.id).filter(|&id| supports(id, mode)).collect()
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-') && !id.ends_with('-') && !id.contains("--")
}

impl MapDescriptor {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != 1 || !valid_id(&self.id) || self.revision == 0 {
            return Err("invalid map identity/schema");
        }
        if self.name.trim() != self.name || self.name.is_empty() || self.name.len() > 80
            || self.name.chars().any(|c| c.is_control() || !c.is_ascii()) {
            return Err("invalid map display name");
        }
        if self.modes.is_empty() || self.modes.len() > 8
            || self.modes.iter().collect::<BTreeSet<_>>().len() != self.modes.len() {
            return Err("invalid supported modes");
        }
        if !self.extent.is_finite() || !(64.0..=8192.0).contains(&self.extent)
            || !(2..=256).contains(&self.resolution) {
            return Err("invalid map dimensions");
        }
        if self.team_spawns.iter().any(|p| p.iter().any(|v| !v.is_finite())
            || p[0] < 0.0 || p[0] > self.extent || p[2] < 0.0 || p[2] > self.extent
            || p[1].abs() > 10000.0) {
            return Err("invalid team spawn region");
        }
        Ok(())
    }
}

/// Validate before installing a catalog: duplicate IDs must never shadow a map.
pub fn validate_catalog(maps: &[MapDescriptor]) -> Result<(), &'static str> {
    if maps.is_empty() || maps.len() > 64 { return Err("invalid catalog size"); }
    let mut ids = BTreeSet::new();
    for map in maps {
        map.validate()?;
        if !ids.insert(&map.id) { return Err("duplicate map identity"); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_rotation_slot_is_an_embedded_original_map() {
        use crate::{map_pack,terrain::{self,MapId}};
        let listed=terrain::maps();
        let slots=[(MapId::Raindance,"raindance"),(MapId::BroadsideClone,"broadside-clone"),
            (MapId::StonehengeClone,"stonehenge-clone"),(MapId::SnowblindClone,"snowblind-clone"),
            (MapId::DesertOfDeathClone,"desert-of-death-clone"),(MapId::Longfield,"longfield"),(MapId::Highgoal,"highgoal"),(MapId::OzarkticBlast,"ozarktic-blast"),(MapId::Reefbreak,"reefbreak")];
        assert_eq!(listed.len(),slots.len());
        for (id,key) in slots {
            assert_eq!(MapId::parse(key),Some(id));
            assert!(listed.iter().any(|m|m.id==id),"{key} listed");
            assert!(!map_pack::on(id).expect("embedded pack").manifest.private_reference,"{key} is original");
        }
    }
    fn descriptor() -> MapDescriptor {
        MapDescriptor { schema: 1, id: "test-map".into(), name: "Test map".into(),
            revision: 1, modes: vec![SupportedMode::Ctf], team_spawns: [[20., 10., 20.], [220., 10., 220.]],
            extent: 256., resolution: 128 }
    }
    #[test]
    fn identities_are_safe_and_unambiguous() {
        for id in [crate::terrain::MapId::Valley, crate::terrain::MapId::Raindance,
            crate::terrain::MapId::BroadsideClone, crate::terrain::MapId::StonehengeClone,
            crate::terrain::MapId::SnowblindClone, crate::terrain::MapId::DesertOfDeathClone, crate::terrain::MapId::Longfield, crate::terrain::MapId::Highgoal, crate::terrain::MapId::OzarkticBlast, crate::terrain::MapId::Reefbreak] {
            assert_eq!(crate::terrain::MapId::parse(id.key()), Some(id));
            assert!(valid_id(id.key()));
        }
        assert_eq!(crate::terrain::MapId::parse("RAINDANCE"), Some(crate::terrain::MapId::Raindance));
        assert_eq!(crate::terrain::MapId::parse("../raindance"), None);
        // Retired: stored preferences and old rotation configs name it.
        assert_eq!(crate::terrain::MapId::parse("skybreak-bastions"), None);
        for id in ["", "../raindance", "a/b", "A", "a\\b", "a b", "-a", "a-", "a--b", ".", "é"] {
            assert!(!valid_id(id), "{id}");
        }
        assert!(!valid_id(&"a".repeat(49)));
        assert!(valid_id("rift-crossing-2"));
        assert!(validate_catalog(&[descriptor()]).is_ok());
        assert!(validate_catalog(&[descriptor(), descriptor()]).is_err());
    }
    #[test]
    fn reject_invalid_bounds_modes_and_versions() {
        let mut d = descriptor(); d.extent = f32::NAN; assert!(d.validate().is_err());
        let mut d = descriptor(); d.team_spawns[0][0] = 257.; assert!(d.validate().is_err());
        let mut d = descriptor(); d.team_spawns[0][1] = f32::INFINITY; assert!(d.validate().is_err());
        let mut d = descriptor(); d.resolution = 257; assert!(d.validate().is_err());
        let mut d = descriptor(); d.modes.clear(); assert!(d.validate().is_err());
        let mut d = descriptor(); d.modes.push(SupportedMode::Ctf); assert!(d.validate().is_err());
        let mut d = descriptor(); d.schema = 2; assert!(d.validate().is_err());
        let mut d = descriptor(); d.revision = 0; assert!(d.validate().is_err());
        let mut value = serde_json::to_value(descriptor()).unwrap();
        value["modes"] = serde_json::json!(["gungame"]);
        assert!(serde_json::from_value::<MapDescriptor>(value).is_err());
        let mut value = serde_json::to_value(descriptor()).unwrap();
        value["script"] = serde_json::json!("execute-me");
        assert!(serde_json::from_value::<MapDescriptor>(value).is_err());
    }

    #[test]
    fn each_mode_lists_only_maps_that_host_it() {
        use crate::terrain::MapId;
        let ctf = maps_for(SupportedMode::Ctf);
        assert_eq!(ctf, vec![MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone, MapId::SnowblindClone, MapId::DesertOfDeathClone, MapId::OzarkticBlast, MapId::Reefbreak]);
        assert_eq!(maps_for(SupportedMode::Football), vec![MapId::Longfield, MapId::Highgoal]);
        let cnh = maps_for(SupportedMode::CaptureAndHold);
        assert!(!cnh.is_empty() && cnh.iter().all(|m| ctf.contains(m)));
    }
}
