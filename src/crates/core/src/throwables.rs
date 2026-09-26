//! Hand grenades (G) and mines (M), after base Tribes' Grenade and MineAmmo.
//! Both are thrown, not fired: hold the key to wind up (throw strength 0.3 to
//! 1.0 of full, as base Tribes' `throwStrength`) and release to throw. A
//! throw takes half a second (base Tribes `throwTime`).
//!
//! - A hand grenade bounces dully and explodes 2 s after the throw (base
//!   Tribes Handgrenade: explosionRadius 10, damageValue 0.5).
//! - A mine bounces to rest, arms a moment later, and goes off when an enemy
//!   comes within 2.5 m, or when a blast nearby sets it off (base Tribes
//!   AntipersonelMine: triggerRadius 2.5, explosionRadius 10, damageValue
//!   0.65). A team can have MINE_TEAM_MAX mines out; base Tribes allows 35
//!   for its larger teams.
//!
//! They travel as `Disc`s (kinds GRENADE_KIND and MINE_KIND) so snapshots and
//! rendering carry them like every other projectile. A resting one has zero
//! velocity; a resting mine's `life` counts down from MINE_LIFE, and it is
//! armed once MINE_ARM of that has passed. Marker: `throw1`.
use super::*;

pub const GRENADE_KIND: u8 = 6;
pub const MINE_KIND: u8 = 7;

/// Throw speed at full strength (m/s) and the share of the thrower's velocity
/// the throw keeps. Base Tribes throws mines harder than grenades (15 vs 9).
pub const GRENADE_THROW: f32 = 20.0;
pub const MINE_THROW: f32 = 24.0;
pub const THROW_INHERIT: f32 = 0.5;
/// Seconds between throws; the weakest throw (a tap) as a share of full; and
/// how long the key is held to wind up a full throw.
pub const THROW_TIME: f32 = 0.5;
pub const MIN_STRENGTH: f32 = 0.3;
pub const WIND_UP: f32 = 1.0;
pub const GRENADE_FUSE: f32 = 2.0;
/// How long a mine may fly or roll before it has to come to rest; one that
/// never settles (in water, say) fizzles.
pub const MINE_FLIGHT: f32 = 8.0;
/// A resting mine lasts this long, and arms after MINE_ARM of it.
pub const MINE_LIFE: f32 = 900.0;
pub const MINE_ARM: f32 = 1.0;
pub const MINE_TRIGGER: f32 = 2.5;
pub const MINE_TEAM_MAX: usize = 12;
/// An enemy blast this close (m) sets a mine off.
pub const MINE_SYMPATHY: f32 = 4.0;
/// Bounce: base Tribes' elasticity 0.15 and friction 1.0 (a dull thud that
/// kills most sliding). Slower than REST on a floor, it stops.
const ELASTICITY: f32 = 0.15;
const SLIDE_KEEP: f32 = 0.55;
const REST: f32 = 1.5;

/// What a player throws.
pub const THROW_NONE: u8 = 0;
pub const THROW_GRENADE: u8 = 1;
pub const THROW_MINE: u8 = 2;

/// Grenades and mines carried (base Tribes $ItemMax: light 5 grenades, heavy
/// 8; 3 mines each).
pub fn max_throwables(class: ArmorClass) -> [u8; 2] {
    match class { ArmorClass::Light => [5, 3], ArmorClass::Heavy => [8, 3] }
}

pub fn is_throwable(kind: u8) -> bool { kind == GRENADE_KIND || kind == MINE_KIND }

/// A mine or grenade at rest.
pub fn resting(d: &Disc) -> bool { is_throwable(d.kind) && d.vel == Vec3::ZERO }

/// A mine that is resting and armed.
pub fn armed(d: &Disc) -> bool { d.kind == MINE_KIND && resting(d) && d.life <= MINE_LIFE - MINE_ARM }

impl World {
    /// Throws requested this tick: `Input::throw` with its strength.
    pub(super) fn step_throws(&mut self, dt: f32) {
        for p in &mut self.players { p.throw_clock = (p.throw_clock - dt).max(0.0); }
        if self.predicting || self.football() { return; }
        for i in 0..self.players.len() {
            let (what, strength) = if self.players[i].is_bot { (THROW_NONE, 0.0) }
                else if self.network_inputs.is_empty() {
                    if i == self.player_id { (self.input.throw, self.input.throw_strength) } else { (THROW_NONE, 0.0) }
                } else { self.network_inputs.get(i).map_or((THROW_NONE, 0.0), |c| (c.throw, c.throw_strength)) };
            if what == THROW_NONE { continue; }
            if let Err(why) = self.throw(i, what, strength) {
                if i == self.player_id && !why.is_empty() { self.msg(why, 1.4); }
            }
        }
    }

    /// Player `i` throws a grenade or mine, wound up `strength` (0..1).
    pub(super) fn throw(&mut self, i: usize, what: u8, strength: f32) -> Result<(), &'static str> {
        let p = &self.players[i];
        if !p.alive || p.throw_clock > 0.0 { return Err(""); }
        let slot = if what == THROW_MINE { 1 } else { 0 };
        if p.throwables[slot] == 0 {
            return Err(if slot == 1 { "NO MINES LEFT" } else { "NO GRENADES LEFT" });
        }
        let kind = if slot == 1 { MINE_KIND } else { GRENADE_KIND };
        if kind == MINE_KIND && self.discs.iter().filter(|d| d.kind == MINE_KIND && d.team == p.team).count() >= MINE_TEAM_MAX {
            return Err("YOUR TEAM HAS THE MAXIMUM OF MINES OUT");
        }
        // Base Tribes: 0.3 + 0.7 x how far the throw was wound up.
        let strength = MIN_STRENGTH + (1.0 - MIN_STRENGTH) * if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 1.0 };
        let dir = look_dir(p.yaw, p.pitch);
        let eye = p.pos + Vec3::Y * EYE;
        // From the chest, a little ahead, never through a wall.
        let want = eye + dir * 0.6 - Vec3::Y * 0.2;
        let origin = match obstacle_hit(self.map, &self.pillars, eye, want, SHOT_RADIUS) {
            Some(t) => eye.lerp(want, (t - 0.1).max(0.0)),
            None => want,
        };
        let speed = if kind == MINE_KIND { MINE_THROW } else { GRENADE_THROW } * strength;
        // A slight upward lift, like an arm's throw.
        let vel = (dir + Vec3::Y * 0.15).normalize() * speed + p.vel * THROW_INHERIT;
        let team = p.team;
        self.discs.push(Disc { pos: origin, vel, team, owner: i,
            life: if kind == MINE_KIND { MINE_FLIGHT } else { GRENADE_FUSE }, kind, spin: 0.0 });
        let p = &mut self.players[i];
        p.throwables[slot] -= 1;
        p.throw_clock = THROW_TIME;
        if i == self.player_id { self.push_event("throw"); }
        else if self.network_inputs.is_empty() { self.spatial_sounds.push(("throw", origin)); }
        Ok(())
    }

    /// A grenade or mine meeting a surface at `contact` with outward
    /// `normal`: a dull bounce, coming to rest on a floor.
    pub(super) fn bounce_throwable(d: &mut Disc, contact: Vec3, normal: Vec3) {
        let inward = d.vel.dot(normal);
        if inward < 0.0 { d.vel -= normal * inward * (1.0 + ELASTICITY); }
        let along = d.vel - normal * d.vel.dot(normal);
        d.vel = normal * d.vel.dot(normal) + along * SLIDE_KEEP;
        d.pos = contact + normal * 0.12;
        if normal.y > 0.6 && d.vel.length() < REST {
            d.vel = Vec3::ZERO;
            if d.kind == MINE_KIND { d.life = MINE_LIFE; }
        }
    }

    /// Armed mines with an enemy in reach: (position, owner, team).
    pub(super) fn tripped_mines(&self) -> Vec<usize> {
        self.discs.iter().enumerate().filter(|(_, d)| armed(d) && self.players.iter().enumerate().any(|(j, p)| {
            p.alive && self.hostile(d.owner, d.team, j) && player_blast_distance(d.pos, p.pos) <= MINE_TRIGGER
        })).map(|(k, _)| k).collect()
    }

    /// Mines of other teams near a blast go off too.
    pub(super) fn sympathetic_mines(&self, pos: Vec3, team: Team) -> Vec<usize> {
        self.discs.iter().enumerate()
            .filter(|(_, d)| d.kind == MINE_KIND && resting(d) && d.team != team && d.pos.distance(pos) <= MINE_SYMPATHY)
            .map(|(k, _)| k).collect()
    }

    /// Full grenades and mines for the player's armor.
    pub(crate) fn restock_throwables(p: &mut Player) {
        p.throwables = max_throwables(p.armor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A player alone on Raindance, on open ground at their spawn.
    fn ready_world() -> World {
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        w.set_mode(crate::map_catalog::SupportedMode::Ctf);
        w.start_match(true);
        w.players.truncate(1);
        let spot = Vec3::new(1060.0, 0.0, 700.0);
        let floor = crate::terrain::support_on(MapId::Raindance, spot + Vec3::Y * 200.0).0;
        w.players[0].pos = Vec3::new(spot.x, floor + PLAYER_RADIUS, spot.z);
        for _ in 0..30 { w.physics_step(); }
        w
    }

    /// Another idle human on `team`, parked out of the way.
    fn add(w: &mut World, team: Team) -> usize {
        let mut p = w.players[0].clone();
        p.team = team;
        p.pos += Vec3::new(0.0, 0.0, 60.0);
        w.players.push(p);
        w.players.len() - 1
    }

    fn throw_one(w: &mut World, what: u8) {
        w.input.throw = what;
        w.input.throw_strength = 1.0;
        w.physics_step();
        w.input.throw = THROW_NONE;
    }

    #[test]
    fn grenades_bounce_then_explode_on_their_fuse() {
        let mut w = ready_world();
        let before = w.players[0].throwables[0];
        w.players[0].pitch = -0.3;
        throw_one(&mut w, THROW_GRENADE);
        assert_eq!(w.players[0].throwables[0], before - 1);
        assert!(w.discs.iter().any(|d| d.kind == GRENADE_KIND));
        // No second throw inside the throw time.
        throw_one(&mut w, THROW_GRENADE);
        assert_eq!(w.players[0].throwables[0], before - 1, "throw time");
        let serial = w.blast_serial;
        let mut steps = 0;
        while w.discs.iter().any(|d| d.kind == GRENADE_KIND) && steps < 400 { w.physics_step(); steps += 1; }
        let seconds = steps as f32 * STEP;
        assert!(w.blast_serial > serial, "it exploded");
        assert!((seconds - GRENADE_FUSE).abs() < 0.2, "on the fuse, not contact: {seconds}");
    }

    #[test]
    fn a_tap_throws_short_and_a_wind_up_throws_far() {
        let land = |strength: f32| {
            let mut w = ready_world();
            w.input.throw = THROW_MINE;
            w.input.throw_strength = strength;
            w.physics_step();
            w.input.throw = THROW_NONE;
            let start = w.players[0].pos;
            for _ in 0..600 { w.physics_step(); if w.discs.iter().any(resting) { break; } }
            let d = w.discs.iter().find(|d| d.kind == MINE_KIND).expect("mine");
            assert!(resting(d), "it comes to rest");
            Vec3::new(d.pos.x - start.x, 0.0, d.pos.z - start.z).length()
        };
        let (short, far) = (land(0.0), land(1.0));
        assert!(far > short * 2.0, "tap {short} m, full {far} m");
    }

    #[test]
    fn mines_arm_and_trip_on_enemies_but_not_teammates() {
        let mut w = ready_world();
        let foe = add(&mut w, Team::Glacier);
        let mate = add(&mut w, Team::Ember);
        let at = w.players[0].pos + Vec3::new(20.0, 0.0, 0.0);
        let floor = crate::terrain::support_on(w.map, at + Vec3::Y * 2.0).0;
        w.discs.push(Disc { pos: Vec3::new(at.x, floor + 0.12, at.z), vel: Vec3::ZERO, team: Team::Ember, owner: 0,
            life: MINE_LIFE, kind: MINE_KIND, spin: 0.0 });
        let mine = w.discs.last().unwrap().pos;
        for k in [foe, mate] { w.players[k].pos = mine + Vec3::new(40.0, 5.0, 0.0); }
        // A teammate standing on it: nothing.
        w.players[mate].pos = Vec3::new(mine.x, floor + PLAYER_RADIUS + 0.02, mine.z);
        for _ in 0..90 { w.physics_step(); }
        assert!(w.discs.iter().any(|d| d.kind == MINE_KIND), "teammates don't trip it");
        w.players[mate].pos = mine + Vec3::new(40.0, 5.0, 0.0);
        // An enemy walking up does.
        let health = w.players[foe].health;
        w.players[foe].pos = Vec3::new(mine.x + 2.0, floor + PLAYER_RADIUS + 0.02, mine.z);
        w.players[foe].vel = Vec3::ZERO;
        w.physics_step();
        w.physics_step();
        assert!(!w.discs.iter().any(|d| d.kind == MINE_KIND), "tripped");
        assert!(w.players[foe].health < health - 40.0 || !w.players[foe].alive, "it hurts: {}", w.players[foe].health);
    }

    #[test]
    fn unarmed_mines_wait_and_blasts_set_them_off() {
        let mut w = ready_world();
        let foe = add(&mut w, Team::Glacier);
        let at = w.players[0].pos + Vec3::new(20.0, 0.0, 0.0);
        let floor = crate::terrain::support_on(w.map, at + Vec3::Y * 2.0).0;
        let mine = Vec3::new(at.x, floor + 0.12, at.z);
        // Just settled: not armed yet, so an enemy on top is safe for a moment.
        w.discs.push(Disc { pos: mine, vel: Vec3::ZERO, team: Team::Ember, owner: 0, life: MINE_LIFE, kind: MINE_KIND, spin: 0.0 });
        assert!(!armed(&w.discs[0]));
        // An enemy blast beside it sets it off.
        w.players[foe].pos = mine + Vec3::new(60.0, 5.0, 0.0);
        let before = w.blast_serial;
        w.explode(mine + Vec3::new(2.0, 0.5, 0.0), foe, 0, Team::Glacier);
        w.physics_step();
        assert!(!w.discs.iter().any(|d| d.kind == MINE_KIND));
        assert!(w.blast_serial >= before + 2, "the disc and the mine");
    }

    #[test]
    fn one_press_throws_once_however_many_ticks_run() {
        let mut w = ready_world();
        w.input.throw = THROW_GRENADE;
        w.input.throw_strength = 0.5;
        // Several ticks in one frame, then well past the throw time.
        w.tick(STEP * 4.5);
        for _ in 0..60 { w.tick(STEP); }
        assert_eq!(w.input.throw, THROW_NONE, "the intent is used up");
        assert_eq!(w.players[0].throwables[0], max_throwables(ArmorClass::Light)[0] - 1);
    }

    #[test]
    fn carrying_limits_and_team_mine_cap() {
        let mut w = ready_world();
        assert_eq!(w.players[0].throwables, max_throwables(ArmorClass::Light));
        w.players[0].throwables = [0, 0];
        assert_eq!(w.throw(0, THROW_GRENADE, 1.0), Err("NO GRENADES LEFT"));
        assert_eq!(w.throw(0, THROW_MINE, 1.0), Err("NO MINES LEFT"));
        w.players[0].throwables = [0, 3];
        for _ in 0..MINE_TEAM_MAX {
            w.discs.push(Disc { pos: Vec3::ZERO, vel: Vec3::ZERO, team: Team::Ember, owner: 0, life: MINE_LIFE, kind: MINE_KIND, spin: 0.0 });
        }
        assert_eq!(w.throw(0, THROW_MINE, 1.0), Err("YOUR TEAM HAS THE MAXIMUM OF MINES OUT"));
        assert_eq!(max_throwables(ArmorClass::Heavy), [8, 3]);
    }
}
