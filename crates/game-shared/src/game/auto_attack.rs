use crate::network::ChampionId;

const MIN_COMBO_LENGTH: usize = 3;
const DEFAULT_ATTACKS_PER_SECOND: f32 = 1.0;
const FIRST_COMBO_HIT_DAMAGE: f32 = 6.0;
const LAST_COMBO_HIT_DAMAGE: f32 = 18.0;

/// Seconds after the last accepted auto attack before the combo starts over.
pub const AUTO_ATTACK_COMBO_RESET_SECONDS: f32 = 2.0;

/// Description:
/// Defines champion-specific auto-attack combo tuning shared by client prediction and the server.
///
/// Fields:
/// - `combo_length`: Number of attacks in the champion's repeating combo.
/// - `attacks_per_second`: Attack speed used to compute the authoritative attack cooldown.
/// - `first_hit_damage`: Damage dealt by the first combo hit.
/// - `last_hit_damage`: Damage dealt by the final combo hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoAttackCombo {
    pub combo_length: usize,
    pub attacks_per_second: f32,
    pub first_hit_damage: f32,
    pub last_hit_damage: f32,
}

impl AutoAttackCombo {
    /// Description:
    /// Returns the cooldown in seconds between accepted attacks.
    pub fn cooldown_seconds(self) -> f32 {
        1.0 / self.attacks_per_second.max(f32::EPSILON)
    }

    /// Description:
    /// Returns the damage for one zero-based combo stage.
    pub fn damage_for_stage(self, stage: usize) -> f32 {
        let combo_length = self.combo_length.max(MIN_COMBO_LENGTH);
        if combo_length <= 1 {
            return self.last_hit_damage.max(self.first_hit_damage);
        }

        let clamped_stage = stage.min(combo_length - 1);
        let progress = clamped_stage as f32 / (combo_length - 1) as f32;
        self.first_hit_damage + (self.last_hit_damage - self.first_hit_damage) * progress
    }
}

/// Description:
/// Returns champion-specific auto-attack combo tuning.
///
/// Params:
/// - `champion`: Champion whose combo tuning should be resolved.
///
/// Returns:
/// - Shared auto-attack combo tuning with a minimum combo length of three.
pub fn auto_attack_combo(champion: ChampionId) -> AutoAttackCombo {
    let combo_length = match champion.0 {
        6607 => 3, // Ignara
        6606 => 4, // Lira
        6608 => 5, // Yuna
        6609 => 3, // Sophia
        _ => MIN_COMBO_LENGTH,
    }
    .max(MIN_COMBO_LENGTH);

    AutoAttackCombo {
        combo_length,
        attacks_per_second: DEFAULT_ATTACKS_PER_SECOND,
        first_hit_damage: FIRST_COMBO_HIT_DAMAGE,
        last_hit_damage: LAST_COMBO_HIT_DAMAGE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the champion combo lengths match ticket step for the shared auto-attack tuning system.
    #[test]
    fn champion_combo_lengths_match_ticket() {
        assert_eq!(auto_attack_combo(ChampionId(6607)).combo_length, 3);
        assert_eq!(auto_attack_combo(ChampionId(6606)).combo_length, 4);
        assert_eq!(auto_attack_combo(ChampionId(6608)).combo_length, 5);
        assert_eq!(auto_attack_combo(ChampionId(6609)).combo_length, 3);
    }

    /// Runs the unknown champion gets minimum combo length step for the shared auto-attack tuning system.
    #[test]
    fn unknown_champion_gets_minimum_combo_length() {
        assert_eq!(
            auto_attack_combo(ChampionId(9999)).combo_length,
            MIN_COMBO_LENGTH
        );
    }

    /// Runs the combo damage increases until final hit step for the shared auto-attack tuning system.
    #[test]
    fn combo_damage_increases_until_final_hit() {
        let combo = auto_attack_combo(ChampionId(6608));
        let damages = (0..combo.combo_length)
            .map(|stage| combo.damage_for_stage(stage))
            .collect::<Vec<_>>();

        assert!(damages.windows(2).all(|window| window[0] < window[1]));
        assert_eq!(damages.first().copied(), Some(FIRST_COMBO_HIT_DAMAGE));
        assert_eq!(damages.last().copied(), Some(LAST_COMBO_HIT_DAMAGE));
    }

    /// Runs the attack speed maps to cooldown step for the shared auto-attack tuning system.
    #[test]
    fn attack_speed_maps_to_cooldown() {
        let combo = auto_attack_combo(ChampionId(6606));
        assert_eq!(combo.cooldown_seconds(), 1.0);
    }
}
