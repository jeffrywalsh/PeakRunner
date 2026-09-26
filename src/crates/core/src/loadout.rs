//! Armor classes, ammunition, the repair tool, the mortar and inventory
//! purchases. Light armor keeps the approved movement; heavy armor follows
//! base Tribes (twice the armor, a slow walk, a weak long jet). Every weapon
//! has limited ammunition, refilled at an inventory station. Marker: `loadout1`.
use super::*;

/// Which armor a player wears. Chosen at an inventory station; kept across
/// deaths. Football ignores it (everyone wears the football armor).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmorClass { #[default] Light, Heavy }

/// Heavy armor, from base Tribes' harmor against larmor: mass 18 vs 9, twice
/// the armor (maxDamage 1.32 vs 0.66), a jet that barely out-pulls gravity
/// (jetForce 385 on mass 18), 110 energy draining 1.1 per tick vs 0.8, the
/// same jump speed, and a much slower run (5 vs 11). Heavy is hard to fly: the
/// jet climbs at about 3.5 m/s² net, pushes sideways weakly and adds speed only
/// up to 36 km/h. Skiing is its real speed (momentum is kept, never capped).
pub const HEAVY_ARMOR: Armor = Armor { jet: 23.5, side: 8.0, drain: 11.3, regen: 6.5, min_jet: MIN_JET_ENERGY,
    jump: 1.0, fade_up: true, air_cap: 10.0, ski_drag: 0.0 };
/// Walking speed, damage taken and mass relative to light armor.
pub fn walk_scale(class: ArmorClass) -> f32 { if class == ArmorClass::Heavy { 0.55 } else { 1.0 } }
pub fn damage_taken(class: ArmorClass) -> f32 { if class == ArmorClass::Heavy { 0.5 } else { 1.0 } }
pub fn mass_scale(class: ArmorClass) -> f32 { if class == ArmorClass::Heavy { 2.0 } else { 1.0 } }

/// Ammunition per weapon slot (disc, chaingun, grenades or mortar shells).
/// Light armor uses base Tribes' limits; heavy carries more of each.
pub fn max_ammo(class: ArmorClass) -> [u16; 4] {
    // The fourth slot is the railgun's (heavy only; the light armor's laser
    // runs on energy).
    match class { ArmorClass::Light => [15, 100, 10, 0], ArmorClass::Heavy => [25, 200, 10, 20] }
}

/// Heavy armor's third weapon is the mortar instead of the grenade launcher.
pub fn has_mortar(class: ArmorClass) -> bool { class == ArmorClass::Heavy }

/// Mortar: a long, fast lob that explodes on impact. Like the grenade
/// launcher's shell it has a brief launch safety: for MORTAR_ARM seconds after
/// firing it bounces off whatever it meets (a short shot inside a building
/// rattles round the room), then the next contact sets it off.
pub const MORTAR_KIND: u8 = 5;
pub const MORTAR_SPEED: f32 = 65.0;
pub const MORTAR_RELOAD: f32 = 2.0;
/// Flight time before a shell that never hits anything goes off.
pub const MORTAR_LIFE: f32 = 20.0;
pub const MORTAR_ARM: f32 = 0.35;
/// Whether a shell is past its launch safety.
pub fn mortar_armed(d: &Disc) -> bool { d.kind == MORTAR_KIND && d.life <= MORTAR_LIFE - MORTAR_ARM }

/// Repair tool (hold Q): a beam to whatever of your team's is under your
/// crosshair within REPAIR_RANGE, else yourself. It spends REPAIR_ENERGY per
/// second (the base Tribes repair gun's 10) and needs REPAIR_MIN_ENERGY to run,
/// so repairs take a while and cost your jets.
pub const REPAIR_RANGE: f32 = 12.0;
pub const REPAIR_ENERGY: f32 = 12.0;
pub const REPAIR_MIN_ENERGY: f32 = 3.0;
/// Health (of 100) a player regains per second, and hull per second for equipment.
pub const REPAIR_PLAYER: f32 = 6.0;
pub const REPAIR_HULL: f32 = 20.0;

/// An inventory purchase, sent as a command intent.
pub const BUY_NONE: u8 = 0;
pub const BUY_LIGHT: u8 = 1;
pub const BUY_HEAVY: u8 = 2;
pub const BUY_TURRET: u8 = 3;
pub const BUY_WALL: u8 = 4;
pub const BUY_FIELD: u8 = 5;
pub const BUY_AMMO: u8 = 6;
/// The rifle for your armor (sim::rifles): the laser in light, the railgun in heavy.
pub const BUY_RIFLE: u8 = 7;

/// An ammo pack dropped where a player died, drawn from what they carried
/// (`World::drop_loot`), for anyone who runs over it, gone after
/// LOOT_SECONDS. A picker takes only what matches their own kit: the third
/// weapon's rounds only when their armor carries the same one (grenade
/// launcher or mortar). Marker: `loot2`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loot {
    pub pos: Vec3,
    pub ammo: [u16; 4],
    pub age: f32,
    /// The dead player's armor: says whether `ammo[2]` is grenade-launcher
    /// rounds or mortar shells.
    #[serde(default)]
    pub armor: ArmorClass,
    /// Hand grenades (never mines).
    #[serde(default)]
    pub grenades: u8,
}
/// Loot draws 1..=LOOT_ROLL discs and third-weapon rounds, and up to this
/// many hand grenades (at least one), from what the dead carried.
pub const LOOT_ROLL: u16 = 5;
pub const LOOT_SECONDS: f32 = 20.0;
pub const LOOT_REACH: f32 = 2.0;

/// What a repair beam is working on.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Repairing { Player(usize), Equipment(usize), Deployable(usize) }

impl World {
    /// The armor player `i` wears now (football overrides the class).
    pub fn armor_of(&self, i: usize) -> Armor {
        if self.football() { return football::FOOTBALL_ARMOR; }
        match self.players.get(i).map(|p| p.armor) {
            Some(ArmorClass::Heavy) => HEAVY_ARMOR,
            _ => STANDARD_ARMOR,
        }
    }

    /// Apply `amount` of damage to player `i`, scaled by their armor.
    pub(super) fn hurt(&mut self, i: usize, amount: f32) {
        let scale = if self.football() { 1.0 } else { damage_taken(self.players[i].armor) * self.twist_damage() };
        self.players[i].health -= amount * scale;
    }

    /// Full ammunition for the player's armor.
    pub(super) fn refill(&mut self, i: usize) {
        self.players[i].ammo = max_ammo(self.players[i].armor);
        World::restock_throwables(&mut self.players[i]);
    }

    /// Spend one round for weapon slot `w`; false when empty.
    pub(super) fn take_round(&mut self, i: usize, w: u8) -> bool {
        let slot = &mut self.players[i].ammo[w.min(2) as usize];
        if *slot == 0 { return false; }
        *slot -= 1;
        true
    }

    /// A dying player drops a pack of what they carried: 1-5 discs and 1-5
    /// third-weapon rounds (never more than they had), all their chaingun
    /// rounds, and 1-5 hand grenades (at least one, even if they had none;
    /// never more than they had otherwise). No mines, no railgun slugs.
    pub(super) fn drop_loot(&mut self, i: usize) {
        if self.predicting || self.football() { return; }
        let roll = |w: &mut Self, have: u16| -> u16 {
            let n = 1 + ((w.rng() * LOOT_ROLL as f32) as u16).min(LOOT_ROLL - 1);
            n.min(have)
        };
        let have = self.players[i].ammo;
        let carried = self.players[i].throwables[0] as u16;
        let discs = roll(self, have[0]);
        let third = roll(self, have[2]);
        let grenades = roll(self, carried.max(1)).max(1) as u8;
        let p = &self.players[i];
        let floor = crate::terrain::support_on(self.map, p.pos + Vec3::Y * 0.5).0;
        self.loot.push(Loot { pos: Vec3::new(p.pos.x, floor, p.pos.z), ammo: [discs, have[1], third, 0], age: 0.0,
            armor: p.armor, grenades });
        if self.loot.len() > 24 { self.loot.remove(0); }
    }

    /// Loot ages out, and anyone alive running over it takes what they can carry.
    pub(super) fn step_loot(&mut self, dt: f32) {
        if self.predicting { return; }
        for l in &mut self.loot { l.age += dt; }
        self.loot.retain(|l| l.age < LOOT_SECONDS);
        for k in 0..self.loot.len() {
            for i in 0..self.players.len() {
                let p = &self.players[i];
                if !p.alive || p.pos.distance(self.loot[k].pos + Vec3::Y * 0.5) > LOOT_REACH { continue; }
                let max = max_ammo(p.armor);
                // The third weapon's rounds only fit the same weapon.
                let third = has_mortar(p.armor) == has_mortar(self.loot[k].armor);
                let mut took = false;
                let room = (throwables::max_throwables(p.armor)[0]).saturating_sub(p.throwables[0]);
                let take = room.min(self.loot[k].grenades);
                if take > 0 {
                    self.players[i].throwables[0] += take;
                    self.loot[k].grenades -= take;
                    took = true;
                }
                for w in 0..3 {
                    if w == 2 && !third { continue; }
                    let room = max[w].saturating_sub(self.players[i].ammo[w]);
                    let take = room.min(self.loot[k].ammo[w]);
                    if take > 0 {
                        self.players[i].ammo[w] += take;
                        self.loot[k].ammo[w] -= take;
                        took = true;
                    }
                }
                if took && i == self.player_id { self.push_event("buy"); }
            }
        }
        // Chaingun rounds or third-weapon rounds nobody here can use still
        // lie there until someone can, or the pack times out.
        self.loot.retain(|l| l.ammo.iter().any(|&n| n > 0) || l.grenades > 0);
    }

    /// Repair tool for every player holding it, once per physics step.
    pub(super) fn step_repair(&mut self, dt: f32) {
        if self.predicting { return; }
        for i in 0..self.players.len() {
            self.players[i].repair_beam = None;
            let wants = if self.players[i].is_bot { self.players[i].bot_repair }
                else if self.network_inputs.is_empty() { i == self.player_id && self.input.repair }
                else { self.network_inputs.get(i).is_some_and(|c| c.repair) };
            let p = &self.players[i];
            if !wants || !p.alive || p.stun > 0.0 || p.energy < REPAIR_MIN_ENERGY || self.football() { continue; }
            let target = self.repair_target(i);
            let spend = (REPAIR_ENERGY * dt).min(self.players[i].energy);
            let work = spend / REPAIR_ENERGY;
            let at = match target {
                Some(Repairing::Player(j)) => {
                    let q = &mut self.players[j];
                    if q.health >= 100.0 { continue; }
                    q.health = (q.health + REPAIR_PLAYER * work).min(100.0);
                    q.pos + Vec3::Y * 0.9
                }
                Some(Repairing::Equipment(k)) => {
                    let d = &crate::equipment::definitions(self.map)[k];
                    if self.equipment[k].health >= d.max_health() { continue; }
                    self.equipment[k].repair(d, REPAIR_HULL * work);
                    d.pos()
                }
                Some(Repairing::Deployable(k)) => {
                    let e = &mut self.deployables[k];
                    let max = deploy::max_health(e.kind);
                    if e.health >= max { continue; }
                    e.health = (e.health + REPAIR_HULL * work).min(max);
                    e.center()
                }
                None => {
                    let q = &mut self.players[i];
                    if q.health >= 100.0 { continue; }
                    q.health = (q.health + REPAIR_PLAYER * work).min(100.0);
                    q.pos + Vec3::Y * 0.9
                }
            };
            self.players[i].energy -= spend;
            self.players[i].repair_beam = Some(at);
        }
    }

    /// The nearest friendly thing within REPAIR_RANGE under player `i`'s
    /// crosshair and in sight: a teammate, equipment or a deployable.
    fn repair_target(&self, i: usize) -> Option<Repairing> {
        let p = &self.players[i];
        let eye = p.pos + Vec3::Y * EYE;
        let dir = look_dir(p.yaw, p.pitch);
        let team = p.team;
        let mut best: Option<(f32, Repairing)> = None;
        let mut consider = |at: Vec3, reach: f32, what: Repairing| {
            let to = at - eye;
            let along = to.dot(dir);
            if along <= 0.0 || along > REPAIR_RANGE || (to - dir * along).length() > reach { return; }
            if best.is_some_and(|(d, _)| d <= along) { return; }
            best = Some((along, what));
        };
        for (j, o) in self.players.iter().enumerate() {
            if j != i && o.alive && o.team == team { consider(o.pos + Vec3::Y * 0.8, 1.0, Repairing::Player(j)); }
        }
        for (k, d) in crate::equipment::definitions(self.map).iter().enumerate() {
            if d.team as usize == team.idx() { consider(d.pos(), d.radius + 0.5, Repairing::Equipment(k)); }
        }
        for (k, e) in self.deployables.iter().enumerate() {
            if e.team == team { consider(e.center(), 1.6, Repairing::Deployable(k)); }
        }
        let (_, what) = best?;
        let at = match what {
            Repairing::Player(j) => self.players[j].pos + Vec3::Y * 0.8,
            Repairing::Equipment(k) => crate::equipment::definitions(self.map)[k].pos(),
            Repairing::Deployable(k) => self.deployables[k].center(),
        };
        let reach = eye.distance(at);
        let clear = obstacle_hit(self.map, &self.pillars, eye, at, 0.0).is_none_or(|t| t * reach > reach - 1.8);
        clear.then_some(what)
    }

    /// Inventory purchases: a player at their team's powered inventory station
    /// may change armor (refilling ammunition) or take a deployable pack.
    pub(super) fn step_purchases(&mut self) {
        if self.predicting || self.football() { return; }
        for i in 0..self.players.len() {
            let buy = if self.players[i].is_bot { BUY_NONE }
                else if self.network_inputs.is_empty() { if i == self.player_id { self.input.buy } else { BUY_NONE } }
                else { self.network_inputs.get(i).map_or(BUY_NONE, |c| c.buy) };
            if buy == BUY_NONE || !self.players[i].alive || !self.at_inventory(i) { continue; }
            match buy {
                BUY_LIGHT | BUY_HEAVY => {
                    let class = if buy == BUY_HEAVY { ArmorClass::Heavy } else { ArmorClass::Light };
                    if self.players[i].armor != class {
                        self.players[i].armor = class;
                        self.players[i].energy = self.players[i].energy.min(ENERGY_MAX);
                        self.ball_lost(i);
                    }
                    self.refill(i);
                    if i == self.player_id { self.push_event("buy"); }
                }
                BUY_RIFLE => {
                    self.players[i].rifle = true;
                    self.refill(i);
                    if i == self.player_id { self.push_event("buy"); }
                }
                BUY_TURRET | BUY_WALL | BUY_FIELD | BUY_AMMO => {
                    self.players[i].pack = Some(match buy {
                        BUY_TURRET => deploy::DeployKind::Turret,
                        BUY_WALL => deploy::DeployKind::Wall,
                        BUY_FIELD => deploy::DeployKind::Field,
                        _ => deploy::DeployKind::Ammo,
                    });
                    if i == self.player_id { self.push_event("buy"); }
                }
                _ => {}
            }
        }
    }

    /// Whether player `i` stands at one of their team's powered inventory stations.
    pub fn at_inventory(&self, i: usize) -> bool {
        let p = &self.players[i];
        crate::equipment::definitions(self.map).iter().zip(&self.equipment).any(|(d, s)| {
            // In Deathmatch there are no sides: any station serves anyone.
            d.kind == crate::equipment::Kind::Inventory && (self.ffa() || d.team as usize == p.team.idx()) && s.powered && s.health > 0.0
                && p.pos.distance(d.pos()) <= d.radius + 1.5
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_catalog::SupportedMode;

    fn solo(armor: ArmorClass) -> World {
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        w.set_mode(SupportedMode::Ctf);
        w.start_match(true);
        w.players.truncate(1);
        w.players[0].armor = armor;
        w.refill(0);
        let spot = Vec3::new(1060.0, 0.0, 700.0);
        let floor = crate::terrain::support_on(MapId::Raindance, spot + Vec3::Y * 200.0).0;
        w.players[0].pos = Vec3::new(spot.x, floor + PLAYER_RADIUS, spot.z);
        w
    }

    #[test]
    fn heavy_armor_takes_half_damage_walks_slower_and_climbs_slower() {
        let mut light = solo(ArmorClass::Light);
        let mut heavy = solo(ArmorClass::Heavy);
        light.hurt(0, 40.0);
        heavy.hurt(0, 40.0);
        assert_eq!((light.players[0].health, heavy.players[0].health), (60.0, 80.0));
        for w in [&mut light, &mut heavy] {
            w.players[0].health = 100.0;
            w.input.move_z = 1.0;
            for _ in 0..90 { w.physics_step(); }
        }
        let flat = |w: &World| Vec2::new(w.players[0].vel.x, w.players[0].vel.z).length();
        assert!(flat(&heavy) < flat(&light) * 0.7, "heavy walks {} vs {}", flat(&heavy), flat(&light));
        for w in [&mut light, &mut heavy] {
            w.input.move_z = 0.0;
            w.players[0].vel = Vec3::ZERO;
            w.players[0].energy = ENERGY_MAX;
            w.input.jet = true;
            for _ in 0..60 { w.physics_step(); }
        }
        assert!(heavy.players[0].vel.y < light.players[0].vel.y * 0.4, "heavy jet barely climbs: {} vs {}",
            heavy.players[0].vel.y, light.players[0].vel.y);
        assert!(heavy.players[0].energy > light.players[0].energy, "but burns its tank slower");
        // Jetting sideways from a stand, heavy tops out far below light.
        for w in [&mut light, &mut heavy] {
            w.players[0].vel = Vec3::ZERO;
            w.players[0].energy = ENERGY_MAX;
            w.players[0].pos.y += 30.0;
            w.input.move_z = 1.0;
            for _ in 0..150 { w.physics_step(); }
        }
        let air = |w: &World| Vec2::new(w.players[0].vel.x, w.players[0].vel.z).length();
        assert!(air(&heavy) <= HEAVY_ARMOR.air_cap + 0.5 && air(&light) > 15.0, "air speed {} vs {}", air(&heavy), air(&light));
    }

    #[test]
    fn ammo_runs_out_and_clicks_until_an_inventory_restocks_it() {
        let mut w = solo(ArmorClass::Light);
        assert_eq!(w.players[0].ammo, max_ammo(ArmorClass::Light));
        w.players[0].ammo[0] = 2;
        w.input.fire = true;
        w.input.weapon = 0;
        for _ in 0..(4.0 / STEP) as usize { w.tick(STEP); }
        assert_eq!(w.players[0].ammo[0], 0);
        assert_eq!(w.players[0].shots, 2, "two discs, then nothing");
        let heavy = max_ammo(ArmorClass::Heavy);
        let light = max_ammo(ArmorClass::Light);
        assert!(heavy.iter().zip(light).all(|(h, l)| *h >= l) && heavy[0] > light[0] && heavy[1] > light[1],
            "heavy carries more");
    }

    #[test]
    fn a_mortar_lobs_far_and_blows_up_on_impact() {
        let mut w = solo(ArmorClass::Heavy);
        w.input.weapon = 2;
        w.players[0].pitch = 0.35;
        let from = w.players[0].pos;
        w.input.fire = true;
        w.tick(STEP * 2.0);
        w.input.fire = false;
        assert_eq!(w.players[0].ammo[2], max_ammo(ArmorClass::Heavy)[2] - 1);
        assert!(w.discs.iter().any(|d| d.kind == MORTAR_KIND), "a shell is in the air");
        for _ in 0..(12.0 / STEP) as usize {
            w.physics_step();
            if w.explosions.iter().any(|e| e.kind == MORTAR_KIND) { break; }
        }
        let blast = w.explosions.iter().find(|e| e.kind == MORTAR_KIND).expect("it exploded");
        let range = Vec2::new(blast.pos.x - from.x, blast.pos.z - from.z).length();
        assert!(range > 120.0, "a long lob: {range} m");
        assert!(!w.discs.iter().any(|d| d.kind == MORTAR_KIND), "on impact, nothing left lying");
        assert!(crate::combat::MORTAR.max_damage > crate::combat::GRENADE.max_damage);
        assert!(crate::combat::MORTAR.radius > crate::combat::GRENADE.radius);
    }

    #[test]
    fn a_fresh_mortar_bounces_an_armed_one_explodes() {
        for armed in [false, true] {
            let mut w = solo(ArmorClass::Heavy);
            let at = w.players[0].pos + Vec3::new(15.0, 0.0, 0.0);
            let floor = crate::terrain::support_on(w.map, at + Vec3::Y * 5.0).0;
            let life = if armed { MORTAR_LIFE - 1.0 } else { MORTAR_LIFE };
            w.discs.push(Disc { pos: Vec3::new(at.x, floor + 0.6, at.z), vel: Vec3::new(0.0, -30.0, 0.0), team: Team::Ember,
                owner: 0, life, kind: MORTAR_KIND, spin: 0.0 });
            w.physics_step();
            if armed {
                assert!(w.explosions.iter().any(|e| e.kind == MORTAR_KIND), "armed: explodes on contact");
            } else {
                let d = w.discs.iter().find(|d| d.kind == MORTAR_KIND).expect("launch safety: it bounced");
                assert!(d.vel.y > 0.0, "off the floor");
                assert!(w.explosions.is_empty());
            }
        }
    }

    #[test]
    fn purchases_need_your_own_powered_inventory_station() {
        let mut w = solo(ArmorClass::Light);
        w.input.buy = BUY_HEAVY;
        w.step_purchases();
        assert_eq!(w.players[0].armor, ArmorClass::Light, "not away from a station");
        let defs = crate::equipment::definitions(MapId::Raindance);
        let s = defs.iter().position(|d| d.kind == crate::equipment::Kind::Inventory && d.team == 0).unwrap();
        w.players[0].team = Team::Ember;
        w.players[0].pos = defs[s].pos();
        w.players[0].ammo = [0, 0, 0, 0];
        w.step_purchases();
        assert_eq!(w.players[0].armor, ArmorClass::Heavy);
        assert_eq!(w.players[0].ammo, max_ammo(ArmorClass::Heavy), "restocked for the new armor");
        w.input.buy = BUY_WALL;
        w.step_purchases();
        assert_eq!(w.players[0].pack, Some(deploy::DeployKind::Wall));
        // The other team's station sells you nothing.
        w.players[0].team = Team::Glacier;
        w.input.buy = BUY_LIGHT;
        w.step_purchases();
        assert_eq!(w.players[0].armor, ArmorClass::Heavy);
    }

    #[test]
    fn the_dead_drop_a_roll_of_what_they_carried_and_it_vanishes_after_20_s() {
        let mut w = solo(ArmorClass::Light);
        let mut other = w.players[0].clone();
        other.team = Team::Glacier;
        other.pos += Vec3::new(30.0, 0.0, 0.0);
        w.players.push(other);
        // Across many deaths: discs and grenade rounds roll 1-5 (never more
        // than carried), all the chaingun rounds, 1+ hand grenades, no mines.
        let mut seen = std::collections::HashSet::new();
        for _ in 0..60 {
            w.loot.clear();
            w.respawn(1);
            w.players[1].ammo = [10, 60, 4, 0];
            w.players[1].throwables = [3, 3];
            w.kill(1, None, "Fall");
            let l = &w.loot[0];
            assert!((1..=5).contains(&l.ammo[0]) && (1..=4).contains(&l.ammo[2]), "{:?}", l.ammo);
            assert_eq!(l.ammo[1], 60);
            assert!((1..=3).contains(&l.grenades));
            seen.insert(l.ammo[0]);
        }
        assert!(seen.len() >= 4, "discs vary: {seen:?}");
        // Somebody with no grenades still drops one.
        w.loot.clear(); w.respawn(1);
        w.players[1].throwables = [0, 3];
        w.players[1].ammo = [0, 0, 0, 0];
        w.kill(1, None, "Fall");
        assert_eq!((w.loot[0].ammo, w.loot[0].grenades), ([0, 0, 0, 0], 1));
        // A pack: an enemy runs over it and takes what fits.
        w.loot.clear(); w.respawn(1);
        w.players[1].ammo = [10, 60, 4, 0];
        let at = w.players[1].pos;
        w.kill(1, None, "Fall");
        w.loot[0].ammo = [5, 60, 3, 0];
        w.loot[0].grenades = 2;
        w.players[0].ammo = [13, 70, 8, 0];
        w.players[0].throwables = [4, 0];
        w.players[0].pos = at;
        w.step_loot(STEP);
        assert_eq!(w.players[0].ammo, [15, 100, 10, 0], "discs to max, chaingun filled, grenade rounds");
        assert_eq!(w.players[0].throwables, [5, 0], "a hand grenade, no mines");
        assert_eq!((w.loot[0].ammo, w.loot[0].grenades), ([3, 30, 1, 0], 1), "the rest stays");
        // A heavy's mortar can't take grenade-launcher rounds.
        w.players[0].armor = ArmorClass::Heavy;
        w.players[0].ammo = [0, 0, 0, 0];
        w.step_loot(STEP);
        assert_eq!(w.players[0].ammo[2], 0, "wrong third weapon");
        assert_eq!(w.loot[0].ammo[2], 1);
        // Untouched, it's gone after 20 s.
        w.players[0].pos += Vec3::new(50.0, 0.0, 0.0);
        for _ in 0..(LOOT_SECONDS / STEP) as usize + 2 { w.step_loot(STEP); }
        assert!(w.loot.is_empty());
    }

    #[test]
    fn an_empty_chaingun_clicks_without_touching_its_cooldown() {
        let mut w = solo(ArmorClass::Light);
        w.players[0].ammo[1] = 0;
        w.input.weapon = 1;
        w.input.fire = true;
        for _ in 0..30 {
            w.tick(STEP);
            assert_eq!(w.players[0].cooldown, 0.0, "no fake shots for the viewmodel");
        }
    }
}
