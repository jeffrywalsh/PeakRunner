//! Player-to-player collision, in every mode: every body blocks every other,
//! teammates included (as bodies did in the Tribes engine), so defenders can
//! block flag carriers and teammates can't walk through each other. Hits are
//! swept across the physics step, so fast skiers can't pass through each other
//! between ticks. Teammates only ever push each other apart: no damage, no
//! tackles. Between enemies, outside Football a hit does light damage; in
//! Football the bodies swap velocities outright and the slower player loses
//! (`football::World::football_hit`, the classic mod's rule), and nobody
//! tackles a player who is down (they still block). Server and offline only; client
//! prediction does not resolve hits against other players. Marker: `bump1`.
use super::*;

/// Bodies are cylinders the size of the base game's light armor collision
/// box (0.5 each side of the centre, 2.3 m tall).
const BODY_RADIUS: f32 = 0.5;
/// Centres closer than a body's full height can touch: a player dropping
/// onto another's head connects.
const BODY_HEIGHT: f32 = 2.3;
/// Speed along the line of impact passes from one body to the other almost
/// whole: a blocker stops a skier and is knocked away themselves.
const RESTITUTION: f32 = 0.8;
/// Light damage outside football: nothing for a walking bump, a few points
/// for a running collision, at most 20 for a full-speed hit.
const BUMP_FREE: f32 = 8.0;
const BUMP_SCALE: f32 = 0.45;
const BUMP_MAX: f32 = 20.0;

/// Damage to each player from the velocity change `dv` a hit gave them.
pub fn bump_damage(dv: f32) -> f32 {
    ((dv - BUMP_FREE).max(0.0) * BUMP_SCALE).min(BUMP_MAX).floor()
}

/// First moment in [0, 1] when bodies moving from `rel0` to `rel1` (b minus
/// a, horizontal) come within `reach`, if they do this tick. Catches fast
/// players who would otherwise pass through each other between ticks.
fn contact_time(rel0: Vec2, rel1: Vec2, reach: f32) -> Option<f32> {
    if rel0.length() < reach { return Some(0.0); }
    let d = rel1 - rel0;
    let (qa, qb, qc) = (d.dot(d), 2.0 * rel0.dot(d), rel0.dot(rel0) - reach * reach);
    if qa < 1e-9 { return None; }
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 { return None; }
    let t = (-qb - disc.sqrt()) / (2.0 * qa);
    (0.0..=1.0).contains(&t).then_some(t)
}

impl World {
    /// Resolve hits between enemies, sweeping each pair from `before` (the
    /// positions at the start of this physics step) to now.
    pub(super) fn step_bumps(&mut self, before: &[Vec3]) {
        if self.predicting { return; }
        let n = self.players.len();
        let football = self.football();
        for a in 0..n {
            for b in a + 1..n {
                let (pa, pb) = (&self.players[a], &self.players[b]);
                if !pa.alive || !pb.alive || pa.remote || pb.remote { continue; }
                // Teammates, and anyone touching a player who is down, only
                // push apart: the game rules below are for enemy hits.
                let enemies = self.hostile(a, pa.team, b);
                let plays = enemies && !(football && (pa.stun > 0.0 || pb.stun > 0.0));
                let (a1, b1) = (pa.pos, pb.pos);
                // Sweep from the start of the step unless someone teleported.
                let (a0, b0) = match (before.get(a), before.get(b)) {
                    (Some(&a0), Some(&b0)) if a0.distance(a1) < 10.0 && b0.distance(b1) < 10.0 => (a0, b0),
                    _ => (a1, b1),
                };
                let flat = |v: Vec3| Vec2::new(v.x, v.z);
                let Some(t) = contact_time(flat(b0 - a0), flat(b1 - a1), BODY_RADIUS * 2.0) else { continue };
                let (at_a, at_b) = (a0.lerp(a1, t), b0.lerp(b1, t));
                if (at_b.y - at_a.y).abs() >= BODY_HEIGHT { continue; }
                // A pass-through is caught where the bodies first touched.
                if t > 0.0 { self.players[a].pos = at_a; self.players[b].pos = at_b; }
                let d = at_b - at_a;
                let flat = Vec2::new(d.x, d.z);
                let h = flat.length();
                let normal = if h > 1e-3 { Vec3::new(flat.x / h, 0.0, flat.y / h) } else { Vec3::X };
                // Separate, never through a wall: whoever can't move stays put.
                let push = normal * ((BODY_RADIUS * 2.0 - h) * 0.5);
                for (i, dir) in [(a, -1.0f32), (b, 1.0)] {
                    let from = self.players[i].pos;
                    let to = from + push * dir;
                    if obstacle_hit(self.map, &self.pillars, from, to, PLAYER_RADIUS * 0.5).is_none() {
                        self.players[i].pos = to;
                    }
                }
                let (va, vb) = (self.players[a].vel, self.players[b].vel);
                let closing = (va - vb).dot(normal);
                if closing <= 0.0 { continue; }
                // Heavier armor is harder to shove: the exchange is shared by mass.
                let (ma, mb) = if football { (1.0, 1.0) } else {
                    (loadout::mass_scale(self.players[a].armor), loadout::mass_scale(self.players[b].armor)) };
                let impulse = closing * (1.0 + RESTITUTION) / (1.0 / ma + 1.0 / mb);
                let (dva, dvb) = (impulse / ma, impulse / mb);
                let dv = dva.max(dvb);
                if football && plays {
                    // As in the classic football mod: the two bodies swap
                    // velocities outright, so a tackler stops dead and the
                    // player hit flies on at the tackler's speed.
                    self.players[a].vel = vb;
                    self.players[b].vel = va;
                } else {
                    self.players[a].vel -= normal * dva;
                    self.players[b].vel += normal * dvb;
                }
                let at = (self.players[a].pos + self.players[b].pos) * 0.5;
                if a == self.player_id || b == self.player_id {
                    self.push_event("bump");
                    self.trauma = (self.trauma + (dv / 40.0).min(0.5)).min(1.0);
                } else if self.network_inputs.is_empty() {
                    self.spatial_sounds.push(("bump", at));
                }
                if !plays { continue; }
                if football {
                    self.football_hit(a, b, va, vb);
                    continue;
                }
                for (i, other, dv) in [(a, b, dva), (b, a, dvb)] {
                    let damage = bump_damage(dv);
                    if damage <= 0.0 || !self.players[i].alive { continue; }
                    self.hurt(i, damage);
                    if self.players[i].health <= 0.0 { self.kill(i, Some(other), "Body check"); }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (World, usize, usize) {
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        w.start_match(true);
        w.players.truncate(1);
        let mut enemy = w.players[0].clone();
        enemy.team = Team::Glacier;
        enemy.is_bot = false;
        w.players.push(enemy);
        (w, 0, 1)
    }

    #[test]
    fn a_blocker_stops_a_fast_carrier_and_both_take_light_damage() {
        let (mut w, a, b) = pair();
        let at = w.players[a].pos;
        w.players[b].pos = at + Vec3::new(0.8, 0.0, 0.0);
        w.players[a].vel = Vec3::new(40.0, 0.0, 0.0);
        w.players[b].vel = Vec3::ZERO;
        w.step_bumps(&w.players.iter().map(|p| p.pos).collect::<Vec<_>>());
        assert!(w.players[a].vel.x < 6.0, "the runner is stopped: {}", w.players[a].vel.x);
        assert!(w.players[b].vel.x > 30.0, "the blocker is knocked on");
        let taken = 100.0 - w.players[a].health;
        assert!(taken > 0.0 && taken <= BUMP_MAX, "{taken}");
        assert_eq!(w.players[a].health, w.players[b].health);
        assert!(w.players[a].pos.distance(w.players[b].pos) >= BODY_RADIUS * 2.0 - 1e-3);
    }

    #[test]
    fn walking_bumps_do_no_damage_and_teammates_block_without_harm() {
        assert_eq!(bump_damage(5.0), 0.0);
        assert!(bump_damage(20.0) > 0.0 && bump_damage(20.0) <= 6.0);
        assert_eq!(bump_damage(200.0), BUMP_MAX);
        let (mut w, a, b) = pair();
        w.players[b].team = Team::Ember;
        let at = w.players[a].pos;
        w.players[b].pos = at + Vec3::new(0.5, 0.0, 0.0);
        w.players[a].vel = Vec3::new(30.0, 0.0, 0.0);
        w.step_bumps(&w.players.iter().map(|p| p.pos).collect::<Vec<_>>());
        assert!(w.players[a].vel.x < 6.0, "a teammate blocks the runner: {}", w.players[a].vel.x);
        assert!(w.players[a].pos.distance(w.players[b].pos) >= 2.0 * BODY_RADIUS - 1e-3, "pushed apart");
        assert_eq!((w.players[a].health, w.players[b].health), (100.0, 100.0), "no friendly damage");
    }

    #[test]
    fn separating_players_are_left_alone_and_a_fatal_hit_credits_the_other() {
        let (mut w, a, b) = pair();
        let at = w.players[a].pos;
        w.players[b].pos = at + Vec3::new(0.8, 0.0, 0.0);
        w.players[a].vel = Vec3::new(-5.0, 0.0, 0.0);
        w.step_bumps(&w.players.iter().map(|p| p.pos).collect::<Vec<_>>());
        assert_eq!(w.players[a].vel.x, -5.0);
        w.players[a].health = 3.0;
        w.players[b].pos = w.players[a].pos + Vec3::new(0.8, 0.0, 0.0);
        w.players[a].vel = Vec3::new(60.0, 0.0, 0.0);
        w.step_bumps(&w.players.iter().map(|p| p.pos).collect::<Vec<_>>());
        assert!(!w.players[a].alive);
        assert_eq!(w.players[b].frags, 1);
    }

    #[test]
    fn fast_players_cannot_pass_through_each_other_between_ticks() {
        for closing in [30.0f32, 60.0, 90.0, 120.0] {
            for k in 0..12 {
                let (mut w, a, b) = pair();
                let at = w.players[a].pos;
                // Start just outside contact, far enough to cross in one step.
                let gap = 2.0 * BODY_RADIUS + (k + 1) as f32 / 13.0 * closing * STEP;
                w.players[b].pos = at + Vec3::new(0.0, 0.0, gap);
                w.players[a].vel = Vec3::new(0.0, 0.0, closing / 2.0);
                w.players[b].vel = Vec3::new(0.0, 0.0, -closing / 2.0);
                let before: Vec<Vec3> = w.players.iter().map(|p| p.pos).collect();
                for i in [a, b] { let v = w.players[i].vel; w.players[i].pos += v * STEP; }
                w.step_bumps(&before);
                assert!(w.players[a].vel.z < closing / 2.0 - 1.0, "{closing} m/s, gap {gap}: passed through");
                assert!(w.players[b].pos.z > w.players[a].pos.z, "bodies stay in order");
            }
        }
    }

    #[test]
    fn football_teammates_block_but_never_tackle() {
        let (mut w, a, b) = pair();
        w.set_mode(crate::map_catalog::SupportedMode::Football);
        w.ball = football::Ball { active: true, in_play: true, carrier: Some(a), ..Default::default() };
        w.players[b].team = w.players[a].team;
        let at = w.players[a].pos;
        w.players[b].pos = at + Vec3::new(0.9, 0.0, 0.0);
        w.players[b].vel = Vec3::new(-30.0, 0.0, 0.0);
        w.step_bumps(&w.players.iter().map(|p| p.pos).collect::<Vec<_>>());
        assert!(w.players[b].vel.x > -10.0, "blocked");
        assert!(w.ball.carried_by(a), "no fumble from a teammate");
        assert_eq!((w.players[a].stun, w.players[b].stun), (0.0, 0.0), "no tackle");
    }
}
