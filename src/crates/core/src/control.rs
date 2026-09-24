//! Capture & Hold control points. Pure, server-authoritative rules shared by the
//! CTF-active centre points and the Capture & Hold mode. Maps declare points in
//! the manifest (`control_points`); maps without them are unchanged.
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Uncontested presence needed to flip a point to your team.
pub const CAPTURE_SECONDS: f32 = 10.0;
/// With nobody inside, partial progress drains back at half the capture rate.
pub const DECAY_SECONDS: f32 = 20.0;
pub const DEFAULT_RADIUS: f32 = 12.0;
pub const DEFAULT_DRAIN_RADIUS: f32 = 60.0;
/// Energy per second drained from the holder's enemies inside a drain field.
/// Jetting costs 15/s and regen is 12/s, so with 10/s drained a full 60-energy
/// tank lasts (60 - 3) / 25 = 2.3 s of jetting (3.8 s normally) and refills at
/// only 2/s (12/s normally): one short hop every few seconds, never sustained flight.
pub const DEFAULT_DRAIN_RATE: f32 = 10.0;
/// Capture & Hold scoring: points per second per held point, first to the target wins.
pub const CNH_POINTS_PER_SECOND: f32 = 1.0;
pub const CNH_TARGET: u32 = 300;
/// Presence is a cylinder over the point: players stand 1.2 m above the floor.
const BELOW: f32 = 2.0;
const ABOVE: f32 = 10.0;

fn default_radius() -> f32 { DEFAULT_RADIUS }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drain {
    #[serde(default = "default_drain_radius")]
    pub radius: f32,
    #[serde(default = "default_drain_rate")]
    pub rate: f32,
}
fn default_drain_radius() -> f32 { DEFAULT_DRAIN_RADIUS }
fn default_drain_rate() -> f32 { DEFAULT_DRAIN_RATE }

/// Manifest entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub pos: [f32; 3],
    #[serde(default = "default_radius")]
    pub radius: f32,
    #[serde(default)]
    pub ctf_active: bool,
    #[serde(default)]
    pub drain: Option<Drain>,
}

pub fn validate(defs: &[Definition]) -> Result<(), String> {
    if defs.len() > 8 { return Err("Too many control points".into()); }
    let mut ids = std::collections::BTreeSet::new();
    for d in defs {
        if !crate::map_catalog::valid_id(&d.id) || !ids.insert(d.id.as_str()) {
            return Err("Invalid control point id".into());
        }
        if d.name.is_empty() || d.name.len() > 24 || d.name.trim() != d.name
            || !d.name.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-') {
            return Err("Invalid control point name".into());
        }
        if d.pos.iter().any(|v| !v.is_finite() || v.abs() > 10000.0)
            || !(2.0..=40.0).contains(&d.radius) {
            return Err("Invalid control point position/radius".into());
        }
        if let Some(drain) = &d.drain {
            if !(d.radius..=200.0).contains(&drain.radius) || !(0.0..=40.0).contains(&drain.rate) {
                return Err("Invalid control point drain".into());
            }
        }
    }
    Ok(())
}

/// Runtime point: definition plus state. Sent whole in snapshots so clients
/// draw rings and predict drain without separate map data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub name: String,
    pub pos: Vec3,
    pub radius: f32,
    pub drain_radius: f32,
    pub drain_rate: f32,
    /// Simulated and drawn in the current mode.
    pub active: bool,
    /// Owning team index, if any.
    pub owner: Option<u8>,
    /// 0..1 toward `capturing`.
    pub progress: f32,
    pub capturing: Option<u8>,
    pub contested: bool,
}

impl Point {
    pub fn from_def(d: &Definition, active: bool) -> Self {
        let drain = d.drain.clone();
        Self { name: d.name.clone(), pos: Vec3::from_array(d.pos), radius: d.radius,
            drain_radius: drain.as_ref().map_or(0.0, |x| x.radius),
            drain_rate: drain.map_or(0.0, |x| x.rate),
            active, owner: None, progress: 0.0, capturing: None, contested: false }
    }
    pub fn contains(&self, pos: Vec3) -> bool {
        let d = pos - self.pos;
        Vec3::new(d.x, 0.0, d.z).length() <= self.radius && d.y >= -BELOW && d.y <= ABOVE
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Point index, new owner, previous owner.
    Captured { point: usize, team: u8, previous: Option<u8> },
    ContestedStart { point: usize },
}

/// Advance every active point by `dt` given living players as (team, position).
pub fn step(points: &mut [Point], players: &[(u8, Vec3)], dt: f32) -> Vec<Event> {
    let mut events = Vec::new();
    for (i, p) in points.iter_mut().enumerate() {
        if !p.active { continue; }
        let mut inside = [0usize; 2];
        for &(team, pos) in players {
            if (team as usize) < 2 && p.contains(pos) { inside[team as usize] += 1; }
        }
        let contested = inside[0] > 0 && inside[1] > 0;
        if contested && !p.contested { events.push(Event::ContestedStart { point: i }); }
        p.contested = contested;
        if contested { continue; }
        let rate = dt / CAPTURE_SECONDS;
        let present = if inside[0] > 0 { Some(0u8) } else if inside[1] > 0 { Some(1u8) } else { None };
        match present {
            None => {
                p.progress = (p.progress - dt / DECAY_SECONDS).max(0.0);
                if p.progress == 0.0 { p.capturing = None; }
            }
            Some(team) if p.owner == Some(team) => {
                // Defenders standing on their point undo enemy progress.
                p.progress = (p.progress - rate).max(0.0);
                if p.progress == 0.0 { p.capturing = None; }
            }
            Some(team) => {
                if p.capturing.is_some_and(|c| c != team) && p.progress > 0.0 {
                    p.progress = (p.progress - rate).max(0.0);
                    if p.progress == 0.0 { p.capturing = None; }
                } else {
                    p.capturing = Some(team);
                    p.progress += rate;
                    if p.progress >= 1.0 - 1e-4 {
                        let previous = p.owner;
                        p.owner = Some(team);
                        p.progress = 0.0;
                        p.capturing = None;
                        events.push(Event::Captured { point: i, team, previous });
                    }
                }
            }
        }
    }
    events
}

/// Energy per second drained from a player of `team` at `pos`: the strongest
/// field of any active point held by the other team. Neutral points never drain.
pub fn drain_at(points: &[Point], team: u8, pos: Vec3) -> f32 {
    points.iter().filter(|p| p.active && p.drain_rate > 0.0 && p.owner.is_some_and(|o| o != team))
        .filter(|p| { let d = pos - p.pos; Vec3::new(d.x, 0.0, d.z).length() <= p.drain_radius })
        .map(|p| p.drain_rate).fold(0.0, f32::max)
}

/// Capture & Hold score accrual; returns whole points earned per team this step.
pub fn score(points: &[Point], acc: &mut [f32; 2], dt: f32) -> [u32; 2] {
    let mut out = [0; 2];
    for p in points.iter().filter(|p| p.active) {
        if let Some(o) = p.owner { acc[o as usize] += CNH_POINTS_PER_SECOND * dt; }
    }
    for t in 0..2 {
        // Tolerate float accumulation just shy of a whole point.
        let whole = (acc[t] + 1e-3).floor();
        out[t] = whole as u32;
        acc[t] -= whole;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    const DT: f32 = 1.0 / 60.0;
    fn point() -> Point {
        Point::from_def(&Definition { id: "beacon".into(), name: "Beacon".into(), pos: [0.0; 3], radius: 12.0,
            ctf_active: true, drain: Some(Drain { radius: 60.0, rate: DEFAULT_DRAIN_RATE }) }, true)
    }
    fn run(p: &mut [Point], players: &[(u8, Vec3)], seconds: f32) -> Vec<Event> {
        let mut all = Vec::new();
        for _ in 0..(seconds / DT).round() as usize { all.extend(step(p, players, DT)); }
        all
    }
    const IN: Vec3 = Vec3::new(3.0, 1.2, 0.0);
    #[test]
    fn ten_seconds_uncontested_flips_the_point() {
        let mut p = [point()];
        assert!(run(&mut p, &[(0, IN)], 9.9).is_empty());
        assert_eq!(p[0].owner, None);
        let e = run(&mut p, &[(0, IN)], 0.2);
        assert_eq!(e, vec![Event::Captured { point: 0, team: 0, previous: None }]);
        assert_eq!(p[0].owner, Some(0));
        // Taking it from the other team also takes 10 s.
        assert!(run(&mut p, &[(1, IN)], 9.9).is_empty());
        assert_eq!(run(&mut p, &[(1, IN)], 0.2), vec![Event::Captured { point: 0, team: 1, previous: Some(0) }]);
    }
    #[test]
    fn contested_pauses_and_empty_decays() {
        let mut p = [point()];
        run(&mut p, &[(0, IN)], 5.0);
        let before = p[0].progress;
        let e = run(&mut p, &[(0, IN), (1, IN)], 3.0);
        assert_eq!(e, vec![Event::ContestedStart { point: 0 }]);
        assert!(p[0].contested && (p[0].progress - before).abs() < 1e-5);
        run(&mut p, &[], 4.0);
        assert!(!p[0].contested && (p[0].progress - (before - 0.2)).abs() < 0.01, "decays at 1/20 per s");
        run(&mut p, &[], 20.0);
        assert_eq!((p[0].progress, p[0].capturing), (0.0, None));
    }
    #[test]
    fn enemy_progress_must_be_undone_and_outsiders_do_not_count() {
        let mut p = [point()];
        run(&mut p, &[(0, IN)], 6.0);
        run(&mut p, &[(1, IN)], 6.0); // undoes 6 s of Ember progress
        assert!(p[0].progress < 0.01 && p[0].owner.is_none());
        let far = Vec3::new(13.0, 1.2, 0.0);
        let high = Vec3::new(0.0, 11.0, 0.0);
        assert!(run(&mut p, &[(1, far), (1, high)], 12.0).is_empty());
    }
    #[test]
    fn drain_only_hits_enemies_of_the_holder() {
        let mut p = [point()];
        let near = Vec3::new(40.0, 5.0, 0.0);
        assert_eq!(drain_at(&p, 1, near), 0.0, "neutral points never drain");
        run(&mut p, &[(0, IN)], 10.1);
        assert_eq!(drain_at(&p, 1, near), DEFAULT_DRAIN_RATE);
        assert_eq!(drain_at(&p, 0, near), 0.0, "holders are unaffected");
        assert_eq!(drain_at(&p, 1, Vec3::new(61.0, 5.0, 0.0)), 0.0);
        p[0].active = false;
        assert_eq!(drain_at(&p, 1, near), 0.0);
    }
    #[test]
    fn capture_and_hold_scores_per_held_point() {
        let mut p = [point(), point()];
        p[0].owner = Some(0); p[1].owner = Some(0);
        let mut acc = [0.0; 2];
        let mut total = [0u32; 2];
        for _ in 0..600 { let s = score(&p, &mut acc, DT); total[0] += s[0]; total[1] += s[1]; }
        assert_eq!(total, [20, 0], "two held points for 10 s");
    }
    #[test]
    fn manifest_entries_are_validated() {
        let json = r#"[{"id":"beacon","name":"Beacon","pos":[1,2,3],"ctf_active":true,"drain":{}}]"#;
        let defs: Vec<Definition> = serde_json::from_str(json).unwrap();
        assert_eq!(defs[0].radius, DEFAULT_RADIUS);
        assert_eq!(defs[0].drain, Some(Drain { radius: DEFAULT_DRAIN_RADIUS, rate: DEFAULT_DRAIN_RATE }));
        assert!(validate(&defs).is_ok());
        for bad in [r#"[{"id":"Bad Id","name":"Beacon","pos":[0,0,0]}]"#,
                    r#"[{"id":"a","name":"<b>","pos":[0,0,0]}]"#,
                    r#"[{"id":"a","name":"A","pos":[0,0,0],"radius":0.5}]"#,
                    r#"[{"id":"a","name":"A","pos":[0,0,0],"drain":{"radius":1}}]"#,
                    r#"[{"id":"a","name":"A","pos":[0,0,0]},{"id":"a","name":"B","pos":[0,0,0]}]"#] {
            let defs: Vec<Definition> = serde_json::from_str(bad).unwrap();
            assert!(validate(&defs).is_err(), "{bad}");
        }
        assert!(serde_json::from_str::<Vec<Definition>>(r#"[{"id":"a","name":"A","pos":[0,0,0],"script":1}]"#).is_err());
    }
}
