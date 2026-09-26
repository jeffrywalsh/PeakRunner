//! The rifle slot (weapon 4): bought at an inventory station, never carried
//! by default, lost on death. Which rifle you hold follows your armor:
//!
//! - Laser rifle (light armor): spends energy, fires a straight beam from
//!   your eye along the crosshair that hits instantly at any range on the
//!   map.
//! - Railgun (heavy armor): fires a slug that flies straight and very fast
//!   but not instantly, keeps a little of your own motion, and hits hard.
//!   It uses its own ammunition.
//!
//! Both can be fired from the hip; the client zooms while the zoom key is
//! held (a view change only). A laser shot travels to clients as a short-lived
//! `Disc` of kind LASER_KIND whose `vel` holds the beam (start to end), so it
//! renders for everyone; it never moves or collides. Marker: `rifle1`.
use super::*;

pub const RIFLE_SLOT: u8 = 3;
pub const LASER_KIND: u8 = 8;
pub const RAIL_KIND: u8 = 9;

/// Laser: energy per shot (and the least you need to fire), damage, reload,
/// and how long the beam stays visible.
pub const LASER_ENERGY: f32 = 24.0;
pub const LASER_DAMAGE: f32 = 50.0;
pub const LASER_RELOAD: f32 = 1.1;
pub const LASER_RANGE: f32 = 2600.0;
pub const LASER_FADE: f32 = 0.18;
/// Railgun: slug speed, the share of your velocity it keeps, damage, reload,
/// and flight time before it expires.
pub const RAIL_SPEED: f32 = 640.0;
pub const RAIL_INHERIT: f32 = 0.15;
pub const RAIL_DAMAGE: f32 = 78.0;
pub const RAIL_RELOAD: f32 = 1.6;
pub const RAIL_LIFE: f32 = 3.0;
/// Field of view while zoomed (degrees).
pub const ZOOM_FOV: f32 = 22.0;

/// Whether player `p`'s rifle is the railgun (heavy) or the laser (light).
pub fn is_railgun(p: &Player) -> bool { p.armor == ArmorClass::Heavy }

pub fn rifle_reload(p: &Player) -> f32 { if is_railgun(p) { RAIL_RELOAD } else { LASER_RELOAD } }

pub fn rifle_name(p: &Player) -> &'static str { if is_railgun(p) { "Railgun" } else { "Laser rifle" } }

impl World {
    /// The weapon slot player `i` may hold when they ask for `wanted`: the
    /// rifle slot only once they've bought a rifle.
    pub fn allowed_weapon(&self, i: usize, wanted: u8) -> u8 {
        let p = &self.players[i];
        if wanted == RIFLE_SLOT && !p.rifle { p.weapon.min(2) } else { wanted.min(RIFLE_SLOT) }
    }

    /// Spend what a rifle shot costs; false when it can't fire.
    pub(super) fn take_rifle_round(&mut self, i: usize) -> bool {
        let p = &mut self.players[i];
        if !p.rifle { return false; }
        if is_railgun(p) {
            if p.ammo[3] == 0 { return false; }
            p.ammo[3] -= 1;
        } else {
            if p.energy < LASER_ENERGY { return false; }
            p.energy -= LASER_ENERGY;
        }
        true
    }

    /// Fire player `i`'s rifle (the shot is already paid for).
    pub(super) fn fire_rifle(&mut self, i: usize) {
        self.players[i].shots += 1;
        let p = &self.players[i];
        let dir = look_dir(p.yaw, p.pitch);
        let eye = p.pos + Vec3::Y * EYE;
        let spd = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
        let muzzle = muzzle_origin(eye, dir, RIFLE_SLOT, camera_fov(spd));
        let muzzle = match obstacle_hit(self.map, &self.pillars, eye, muzzle, SHOT_RADIUS) {
            Some(t) => eye.lerp(muzzle, (t-0.05/eye.distance(muzzle).max(0.05)).max(0.0)),
            None => muzzle,
        };
        let (team, rail, vel) = (p.team, is_railgun(p), p.vel);
        self.players[i].cooldown = rifle_reload(&self.players[i]);
        if rail {
            // A slug along the crosshair from the muzzle, keeping a little of
            // the shooter's motion.
            let aim = eye+dir*200.0;
            let shot = (aim-muzzle).normalize_or(dir);
            self.discs.push(Disc { pos: muzzle, vel: shot*RAIL_SPEED+vel*RAIL_INHERIT, team, owner: i,
                life: RAIL_LIFE, kind: RAIL_KIND, spin: 0.0 });
            if i == self.player_id { self.trauma = (self.trauma+0.1).min(1.0); self.push_event("rail"); }
            else if self.network_inputs.is_empty() { self.spatial_sounds.push(("rail", muzzle)); }
            return;
        }
        if i == self.player_id { self.push_event("laser"); }
        else if self.network_inputs.is_empty() { self.spatial_sounds.push(("laser", muzzle)); }
        if self.predicting { return; }
        // The beam: the first thing along the crosshair from the eye.
        let end = eye+dir*LASER_RANGE;
        let mut t = obstacle_hit(self.map, &self.pillars, eye, end, 0.0).unwrap_or(1.0);
        let mut victim = None;
        let mut deploy = None;
        let mut equipment = None;
        if let Some((u, k)) = self.deployable_hit(eye, end, 0.0, Some(team)) {
            if u < t { t = u; deploy = Some(k); }
        }
        for (idx, (obj, state)) in crate::equipment::definitions(self.map).iter().zip(&self.equipment).enumerate() {
            if obj.team as usize == team.idx() || state.health <= 0.0 { continue; }
            if let Some(u) = segment_sphere(eye, end, obj.pos(), obj.radius) {
                if u < t { t = u; equipment = Some(idx); deploy = None; }
            }
        }
        for (idx, pl) in self.players.iter().enumerate() {
            if !pl.alive || !self.hostile(i, team, idx) { continue; }
            if let Some(u) = segment_sphere(eye, end, pl.pos+Vec3::Y*0.9, PLAYER_RADIUS+0.45) {
                if u < t { t = u; victim = Some(idx); equipment = None; deploy = None; }
            }
        }
        let hit = eye.lerp(end, t);
        self.discs.push(Disc { pos: muzzle, vel: hit-muzzle, team, owner: i, life: LASER_FADE, kind: LASER_KIND, spin: 0.0 });
        if let Some(k) = deploy { self.damage_deployable(k, LASER_DAMAGE); }
        if let Some(idx) = equipment {
            let defs = crate::equipment::definitions(self.map);
            if self.equipment[idx].damage(&defs[idx], LASER_DAMAGE, true) { self.equipment_destroyed(&defs[idx]); }
        }
        if let Some(idx) = victim { self.rifle_hit(idx, i, LASER_DAMAGE, "Laser rifle"); }
    }

    /// A rifle hit on player `idx` by `owner`.
    pub(super) fn rifle_hit(&mut self, idx: usize, owner: usize, damage: f32, weapon: &str) {
        if !self.players[idx].alive { return; }
        self.hurt(idx, damage);
        if let Some(p) = self.players.get_mut(owner) { p.hits += 1; }
        if owner == self.player_id { self.hitmarker = 1.0; self.push_event("hit"); }
        if idx == self.player_id { self.damage_flash = 0.5; self.push_event("pain"); }
        if self.players[idx].health <= 0.0 {
            self.kill(idx, Some(owner), weapon);
            if owner == self.player_id { self.kills += 1; self.msg("FRAG", 1.1); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_catalog::SupportedMode;

    /// A player on open ground with an enemy standing `ahead` metres in front.
    fn range(armor: ArmorClass, ahead: f32) -> World {
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        w.set_mode(SupportedMode::Ctf);
        w.start_match(true);
        w.players.truncate(1);
        let spot = Vec3::new(1060.0, 0.0, 700.0);
        let floor = crate::terrain::support_on(MapId::Raindance, spot+Vec3::Y*200.0).0;
        w.players[0].pos = Vec3::new(spot.x, floor+PLAYER_RADIUS, spot.z);
        w.players[0].armor = armor;
        w.refill(0);
        w.players[0].yaw = 0.0; w.players[0].pitch = 0.0;
        let mut foe = w.players[0].clone();
        foe.team = Team::Glacier;
        foe.pos = w.players[0].pos+Vec3::new(0.0, 0.0, -ahead);
        foe.pos.y = crate::terrain::support_on(MapId::Raindance, foe.pos+Vec3::Y*200.0).0+PLAYER_RADIUS;
        w.players.push(foe);
        // Aim at the foe's chest.
        let d = (w.players[1].pos+Vec3::Y*0.9)-(w.players[0].pos+Vec3::Y*EYE);
        w.players[0].pitch = (d.y/d.length()).asin();
        w
    }

    fn fire(w: &mut World) {
        w.input.weapon = RIFLE_SLOT;
        w.input.fire = true;
        w.tick(STEP*2.0);
        w.input.fire = false;
    }

    #[test]
    fn rifles_are_bought_not_carried_and_follow_the_armor() {
        let mut w = range(ArmorClass::Light, 30.0);
        assert!(!w.players[0].rifle, "not carried by default");
        assert_eq!(w.allowed_weapon(0, RIFLE_SLOT), 0);
        w.players[0].rifle = true;
        assert_eq!(w.allowed_weapon(0, RIFLE_SLOT), RIFLE_SLOT);
        assert!(!is_railgun(&w.players[0]));
        w.players[0].armor = ArmorClass::Heavy;
        assert!(is_railgun(&w.players[0]));
        // Lost on death.
        w.kill(0, None, "Fall");
        assert!(!w.players[0].rifle);
    }

    #[test]
    fn the_laser_hits_instantly_at_long_range_and_costs_energy() {
        for ahead in [20.0, 180.0] {
            let mut w = range(ArmorClass::Light, ahead);
            w.players[0].rifle = true;
            w.players[0].energy = ENERGY_MAX;
            fire(&mut w);
            assert!(w.players[1].health <= 100.0-LASER_DAMAGE+0.01, "{ahead} m: hit at once ({})", w.players[1].health);
            assert!(w.players[0].energy <= ENERGY_MAX-LASER_ENERGY+1.0, "spent energy");
            assert!(w.discs.iter().any(|d| d.kind == LASER_KIND), "a visible beam");
        }
        // Without the energy it doesn't fire.
        let mut w = range(ArmorClass::Light, 20.0);
        w.players[0].rifle = true;
        w.players[0].energy = LASER_ENERGY-1.0;
        fire(&mut w);
        assert_eq!(w.players[1].health, 100.0);
    }

    #[test]
    fn the_railgun_slug_is_fast_but_not_instant_and_hits_hard() {
        let mut w = range(ArmorClass::Heavy, 200.0);
        w.players[0].rifle = true;
        w.players[0].vel = Vec3::new(10.0, 0.0, 0.0);
        let rounds = w.players[0].ammo[3];
        assert!(rounds > 0);
        fire(&mut w);
        assert_eq!(w.players[0].ammo[3], rounds-1);
        let slug = w.discs.iter().find(|d| d.kind == RAIL_KIND).expect("a slug in flight").clone();
        assert!(w.players[1].health == 100.0, "not instant at 200 m");
        let sideways = slug.vel.x;
        assert!(sideways > 0.5 && sideways < 3.0, "a little of the shooter's motion: {sideways}");
        for _ in 0..30 { w.physics_step(); }
        assert!(w.players[1].health <= 100.0-RAIL_DAMAGE*0.5+0.01 || !w.players[1].alive, "hit hard: {}", w.players[1].health);
    }
}
