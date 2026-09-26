//! Deathmatch (every player for themselves) and Team Deathmatch, on any map.
//!
//! - No flags or capture points; the map's turrets, sensors and stations stay.
//! - Deathmatch: every other player is an enemy (`World::hostile`); the first
//!   to FFA_FRAG_LIMIT frags wins, or the top fragger when time runs out.
//!   Anyone may use any inventory station; turrets stand idle and packs
//!   can't be deployed, since both take a side.
//! - Team Deathmatch: each enemy frag scores one for the team; first to
//!   TDM_FRAG_LIMIT.
//! - Each round the server rolls `crate::conditions::Conditions` (time of day,
//!   weather, wind, a twist) and sends them in snapshots. Marker: `dm1`.
use super::*;
use crate::conditions::{Conditions, Twist};
use crate::map_catalog::SupportedMode;

pub const FFA_FRAG_LIMIT: u32 = 20;
pub const TDM_FRAG_LIMIT: u32 = 40;
pub const DEATHMATCH_TIME: f32 = 10.0 * 60.0;
/// Damage taken under the glass-cannon twist.
pub const GLASS_CANNON: f32 = 1.5;

impl World {
    /// Every player for themselves.
    pub fn ffa(&self) -> bool { self.mode == SupportedMode::Deathmatch }

    pub fn deathmatch(&self) -> bool { self.mode.deathmatch() }

    /// Whether a shot or blast from `owner` (of `team`) may hurt player
    /// `victim`. Nobody is hostile to themselves; in Deathmatch everyone else
    /// is; otherwise the other team is. An owner that isn't a player (a
    /// turret, a test) goes by team.
    pub fn hostile(&self, owner: usize, team: Team, victim: usize) -> bool {
        if owner == victim { return false; }
        if self.ffa() && owner < self.players.len() { return true; }
        self.players.get(victim).is_some_and(|v| v.team != team)
    }

    /// Mix fresh entropy into the world's random stream (the server does this
    /// with the clock, so rounds don't repeat).
    pub fn reseed(&mut self, seed: u32) {
        self.rng ^= seed.wrapping_mul(0x9E37_79B9) | 1;
    }

    pub fn rng_state(&self) -> u32 { self.rng }

    /// Roll this round's conditions: random in the deathmatch modes, the
    /// map's own in every other mode.
    pub fn roll_conditions(&mut self) {
        self.conditions = if self.deathmatch() { Conditions::roll(|| self.rng()) } else { Conditions::default() };
    }

    /// Spawn points for Deathmatch: every authored point of both teams.
    pub(super) fn ffa_spawn(&mut self, i: usize) -> Option<(Vec3, f32)> {
        let mut points = crate::terrain::spawn_points_on(self.map, true);
        points.extend(crate::terrain::spawn_points_on(self.map, false));
        if points.is_empty() {
            points = vec![(self.stand(true), std::f32::consts::PI), (self.stand(false), 0.0)];
        }
        // The point farthest from its nearest living opponent, among a few
        // random candidates, so spawns scatter but rarely land on a fight.
        let mut best: Option<(f32, usize)> = None;
        for _ in 0..4 {
            let k = ((self.rng() * points.len() as f32) as usize).min(points.len() - 1);
            let near = self.players.iter().enumerate().filter(|(j, o)| *j != i && o.alive)
                .map(|(_, o)| o.pos.distance(points[k].0)).fold(f32::MAX, f32::min);
            if best.is_none_or(|(d, _)| near > d) { best = Some((near, k)); }
        }
        best.map(|(_, k)| points[k])
    }

    /// The twist's effect on a fresh spawn.
    pub(super) fn apply_twist(&mut self, i: usize) {
        match self.conditions.twist {
            Twist::Heavies => {
                self.players[i].armor = ArmorClass::Heavy;
                self.refill(i);
            }
            Twist::Rifles => self.players[i].rifle = true,
            Twist::None | Twist::GlassCannon => {}
        }
    }

    pub(super) fn twist_damage(&self) -> f32 {
        if self.conditions.twist == Twist::GlassCannon { GLASS_CANNON } else { 1.0 }
    }

    /// Deathmatch scoring after `killer` frags `victim`.
    pub(super) fn deathmatch_frag(&mut self, killer: Option<usize>, victim: usize) {
        if !self.deathmatch() || self.predicting || self.state != MatchState::Playing { return; }
        let Some(k) = killer.filter(|&k| k < self.players.len()) else { return };
        if k == victim {
            // Killing yourself costs a frag in Deathmatch.
            if self.ffa() { self.players[k].frags = self.players[k].frags.saturating_sub(1); }
            return;
        }
        if !self.hostile(k, self.players[k].team, victim) { return; }
        if self.ffa() {
            if self.players[k].frags >= FFA_FRAG_LIMIT { self.end_deathmatch(); }
        } else {
            let t = self.players[k].team.idx();
            self.score[t] += 1;
            if self.score[t] >= TDM_FRAG_LIMIT { self.end_deathmatch(); }
        }
    }

    /// The Deathmatch leader: most frags, then fewest deaths.
    pub fn ffa_leader(&self) -> Option<usize> {
        self.players.iter().enumerate().filter(|(_, p)| p.net_id != 0 || !p.remote)
            .max_by_key(|(_, p)| (p.frags, std::cmp::Reverse(p.losses))).map(|(i, _)| i)
    }

    pub(super) fn end_deathmatch(&mut self) {
        self.state = MatchState::Ended;
        let text = if self.ffa() {
            match self.ffa_leader() {
                Some(i) if i == self.player_id => "YOU WIN THE DEATHMATCH".to_string(),
                Some(i) => format!("{} WINS THE DEATHMATCH", self.display_name(i).to_uppercase()),
                None => "MATCH OVER".to_string(),
            }
        } else {
            let mine = self.players.get(self.player_id).map_or(0, |p| p.team.idx());
            if self.score[mine] > self.score[1 - mine] { "VICTORY".into() }
            else if self.score[mine] < self.score[1 - mine] { "DEFEAT".into() } else { "DRAW".into() }
        };
        self.msg(&text, 8.0);
        self.push_event("end");
    }

    /// Where a Deathmatch bot heads when it sees nobody: the nearest enemy.
    pub(super) fn deathmatch_goal(&self, i: usize) -> Option<Vec3> {
        if !self.deathmatch() { return None; }
        let (pos, team) = (self.players[i].pos, self.players[i].team);
        self.players.iter().enumerate()
            .filter(|(j, o)| o.alive && self.hostile(i, team, *j))
            .min_by(|(_, a), (_, b)| a.pos.distance(pos).total_cmp(&b.pos.distance(pos)))
            .map(|(_, o)| o.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arena(mode: SupportedMode) -> World {
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        w.set_mode(mode);
        w.start_match(true);
        w
    }

    #[test]
    fn deathmatch_makes_teammates_targets_and_team_deathmatch_does_not() {
        let w = arena(SupportedMode::Deathmatch);
        let mate = (1..w.players.len()).find(|&j| w.players[j].team == w.players[0].team).unwrap();
        assert!(w.hostile(0, w.players[0].team, mate));
        assert!(!w.hostile(0, w.players[0].team, 0), "never yourself");
        let w = arena(SupportedMode::TeamDeathmatch);
        assert!(!w.hostile(0, w.players[0].team, mate));
        let foe = (1..w.players.len()).find(|&j| w.players[j].team != w.players[0].team).unwrap();
        assert!(w.hostile(0, w.players[0].team, foe));
    }

    #[test]
    fn a_disc_hurts_a_teammate_only_in_deathmatch() {
        for (mode, hurt) in [(SupportedMode::Deathmatch, true), (SupportedMode::TeamDeathmatch, false), (SupportedMode::Ctf, false)] {
            let mut w = arena(mode);
            w.conditions = Conditions::default();
            let mate = (1..w.players.len()).find(|&j| w.players[j].team == w.players[0].team).unwrap();
            let team = w.players[0].team;
            w.players[mate].health = 100.0;
            let at = w.players[mate].pos + Vec3::Y * 0.7;
            w.explode(at, 0, 0, team);
            assert_eq!(w.players[mate].health < 100.0, hurt, "{mode:?}");
        }
    }

    #[test]
    fn frags_score_and_end_the_match() {
        let mut w = arena(SupportedMode::TeamDeathmatch);
        let foe = (1..w.players.len()).find(|&j| w.players[j].team != w.players[0].team).unwrap();
        w.kill(foe, Some(0), "Disc");
        assert_eq!(w.score[w.players[0].team.idx()], 1);
        w.score[w.players[0].team.idx()] = TDM_FRAG_LIMIT - 1;
        w.respawn(foe);
        w.kill(foe, Some(0), "Disc");
        assert_eq!(w.state, MatchState::Ended);

        let mut w = arena(SupportedMode::Deathmatch);
        w.players[0].frags = FFA_FRAG_LIMIT - 1;
        w.kill(1, Some(0), "Disc");
        assert_eq!(w.state, MatchState::Ended);
        assert_eq!(w.ffa_leader(), Some(0));
        // Suicide costs a frag.
        let mut w = arena(SupportedMode::Deathmatch);
        w.players[2].frags = 3;
        w.kill(2, Some(2), "Suicide");
        assert_eq!(w.players[2].frags, 2);
    }

    #[test]
    fn deathmatch_rolls_conditions_and_other_modes_keep_the_map_look() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..40 {
            let mut w = World::new();
            w.set_map(MapId::Raindance);
            w.set_mode(SupportedMode::Deathmatch);
            w.reseed(seed * 7919 + 1);
            w.start_match(true);
            seen.insert(format!("{:?}{:?}", w.conditions.time, w.conditions.weather));
            assert!(w.flags.iter().all(|f| f.carrier.is_none()));
            assert!(w.points.iter().all(|p| !p.active), "no capture points");
        }
        assert!(seen.len() > 6, "rounds vary: {seen:?}");
        let w = arena(SupportedMode::Ctf);
        assert_eq!(w.conditions, Conditions::default());
    }

    #[test]
    fn twists_change_spawns_and_damage() {
        let mut w = arena(SupportedMode::Deathmatch);
        w.conditions.twist = Twist::Heavies;
        w.kill(1, None, "Fall");
        w.respawn(1);
        assert_eq!(w.players[1].armor, ArmorClass::Heavy);
        w.conditions.twist = Twist::Rifles;
        w.kill(1, None, "Fall");
        w.respawn(1);
        assert!(w.players[1].rifle);
        w.conditions.twist = Twist::GlassCannon;
        let before = w.players[1].health;
        w.hurt(1, 10.0);
        let glass = before - w.players[1].health;
        w.conditions.twist = Twist::None;
        let before = w.players[1].health;
        w.hurt(1, 10.0);
        assert!((glass / (before - w.players[1].health) - GLASS_CANNON).abs() < 1e-3);
    }

    #[test]
    fn bots_hunt_each_other_in_deathmatch() {
        let mut w = arena(SupportedMode::Deathmatch);
        w.players[0].is_bot = true;
        w.assign_personality(0);
        for _ in 0..(120.0 / STEP) as usize {
            if w.state != MatchState::Playing { break; }
            w.physics_step();
        }
        let frags: u32 = w.players.iter().map(|p| p.frags).sum();
        let mates = w.players.iter().filter(|p| p.team == w.players[0].team).count();
        println!("deathmatch bots: {frags} frags in 2 min, {mates} on team 0");
        assert!(frags >= 3, "bots must fight: {frags}");
    }
}
