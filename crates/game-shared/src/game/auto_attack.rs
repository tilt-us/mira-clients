use crate::network::ChampionId;

const DEFAULT_ATTACKS_PER_SECOND: f32 = 1.0;

/// Number of hits in the standard character auto-attack combo.
pub const AUTO_ATTACK_COMBO_LENGTH: usize = 4;

/// Base damage dealt by a character auto attack before combo scaling.
pub const CHARACTER_BASE_ATTACK_DAMAGE: f32 = 50.0;

/// Damage multiplier for the first auto-attack combo hit.
pub const FIRST_AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIER: f32 = 0.90;

/// Damage multiplier for the second auto-attack combo hit.
pub const SECOND_AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIER: f32 = 1.05;

/// Damage multiplier for the third auto-attack combo hit.
pub const THIRD_AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIER: f32 = 1.15;

/// Damage multiplier for the fourth auto-attack combo hit.
pub const FOURTH_AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIER: f32 = 1.25;

/// Damage multipliers indexed by zero-based auto-attack combo stage.
pub const AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIERS: [f32; AUTO_ATTACK_COMBO_LENGTH] = [
    FIRST_AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIER,
    SECOND_AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIER,
    THIRD_AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIER,
    FOURTH_AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIER,
];

/// Seconds after the last accepted auto attack before the combo starts over.
pub const AUTO_ATTACK_COMBO_RESET_SECONDS: f32 = 2.0;

/// Maximum basic-attack distance before accounting for a target's hit radius.
pub const AUTO_ATTACK_RANGE: f32 = 5.0;

/// Minimum time an auto-attack projectile remains in flight.
pub const AUTO_ATTACK_PROJECTILE_MIN_TRAVEL_SECONDS: f32 = 0.075;

/// Maximum time an auto-attack projectile remains in flight at maximum attack range.
pub const AUTO_ATTACK_PROJECTILE_MAX_TRAVEL_SECONDS: f32 = 0.45;

/// Returns the synchronized flight duration for an auto-attack projectile.
pub fn auto_attack_projectile_travel_seconds(distance: f32) -> f32 {
    let range_ratio = (distance / AUTO_ATTACK_RANGE).clamp(0.0, 1.0);
    (range_ratio * AUTO_ATTACK_PROJECTILE_MAX_TRAVEL_SECONDS).clamp(
        AUTO_ATTACK_PROJECTILE_MIN_TRAVEL_SECONDS,
        AUTO_ATTACK_PROJECTILE_MAX_TRAVEL_SECONDS,
    )
}

/// Defines shared auto-attack combo tuning used by client prediction and the server.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoAttackCombo {
    /// Number of hits before the combo returns to its first stage.
    pub combo_length: usize,
    /// Number of accepted auto attacks per second.
    pub attacks_per_second: f32,
}

impl AutoAttackCombo {
    /// Returns the cooldown between accepted attacks in seconds.
    pub fn cooldown_seconds(self) -> f32 {
        1.0 / self.attacks_per_second.max(f32::EPSILON)
    }

    /// Returns the damage for one zero-based combo stage.
    pub fn damage_for_stage(self, stage: usize) -> f32 {
        let combo_length = self.combo_length.clamp(1, AUTO_ATTACK_COMBO_LENGTH);
        let combo_stage = stage % combo_length;

        CHARACTER_BASE_ATTACK_DAMAGE * AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIERS[combo_stage]
    }
}

/// Returns the standard auto-attack combo tuning for a character.
pub fn auto_attack_combo(_champion: ChampionId) -> AutoAttackCombo {
    AutoAttackCombo {
        combo_length: AUTO_ATTACK_COMBO_LENGTH,
        attacks_per_second: DEFAULT_ATTACKS_PER_SECOND,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_champion_uses_the_standard_four_hit_combo() {
        for champion in [
            ChampionId::IGNARA,
            ChampionId::LIRA,
            ChampionId::YUNA,
            ChampionId::SOPHIA,
            ChampionId(9999),
        ] {
            assert_eq!(
                auto_attack_combo(champion).combo_length,
                AUTO_ATTACK_COMBO_LENGTH
            );
        }
    }

    #[test]
    fn combo_damage_uses_the_configured_base_damage_and_multipliers() {
        let combo = auto_attack_combo(ChampionId::IGNARA);
        let damages = (0..combo.combo_length)
            .map(|stage| combo.damage_for_stage(stage))
            .collect::<Vec<_>>();

        assert_eq!(CHARACTER_BASE_ATTACK_DAMAGE, 50.0);
        assert_eq!(
            AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIERS,
            [0.90, 1.05, 1.15, 1.25]
        );
        assert_eq!(
            damages,
            AUTO_ATTACK_COMBO_DAMAGE_MULTIPLIERS
                .map(|multiplier| CHARACTER_BASE_ATTACK_DAMAGE * multiplier)
        );
    }

    #[test]
    fn combo_restarts_after_the_fourth_hit() {
        let combo = auto_attack_combo(ChampionId::LIRA);

        assert_eq!(
            combo.damage_for_stage(AUTO_ATTACK_COMBO_LENGTH),
            combo.damage_for_stage(0)
        );
    }

    #[test]
    fn attack_speed_maps_to_cooldown() {
        let combo = auto_attack_combo(ChampionId::LIRA);
        assert_eq!(combo.cooldown_seconds(), 1.0);
    }

    #[test]
    fn projectile_travel_time_is_clamped_to_the_configured_range() {
        assert_eq!(
            auto_attack_projectile_travel_seconds(0.0),
            AUTO_ATTACK_PROJECTILE_MIN_TRAVEL_SECONDS
        );
        assert_eq!(
            auto_attack_projectile_travel_seconds(AUTO_ATTACK_RANGE),
            AUTO_ATTACK_PROJECTILE_MAX_TRAVEL_SECONDS
        );
        assert_eq!(
            auto_attack_projectile_travel_seconds(AUTO_ATTACK_RANGE * 2.0),
            AUTO_ATTACK_PROJECTILE_MAX_TRAVEL_SECONDS
        );
    }
}
