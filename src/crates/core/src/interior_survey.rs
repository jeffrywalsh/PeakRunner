//! Standing interior survey using the same structure ray as the reference overlay.
//!
//! Local axes are [right, forward, up] metres. Up is the deck normal stored
//! with a private reference base, which is world Y for both Broadside bases.
//! A fit cell is a place a player sphere can stand: the overlay ray, cast from
//! the standing center, clears at least one player radius on every horizontal
//! axis and 1.6 m overhead.
use serde::Serialize;

use crate::map_pack::{MapPack, ReferenceBase};
use crate::terrain::{MapId, PLAYER_RADIUS};
use glam::Vec3;

/// Named points carried over from the collision audit so the overlay ray can
/// be compared with those older outside measurements. Not a room list.
pub const AUDIT_PROBES: &[(&str, [f32; 3])] = &[
    ("entry", [0.0, -29.0, 2.0]),
    ("lower_inventory", [0.0, 2.0, -4.0]),
    ("hall", [0.0, 10.0, 9.0]),
    ("flag", [0.0, -12.0, 16.0]),
    ("gallery", [-15.0, 18.0, 16.0]),
    ("armory", [0.0, -12.0, 27.0]),
    ("spine", [6.0, 12.0, 33.0]),
    ("generator", [8.0, 15.0, 40.0]),
    ("defence", [8.0, 0.0, 47.0]),
    ("roof", [8.0, 0.0, 59.0]),
];

#[derive(Clone, Copy)]
pub struct BaseFrame {
    pub origin: Vec3,
    pub right: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
}

impl BaseFrame {
    pub fn to_local(&self, world: Vec3) -> Vec3 {
        let delta = world - self.origin;
        Vec3::new(self.right.dot(delta), self.forward.dot(delta), self.up.dot(delta))
    }

    pub fn to_world(&self, local: Vec3) -> Vec3 {
        self.origin + self.right * local.x + self.forward * local.y + self.up * local.z
    }
}

pub fn skybreak_frames() -> [BaseFrame; 2] {
    // Mesh.point yaw: world offset is (right*sign, up, forward*sign).
    // Ember yaw is 180 degrees, glacier yaw is 0. Matches the fortress tests.
    [
        signed_frame(Vec3::new(1024.0, 240.0, 875.0), -1.0),
        signed_frame(Vec3::new(1024.0, 240.0, 1173.0), 1.0),
    ]
}

fn signed_frame(origin: Vec3, sign: f32) -> BaseFrame {
    BaseFrame {
        origin,
        right: Vec3::new(sign, 0.0, 0.0),
        forward: Vec3::new(0.0, 0.0, sign),
        up: Vec3::Y,
    }
}

pub fn reference_frames(bases: &[ReferenceBase]) -> Vec<BaseFrame> {
    bases.iter().map(|base| {
        let rows = &base.world_to_local;
        // Rows, matching the overlay: local[i] = row[i] dot (world - origin).
        BaseFrame {
            origin: Vec3::from_array(base.position),
            right: Vec3::from_array(rows[0]),
            forward: Vec3::from_array(rows[1]),
            up: Vec3::from_array(rows[2]),
        }
    }).collect()
}

pub fn frames_for(pack: &MapPack, map: MapId) -> Result<Vec<BaseFrame>, String> {
    if !pack.manifest.reference_bases.is_empty() {
        return Ok(reference_frames(&pack.manifest.reference_bases));
    }
    if map == MapId::Skybreak {
        return Ok(skybreak_frames().to_vec());
    }
    Err("This map has no base frame. Refusing to guess one.".into())
}

/// Distance along `direction` to the first solid face. This is the private
/// reference overlay: radius 0, `MapPack::sweep`, reported in metres.
pub fn structure_distance(pack: &MapPack, origin: Vec3, direction: Vec3, reach: f32) -> Option<f32> {
    let direction = direction.normalize_or_zero();
    if direction.length_squared() < 0.5 || reach <= 0.0 { return None; }
    pack.sweep(origin, origin + direction * reach, 0.0).map(|(t, _)| t * reach)
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct AxisRays {
    pub left: Option<f32>,
    pub right: Option<f32>,
    pub back: Option<f32>,
    pub front: Option<f32>,
    pub floor: Option<f32>,
    pub ceiling: Option<f32>,
}

pub fn structure_rays(pack: &MapPack, origin: Vec3, frame: &BaseFrame, reach: f32) -> AxisRays {
    let cast = |direction: Vec3| structure_distance(pack, origin, direction, reach);
    AxisRays {
        left: cast(-frame.right),
        right: cast(frame.right),
        back: cast(-frame.forward),
        front: cast(frame.forward),
        floor: cast(-frame.up),
        ceiling: cast(frame.up),
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ProbeSample {
    pub name: String,
    pub at: [f32; 3],
    pub rays: AxisRays,
    /// How far the walking body can move forward before a non-floor contact.
    pub body_forward: Option<f32>,
}

pub fn probe_at(pack: &MapPack, frame: &BaseFrame, name: &str, at: [f32; 3]) -> ProbeSample {
    let origin = frame.to_world(Vec3::new(at[0], at[1], at[2]));
    let end = frame.to_world(Vec3::new(at[0], at[1] + 6.0, at[2]));
    let body_forward = pack.body_sweep(origin, end).and_then(|(t, normal)| {
        if frame.up.dot(normal) > 0.7 { None } else { Some(t * 6.0) }
    });
    ProbeSample {
        name: name.to_string(),
        at,
        rays: structure_rays(pack, origin, frame, 150.0),
        body_forward,
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct StandSample {
    pub right: f32,
    pub forward: f32,
    pub floor: f32,
    pub headroom: f32,
    pub left: f32,
    pub right_clear: f32,
    pub back: f32,
    pub front: f32,
    pub fits: bool,
}

const OPEN: f32 = 80.0;

fn clearance(value: Option<f32>) -> f32 { value.unwrap_or(OPEN) }

/// Floors under one horizontal cell. `step` scales the authored sample; the
/// ray origin is the standing center the overlay would use on that floor.
pub fn column_stands(pack: &MapPack, frame: &BaseFrame, right: f32, forward: f32) -> Vec<StandSample> {
    let mut height = 78.0;
    let mut samples = Vec::new();
    for _ in 0..48 {
        let origin = frame.to_world(Vec3::new(right, forward, height));
        let end = frame.to_world(Vec3::new(right, forward, height - 160.0));
        let Some((t, normal)) = pack.sweep(origin, end, 0.0) else { break };
        let local = frame.to_local(origin.lerp(end, t));
        if local.z >= height - 0.05 { break; }
        if frame.up.dot(normal) > 0.45 && (-70.0..74.0).contains(&local.z) {
            let center = frame.to_world(Vec3::new(right, forward, local.z + PLAYER_RADIUS));
            let rays = structure_rays(pack, center, frame, OPEN);
            let left = clearance(rays.left);
            let right_clear = clearance(rays.right);
            let back = clearance(rays.back);
            let front = clearance(rays.front);
            let headroom = clearance(rays.ceiling);
            // Clearance is to the face, from the standing center. The body
            // penetrates anything closer than its radius; a 1 m wall is not a hall.
            let room = PLAYER_RADIUS - 0.01;
            let fits = left >= room && right_clear >= room && back >= room && front >= room && headroom >= 1.6;
            let fresh = samples.last().is_none_or(|prior: &StandSample| (prior.floor - local.z).abs() > 0.35);
            if fresh {
                samples.push(StandSample {
                    right, forward, floor: local.z, headroom, left, right_clear, back, front, fits,
                });
            }
        }
        let next = local.z - 0.3;
        if next >= height - 0.01 { break; }
        height = next;
    }
    samples
}

pub fn standing_samples(pack: &MapPack, frame: &BaseFrame, step: f32) -> Vec<StandSample> {
    let step = if step.is_finite() && step >= 0.5 && step <= 4.0 { step } else { 1.0 };
    let extent = |limit: f32| (limit / step).round() as i32;
    let mut samples = Vec::new();
    for ix in -extent(46.0)..=extent(46.0) {
        for iz in -extent(56.0)..=extent(56.0) {
            samples.extend(column_stands(pack, frame, ix as f32 * step, iz as f32 * step));
        }
    }
    samples
}

pub fn nearest_flag_local(pack: &MapPack, frame: &BaseFrame) -> Option<Vec3> {
    pack.manifest.flags.iter()
        .map(|flag| frame.to_local(Vec3::from_array(*flag)))
        .min_by(|a, b| {
            let ah = a.x * a.x + a.y * a.y;
            let bh = b.x * b.x + b.y * b.y;
            ah.total_cmp(&bh)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::MapId;

    #[test]
    fn skybreak_survey_frame_matches_the_fortress_flag() {
        let pack = crate::map_pack::on(MapId::Skybreak).unwrap();
        let frame = &skybreak_frames()[0];
        let flag = nearest_flag_local(pack, frame).unwrap();
        assert!(flag.x.abs() < 0.05, "{flag:?}");
        assert!((flag.y + 12.0).abs() < 0.05, "{flag:?}");
        assert!((flag.z - 15.05).abs() < 0.05, "{flag:?}");
        let world = frame.to_world(Vec3::new(0.0, -12.0, 15.05));
        assert!((world - Vec3::new(1024.0, 255.05, 887.0)).length() < 0.02);
    }

    #[test]
    fn reference_frame_matches_the_overlay_matrix() {
        let base = crate::map_pack::ReferenceBase {
            name: "Base".into(),
            position: [10.0, 20.0, 30.0],
            world_to_local: [[0.866, 0.0, 0.5], [-0.5, 0.0, 0.866], [0.0, 1.0, 0.0]],
        };
        let frame = &reference_frames(std::slice::from_ref(&base))[0];
        let world = Vec3::new(11.0, 24.0, 32.0);
        let local = frame.to_local(world);
        assert!((local.x - 1.866).abs() < 1e-3 && (local.y - 1.232).abs() < 1e-3 && (local.z - 4.0).abs() < 1e-3, "{local:?}");
        assert!((frame.to_world(local) - world).length() < 1e-3);
    }

    #[test]
    fn skybreak_hall_stand_matches_the_overlay_ray() {
        let pack = crate::map_pack::on(MapId::Skybreak).unwrap();
        let frame = &skybreak_frames()[0];
        let stands = column_stands(pack, frame, 0.0, 10.0);
        let hall = stands.iter().find(|s| (s.floor - 7.0).abs() < 0.2).expect("hall floor");
        assert!(hall.fits, "{hall:?}");
        assert!((hall.left - 11.0).abs() < 0.15 && (hall.right_clear - 11.0).abs() < 0.15, "{hall:?}");
        assert!((hall.back - 17.5).abs() < 0.15 && (hall.front - 12.5).abs() < 0.15, "{hall:?}");
        // Probe at height 9 reads a 14 m ceiling, so the lid is near 23, not 14.
        assert!((hall.headroom - 15.48).abs() < 0.25, "{hall:?}");
        let rays = probe_at(pack, frame, "hall", [0.0, 10.0, 9.0]).rays;
        assert!((rays.left.unwrap() - 11.0).abs() < 0.15);
        assert!((rays.right.unwrap() - 11.0).abs() < 0.15);
    }

    #[test]
    fn skybreak_hall_wall_is_not_a_standing_cell() {
        let pack = crate::map_pack::on(MapId::Skybreak).unwrap();
        let frame = &skybreak_frames()[0];
        let stands = column_stands(pack, frame, 11.5, 10.0);
        let wall: Vec<_> = stands.iter().filter(|s| (s.floor - 7.0).abs() < 0.4).collect();
        assert!(!wall.is_empty(), "{stands:?}");
        assert!(wall.iter().all(|s| !s.fits), "{wall:?}");
    }
}
