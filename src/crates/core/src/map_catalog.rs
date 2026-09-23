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

/// Only implemented rules can be advertised. DM/TDM belong to later branches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportedMode { Ctf }

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
    fn original_slots_are_embedded_and_reference_packs_are_listed_only_when_installed() {
        use crate::{map_pack,terrain::{self,MapId}};
        assert_eq!(MapId::parse("broadside-clone"),Some(MapId::BroadsideClone));
        assert_eq!(MapId::parse("stonehenge-clone"),Some(MapId::StonehengeClone));
        let listed=terrain::maps();
        for id in [MapId::BroadsideClone,MapId::StonehengeClone] {
            assert!(listed.iter().any(|m|m.id==id));
            assert!(!map_pack::on(id).unwrap().manifest.private_reference);
        }
        for id in [MapId::SnowblindClone,MapId::DesertOfDeathClone] {
            let installed=map_pack::on(id);
            assert_eq!(listed.iter().any(|m|m.id==id),installed.is_some());
            if let Some(pack)=installed { assert!(pack.manifest.private_reference); }
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
            crate::terrain::MapId::SnowblindClone, crate::terrain::MapId::DesertOfDeathClone] {
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
        value["modes"] = serde_json::json!(["deathmatch"]);
        assert!(serde_json::from_value::<MapDescriptor>(value).is_err());
        let mut value = serde_json::to_value(descriptor()).unwrap();
        value["script"] = serde_json::json!("execute-me");
        assert!(serde_json::from_value::<MapDescriptor>(value).is_err());
    }
}
