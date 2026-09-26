//! Deployables: turrets, walls, force fields and ammo stations bought at an inventory
//! station (one pack at a time) and placed where you aim (the client shows a
//! hologram from `aim_placement` while you line it up, and sends the turn). Each team may have only so many of each standing. Walls block
//! everyone and everything; force fields block only the other team and its
//! shots. An ammo station restocks teammates who use it (hold E beside it).
//! All take damage, can be repaired with the repair tool, and are
//! destroyed at zero. Server-owned; snapshots carry them whole. Marker: `loadout1`.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployKind { Turret, Wall, Field, Ammo }

/// A unit being lined up, and why it can't go there (None: it can).
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub unit: Deployable,
    pub problem: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Deployable {
    pub kind: DeployKind,
    pub team: Team,
    /// Base, on the floor.
    pub pos: Vec3,
    /// Facing (a panel spans the deployer's left-right).
    pub yaw: f32,
    pub health: f32,
    pub cooldown: f32,
    /// A turret's current aim.
    pub aim: Vec3,
}

/// Standing deployables of one kind a team may have at once.
pub fn team_limit(kind: DeployKind) -> usize {
    match kind { DeployKind::Turret => 4, DeployKind::Wall => 6, DeployKind::Field => 4, DeployKind::Ammo => 5 }
}
pub fn max_health(kind: DeployKind) -> f32 {
    match kind { DeployKind::Turret => 150.0, DeployKind::Wall => 400.0, DeployKind::Field => 250.0, DeployKind::Ammo => 150.0 }
}
/// Half-extents of an ammo station crate.
pub const AMMO_HALF: Vec3 = Vec3::new(0.55, 0.45, 0.4);
/// How close you must stand to use an ammo station.
pub const AMMO_REACH: f32 = 2.5;
/// A box-shaped deployable's half-extents (a turret is a post instead).
pub fn half_extents(kind: DeployKind) -> Option<Vec3> {
    match kind { DeployKind::Turret => None, DeployKind::Ammo => Some(AMMO_HALF), _ => Some(PANEL_HALF) }
}
/// Half-extents of a wall or field panel: 4 m wide, 3.2 m tall, 0.36 m thick.
pub const PANEL_HALF: Vec3 = Vec3::new(2.0, 1.6, 0.18);
/// A deployed turret is a squat post with this body radius.
pub const TURRET_RADIUS: f32 = 0.7;
const TURRET_HEIGHT: f32 = 1.3;
/// How far away (from your eye, along your aim) a deployable can go down.
pub const DEPLOY_RANGE: f32 = 12.0;
/// Keep clear of flag stands and spawns.
const FLAG_CLEAR: f32 = 12.0;
const SPAWN_CLEAR: f32 = 6.0;
const DEPLOY_SPACING: f32 = 3.0;
/// A deployed turret: chaingun rounds, shorter range than a base turret.
pub const TURRET_PROFILE: crate::equipment::TurretProfile = crate::equipment::TurretProfile {
    range: 70.0, sensed_range: 70.0, fires: true, speed: BOLT_SPEED, life: 1.0, cooldown: 0.25, leads: false, projectile: 1,
};

impl Deployable {
    pub fn center(&self) -> Vec3 {
        match self.kind {
            DeployKind::Turret => self.pos + Vec3::Y * 0.9,
            DeployKind::Ammo => self.pos + Vec3::Y * AMMO_HALF.y,
            _ => self.pos + Vec3::Y * PANEL_HALF.y,
        }
    }
    /// Blocks a body or shot of `team` (None: everything, e.g. sight lines).
    pub fn blocks(&self, team: Option<Team>) -> bool {
        self.kind != DeployKind::Field || team != Some(self.team)
    }
    fn to_local(&self, p: Vec3) -> Vec3 {
        Mat4::from_rotation_y(-self.yaw).transform_point3(p - self.center())
    }
    /// First fraction along `a` to `b` where a sphere of `radius` touches it.
    pub fn hit(&self, a: Vec3, b: Vec3, radius: f32) -> Option<f32> {
        match self.kind {
            DeployKind::Turret => {
                segment_sphere(a, b, self.center(), TURRET_RADIUS + radius)
            }
            kind => {
                let half = half_extents(kind).unwrap_or(PANEL_HALF) + Vec3::splat(radius);
                segment_box(self.to_local(a), self.to_local(b), -half, half)
            }
        }
    }
}

impl World {
    /// Nearest deployable hit along `a` to `b` by a sphere of `radius` that
    /// blocks `team`, as (fraction, index).
    pub fn deployable_hit(&self, a: Vec3, b: Vec3, radius: f32, team: Option<Team>) -> Option<(f32, usize)> {
        self.deployables.iter().enumerate().filter(|(_, e)| e.blocks(team))
            .filter_map(|(k, e)| e.hit(a, b, radius).map(|t| (t, k)))
            .min_by(|x, y| x.0.total_cmp(&y.0))
    }

    /// Map, pillars and deployables together.
    pub fn blocked(&self, a: Vec3, b: Vec3, radius: f32, team: Option<Team>) -> Option<f32> {
        let map = obstacle_hit(self.map, &self.pillars, a, b, radius);
        let dep = self.deployable_hit(a, b, radius, team).map(|(t, _)| t);
        match (map, dep) { (Some(x), Some(y)) => Some(x.min(y)), (x, y) => x.or(y) }
    }

    /// Push player `i` out of any deployable that blocks them.
    pub(super) fn collide_deployables(&mut self, i: usize) {
        let team = self.players[i].team;
        for k in 0..self.deployables.len() {
            let e = &self.deployables[k];
            if !e.blocks(Some(team)) { continue; }
            let p = &self.players[i];
            let r = PLAYER_RADIUS;
            match e.kind {
                DeployKind::Turret => {
                    let c = e.pos;
                    let flat = Vec2::new(p.pos.x - c.x, p.pos.z - c.z);
                    let d = flat.length();
                    if d >= TURRET_RADIUS + r || p.pos.y > c.y + TURRET_HEIGHT + r || p.pos.y < c.y - 1.0 { continue; }
                    let n = if d > 1e-3 { flat / d } else { Vec2::X };
                    let out = Vec3::new(n.x, 0.0, n.y);
                    let p = &mut self.players[i];
                    p.pos += out * (TURRET_RADIUS + r - d);
                    let vn = p.vel.dot(out);
                    if vn < 0.0 { p.vel -= out * vn; }
                }
                kind => {
                    let local = e.to_local(p.pos);
                    let half = half_extents(kind).unwrap_or(PANEL_HALF) + Vec3::new(r, r, r);
                    if local.x.abs() >= half.x || local.y.abs() >= half.y + 0.5 || local.z.abs() >= half.z { continue; }
                    // Out through the nearer face: the panel's front or back,
                    // or round an end.
                    let (push, axis) = if half.z - local.z.abs() < half.x - local.x.abs() {
                        ((half.z - local.z.abs()) * local.z.signum(), Vec3::Z)
                    } else {
                        ((half.x - local.x.abs()) * local.x.signum(), Vec3::X)
                    };
                    let out = Mat4::from_rotation_y(e.yaw).transform_vector3(axis);
                    let p = &mut self.players[i];
                    p.pos += out * push;
                    let dir = out * push.signum();
                    let vn = p.vel.dot(dir);
                    if vn < 0.0 { p.vel -= dir * vn; }
                }
            }
        }
    }

    /// The deploy intent: place the carried pack where the player aims,
    /// turned by the command's `deploy_turn`, if the team has room for another
    /// and the spot is clear.
    pub(super) fn step_deploys(&mut self) {
        // Deployables take a side, so Deathmatch has none.
        if self.predicting || self.football() || self.ffa() { return; }
        for i in 0..self.players.len() {
            let (wants, turn) = if self.players[i].is_bot { (false, 0.0) }
                else if self.network_inputs.is_empty() {
                    if i == self.player_id { (self.input.deploy, self.input.deploy_turn) } else { (false, 0.0) }
                } else { self.network_inputs.get(i).map_or((false, 0.0), |c| (c.deploy, c.deploy_turn)) };
            let Some(kind) = self.players[i].pack.filter(|_| wants && self.players[i].alive) else { continue };
            match self.place(i, kind, turn) {
                Ok(e) => {
                    self.deployables.push(e);
                    self.players[i].pack = None;
                    if i == self.player_id { self.push_event("deploy"); }
                }
                Err(why) => if i == self.player_id { self.msg(why, 1.6); },
            }
        }
    }

    /// Where player `i` would put down a `kind`, or why they can't.
    pub fn place(&self, i: usize, kind: DeployKind, turn: f32) -> Result<Deployable, &'static str> {
        match self.aim_placement(i, kind, turn) {
            None => Err("TOO FAR AWAY"),
            Some(Placement { problem: Some(why), .. }) => Err(why),
            Some(Placement { unit, .. }) => Ok(unit),
        }
    }

    /// The unit player `i` is lining up: where their aim meets a surface
    /// within DEPLOY_RANGE, facing their yaw plus `turn`, and the first reason
    /// it can't go there (None when it can). None when nothing is in reach.
    /// The client draws this as the hologram; the server checks it again.
    pub fn aim_placement(&self, i: usize, kind: DeployKind, turn: f32) -> Option<Placement> {
        let p = self.players.get(i)?;
        let eye = p.pos + Vec3::Y * EYE;
        let dir = look_dir(p.yaw, p.pitch);
        let far = eye + dir * DEPLOY_RANGE;
        let t = self.blocked(eye, far, 0.0, None)?;
        let spot = eye.lerp(far, t);
        let on_unit = self.deployable_hit(eye, far, 0.0, None).is_some_and(|(u, _)| u <= t + 1e-4);
        let (floor, normal) = crate::terrain::support_on(self.map, spot + Vec3::Y * 0.4);
        let flat = normal.y >= 0.85 && (floor - spot.y).abs() <= 0.6;
        let pos = Vec3::new(spot.x, if flat { floor } else { spot.y }, spot.z);
        let yaw = p.yaw + if turn.is_finite() { turn } else { 0.0 };
        let (fwd, right) = move_basis(yaw);
        let unit = Deployable { kind, team: p.team, pos, yaw, health: max_health(kind), cooldown: 0.0, aim: fwd };
        let problem = (|| {
            let standing = self.deployables.iter().filter(|e| e.team == p.team && e.kind == kind).count();
            if standing >= team_limit(kind) { return Some("YOUR TEAM HAS THE MAXIMUM OF THOSE DEPLOYED"); }
            if on_unit { return Some("NO ROOM THERE"); }
            if !flat { return Some("NO FLAT GROUND THERE"); }
            let flags = [self.flags[0].home, self.flags[1].home];
            if flags.iter().any(|f| f.distance(pos) < FLAG_CLEAR) { return Some("TOO CLOSE TO A FLAG"); }
            for team in [true, false] {
                if crate::terrain::spawn_points_on(self.map, team).iter().any(|(s, _)| s.distance(pos) < SPAWN_CLEAR) {
                    return Some("TOO CLOSE TO A SPAWN");
                }
            }
            if self.deployables.iter().any(|e| e.pos.distance(pos) < DEPLOY_SPACING) { return Some("TOO CLOSE TO ANOTHER DEPLOYABLE"); }
            // Room overhead for the unit itself.
            if self.blocked(pos + Vec3::Y * 0.3, pos + Vec3::Y * 1.2, 0.0, None).is_some() { return Some("NO ROOM THERE"); }
            if matches!(kind, DeployKind::Wall | DeployKind::Field) {
                // The whole panel must fit: nothing solid across it or under its ends.
                let mid = pos + Vec3::Y * PANEL_HALF.y;
                if self.blocked(mid - right * PANEL_HALF.x, mid + right * PANEL_HALF.x, 0.1, None).is_some() {
                    return Some("NO ROOM THERE");
                }
                for end in [-1.0f32, 1.0] {
                    let foot = pos + right * PANEL_HALF.x * end;
                    let under = crate::terrain::support_on(self.map, foot + Vec3::Y * 1.5).0;
                    if (under - floor).abs() > 1.0 { return Some("NO FLAT GROUND THERE"); }
                }
            }
            if self.players.iter().any(|o| o.alive && unit.hit(o.pos - Vec3::Y * 0.3, o.pos + Vec3::Y * 1.5, PLAYER_RADIUS).is_some()) {
                return Some("SOMEONE IS IN THE WAY");
            }
            None
        })();
        Some(Placement { unit, problem })
    }

    /// Damage deployable `k`; destroyed ones blow up (harmlessly) and vanish.
    pub(super) fn damage_deployable(&mut self, k: usize, amount: f32) {
        let Some(e) = self.deployables.get_mut(k) else { return };
        e.health -= amount;
    }

    /// Ammo stations: a living player holding use (E) within reach of one of
    /// their team's restocks every weapon; bots restock just by standing there.
    pub(super) fn step_ammo_stations(&mut self) {
        if self.predicting || self.football() { return; }
        for i in 0..self.players.len() {
            let p = &self.players[i];
            if !p.alive { continue; }
            let using = if p.is_bot { true } else if self.network_inputs.is_empty() { i == self.player_id && self.input.interact }
                else { self.network_inputs.get(i).is_some_and(|c| c.interact) };
            let near = self.deployables.iter().any(|e| e.kind == DeployKind::Ammo && e.team == p.team && e.center().distance(p.pos) <= AMMO_REACH);
            if using && near && (p.ammo != loadout::max_ammo(p.armor) || p.throwables != super::throwables::max_throwables(p.armor)) {
                self.refill(i);
                if i == self.player_id { self.push_event("buy"); }
            }
        }
    }

    /// Whether player `i` stands at one of their team's ammo stations.
    pub fn at_ammo_station(&self, i: usize) -> bool {
        let p = &self.players[i];
        self.deployables.iter().any(|e| e.kind == DeployKind::Ammo && e.team == p.team && e.center().distance(p.pos) <= AMMO_REACH)
    }

    /// Turrets track and fire; wrecks are cleared with a blast.
    pub(super) fn step_deployables(&mut self, dt: f32) {
        if self.predicting { return; }
        let mut gone = Vec::new();
        for k in 0..self.deployables.len() {
            if self.deployables[k].health <= 0.0 { gone.push(k); continue; }
            if self.deployables[k].kind != DeployKind::Turret { continue; }
            let (origin, team) = (self.deployables[k].center(), self.deployables[k].team);
            self.deployables[k].cooldown = (self.deployables[k].cooldown - dt).max(0.0);
            let candidates = self.players.iter().enumerate().filter(|(_, p)| p.alive)
                .map(|(index, p)| crate::equipment::Candidate { index, team: p.team.idx() as u8, pos: p.pos, vel: p.vel });
            let clear = |a: Vec3, b: Vec3| {
                obstacle_hit(self.map, &self.pillars, a, b, 0.0).is_none()
                    && self.deployables.iter().enumerate().all(|(j, e)| j == k || !e.blocks(Some(team)) || e.hit(a, b, 0.0).is_none())
            };
            let Some(hit) = crate::equipment::acquire_target(origin, TURRET_RADIUS, team.idx() as u8, &TURRET_PROFILE,
                false, candidates, clear) else { continue };
            self.deployables[k].aim = hit.aim;
            if self.deployables[k].cooldown <= 0.0 && self.discs.len() < 256 {
                self.discs.push(Disc { pos: hit.muzzle, vel: hit.aim * TURRET_PROFILE.speed, team, owner: MAX_PLAYERS,
                    life: TURRET_PROFILE.life, kind: TURRET_PROFILE.projectile, spin: -(k as f32) - 1.0 });
                self.deployables[k].cooldown = TURRET_PROFILE.cooldown;
            }
        }
        for k in gone.into_iter().rev() {
            let e = self.deployables.remove(k);
            self.blast_serial += 1;
            self.explosions.push(Explosion { pos: e.center(), age: 0.0, max_r: 4.0, kind: 4 });
            self.spatial_sounds.push(("boom", e.center()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_catalog::SupportedMode;

    /// A player alone on Raindance, standing on open ground at their spawn,
    /// carrying a pack.
    fn world(kind: DeployKind) -> World {
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        w.set_mode(SupportedMode::Ctf);
        w.start_match(true);
        w.players.truncate(1);
        let spot = Vec3::new(1060.0, 0.0, 700.0);
        let floor = crate::terrain::support_on(MapId::Raindance, spot + Vec3::Y * 200.0).0;
        w.players[0].pos = Vec3::new(spot.x, floor + PLAYER_RADIUS, spot.z);
        w.players[0].pack = Some(kind);
        // Looking down at the ground a few metres ahead.
        w.players[0].pitch = -0.3;
        w
    }

    #[test]
    fn a_wall_goes_down_ahead_and_blocks_everyone_a_field_only_the_enemy() {
        for kind in [DeployKind::Wall, DeployKind::Field] {
            let mut w = world(kind);
            let yaw = (0..16).map(|k| k as f32 * 0.39).find(|&y| { w.players[0].yaw = y; w.place(0, kind, 0.0).is_ok() })
                .expect("somewhere to deploy");
            w.players[0].yaw = yaw;
            w.input.deploy = true;
            w.step_deploys();
            assert_eq!(w.deployables.len(), 1, "{kind:?}");
            assert!(w.players[0].pack.is_none());
            let e = w.deployables[0].clone();
            let (fwd, _) = move_basis(yaw);
            // A shot straight through the panel.
            let a = e.center() - fwd * 3.0;
            let b = e.center() + fwd * 3.0;
            assert!(w.blocked(a, b, 0.1, Some(Team::Glacier)).is_some(), "stops enemy shots");
            assert_eq!(w.blocked(a, b, 0.1, Some(Team::Ember)).is_some(), kind == DeployKind::Wall, "own team through a field");
            // A player walking into it from in front is held back unless it's their field.
            for team in [Team::Ember, Team::Glacier] {
                w.players[0].team = team;
                w.players[0].pos = e.pos + Vec3::Y * PLAYER_RADIUS - fwd * 0.3;
                w.players[0].vel = fwd * 8.0;
                w.collide_deployables(0);
                let passes = kind == DeployKind::Field && team == Team::Ember;
                let inside = e.to_local(w.players[0].pos).z.abs() < PANEL_HALF.z + PLAYER_RADIUS - 0.01;
                assert_eq!(inside, passes, "{kind:?} {team:?}");
            }
        }
    }

    #[test]
    fn the_unit_goes_where_you_aim_turns_and_vanishes_out_of_reach() {
        let mut w = world(DeployKind::Turret);
        let yaw = (0..16).map(|k| k as f32 * 0.39).find(|&y| { w.players[0].yaw = y; w.place(0, DeployKind::Turret, 0.0).is_ok() })
            .expect("somewhere to deploy");
        w.players[0].yaw = yaw;
        let eye = w.players[0].pos;
        let flat = |e: &Deployable| Vec2::new(e.pos.x - eye.x, e.pos.z - eye.z).length();
        // Looking lower puts it nearer; looking at the horizon puts it out of reach.
        w.players[0].pitch = -0.6;
        let near = w.aim_placement(0, DeployKind::Turret, 0.0).expect("in reach");
        w.players[0].pitch = -0.3;
        let farther = w.aim_placement(0, DeployKind::Turret, 0.0).expect("in reach");
        assert!(flat(&farther.unit) > flat(&near.unit) + 1.0, "{} vs {}", flat(&farther.unit), flat(&near.unit));
        w.players[0].pitch = 0.2;
        assert!(w.aim_placement(0, DeployKind::Turret, 0.0).is_none(), "nothing within reach: no hologram");
        assert_eq!(w.place(0, DeployKind::Turret, 0.0).err(), Some("TOO FAR AWAY"));
        // The turn rotates it, and the server places exactly the previewed unit.
        w.players[0].pitch = -0.3;
        let turned = w.aim_placement(0, DeployKind::Turret, 0.5).unwrap();
        assert!((turned.unit.yaw - (yaw + 0.5)).abs() < 1e-5);
        w.input.deploy = true;
        w.input.deploy_turn = 0.5;
        w.step_deploys();
        assert_eq!(w.deployables.last(), Some(&turned.unit));
        // A unit where one already stands is refused but still shown (gray).
        w.players[0].pack = Some(DeployKind::Turret);
        let blocked = w.aim_placement(0, DeployKind::Turret, 0.0).expect("still shown");
        assert!(blocked.problem.is_some(), "can't go there");
    }

    #[test]
    fn teams_are_limited_and_bad_spots_refused() {
        let mut w = world(DeployKind::Turret);
        let yaw = (0..16).map(|k| k as f32 * 0.39).find(|&y| { w.players[0].yaw = y; w.place(0, DeployKind::Turret, 0.0).is_ok() }).unwrap();
        w.players[0].yaw = yaw;
        for k in 0..team_limit(DeployKind::Turret) {
            w.deployables.push(Deployable { kind: DeployKind::Turret, team: Team::Ember, pos: Vec3::new(10.0 + k as f32 * 5.0, 0.0, 10.0),
                yaw: 0.0, health: 150.0, cooldown: 0.0, aim: Vec3::Z });
        }
        assert_eq!(w.place(0, DeployKind::Turret, 0.0).err(), Some("YOUR TEAM HAS THE MAXIMUM OF THOSE DEPLOYED"));
        w.deployables.clear();
        w.players[0].pos = w.flags[0].home + Vec3::Y * 0.5;
        assert!(w.place(0, DeployKind::Turret, 0.0).is_err(), "not on the flag");
    }

    #[test]
    fn a_turret_shoots_enemies_takes_damage_and_blows_up() {
        let mut w = world(DeployKind::Turret);
        let yaw = (0..16).map(|k| k as f32 * 0.39).find(|&y| { w.players[0].yaw = y; w.place(0, DeployKind::Turret, 0.0).is_ok() }).unwrap();
        w.players[0].yaw = yaw;
        w.input.deploy = true;
        w.step_deploys();
        let e = w.deployables[0].clone();
        let mut enemy = w.players[0].clone();
        enemy.team = Team::Glacier;
        enemy.pack = None;
        let (fwd, _) = move_basis(yaw);
        enemy.pos = e.pos + fwd * 20.0 + Vec3::Y * 1.0;
        w.players.push(enemy);
        w.input.deploy = false;
        for _ in 0..30 { w.step_deployables(STEP); }
        assert!(w.discs.iter().any(|d| d.owner == MAX_PLAYERS), "fired at the enemy");
        w.damage_deployable(0, 1000.0);
        w.step_deployables(STEP);
        assert!(w.deployables.is_empty());
        assert!(w.explosions.iter().any(|x| x.kind == 4));
    }

    #[test]
    fn an_ammo_station_restocks_teammates_who_use_it_and_blocks_like_a_crate() {
        let mut w = world(DeployKind::Ammo);
        let yaw = (0..16).map(|k| k as f32 * 0.39).find(|&y| { w.players[0].yaw = y; w.place(0, DeployKind::Ammo, 0.0).is_ok() }).unwrap();
        w.players[0].yaw = yaw;
        w.input.deploy = true;
        w.step_deploys();
        let e = w.deployables[0].clone();
        assert_eq!(e.kind, DeployKind::Ammo);
        w.input.deploy = false;
        // Standing by it and using it refills; not using it, or an enemy, doesn't.
        let (fwd, _) = move_basis(yaw);
        w.players[0].pos = e.center() - fwd * 1.6;
        w.players[0].ammo = [0, 0, 0, 0];
        w.step_ammo_stations();
        assert_eq!(w.players[0].ammo, [0, 0, 0, 0], "hold E to use it");
        w.input.interact = true;
        w.step_ammo_stations();
        assert_eq!(w.players[0].ammo, loadout::max_ammo(w.players[0].armor));
        w.players[0].team = Team::Glacier;
        w.players[0].ammo = [0, 0, 0, 0];
        w.step_ammo_stations();
        assert_eq!(w.players[0].ammo, [0, 0, 0, 0], "not the other team's");
        // It's a solid crate: shots stop at it.
        assert!(w.blocked(e.center() - fwd * 3.0, e.center() + fwd * 3.0, 0.1, Some(Team::Ember)).is_some());
    }
}
