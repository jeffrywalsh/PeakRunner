//! Water volumes: flat-surfaced bodies of water declared in a map manifest.
//!
//! A volume slows anything moving through it. Physics runs in the shared
//! simulation, so a client predicting its own player agrees with the server.
//! Maps without `water_volumes` are untouched: the legacy `water` plane is
//! render-only and has no effect on movement.

use glam::{Vec2, Vec3};
use serde::Deserialize;

use crate::terrain::{MapId, EYE, PLAYER_RADIUS};

/// Body height used for immersion: feet (support contact) to just above the eye.
pub const BODY_HEIGHT: f32 = PLAYER_RADIUS + EYE + 0.2;
/// Horizontal drag rate (1/s) at full immersion; scales with immersion^1.5.
pub const SWIM_DRAG: f32 = 3.0;
/// Extra drag (1/s) while skiing in any water deeper than the ankle band.
pub const SKI_DRAG: f32 = 1.5;
/// Immersion at which the ski drag reaches full strength (roughly ankle depth).
pub const SKI_FULL_AT: f32 = 0.25;
/// Buoyancy starts above this immersion, so waders keep their footing.
pub const FLOAT_FROM: f32 = 0.6;
/// Buoyancy balances gravity at this immersion: a swimmer floats with the
/// eye near the surface.
pub const FLOAT_AT: f32 = 0.9;
/// Jetting costs this much more energy while waist-deep or deeper.
pub const JET_ENERGY_FACTOR: f32 = 1.5;
/// Immersion counted as "deep" for the jet energy cost.
pub const DEEP: f32 = 0.5;
/// Walking top speed is scaled by `1 - WADE_SLOW * immersion`.
pub const WADE_SLOW: f32 = 0.6;
/// Drag rate (1/s) on discs and grenades travelling underwater.
pub const PROJECTILE_DRAG: f32 = 5.0;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Volume {
    /// World-space surface height.
    pub surface: f32,
    /// Axis-aligned footprint `[x0, z0, x1, z1]`; exclusive with `polygon`.
    #[serde(default)]
    pub rect: Option<[f32; 4]>,
    /// Convex or simple polygon footprint `[[x, z], ...]`, 3–64 points.
    #[serde(default)]
    pub polygon: Option<Vec<[f32; 2]>>,
    /// Optional depth: the volume ends this far below the surface.
    #[serde(default)]
    pub depth: Option<f32>,
    /// Current: the water drags movement toward this horizontal velocity (m/s).
    #[serde(default)]
    pub flow: [f32; 2],
    /// Optional linear RGB used for the underwater tint.
    #[serde(default)]
    pub color: Option<[f32; 3]>,
}

impl Volume {
    pub fn validate(&self) -> Result<(), String> {
        let bad = |v: f32| !v.is_finite() || v.abs() > 10000.0;
        if bad(self.surface) {return Err("Invalid water surface".into());}
        match (&self.rect, &self.polygon) {
            (Some(r), None) => {
                if r.iter().any(|v| bad(*v)) || r[2] <= r[0] || r[3] <= r[1] {
                    return Err("Invalid water rect".into());
                }
            }
            (None, Some(p)) => {
                if p.len() < 3 || p.len() > 64 || p.iter().flatten().any(|v| bad(*v)) {
                    return Err("Invalid water polygon".into());
                }
                let area: f32 = (0..p.len()).map(|i| {
                    let (a, b) = (p[i], p[(i + 1) % p.len()]);
                    a[0] * b[1] - b[0] * a[1]
                }).sum();
                if area.abs() < 1.0 {return Err("Degenerate water polygon".into());}
            }
            _ => return Err("Water volume needs exactly one of rect or polygon".into()),
        }
        if let Some(d) = self.depth {
            if !d.is_finite() || d <= 0.0 || d > 1000.0 {return Err("Invalid water depth".into());}
        }
        if self.flow.iter().any(|v| !v.is_finite() || v.abs() > 30.0) {
            return Err("Invalid water flow".into());
        }
        if let Some(c) = self.color {
            if c.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) {
                return Err("Invalid water color".into());
            }
        }
        Ok(())
    }

    /// True when (x, z) lies inside the footprint.
    pub fn covers(&self, x: f32, z: f32) -> bool {
        if let Some(r) = self.rect {
            return x >= r[0] && x <= r[2] && z >= r[1] && z <= r[3];
        }
        let Some(p) = &self.polygon else {return false};
        let mut inside = false;
        let mut j = p.len() - 1;
        for i in 0..p.len() {
            let (a, b) = (p[i], p[j]);
            if (a[1] > z) != (b[1] > z) && x < (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0] {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// True when a point is inside the water body (below the surface).
    pub fn contains(&self, p: Vec3) -> bool {
        p.y <= self.surface
            && self.depth.map_or(true, |d| p.y >= self.surface - d)
            && self.covers(p.x, p.z)
    }

    /// Footprint outline for rendering (rect corners or the polygon).
    pub fn outline(&self) -> Vec<[f32; 2]> {
        if let Some(r) = self.rect {
            return vec![[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]];
        }
        self.polygon.clone().unwrap_or_default()
    }
}

pub fn validate(volumes: &[Volume]) -> Result<(), String> {
    if volumes.len() > 32 {return Err("Too many water volumes".into());}
    volumes.iter().try_for_each(Volume::validate)
}

/// Water declared by a map's pack. Maps without a pack have none.
pub fn volumes(map: MapId) -> &'static [Volume] {
    crate::map_pack::on(map).map_or(&[], |p| p.manifest.water_volumes.as_slice())
}

/// The volume whose water contains `p`, if any, among the map's water and any
/// staged volumes (QA and tests).
pub fn at<'a>(map: MapId, staged: &'a [Volume], p: Vec3) -> Option<&'a Volume> {
    volumes(map).iter().chain(staged).find(|v| v.contains(p))
}

/// How much of a player's body at centre `pos` is under water, 0..=1, and the
/// volume responsible. Feet are `PLAYER_RADIUS` below the centre.
pub fn immersion<'a>(map: MapId, staged: &'a [Volume], pos: Vec3) -> (f32, Option<&'a Volume>) {
    let feet = pos.y - PLAYER_RADIUS;
    let mut best = (0.0, None);
    for v in volumes(map).iter().chain(staged) {
        if !v.covers(pos.x, pos.z) {continue;}
        if let Some(d) = v.depth {
            if pos.y + EYE < v.surface - d {continue;}
        }
        let depth = ((v.surface - feet) / BODY_HEIGHT).clamp(0.0, 1.0);
        if depth > best.0 {best = (depth, Some(v));}
    }
    best
}

/// True when the eye at `eye` is under a volume's surface; returns its tint.
pub fn eye_under(map: MapId, staged: &[Volume], eye: Vec3) -> Option<[f32; 3]> {
    at(map, staged, eye).map(|v| v.color.unwrap_or([0.12, 0.28, 0.34]))
}

/// Staged volumes from the QA-only `QA_WATER` variable:
/// `surface,x0,z0,x1,z1[,flow_x,flow_z];...`. Empty unless set.
pub fn qa_staged() -> Vec<Volume> {
    let Ok(text) = std::env::var("QA_WATER") else {return Vec::new()};
    text.split(';').filter_map(|part| {
        let n: Vec<f32> = part.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if n.len() < 5 {return None;}
        let v = Volume {surface: n[0], rect: Some([n[1], n[2], n[3], n[4]]),
            flow: [n.get(5).copied().unwrap_or(0.), n.get(6).copied().unwrap_or(0.)], ..Default::default()};
        v.validate().ok().map(|_| v)
    }).collect()
}

/// Drag rate (1/s) on horizontal motion at an immersion level.
pub fn drag_rate(immersion: f32, skiing: bool) -> f32 {
    let base = SWIM_DRAG * immersion.powf(1.5);
    let ski = if skiing {SKI_DRAG * (immersion / SKI_FULL_AT).min(1.0)} else {0.0};
    base + ski
}

/// Apply water drag and buoyancy to a player's velocity. Horizontal motion
/// relaxes toward the current's flow; vertical motion is damped and lifted.
/// Call only when `immersion > 0`, so dry movement is untouched.
pub fn apply(vel: &mut Vec3, immersion: f32, skiing: bool, volume: &Volume, gravity: f32, dt: f32) {
    let k = drag_rate(immersion, skiing);
    let keep = (-k * dt).exp();
    let flow = Vec2::from(volume.flow);
    let h = Vec2::new(vel.x, vel.z);
    let h = flow + (h - flow) * keep;
    vel.x = h.x;
    vel.z = h.y;
    let lift = ((immersion - FLOAT_FROM) / (FLOAT_AT - FLOAT_FROM)).max(0.0);
    vel.y += gravity * lift * dt;
    vel.y *= (-SWIM_DRAG * immersion.powf(1.5) * dt).exp();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> Volume {
        Volume {surface: 100.0, rect: Some([0.0, 0.0, 50.0, 50.0]), ..Default::default()}
    }

    #[test]
    fn validation_rejects_bad_volumes() {
        assert!(pool().validate().is_ok());
        let both = Volume {polygon: Some(vec![[0., 0.], [1., 0.], [0., 1.]]), ..pool()};
        assert!(both.validate().is_err());
        let none = Volume {rect: None, ..pool()};
        assert!(none.validate().is_err());
        assert!(Volume {rect: Some([5., 0., 1., 5.]), ..pool()}.validate().is_err());
        assert!(Volume {depth: Some(-1.), ..pool()}.validate().is_err());
        assert!(Volume {flow: [99., 0.], ..pool()}.validate().is_err());
        assert!(Volume {color: Some([2., 0., 0.]), ..pool()}.validate().is_err());
        let flat = Volume {rect: None, polygon: Some(vec![[0., 0.], [1., 0.], [2., 0.]]), ..pool()};
        assert!(flat.validate().is_err());
        assert!(validate(&vec![pool(); 33]).is_err());
    }

    #[test]
    fn manifest_json_parses_and_rejects_unknown_fields() {
        let v: Vec<Volume> = serde_json::from_str(r#"[
            {"surface": 54, "rect": [640, 864, 1360, 1014], "flow": [1.5, 0], "color": [0.1, 0.3, 0.3]},
            {"surface": 20, "polygon": [[0, 0], [30, 0], [30, 30], [0, 30]], "depth": 4}
        ]"#).unwrap();
        assert_eq!(v.len(), 2);
        assert!(validate(&v).is_ok());
        assert!(serde_json::from_str::<Vec<Volume>>(r#"[{"surface": 1, "rect": [0,0,1,1], "speed": 3}]"#).is_err());
    }

    #[test]
    fn polygon_and_depth_containment() {
        let tri = Volume {rect: None, polygon: Some(vec![[0., 0.], [40., 0.], [0., 40.]]), depth: Some(5.), ..pool()};
        assert!(tri.contains(Vec3::new(5., 99., 5.)));
        assert!(!tri.contains(Vec3::new(30., 99., 30.)));
        assert!(!tri.contains(Vec3::new(5., 101., 5.)));
        assert!(!tri.contains(Vec3::new(5., 94., 5.)));
    }

    #[test]
    fn drag_grows_with_depth_and_skiing() {
        assert_eq!(drag_rate(0.0, false), 0.0);
        let (ankle, waist, swim) = (drag_rate(0.1, false), drag_rate(0.5, false), drag_rate(1.0, false));
        assert!(ankle < waist && waist < swim);
        assert!(drag_rate(0.1, true) > ankle + 0.5, "water brakes a skier even at the ankle");
    }

    #[test]
    fn flow_carries_a_still_swimmer() {
        let v = Volume {flow: [3.0, 0.0], ..pool()};
        let mut vel = Vec3::ZERO;
        for _ in 0..600 {apply(&mut vel, 1.0, false, &v, 20.0, 1. / 60.);}
        assert!((vel.x - 3.0).abs() < 0.05, "{vel:?}");
    }
}
