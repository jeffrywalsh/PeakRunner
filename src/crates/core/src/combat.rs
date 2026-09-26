//! Original implementation of researched T2-style explosive balance.
//! Values and deliberate PeakRunner differences: docs/weapon-damage.md.

#[derive(Clone, Copy, Debug)]
pub struct BlastProfile {
    pub radius: f32,
    pub max_damage: f32,
    pub edge_fraction: f32,
    /// Blast impulse, converted to a velocity change using player mass.
    pub impulse: f32,
    /// 0: T2's linear falloff to `edge_fraction` at the radius. Otherwise a
    /// concentrated blast: `max_damage / (1 + (d / core)²)`, half damage at
    /// `core` metres, fading fast (the kick still reaches the full radius).
    pub core: f32,
}

impl BlastProfile {
    /// Distance from the blast to the target's collision surface, in metres.
    /// T2 keeps 12% damage at the edge; beyond the radius there is no damage.
    pub fn damage(self, distance: f32) -> f32 {
        if !distance.is_finite() || distance > self.radius {
            return 0.0;
        }
        if self.core > 0.0 {
            let k = distance.max(0.0) / self.core;
            return self.max_damage / (1.0 + k * k);
        }
        self.max_damage * (1.0 - (1.0 - self.edge_fraction) * distance.max(0.0) / self.radius)
    }
}

// T2 light armour: maxDamage=.66, disc scale=1, grenade scale=1.2.
// PeakRunner currently has one 100-health player class, not three armour tiers.
pub const DISC: BlastProfile = BlastProfile {
    radius: 7.5,
    max_damage: 100.0 * 0.50 / 0.66,
    edge_fraction: 0.12,
    impulse: 2000.0,
    core: 0.0,
};

/// Heavy armor's mortar: a concentrated blast with an intense shock. A
/// near-direct hit (within about 0.25 m of the body) kills light armor;
/// 0.5 m away it takes two (74 each); a metre off, 33; two metres, 12. The
/// kick is huge and reaches out 20 m.
pub const MORTAR: BlastProfile = BlastProfile {
    radius: 20.0,
    max_damage: 125.0,
    edge_fraction: 0.0,
    impulse: 5000.0,
    core: 0.6,
};

/// Share of the mortar's damage and shock the grenade launcher's shell has.
pub const GRENADE_SHARE: f32 = 0.4;
/// Grenade launcher: the mortar's concentrated shape at GRENADE_SHARE
/// strength (50 at the body, 30 half a metre off), over 15 m.
pub const GRENADE: BlastProfile = BlastProfile {
    radius: 15.0,
    max_damage: MORTAR.max_damage * GRENADE_SHARE,
    edge_fraction: 0.0,
    impulse: MORTAR.impulse * GRENADE_SHARE,
    core: MORTAR.core,
};

/// Hand grenade: base Tribes Handgrenade, damageValue 0.5 over 10 m.
pub const HAND_GRENADE: BlastProfile = BlastProfile {
    radius: 10.0,
    max_damage: 100.0 * 0.5 / 0.66,
    edge_fraction: 0.12,
    impulse: 1500.0,
    core: 0.0,
};

/// Mine: base Tribes AntipersonelMine, damageValue 0.65 over 10 m.
pub const MINE: BlastProfile = BlastProfile {
    radius: 10.0,
    max_damage: 100.0 * 0.65 / 0.66,
    edge_fraction: 0.12,
    impulse: 2000.0,
    core: 0.0,
};

/// The grenade launcher and mortar against equipment and deployables: their
/// earlier linear T2 splash, so shields still break to two focused attackers
/// (docs/weapon-damage.md). Everything else is the same as against players.
pub const GRENADE_STRUCTURE: BlastProfile = BlastProfile {
    radius: 15.0,
    max_damage: 100.0 * 0.40 * 1.20 / 0.66,
    edge_fraction: 0.12,
    impulse: 1500.0,
    core: 0.0,
};
pub const MORTAR_STRUCTURE: BlastProfile = BlastProfile {
    radius: 20.0,
    max_damage: 100.0 * 1.0 / 0.66,
    edge_fraction: 0.12,
    impulse: 3200.0,
    core: 0.0,
};

pub fn structure_blast(kind: u8) -> Option<BlastProfile> {
    match kind { 2 => Some(GRENADE_STRUCTURE), 5 => Some(MORTAR_STRUCTURE), _ => player_weapon_blast(kind) }
}

pub fn player_weapon_blast(kind: u8) -> Option<BlastProfile> {
    match kind { 0 => Some(DISC), 2 => Some(GRENADE), 5 => Some(MORTAR), 6 => Some(HAND_GRENADE), 7 => Some(MINE), _ => None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t2_light_armour_conversion_and_falloff() {
        for (p, peak) in [(DISC, 75.75758)] {
            assert!((p.damage(0.) - peak).abs() < 0.0001);
            assert!((p.damage(p.radius * 0.5) - peak * 0.56).abs() < 0.0001);
            assert!((p.damage(p.radius) - peak * 0.12).abs() < 0.0001);
            assert_eq!(p.damage(p.radius + 0.001), 0.);
            assert_eq!(p.damage(f32::NAN), 0.);
            assert_eq!(p.damage(f32::INFINITY), 0.);
            assert_eq!(p.damage(-1.), p.max_damage);
            let mut previous=p.max_damage;
            for i in 1..=100 {
                let damage=p.damage(p.radius*i as f32/100.);
                assert!(damage <= previous);previous=damage;
            }
        }
    }

    #[test]
    fn a_mortar_kills_on_a_near_direct_hit_takes_two_half_a_metre_off_and_fades_fast() {
        assert!(MORTAR.damage(0.0) >= 100.0 && MORTAR.damage(0.25) >= 100.0, "near-direct: one hit");
        let half = MORTAR.damage(0.5);
        assert!((50.0..100.0).contains(&half), "0.5 m: two hits ({half})");
        assert!(MORTAR.damage(2.0) < 15.0 && MORTAR.damage(5.0) < 2.0, "fades fast");
        assert_eq!(MORTAR.damage(20.5), 0.0);
        assert!(MORTAR.impulse > 1.5 * DISC.impulse, "an intense shock");
        assert!((GRENADE.damage(0.0) - MORTAR.damage(0.0) * GRENADE_SHARE).abs() < 1e-3);
        assert!((GRENADE.impulse - MORTAR.impulse * GRENADE_SHARE).abs() < 1e-3);
    }

    #[test]
    fn bullets_and_custom_plasma_are_not_rebalanced() {
        assert!(player_weapon_blast(1).is_none());
        assert!(player_weapon_blast(3).is_none());
    }
}
