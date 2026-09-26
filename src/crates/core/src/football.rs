//! Football: an original ball mode in the spirit of the classic community
//! football mods. No weapons: one ball, passes, fumbles, tackles and
//! touchdowns. The server owns every outcome; snapshots carry the ball whole,
//! so clients draw it and announce plays by diffing state.
//!
//! Stadium maps declare the field in their manifest (`football`: end zones,
//! kickoff spots, zone radius), and only they offer the mode. Without one (tests),
//! each team's end zone is its own flag stand and a new ball appears part of the
//! way from the receiving team's end zone to the other. Marker: `ball1`.
use super::*;
use crate::feed::Entry;
use crate::map_catalog::SupportedMode;

/// Football armor, after the classic football mod's light armor: jetForce 400
/// on mass 9 (44.4 m/s² against gravity 20) for about half a second (drain 4
/// per 32 ms tick against a 60-energy tank), recharge 8/s, and the lighter
/// Tribes jump (impulse 50 on mass 9 = 5.56 m/s). A short, hard burst that
/// only just reaches a raised goal, then a long wait to refill.
pub const FOOTBALL_ARMOR: Armor = Armor { jet: 400.0 / 9.0, side: 0.8 * 400.0 / 9.0, drain: 120.0,
    regen: 8.0, min_jet: 1.0, jump: (50.0 / 9.0) / 8.34, fade_up: false, air_cap: crate::sim::JET_THRUST_CAP,
    ski_drag: FOOTBALL_SKI_DRAG };
/// Football skiing loses speed on the flat (as in Tribes football, where a
/// runner keeps tapping the jet to hold pace). CTF skiing is unchanged.
pub const FOOTBALL_SKI_DRAG: f32 = 1.0;

/// Top of a raised goal platform above the floor (Highgoal): the jet alone
/// only just gets you up there; a jump and jet together clears it easily.
pub const RAISED_GOAL_HEIGHT: f32 = 7.0;

/// Classic football plays on time: two 15-minute halves (the tasermod's server
/// config), with a score limit high enough (the missions' usual 40) that it
/// rarely ends a game early.
pub const HALF_SECONDS: f32 = 15.0 * 60.0;
pub const TOUCHDOWNS_TO_WIN: u32 = 40;
/// Respawn delay and pre-match warmup from the classic server config.
pub const RESPAWN_SECONDS: f32 = 1.0;
pub const WARMUP_SECONDS: f32 = 20.0;
/// Carrying the ball into this sphere around the enemy flag stand scores.
pub const END_ZONE_RADIUS: f32 = 12.0;
/// Closing speed (rounded to whole m/s) at which the slower player is tackled.
pub const TACKLE_SPEED: f32 = 15.0;
/// Below TACKLE_SPEED, down to this, a slower carrier fumbles instead.
pub const FUMBLE_SPEED: f32 = 2.0;
/// How long a tackled player stays down, unable to move.
pub const TACKLE_STUN: f32 = 2.0;
pub const TACKLE_DAMAGE: f32 = 30.0;
/// The carrier runs at this fraction of walking speed; skiing and jets are
/// unchanged.
pub const CARRIER_WALK: f32 = 0.72;
/// A break at half time: everyone regroups and the other side gets the ball.
pub const HALFTIME_BREAK: f32 = 10.0;
/// Personal points for a touchdown and for tackling the carrier.
pub const TOUCHDOWN_POINTS: u32 = 5;
pub const TACKLE_POINTS: u32 = 1;
pub const BALL_RADIUS: f32 = 0.3;
const PICKUP_RADIUS: f32 = 2.2;
/// Health lost per second outside the field's bounds (0.065 of a 0.66 armor).
const OUT_OF_BOUNDS_DAMAGE: f32 = 0.065 / 0.66 * 100.0;
/// A ball that sits still this long is reset.
const IDLE_RESET: f32 = 20.0;
/// After a touchdown: players regroup, then a new ball appears.
const REGROUP_AFTER: f32 = 6.0;
const NEW_BALL_AFTER: f32 = 10.0;
const RESET_BALL_AFTER: f32 = 2.0;
/// Passing picks its power from the nearest player within this yaw cone.
const PASS_CONE: f32 = 0.15;
const PASS_RANGE: f32 = 8000.0;
/// Power of a throw with nobody to aim at.
const LOOSE_THROW: f32 = 35.0;
/// Throws leave above the crosshair so a level look still lobs.
const PASS_LOFT: f32 = 0.25;
const THROWER_NO_CATCH: f32 = 0.5;
const FUMBLE_POP: f32 = 10.0;
/// A fumbling carrier can't grab the ball straight back: it pops out inside
/// their pickup reach, so without this a fumble undid itself next tick.
const FUMBLER_NO_CATCH: f32 = 0.75;
/// Throw elevation (look pitch plus the loft) passes unchanged up to this,
/// then eases so a throw aimed straight up still leans forward: at most
/// MAX_THROW_ELEVATION (65 degrees, 25 degrees off vertical).
const THROW_EASE_FROM: f32 = 0.8;
pub const MAX_THROW_ELEVATION: f32 = 65.0 * std::f32::consts::PI / 180.0;
/// New balls appear this far from the receiving team's end zone to the other.
const KICKOFF_FRACTION: f32 = 0.35;

/// A map's declared football field (manifest `football`). Index 0 is Ember's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    /// The end zone each team defends (centre of its scoring sphere).
    pub end_zones: [[f32; 3]; 2],
    /// Where a new ball appears on each team's side.
    pub kickoff: [[f32; 3]; 2],
    #[serde(default = "default_zone_radius")]
    pub radius: f32,
    /// In-bounds rectangle `[x0, z0, x1, z1]`: a ball outside it is reset.
    #[serde(default)]
    pub bounds: Option<[f32; 4]>,
}
fn default_zone_radius() -> f32 { END_ZONE_RADIUS }

impl Field {
    pub fn validate(&self) -> Result<(), String> {
        let finite = |p: &[f32; 3]| p.iter().all(|v| v.is_finite() && v.abs() <= 10000.0);
        if !self.end_zones.iter().chain(&self.kickoff).all(finite) || !(4.0..=40.0).contains(&self.radius) {
            return Err("Invalid football field".into());
        }
        if let Some([x0, z0, x1, z1]) = self.bounds {
            let inside = |p: &[f32; 3]| p[0] > x0 && p[0] < x1 && p[2] > z0 && p[2] < z1;
            if !(x0 < x1 && z0 < z1) || !self.end_zones.iter().chain(&self.kickoff).all(inside) {
                return Err("Invalid football bounds".into());
            }
        }
        Ok(())
    }
}

/// Whether `map` declares a football field (and so offers the mode).
pub fn has_field(map: MapId) -> bool {
    crate::map_pack::on(map).is_some_and(|p| p.manifest.football.is_some())
}

fn field(map: MapId) -> Option<&'static Field> {
    crate::map_pack::on(map).and_then(|p| p.manifest.football.as_ref())
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ball {
    /// Football is the mode in force.
    pub active: bool,
    /// A ball is on the field. False between a score and the next ball.
    pub in_play: bool,
    pub pos: Vec3,
    pub vel: Vec3,
    pub carrier: Option<usize>,
    /// Who last threw or dropped the ball (the classic mod's ball owner).
    pub thrown_by: Option<usize>,
    /// Team that last held the ball.
    pub last_team: Option<Team>,
    /// Seconds before the thrower can catch their own throw.
    pub no_catch: f32,
    /// Seconds the ball has sat still.
    pub idle: f32,
    /// Seconds until the next ball when none is in play.
    pub respawn: f32,
    /// Seconds until everyone is sent back to spawn (0 = none pending).
    pub regroup: f32,
    /// Team whose side the next ball appears on.
    pub next_side: u8,
    /// 0 in the first half, 1 in the second.
    pub half: u8,
    /// Seconds towards the next out-of-bounds damage tick.
    #[serde(default)]
    pub oob_clock: f32,
}

impl Ball {
    pub fn carried_by(&self, i: usize) -> bool { self.active && self.carrier == Some(i) }
    pub fn loose(&self) -> bool { self.active && self.in_play && self.carrier.is_none() }
}

/// The throw's elevation for a look pitch: pitch plus the loft, eased above
/// THROW_EASE_FROM so looking straight up (pitch 1.52) throws at
/// MAX_THROW_ELEVATION, leaning forward along your facing.
pub fn throw_elevation(pitch: f32) -> f32 {
    let raw = pitch + PASS_LOFT;
    if raw <= THROW_EASE_FROM { return raw; }
    let top = 1.52 + PASS_LOFT;
    THROW_EASE_FROM + (raw - THROW_EASE_FROM).min(top - THROW_EASE_FROM) * (MAX_THROW_ELEVATION - THROW_EASE_FROM) / (top - THROW_EASE_FROM)
}

/// Throw speed for a receiver `distance` metres away: the classic mod's
/// `d / (sqrt(d) / 6.454)`, i.e. `6.454 · sqrt(d)`, uncapped (its receiver
/// search reached 8 km). At a level look the loft makes the throw land about
/// that far away under the sim's gravity: range = v² sin(2·0.25) / 20 ≈ d.
pub fn pass_speed(distance: f32) -> f32 {
    6.454 * distance.max(0.0).sqrt()
}
/// Share of the thrower's own velocity the ball keeps. The classic mod set
/// the thrower's velocity to zero around `GameBase::throw`, so none.
pub const PASS_INHERIT: f32 = 0.0;

fn team_name(t: Team) -> &'static str { if t == Team::Ember { "EMBER" } else { "GLACIER" } }

impl World {
    pub fn football(&self) -> bool { self.mode == SupportedMode::Football }

    /// Match clock: two halves in Football, the standard match otherwise.
    pub fn match_time(&self) -> f32 {
        if self.football() { 2.0 * HALF_SECONDS } else if self.deathmatch() { deathmatch::DEATHMATCH_TIME } else { MATCH_TIME }
    }

    /// The end zone a team defends: the map's declared field, or failing
    /// that its own flag stand.
    pub fn end_zone(&self, team: Team) -> Vec3 {
        field(self.map).map_or(self.flags[team.idx()].home, |f| Vec3::from(f.end_zones[team.idx()]))
    }

    pub fn end_zone_radius(&self) -> f32 { field(self.map).map_or(END_ZONE_RADIUS, |f| f.radius) }

    /// Where a new ball appears on `team`'s side of the field.
    pub fn kickoff_spot(&self, team: Team) -> Vec3 {
        if let Some(f) = field(self.map) { return Vec3::from(f.kickoff[team.idx()]); }
        let own = self.end_zone(team);
        let other = self.end_zone(team.other());
        let p = own.lerp(other, KICKOFF_FRACTION);
        let high = Vec3::new(p.x, own.y.max(other.y) + 80.0, p.z);
        let floor = crate::terrain::support_on(self.map, high).0;
        Vec3::new(p.x, floor.max(p.y - 40.0) + 1.0, p.z)
    }

    /// Fresh ball state for the mode in force: the first ball appears shortly
    /// on a random side.
    pub(super) fn reset_ball(&mut self) {
        if !self.football() { self.ball = Ball::default(); return; }
        let side = if self.rng() < 0.5 { 0 } else { 1 };
        self.ball = Ball { active: true, respawn: RESET_BALL_AFTER, next_side: side, ..Ball::default() };
    }

    fn play(&mut self, text: String) {
        crate::feed::push(&mut self.feed, Entry::Play { text });
    }

    fn spawn_ball(&mut self) {
        let team = if self.ball.next_side == 0 { Team::Ember } else { Team::Glacier };
        let pos = self.kickoff_spot(team);
        let b = &mut self.ball;
        b.in_play = true;
        b.pos = pos;
        b.vel = Vec3::ZERO;
        b.carrier = None;
        b.thrown_by = None;
        b.last_team = None;
        b.idle = 0.0;
        b.no_catch = 0.0;
        self.play("A new ball is in play".into());
        self.push_event("whistle");
    }

    /// No ball until the next one appears, on the side of the team that did
    /// not touch it last.
    fn dead_ball(&mut self, after: f32, side: Team) {
        let b = &mut self.ball;
        b.in_play = false;
        b.carrier = None;
        b.thrown_by = None;
        b.vel = Vec3::ZERO;
        b.respawn = after;
        b.next_side = side.idx() as u8;
    }

    /// A reset ball goes to the side that didn't touch it last, and (as in the
    /// classic mod) everyone is sent back to spawn with it: at once for an idle
    /// ball, after `after` seconds for one that left the field.
    fn loose_reset(&mut self, text: &str, after: f32) {
        let side = self.ball.last_team.map(|t| t.other())
            .unwrap_or(if self.rng() < 0.5 { Team::Ember } else { Team::Glacier });
        self.dead_ball(after.max(STEP * 0.5), side);
        self.ball.regroup = after.max(STEP * 0.5);
        self.play(text.into());
        self.push_event("horn");
    }

    /// The carrier loses the ball: it pops out in a random direction.
    pub(super) fn fumble(&mut self, i: usize) {
        if !self.ball.carried_by(i) { return; }
        let a = self.rng() * std::f32::consts::TAU;
        let p = &self.players[i];
        let b = &mut self.ball;
        b.carrier = None;
        b.thrown_by = Some(i);
        b.pos = p.pos + Vec3::Y * 0.6;
        b.vel = Vec3::new(a.sin() * FUMBLE_POP, 4.0, a.cos() * FUMBLE_POP);
        b.no_catch = FUMBLER_NO_CATCH;
        b.idle = 0.0;
    }

    /// A player who dies, leaves, switches team or respawns drops the ball.
    pub(super) fn ball_lost(&mut self, i: usize) {
        if self.ball.carried_by(i) { self.fumble(i); }
    }

    /// Players outside the field's bounds take 0.065 damage level (9.8% of
    /// light armor) each second until they return, as in the classic mod.
    fn out_of_bounds_damage(&mut self, dt: f32) {
        let Some([x0, z0, x1, z1]) = field(self.map).and_then(|f| f.bounds) else { return };
        self.ball.oob_clock += dt;
        if self.ball.oob_clock < 1.0 { return; }
        self.ball.oob_clock -= 1.0;
        for i in 0..self.players.len() {
            let p = &self.players[i];
            if !p.alive || p.remote || (p.pos.x > x0 && p.pos.x < x1 && p.pos.z > z0 && p.pos.z < z1) { continue; }
            self.players[i].health -= OUT_OF_BOUNDS_DAMAGE;
            if i == self.player_id { self.msg("YOU'RE OUT OF BOUNDS!", 1.2); self.push_event("pain"); }
            if self.players[i].health <= 0.0 { self.kill(i, None, "Out of bounds"); }
        }
    }

    /// Everyone alive back to their spawns with full health and energy.
    fn regroup_players(&mut self) {
        for i in 0..self.players.len() {
            if self.players[i].alive && !self.players[i].remote {
                self.respawn(i);
                self.players[i].stun = 0.0;
            }
        }
    }

    /// Half time: the ball goes dead, everyone regroups and the side that did
    /// not start gets the next ball.
    pub(super) fn check_halftime(&mut self) {
        if !self.ball.active || self.ball.half > 0 || self.time_left > HALF_SECONDS { return; }
        self.ball.half = 1;
        let first = self.ball.next_side;
        let side = if first == 0 { Team::Glacier } else { Team::Ember };
        self.dead_ball(HALFTIME_BREAK, side);
        self.ball.regroup = 0.01;
        self.play("Half time".into());
        self.push_event("halftime");
    }

    /// Fire while carrying: pass, as the classic mod did. Power comes from the
    /// distance to the player under the crosshair, or the nearest one in a
    /// narrow cone ahead (else a fixed loose throw); the throw itself always
    /// goes where you look, lofted 0.25 rad, without your own velocity.
    /// Leading a runner or lofting onto a ledge is the thrower's aim.
    pub(super) fn pass(&mut self, i: usize) {
        if self.predicting || !self.ball.carried_by(i) { return; }
        let p = &self.players[i];
        let eye = p.pos + Vec3::Y * EYE;
        let dir = look_dir(p.yaw, p.pitch);
        let (yaw, pitch, team) = (p.yaw, p.pitch, p.team);
        let mut target: Option<(usize, f32)> = None;
        for (j, o) in self.players.iter().enumerate() {
            if j == i || !o.alive { continue; }
            let to = o.pos + Vec3::Y * 0.4 - eye;
            let along = to.dot(dir);
            if along <= 0.0 || along > PASS_RANGE { continue; }
            if (to - dir * along).length() < 1.8 && target.is_none_or(|(_, d)| along < d)
                && obstacle_hit(self.map, &self.pillars, eye, o.pos + Vec3::Y * 0.4, 0.0).is_none() {
                target = Some((j, along));
            }
        }
        if target.is_none() {
            for (j, o) in self.players.iter().enumerate() {
                if j == i || !o.alive { continue; }
                let to = o.pos - p.pos;
                let angle = (-to.x).atan2(-to.z);
                let diff = (angle - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                let d = to.length();
                if diff.abs() <= PASS_CONE && d <= PASS_RANGE && target.is_none_or(|(_, best)| d < best) {
                    target = Some((j, d));
                }
            }
        }
        let reach = eye + dir * 0.7;
        let origin = match obstacle_hit(self.map, &self.pillars, eye, reach, BALL_RADIUS) {
            Some(t) => eye.lerp(reach, (t - 0.1).max(0.0)),
            None => reach,
        };
        let speed = target.map_or(LOOSE_THROW, |(j, _)| pass_speed(self.players[j].pos.distance(self.players[i].pos)));
        let velocity = look_dir(yaw, throw_elevation(pitch)) * speed + self.players[i].vel * PASS_INHERIT;
        let b = &mut self.ball;
        b.carrier = None;
        b.pos = origin;
        b.vel = velocity;
        b.thrown_by = Some(i);
        b.last_team = Some(team);
        b.no_catch = THROWER_NO_CATCH;
        b.idle = 0.0;
        self.players[i].cooldown = 0.5;
        let name = self.display_name(i);
        let text = match target {
            Some((j, _)) => format!("{name} passes toward {}", self.display_name(j)),
            None => format!("{name} throws the ball"),
        };
        self.play(text);
        if i == self.player_id { self.push_event("pass"); }
        else if self.network_inputs.is_empty() { self.spatial_sounds.push(("pass", origin)); }
    }

    /// The ball, once per physics step: follow the carrier and score, or fly,
    /// bounce, roll and get picked up; then the resets.
    pub(super) fn step_ball(&mut self, dt: f32) {
        if !self.ball.active || self.predicting { return; }
        self.out_of_bounds_damage(dt);
        if self.ball.regroup > 0.0 {
            self.ball.regroup -= dt;
            if self.ball.regroup <= 0.0 { self.ball.regroup = 0.0; self.regroup_players(); }
        }
        if !self.ball.in_play {
            self.ball.respawn -= dt;
            if self.ball.respawn <= 0.0 { self.spawn_ball(); }
            return;
        }
        if let Some(c) = self.ball.carrier {
            let holder = self.players.get(c).filter(|p| p.alive);
            let Some(holder) = holder else { self.ball_lost(c); return; };
            let (pos, vel, team) = (holder.pos, holder.vel, holder.team);
            self.ball.pos = pos;
            self.ball.vel = vel;
            if pos.distance(self.end_zone(team.other())) < self.end_zone_radius() { self.touchdown(c); }
            return;
        }
        self.fly_ball(dt);
        if !self.ball.in_play { return; }
        self.pickup();
    }

    fn fly_ball(&mut self, dt: f32) {
        let map = self.map;
        let far = self.map_size() - 1.0;
        let b = &mut self.ball;
        b.no_catch = (b.no_catch - dt).max(0.0);
        b.vel.y -= GRAVITY * dt;
        if crate::water::at(map, &self.staged_water, b.pos).is_some() {
            // Floats and drags in water.
            b.vel *= (-2.5 * dt).exp();
            b.vel.y += GRAVITY * 1.4 * dt;
        }
        let next = b.pos + b.vel * dt;
        if let Some(t) = obstacle_hit(map, &self.pillars, b.pos, next, BALL_RADIUS) {
            let contact = b.pos.lerp(next, t);
            let normal = grenade_contact_normal(map, &self.pillars, contact, far, b.vel);
            let inward = b.vel.dot(normal);
            if inward < 0.0 { b.vel -= normal * inward * 1.55; }
            b.vel *= 0.8;
            b.pos = contact + normal * 0.03;
        } else {
            b.pos = next;
        }
        let (floor, face) = crate::terrain::support_on(map, b.pos + Vec3::Y * 0.5);
        let floor = floor + BALL_RADIUS;
        let grounded = b.pos.y <= floor + 0.05;
        if b.pos.y < floor && b.pos.y > floor - 2.0 {
            b.pos.y = floor;
            if b.vel.y < 0.0 { b.vel.y = -b.vel.y * 0.45; }
            b.vel.x *= 0.85;
            b.vel.z *= 0.85;
        }
        if grounded {
            // Rolling friction, then rest: a slow ball stops on anything but
            // a steep slope instead of creeping downhill forever.
            b.vel.x *= (-1.5 * dt).exp();
            b.vel.z *= (-1.5 * dt).exp();
            if b.vel.length() < 2.5 && face.y > 0.8 { b.vel = Vec3::ZERO; }
        }
        b.idle = if b.vel == Vec3::ZERO { b.idle + dt } else { 0.0 };
        let fenced = field(map).and_then(|f| f.bounds)
            .is_some_and(|[x0, z0, x1, z1]| b.pos.x < x0 || b.pos.x > x1 || b.pos.z < z0 || b.pos.z > z1);
        let out = fenced || b.pos.x < 2.0 || b.pos.z < 2.0 || b.pos.x > far - 1.0 || b.pos.z > far - 1.0
            || b.pos.y < floor - 30.0 || !b.pos.is_finite();
        let idle = b.idle > IDLE_RESET;
        if out { self.loose_reset("The ball left the field and was reset", RESET_BALL_AFTER); }
        else if idle { self.loose_reset("The ball sat idle and was reset", 0.0); }
    }

    fn pickup(&mut self) {
        let b = &self.ball;
        let best = self.players.iter().enumerate()
            .filter(|(j, p)| p.alive && p.stun <= 0.0 && !(b.no_catch > 0.0 && b.thrown_by == Some(*j)))
            .map(|(j, p)| (j, p.pos.distance(b.pos)))
            .filter(|(_, d)| *d < PICKUP_RADIUS)
            .min_by(|a, c| a.1.total_cmp(&c.1));
        let Some((j, _)) = best else { return };
        let team = self.players[j].team;
        let name = self.display_name(j);
        // The classic mod's wording: a ball nobody threw or dropped, or one
        // barely moving (under 1.1 m/s), is picked up; a moving ball from an
        // enemy is intercepted; anything else is caught.
        let speed = self.ball.vel.length();
        let owner = self.ball.thrown_by.filter(|&k| k < self.players.len()).map(|k| self.players[k].team);
        let (text, event) = match owner {
            None => (format!("{name} picks up the ball"), "catch"),
            Some(_) if speed < 1.1 => (format!("{name} picks up the ball"), "catch"),
            Some(t) if t != team => (format!("{name} intercepts the ball!"), "intercept"),
            Some(_) => (format!("{name} catches the ball"), "catch"),
        };
        // The catch pushes the catcher along the ball's flight: the mod's
        // impulse of 2 x ball speed x 6.5 / mass, on a mass-9 armor.
        let flight = Vec3::new(self.ball.vel.x, 0.0, self.ball.vel.z).normalize_or_zero();
        self.players[j].vel += flight * (2.0 * speed * 6.5 / 9.0 / 9.0) + Vec3::Y * (1.0 / 9.0);
        let b = &mut self.ball;
        b.carrier = Some(j);
        b.thrown_by = None;
        b.last_team = Some(team);
        b.idle = 0.0;
        self.play(text);
        self.push_event(event);
    }

    fn touchdown(&mut self, c: usize) {
        let team = self.players[c].team;
        self.score[team.idx()] += 1;
        self.players[c].frags += TOUCHDOWN_POINTS;
        let name = self.display_name(c);
        self.play(format!("{name} scores a touchdown for {}!", team_name(team)));
        self.dead_ball(NEW_BALL_AFTER, team.other());
        self.ball.last_team = Some(team);
        self.ball.regroup = REGROUP_AFTER;
        self.trauma = 0.45;
        let mine = self.players.get(self.player_id).is_some_and(|p| p.team == team);
        self.push_event(if mine { "touchdown_win" } else { "touchdown_loss" });
        if self.score[team.idx()] >= TOUCHDOWNS_TO_WIN {
            self.state = MatchState::Ended;
            self.msg(if mine { "VICTORY" } else { "DEFEAT" }, 8.0);
            self.push_event("end");
        }
    }

    /// The football consequences of a hit between enemies `a` and `b`, with
    /// their velocities just before it (the bodies have already swapped them).
    /// Exactly the classic football mod's rule (`Player::onCollision` in both
    /// the 4.6 tasermod and the 1v1 mod): speeds are rounded to whole m/s; the
    /// slower player loses, and on a tie both do. A losing carrier fumbles when
    /// the closing speed is 2-14; at 15 or more the loser is tackled. Nobody is
    /// tackled while no ball is in play.
    pub(super) fn football_hit(&mut self, a: usize, b: usize, va: Vec3, vb: Vec3) {
        let closing = (va - vb).length().round();
        let (sa, sb) = (va.length().round(), vb.length().round());
        let losers: &[(usize, usize)] = if sa > sb { &[(b, a)] } else if sb > sa { &[(a, b)] } else { &[(a, b), (b, a)] };
        for &(loser, winner) in losers {
            if !self.players[loser].alive { continue; }
            if self.ball.carried_by(loser) && (FUMBLE_SPEED..TACKLE_SPEED).contains(&closing) {
                self.knock_loose(loser, winner);
            }
            if closing >= TACKLE_SPEED {
                if !self.ball.in_play { return; }
                self.tackle(loser, winner);
            }
        }
    }

    fn knock_loose(&mut self, carrier: usize, by: usize) {
        self.fumble(carrier);
        let (w, l) = (self.display_name(by), self.display_name(carrier));
        self.play(format!("{w} knocks the ball loose from {l}"));
        self.push_event("fumble");
    }

    fn tackle(&mut self, loser: usize, winner: usize) {
        if !self.players[loser].alive { return; }
        if self.ball.carried_by(loser) {
            self.fumble(loser);
            self.players[winner].frags += TACKLE_POINTS;
        }
        let (w, l) = (self.display_name(winner), self.display_name(loser));
        self.play(format!("{w} tackles {l}"));
        self.players[loser].stun = TACKLE_STUN;
        self.players[loser].health -= TACKLE_DAMAGE;
        if loser == self.player_id { self.trauma = (self.trauma + 0.6).min(1.0); }
        self.push_event("tackle");
        if self.players[loser].health <= 0.0 { self.kill(loser, Some(winner), "Tackle"); }
    }

    /// Whether `team`'s end zone stands well above its kickoff floor.
    pub fn raised_goal(&self, team: Team) -> bool {
        self.end_zone(team).y - self.kickoff_spot(team).y > 4.0
    }

    /// Where a bot carrier launches from to leap onto a raised goal: on the
    /// floor 6.5 m out from the zone, on the side it approaches from.
    fn leap_spot(&self, i: usize, zone: Vec3) -> Vec3 {
        let pos = self.players[i].pos;
        let away = Vec3::new(pos.x - zone.x, 0.0, pos.z - zone.z).normalize_or(Vec3::Z);
        let spot = zone + away * 6.5;
        let floor = crate::terrain::support_on(self.map, Vec3::new(spot.x, self.kickoff_spot(Team::Ember).y + 2.0, spot.z)).0;
        Vec3::new(spot.x, floor + 1.0, spot.z)
    }

    /// A bot carrier's inputs near a raised goal: walk the last 12 m to the
    /// launch spot without jetting, wait there for a full tank, then leap:
    /// jump on the first tick and hold jet straight up; once above the goal's
    /// top, let go of the jet, face the zone and drift onto it. The leap ends on landing,
    /// losing the ball, or after 4 s. `None` leaves the bot to its route.
    pub(super) fn bot_leap_inputs(&mut self, i: usize, dt: f32) -> Option<(f32, f32, bool, bool, bool, bool)> {
        let p = &self.players[i];
        if !p.is_bot || !self.football() { return None; }
        let team = p.team;
        if !self.ball.carried_by(i) || !self.raised_goal(team.other()) {
            self.players[i].bot_leap = 0.0;
            return None;
        }
        let zone = self.end_zone(team.other());
        let t = p.bot_leap;
        if t > 0.0 {
            if t > 4.0 || (t > 0.5 && p.on_ground) { self.players[i].bot_leap = 0.0; return None; }
            let above = p.pos.y > zone.y - 1.2 + 0.3;
            let to = zone - p.pos;
            let p = &mut self.players[i];
            p.bot_leap += dt;
            if above { p.yaw = (-to.x).atan2(-to.z); }
            return Some((0.0, if above { 1.0 } else { 0.0 }, false, t <= dt, false, !above));
        }
        let spot = self.leap_spot(i, zone);
        let to = spot - p.pos;
        let flat = Vec2::new(to.x, to.z).length();
        if flat > 12.0 { return None; }
        let ready = flat < 1.5 && p.on_ground && p.energy >= ENERGY_MAX - 1.0;
        let p = &mut self.players[i];
        if ready { p.bot_leap = dt * 0.5; return Some((0.0, 0.0, false, false, false, false)); }
        if flat > 1.0 { p.yaw = (-to.x).atan2(-to.z); }
        Some((0.0, if flat > 1.0 { 1.0 } else { 0.0 }, false, false, false, false))
    }

    /// Offline bots: tackling is a knee to the head, so a bot near the enemy
    /// carrier pounces: a jump and a short jet toward them to get above, then
    /// it drops onto them. Ends on landing or after 1.6 s.
    pub(super) fn bot_pounce_inputs(&mut self, i: usize, dt: f32) -> Option<(f32, f32, bool, bool, bool, bool)> {
        let p = &self.players[i];
        if !p.is_bot || !self.football() || p.stun > 0.0 { return None; }
        let target = self.ball.carrier.filter(|&c| self.players.get(c).is_some_and(|o| o.alive && o.team != p.team))?;
        let o = &self.players[target];
        let lead = o.pos + o.vel * 0.35;
        let to = lead - p.pos;
        let flat = Vec2::new(to.x, to.z).length();
        let t = p.bot_pounce;
        if t <= 0.0 {
            // Start from the ground, in range, with enough jet for the hop.
            if !(p.on_ground && (5.0..14.0).contains(&flat) && p.energy >= 20.0 && to.y.abs() < 3.0) { return None; }
        } else if t > 1.6 || (t > 0.3 && p.on_ground) {
            self.players[i].bot_pounce = 0.0;
            return None;
        }
        // Aim to arrive about 1.8 m above them (knees to the head): predict the
        // height on arrival and jet only while it would come in too low.
        let toward = Vec2::new(p.vel.x, p.vel.z).dot(Vec2::new(to.x, to.z) / flat.max(0.1)).max(11.0);
        let eta = flat / toward;
        let arrive = (p.pos.y - o.pos.y) + (p.vel.y - o.vel.y) * eta - 0.5 * GRAVITY * eta * eta;
        let low = arrive < 1.8;
        let p = &mut self.players[i];
        p.bot_pounce = t + dt;
        p.yaw = (-to.x).atan2(-to.z);
        Some((0.0, 1.0, false, t <= 0.0 && low, false, low && t > 0.0))
    }

    /// Offline bots: where to go in football.
    pub(super) fn football_goal(&self, i: usize, defender: bool, jx: f32, jz: f32) -> Vec3 {
        let me = &self.players[i];
        let team = me.team;
        let jitter = Vec3::new(jx * 0.5, 0.0, jz * 0.5);
        let own = self.end_zone(team);
        let theirs = self.end_zone(team.other());
        let b = &self.ball;
        if !b.in_play { return self.kickoff_spot(team) + jitter; }
        match b.carrier {
            Some(_) if self.raised_goal(team.other()) && b.carrier == Some(i) => self.leap_spot(i, theirs),
            Some(c) if c == i => theirs,
            Some(c) if self.players.get(c).is_some_and(|p| p.team == team) => {
                // Run ahead of our carrier to block for them.
                let cp = self.players[c].pos;
                cp + (theirs - cp).normalize_or_zero() * 15.0 + jitter
            }
            Some(c) if c < self.players.len() => {
                let cp = self.players[c].pos;
                if defender && cp.distance(own) > 150.0 { own + jitter } else { cp + self.players[c].vel * 0.3 }
            }
            _ => {
                if defender && b.pos.distance(own) > 150.0 { own.lerp(b.pos, 0.3) + jitter } else { b.pos }
            }
        }
    }

    /// Offline bots: a carrier under pressure passes to a teammate further
    /// up the field that it can see, facing them so the pass finds them.
    pub(super) fn bot_football_pass(&mut self, i: usize) {
        self.players[i].bot_pass = false;
        if !self.ball.carried_by(i) { return; }
        let me = &self.players[i];
        let (pos, team) = (me.pos, me.team);
        let theirs = self.end_zone(team.other());
        let pressed = self.players.iter().any(|o| o.alive && o.team != team && o.pos.distance(pos) < 12.0);
        if !pressed { return; }
        let mine = pos.distance(theirs);
        let eye = pos + Vec3::Y * EYE;
        let open = self.players.iter().enumerate().filter(|(j, o)| *j != i && o.alive && o.team == team
                && o.pos.distance(theirs) < mine - 10.0 && o.pos.distance(pos) < 60.0)
            .find(|(_, o)| obstacle_hit(self.map, &self.pillars, eye, o.pos + Vec3::Y * 0.4, 0.0).is_none())
            .map(|(_, o)| o.pos);
        if let Some(to) = open {
            let d = to + Vec3::Y * 0.4 - eye;
            let p = &mut self.players[i];
            p.yaw = (-d.x).atan2(-d.z);
            p.pitch = (d.y / d.length().max(0.1)).asin().clamp(-0.7, 0.7);
            p.bot_pass = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field() -> Match {
        let mut m = Match::new(MapId::Raindance);
        m.world.set_mode(SupportedMode::Football);
        let a = m.join(1, "Ann").unwrap();
        let b = m.join(2, "Bob").unwrap();
        assert_ne!(m.world.players[a].team, m.world.players[b].team);
        for _ in 0..((WARMUP_SECONDS + 1.0) / STEP) as usize { m.step(&[]); }
        assert_eq!(m.phase, Phase::Playing);
        m
    }

    fn run(m: &mut Match, seconds: f32) { for _ in 0..(seconds / STEP) as usize { m.step(&[]); } }

    #[test]
    fn a_pass_lands_about_as_far_as_the_receiver() {
        for d in [10.0f32, 30.0, 60.0] {
            let v = pass_speed(d);
            let range = v * v * (2.0 * PASS_LOFT).sin() / GRAVITY;
            assert!((range - d).abs() < d * 0.05, "{d} m pass lands at {range:.1} m");
        }
    }

    #[test]
    fn a_ball_appears_is_picked_up_and_scores_a_touchdown() {
        let mut m = field();
        run(&mut m, 3.0);
        assert!(m.world.ball.in_play, "the first ball appears after a short delay");
        let ball = m.world.ball.pos;
        let side = if m.world.ball.next_side == 0 { Team::Ember } else { Team::Glacier };
        assert!(ball.distance(m.world.kickoff_spot(side)) < 3.0);
        let a = m.world.players.iter().position(|p| p.net_id == 1).unwrap();
        m.world.players[a].pos = ball;
        m.world.players[a].vel = Vec3::ZERO;
        m.step(&[]);
        assert_eq!(m.world.ball.carrier, Some(a));
        assert!(m.world.feed.iter().any(|e| e.line().contains("picks up the ball")));
        let team = m.world.players[a].team;
        m.world.players[a].pos = m.world.end_zone(team.other());
        m.step(&[]);
        assert_eq!(m.world.score[team.idx()], 1);
        assert!(!m.world.ball.in_play, "a touchdown ends the play");
        assert!(m.world.players[a].frags >= TOUCHDOWN_POINTS);
        run(&mut m, REGROUP_AFTER + 0.1);
        assert!(m.world.players[a].pos.distance(m.world.end_zone(team.other())) > END_ZONE_RADIUS,
            "everyone regroups at spawn");
        run(&mut m, NEW_BALL_AFTER - REGROUP_AFTER);
        assert!(m.world.ball.in_play);
        assert_eq!(m.world.ball.next_side, team.other().idx() as u8, "the conceding side gets the ball");
    }

    /// A defender at `defender` m/s runs into a carrier moving at `carrier`
    /// m/s toward them. Returns (ball loose, carrier down, defender down).
    fn hit(defender: f32, carrier: f32) -> (bool, bool, bool) {
        let mut m = field();
        run(&mut m, 3.0);
        let a = m.world.players.iter().position(|p| p.net_id == 1).unwrap();
        let b = m.world.players.iter().position(|p| p.net_id == 2).unwrap();
        let spot = m.world.ball.pos;
        m.world.players[a].pos = spot;
        m.step(&[]);
        assert!(m.world.ball.carried_by(a));
        let at = m.world.players[a].pos;
        m.world.players[a].vel = Vec3::new(carrier, 0.0, 0.0);
        m.world.players[b].pos = at + Vec3::new(0.9, 0.0, 0.0);
        m.world.players[b].vel = Vec3::new(-defender, 0.0, 0.0);
        m.world.step_bumps(&m.world.players.iter().map(|p| p.pos).collect::<Vec<_>>());
        (!m.world.ball.carried_by(a), m.world.players[a].stun > 0.0, m.world.players[b].stun > 0.0)
    }

    #[test]
    fn hits_follow_the_classic_rule_the_slower_player_loses() {
        assert_eq!(hit(1.0, 0.0), (false, false, false), "closing 1: a bump");
        assert_eq!(hit(8.0, 0.0), (true, false, false), "closing 8: the slower carrier fumbles");
        assert_eq!(hit(14.4, 0.0), (true, false, false), "closing 14.4 rounds to 14: fumble");
        assert_eq!(hit(14.6, 0.0), (true, true, false), "closing 14.6 rounds to 15: tackle");
        assert_eq!(hit(24.0, 0.0), (true, true, false), "hard hit: the carrier goes down");
        // The faster player wins whoever carries: a carrier running into a
        // slower defender tackles them and keeps the ball.
        assert_eq!(hit(3.0, 16.0), (false, false, true), "the faster carrier wins");
        // Equal speeds (rounded): both lose, both go down.
        assert_eq!(hit(10.2, 9.8), (true, true, true), "a tie takes both down");
    }

    #[test]
    fn a_tackled_player_cannot_move_until_they_get_up() {
        let mut m = field();
        let a = m.world.players.iter().position(|p| p.net_id == 1).unwrap();
        m.world.players[a].stun = TACKLE_STUN;
        let before = m.world.players[a].pos;
        let push = Command { seq: 1, move_z: 1.0, ..Default::default() };
        let mut commands = vec![None; MAX_PLAYERS];
        for s in 0..(TACKLE_STUN * 0.8 / STEP) as u64 {
            commands[a] = Some(Command { seq: s + 1, ..push });
            m.step(&commands);
        }
        let flat = Vec2::new(m.world.players[a].pos.x - before.x, m.world.players[a].pos.z - before.z);
        assert!(flat.length() < 0.5, "moved {} m while down", flat.length());
        for s in 0..(1.0 / STEP) as u64 {
            commands[a] = Some(Command { seq: 1000 + s, ..push });
            m.step(&commands);
        }
        assert_eq!(m.world.players[a].stun, 0.0);
    }

    #[test]
    fn passes_are_caught_by_teammates_and_intercepted_by_enemies() {
        let mut m = field();
        let c = m.join(3, "Cat").unwrap();
        run(&mut m, 4.0);
        let a = m.world.players.iter().position(|p| p.net_id == 1).unwrap();
        let (team, spot) = (m.world.players[a].team, m.world.ball.pos);
        m.world.players[a].pos = spot;
        m.step(&[]);
        assert!(m.world.ball.carried_by(a));
        let receiver = if m.world.players[c].team == team { c } else {
            m.world.players.iter().position(|p| p.net_id == 2).unwrap()
        };
        let mate = m.world.players[receiver].team == team;
        // Throw straight at a receiver 20 m away on level ground.
        let throw_from = m.world.players[a].pos;
        m.world.players[receiver].pos = throw_from + Vec3::new(0.0, 0.0, -20.0);
        m.world.players[receiver].pos.y = crate::terrain::support_on(MapId::Raindance, m.world.players[receiver].pos + Vec3::Y * 3.0).0 + 1.2;
        m.world.players[a].yaw = 0.0;
        m.world.players[a].pitch = 0.0;
        m.world.pass(a);
        assert!(!m.world.ball.carried_by(a));
        assert!(m.world.feed.iter().any(|e| e.line().contains("passes toward")));
        for _ in 0..(3.0 / STEP) as usize {
            m.world.players[receiver].vel = Vec3::ZERO;
            m.world.players[a].pos = throw_from + Vec3::new(30.0, 0.0, 0.0);
            m.step(&[]);
            if m.world.ball.carrier.is_some() { break; }
        }
        let caught = m.world.ball.carrier;
        let word = if mate { "catches" } else { "intercepts" };
        let lines: Vec<String> = m.world.feed.iter().map(|e| e.line()).collect();
        assert!(lines.iter().any(|l| l.contains(word)), "{word}: carrier {caught:?} {lines:?}");
    }

    #[test]
    fn half_time_regroups_and_gives_the_other_side_the_ball() {
        let mut m = field();
        run(&mut m, 3.0);
        let first = m.world.ball.next_side;
        m.world.time_left = HALF_SECONDS + STEP * 0.5;
        m.step(&[]);
        assert_eq!(m.world.ball.half, 1);
        assert!(!m.world.ball.in_play);
        run(&mut m, HALFTIME_BREAK + 0.2);
        assert!(m.world.ball.in_play);
        assert_ne!(m.world.ball.next_side, first);
    }

    #[test]
    fn an_idle_ball_is_reset_and_other_modes_have_none() {
        let mut m = field();
        run(&mut m, 3.0);
        assert!(m.world.ball.in_play);
        run(&mut m, IDLE_RESET + 1.0);
        assert!(m.world.feed.iter().any(|e| e.line().contains("sat idle")));
        let mut ctf = Match::new(MapId::Raindance);
        ctf.join(1, "Ann").unwrap();
        ctf.join(2, "Bob").unwrap();
        run(&mut ctf, 5.0);
        assert!(!ctf.world.ball.active);
    }

    #[test]
    fn football_state_survives_the_wire() {
        let mut m = field();
        run(&mut m, 3.0);
        let a = m.world.players.iter().position(|p| p.net_id == 1).unwrap();
        m.world.players[a].pos = m.world.ball.pos;
        m.step(&[]);
        m.world.players[a].stun = 1.25;
        let text = serde_json::to_string(&m.snapshot()).unwrap();
        let back: Snapshot = serde_json::from_str(&text).unwrap();
        assert_eq!(back.ball, m.world.ball);
        assert_eq!(back.players[a].stun, 1.25);
        assert_eq!(back.mode, SupportedMode::Football);
        let mut client = World::new();
        assert!(client.apply_snapshot(&back, 1));
        assert!(client.ball.carried_by(a));
    }

    #[test]
    fn longfield_declares_a_valid_field_with_grounded_spawns() {
        assert!(has_field(MapId::Longfield));
        for map in [MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone, MapId::SnowblindClone, MapId::DesertOfDeathClone] {
            assert!(!has_field(map), "{map:?} is a CTF map");
        }
        let f = super::field(MapId::Longfield).unwrap();
        let apart = Vec3::from(f.end_zones[0]).distance(Vec3::from(f.end_zones[1]));
        assert!((240.0..=260.0).contains(&apart), "end zones {apart} m apart");
        for t in 0..2 {
            for p in [f.end_zones[t], f.kickoff[t]] {
                let floor = crate::terrain::support_on(MapId::Longfield, Vec3::from(p) + Vec3::Y * 2.0).0;
                assert!((Vec3::from(p).y - floor - 1.1).abs() < 0.15, "{p:?} sits over the turf at {floor}");
            }
        }
        for (team, points) in crate::map_pack::on(MapId::Longfield).unwrap().manifest.spawn_points.iter().enumerate() {
            assert_eq!(points.len(), 8);
            for p in points {
                let pos = Vec3::new(p[0], p[1], p[2]);
                let floor = crate::terrain::support_on(MapId::Longfield, pos).0;
                assert!((pos.y - floor - 1.2).abs() < 0.02, "team {team} spawn {pos} floor {floor}");
                // Spawns face up the field, toward the other end zone.
                let ahead = look_dir(p[3], 0.0);
                assert!(ahead.dot(Vec3::from(f.end_zones[1 - team]) - pos) > 0.0);
            }
        }
    }

    #[test]
    fn bots_carry_pass_tackle_and_score_on_longfield() {
        let mut w = World::new();
        w.set_map(MapId::Longfield);
        w.set_mode(SupportedMode::Football);
        w.start_match(true);
        assert!(w.players.len() >= 6, "a full bot match");
        let (mut carries, mut tackles) = (0, 0);
        let mut carrier = None;
        let mut down = vec![false; w.players.len()];
        for _ in 0..(240.0 / STEP) as usize {
            if w.state != MatchState::Playing { break; }
            w.physics_step();
            if w.ball.carrier != carrier { if w.ball.carrier.is_some() { carries += 1; } carrier = w.ball.carrier; }
            for (d, p) in down.iter_mut().zip(&w.players) { if p.stun > 0.0 && !*d { tackles += 1; } *d = p.stun > 0.0; }
        }
        let touchdowns = w.score[0] + w.score[1];
        println!("{} players: {carries} carries, {tackles} tackles, score {:?}, {:.0} s left, state {:?}", w.players.len(), w.score, w.time_left, w.state);
        assert!(carries >= 4 && touchdowns >= 1 && tackles >= 1, "{carries} carries, {tackles} tackles, {touchdowns} touchdowns in 4 min");
    }

    /// Peak rise of the body centre on flat turf with a full tank, and how
    /// long a full tank of jet lasts: after settling, `jump_at` is when (s)
    /// jump is tapped (None = never), jet held from `jet_at`, running forward
    /// at `run` m/s first.
    pub(crate) fn peak_rise(mode: SupportedMode, jump_at: Option<f32>, jet_at: f32, run: f32) -> (f32, f32) {
        let mut w = World::new();
        w.set_map(MapId::Longfield);
        w.set_mode(mode);
        w.start_match(true);
        w.players.truncate(1);
        let p = &mut w.players[0];
        p.pos = Vec3::new(1024.0, 101.2, 1000.0); p.yaw = 0.0;
        w.input.move_z = if run > 0.0 { 1.0 } else { 0.0 };
        for _ in 0..(1.5 / STEP) as usize { w.physics_step(); }
        let start = w.players[0].pos;
        w.players[0].energy = ENERGY_MAX;
        let (mut top, mut empty) = (0.0f32, None);
        for k in 0..(6.0 / STEP) as usize {
            let t = k as f32 * STEP;
            w.input.jump = jump_at.is_some_and(|j| t >= j && t < j + 0.05);
            w.input.jet = t >= jet_at;
            w.physics_step();
            let p = &w.players[0];
            top = top.max(p.pos.y - start.y);
            if empty.is_none() && t >= jet_at && p.energy < 1.0 { empty = Some(t - jet_at); }
        }
        (top, empty.unwrap_or(f32::INFINITY))
    }

    #[test]
    fn football_jet_is_a_short_hard_burst() {
        let standing = peak_rise(SupportedMode::Football, None, 0.0, 0.0);
        let together = peak_rise(SupportedMode::Football, Some(0.0), 0.0, 0.0);
        let apex = peak_rise(SupportedMode::Football, Some(0.0), 0.5, 0.0);
        let running = peak_rise(SupportedMode::Football, Some(0.0), 0.0, 11.0);
        let ctf = peak_rise(SupportedMode::Ctf, Some(0.0), 0.0, 0.0);
        println!("football: standing jet {:.2} m ({:.2} s burst), jump+jet {:.2}, jet at jump apex {:.2}, running jump+jet {:.2}; ctf jump+jet {:.2} m",
            standing.0, standing.1, together.0, apex.0, running.0, ctf.0);
        assert!((0.4..=0.6).contains(&standing.1), "a full tank lasts about half a second: {}", standing.1);
        // A raised goal (RAISED_GOAL_HEIGHT): the jet alone only just clears
        // it, a jet late off a jump falls short, a jump and jet clears it easily.
        assert!((RAISED_GOAL_HEIGHT + 0.3..RAISED_GOAL_HEIGHT + 1.5).contains(&standing.0), "jet alone reaches {}", standing.0);
        assert!(apex.0 < RAISED_GOAL_HEIGHT, "a late jet reaches {}", apex.0);
        assert!(together.0 > RAISED_GOAL_HEIGHT + 5.0, "jump and jet reach {}", together.0);
        assert!(ctf.0 > 2.0 * together.0, "CTF armor is unchanged and far stronger");
    }

    /// A carrier stands `from` metres in front of the Highgoal platform's
    /// centre (its edge is 4 m from the centre), launches straight up
    /// (jumping or not) on a full tank, and once above the top lets go of the
    /// jet and drifts forward onto it with air control. Returns whether they scored.
    fn leap_at_goal(from: f32, jump: bool) -> bool {
        let mut w = World::new();
        w.set_map(MapId::Highgoal);
        w.set_mode(SupportedMode::Football);
        w.start_match(true);
        w.players.truncate(1);
        let zone = w.end_zone(Team::Glacier);
        let floor = crate::terrain::support_on(MapId::Highgoal, Vec3::new(zone.x, 105.0, zone.z - from)).0;
        let p = &mut w.players[0];
        p.pos = Vec3::new(zone.x, floor + PLAYER_RADIUS, zone.z - from);
        p.yaw = std::f32::consts::PI;
        w.ball = Ball { active: true, in_play: true, carrier: Some(0), ..Ball::default() };
        for _ in 0..(0.5 / STEP) as usize { w.physics_step(); }
        w.players[0].energy = ENERGY_MAX;
        for k in 0..(6.0 / STEP) as usize {
            let above = w.players[0].pos.y - floor > RAISED_GOAL_HEIGHT + 0.3;
            w.input.jump = jump && k < 3;
            w.input.jet = !above;
            w.input.move_z = if above { 1.0 } else { 0.0 };
            w.physics_step();
            if w.score[0] > 0 { return true; }
        }
        false
    }

    #[test]
    fn the_jet_alone_only_just_reaches_the_raised_goal() {
        let spots = [4.6f32, 5.0, 5.5, 6.0, 7.0, 8.0, 10.0];
        let jumps: Vec<f32> = spots.into_iter().filter(|&d| leap_at_goal(d, true)).collect();
        let jets: Vec<f32> = spots.into_iter().filter(|&d| leap_at_goal(d, false)).collect();
        println!("jump+jet scores from {jumps:?} m out; jet alone from {jets:?}");
        assert!(jumps.len() >= jets.len() && !jets.is_empty(), "the jet alone must just make it");
        assert!(jets.len() < spots.len(), "the jet alone has little room to spare");
    }

    #[test]
    fn bots_leap_onto_raised_goals_and_score_on_highgoal() {
        let mut w = World::new();
        w.set_map(MapId::Highgoal);
        w.set_mode(SupportedMode::Football);
        w.start_match(true);
        let (mut carries, mut carrier) = (0, None);
        for _ in 0..(600.0 / STEP) as usize {
            if w.state != MatchState::Playing { break; }
            w.physics_step();
            if w.ball.carrier != carrier { if w.ball.carrier.is_some() { carries += 1; } carrier = w.ball.carrier; }
        }
        println!("highgoal: {} players, {carries} carries, score {:?}", w.players.len(), w.score);
        assert!(w.score[0] + w.score[1] >= 1, "bots must score by leaping: {carries} carries, {:?}", w.score);
    }

    #[test]
    fn leaving_the_field_hurts_every_second_and_an_idle_ball_regroups_everyone() {
        let mut m = Match::new(MapId::Longfield);
        m.world.set_mode(SupportedMode::Football);
        let a = m.join(1, "Ann").unwrap();
        m.join(2, "Bob").unwrap();
        run(&mut m, WARMUP_SECONDS + 1.0);
        // Up on the stands, outside the bounds.
        let [x0, _, x1, _] = super::field(MapId::Longfield).unwrap().bounds.unwrap();
        let stands = Vec3::new(x1 + 10.0, 145.0, 1024.0);
        m.world.players[a].pos = stands;
        m.world.players[a].health = 100.0;
        let spawns = crate::terrain::spawn_points_on(MapId::Longfield, m.world.players[a].team == Team::Ember);
        run(&mut m, 2.2);
        let lost = 100.0 - m.world.players[a].health;
        // A global once-a-second tick, as in the classic mod: two or three in 2.2 s.
        let ticks = lost / OUT_OF_BOUNDS_DAMAGE;
        assert!((ticks - 2.0).abs() < 0.01 || (ticks - 3.0).abs() < 0.01, "lost {lost}");
        assert!(x0 < x1);
        // An idle ball sends everyone back to spawn.
        run(&mut m, 25.0);
        assert!(m.world.feed.iter().any(|e| e.line().contains("sat idle")));
        let home = m.world.players[a].pos;
        assert!(spawns.iter().any(|(s, _)| s.distance(home) < 3.0), "back at a spawn: {home}");
    }

    #[test]
    fn a_new_football_match_starts_on_a_full_clock_without_a_false_half_time() {
        let mut m = Match::new(MapId::Longfield);
        m.world.set_mode(SupportedMode::Football);
        assert_eq!(m.world.time_left, 2.0 * HALF_SECONDS);
        m.join(1, "Ann").unwrap();
        run(&mut m, 3.0);
        assert_eq!(m.world.ball.half, 0);
        assert!(!m.world.feed.iter().any(|e| e.line().contains("Half time")));
    }

    #[test]
    fn ctrl_k_kills_you_through_the_server_and_drops_the_ball() {
        let mut m = field();
        run(&mut m, 3.0);
        let a = m.world.players.iter().position(|p| p.net_id == 1).unwrap();
        m.world.players[a].pos = m.world.ball.pos;
        m.step(&[]);
        assert!(m.world.ball.carried_by(a));
        let mut commands = vec![None; MAX_PLAYERS];
        commands[a] = Some(Command { seq: 1, suicide: true, ..Default::default() });
        m.step(&commands);
        assert!(!m.world.players[a].alive);
        assert!(!m.world.ball.carried_by(a), "the ball drops");
        assert_eq!(m.world.players[a].losses, 1);
        assert_eq!(m.world.players[a].frags, 0, "no frag for yourself");
        assert!(m.world.feed.iter().any(|e| e.line().contains("took the quick way out")));
        // Football respawns after a second.
        run(&mut m, RESPAWN_SECONDS + 0.1);
        assert!(m.world.players[a].alive);
        // A client never predicts its own death.
        let mut client = World::new();
        assert!(client.apply_snapshot(&m.snapshot(), 1));
        client.predict_command(Command { seq: 2, suicide: true, ..Default::default() });
        assert!(client.players[client.player_id].alive);
    }

    /// A thrower on the Longfield turf passes to a receiver placed `ahead`
    /// metres in front, `up` metres higher (standing on air, held in place
    /// kinematically) and running sideways at `run` m/s. Returns who caught it.
    fn pass_to(ahead: f32, up: f32, run: f32, aim_off: f32) -> Option<usize> {
        let mut w = World::new();
        w.set_map(MapId::Longfield); w.set_mode(SupportedMode::Football); w.start_match(true);
        w.players.truncate(1);
        let mut r = w.players[0].clone(); r.team = w.players[0].team; r.is_bot = false; w.players.push(r);
        let floor = 100.0 + PLAYER_RADIUS;
        // A negative `up` raises the thrower instead (the turf is flat).
        let thrower = Vec3::new(1024.0, floor + (-up).max(0.0), 1060.0);
        let start = Vec3::new(1024.0, floor + up.max(0.0), 1060.0 - ahead);
        w.players[0].pos = thrower; w.players[0].vel = Vec3::ZERO; w.players[0].yaw = aim_off;
        // Look at the receiver the way a player would.
        let eye = thrower + Vec3::Y * EYE;
        let to = start + Vec3::Y * 0.4 - eye;
        w.players[0].pitch = (to.y / to.length()).asin();
        w.ball = Ball { active: true, in_play: true, carrier: Some(0), ..Ball::default() };
        let v = Vec3::new(run, 0.0, 0.0);
        w.players[1].pos = start; w.players[1].vel = v;
        w.pass(0);
        for k in 1..(8.0 / STEP) as usize {
            w.players[1].pos = start + v * (k as f32 * STEP);
            w.players[1].vel = v;
            w.players[0].pos = thrower + Vec3::new(0.0, 0.0, 30.0);
            w.step_ball(STEP);
            if w.ball.carrier.is_some() { return w.ball.carrier; }
        }
        None
    }

    #[test]
    fn passes_reach_the_receiver_you_aim_at_near_and_across_the_field() {
        for ahead in [10.0, 30.0, 60.0, 90.0, 160.0] {
            assert_eq!(pass_to(ahead, 0.0, 0.0, 0.0), Some(1), "{ahead} m ahead");
        }
        assert!((pass_speed(300.0) - 6.454 * 300f32.sqrt()).abs() < 1e-3, "the mod's power, uncapped");
        // Aim off to the side and the ball goes where you aimed, not to them.
        assert_eq!(pass_to(40.0, 0.0, 0.0, 0.12), None, "the aim, not the receiver, sets the direction");
    }

    #[test]
    fn football_skiing_bleeds_speed_on_the_flat_but_ctf_skiing_does_not() {
        let slide = |mode: SupportedMode| {
            let mut w = World::new();
            w.set_map(MapId::Longfield); w.set_mode(mode); w.start_match(true);
            w.players.truncate(1);
            w.players[0].pos = Vec3::new(1024.0, 100.0 + PLAYER_RADIUS, 1060.0);
            w.players[0].vel = Vec3::new(0.0, 0.0, -18.0);
            w.players[0].on_ground = true;
            w.input.jump = true;
            for _ in 0..(3.0 / STEP) as usize { w.physics_step(); }
            Vec2::new(w.players[0].vel.x, w.players[0].vel.z).length()
        };
        let football = slide(SupportedMode::Football);
        let ctf = slide(SupportedMode::Ctf);
        assert!(football < 17.0 && football > 12.0, "football slows: {football}");
        assert!(ctf > 17.5, "ctf keeps its speed: {ctf}");
    }

    #[test]
    fn throws_follow_the_mod_aim_only_no_carried_momentum() {
        let throw = |pitch: f32, run: f32| {
            let mut w = World::new();
            w.set_map(MapId::Longfield); w.set_mode(SupportedMode::Football); w.start_match(true);
            w.players.truncate(1);
            w.players[0].pos = Vec3::new(1024.0, 100.0 + PLAYER_RADIUS, 1060.0);
            w.players[0].yaw = 0.0;
            w.players[0].pitch = pitch;
            w.players[0].vel = Vec3::new(0.0, 0.0, -run);
            w.ball = Ball { active: true, in_play: true, carrier: Some(0), ..Ball::default() };
            w.pass(0);
            w.ball.vel
        };
        let still = throw(1.4, 0.0);
        let running = throw(1.4, 12.0);
        // Looking straight up still throws about 25 degrees forward.
        let up = throw(1.52, 0.0);
        let lean = (-up.z).atan2(up.y).to_degrees();
        assert!((lean - 25.0).abs() < 1.0 && up.y > 25.0, "straight up leans forward: {lean} {up}");
        assert!(still.y > 25.0 && -still.z > 0.2 * still.y, "steep throws go forward too {still}");
        assert!((running - still).length() < 1e-4, "the mod zeroes the thrower's velocity: {running} vs {still}");
        let low = throw(0.0, 0.0);
        let high = throw(0.6, 0.0);
        assert!(high.y > low.y && -low.z > -high.z, "the aim sets the direction");
        // Low throws keep the plain loft.
        assert_eq!(throw_elevation(0.3), 0.3 + PASS_LOFT);
    }

    #[test]
    fn a_fumbling_carrier_cannot_grab_the_ball_straight_back() {
        let mut w = World::new();
        w.set_map(MapId::Longfield); w.set_mode(SupportedMode::Football); w.start_match(true);
        w.players.truncate(1);
        w.players[0].pos = Vec3::new(1024.0, 100.0 + PLAYER_RADIUS, 1060.0);
        w.ball = Ball { active: true, in_play: true, carrier: Some(0), ..Ball::default() };
        w.fumble(0);
        for _ in 0..5 { w.step_ball(STEP); }
        assert!(!w.ball.carried_by(0), "the fumble stands");
    }
}
