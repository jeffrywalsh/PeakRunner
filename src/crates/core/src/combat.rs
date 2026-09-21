//! Original implementation of researched T2-style explosive balance.
//! Values and deliberate PeakRunner differences: docs/weapon-damage.md.

#[derive(Clone, Copy, Debug)]
pub struct BlastProfile {
    pub radius: f32,
    pub max_damage: f32,
    pub edge_fraction: f32,
    /// Blast impulse, converted to a velocity change using player mass.
    pub impulse: f32,
}

impl BlastProfile {
    /// Distance from the blast to the target's collision surface, in metres.
    /// T2 keeps 12% damage at the edge; beyond the radius there is no damage.
    pub fn damage(self, distance: f32) -> f32 {
        if !distance.is_finite() || distance > self.radius {
            return 0.0;
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
};
pub const GRENADE: BlastProfile = BlastProfile {
    radius: 15.0,
    max_damage: 100.0 * 0.40 * 1.20 / 0.66,
    edge_fraction: 0.12,
    impulse: 1500.0,
};

pub fn player_weapon_blast(kind: u8) -> Option<BlastProfile> {
    match kind { 0 => Some(DISC), 2 => Some(GRENADE), _ => None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t2_light_armour_conversion_and_falloff() {
        for (p, peak) in [(DISC, 75.75758), (GRENADE, 72.72727)] {
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
    fn bullets_and_custom_plasma_are_not_rebalanced() {
        assert!(player_weapon_blast(1).is_none());
        assert!(player_weapon_blast(3).is_none());
    }
}
