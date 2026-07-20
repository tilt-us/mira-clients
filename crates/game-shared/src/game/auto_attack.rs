use crate::network::ChampionId;

const MIN_COMBO_LENGTH: usize = 3;
const DEFAULT_ATTACKS_PER_SECOND: f32 = 1.0;
const CHARACTER_AUTO_ATTACK_DAMAGE_MULTIPLIER: f32 = 1.8;
const FIRST_COMBO_HIT_BASE_DAMAGE: f32 = 6.0;
const LAST_COMBO_HIT_BASE_DAMAGE: f32 = 18.0;
const FIRST_COMBO_HIT_DAMAGE: f32 =
    FIRST_COMBO_HIT_BASE_DAMAGE * CHARACTER_AUTO_ATTACK_DAMAGE_MULTIPLIER;
const LAST_COMBO_HIT_DAMAGE: f32 =
    LAST_COMBO_HIT_BASE_DAMAGE * CHARACTER_AUTO_ATTACK_DAMAGE_MULTIPLIER;

/// Seconds after the last accepted auto attack before the combo starts over.
pub const AUTO_ATTACK_COMBO_RESET_SECONDS: f32 = 2.0;

/// Maximum basic-attack distance before accounting for a target's hit radius.
pub const AUTO_ATTACK_RANGE: f32 = 5.0;

/// Defines champion-specific auto-attack combo tuning shared by client prediction and the server.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoAttackCombo {
    pub combo_length: usize,
    pub attacks_per_second: f32,
    pub first_hit_damage: f32,
    pub last_hit_damage: f32,
}

impl AutoAttackCombo {
    /// Returns the cooldown between accepted attacks in seconds.
    pub fn cooldown_seconds(self) -> f32 {
        1.0 / self.attacks_per_second.max(f32::EPSILON)
    }

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

/// Returns champion-specific auto-attack combo tuning.
pub fn auto_attack_combo(champion: ChampionId) -> AutoAttackCombo {
    let combo_length = match champion {
        ChampionId::IGNARA => 3,
        ChampionId::LIRA => 4,
        ChampionId::YUNA => 5,
        ChampionId::SOPHIA => 3,
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

    #[test]
    fn champion_combo_lengths_match_ticket() {
        assert_eq!(auto_attack_combo(ChampionId::IGNARA).combo_length, 3);
        assert_eq!(auto_attack_combo(ChampionId::LIRA).combo_length, 4);
        assert_eq!(auto_attack_combo(ChampionId::YUNA).combo_length, 5);
        assert_eq!(auto_attack_combo(ChampionId::SOPHIA).combo_length, 3);
    }

    #[test]
    fn unknown_champion_gets_minimum_combo_length() {
        assert_eq!(
            auto_attack_combo(ChampionId(9999)).combo_length,
            MIN_COMBO_LENGTH
        );
    }

    #[test]
    fn combo_damage_increases_until_final_hit() {
        let combo = auto_attack_combo(ChampionId::YUNA);
        let damages = (0..combo.combo_length)
            .map(|stage| combo.damage_for_stage(stage))
            .collect::<Vec<_>>();

        assert!(damages.windows(2).all(|window| window[0] < window[1]));
        assert_eq!(damages.first().copied(), Some(FIRST_COMBO_HIT_DAMAGE));
        assert_eq!(damages.last().copied(), Some(LAST_COMBO_HIT_DAMAGE));
    }

    #[test]
    fn character_auto_attack_damage_has_eighty_percent_bonus() {
        let combo = auto_attack_combo(ChampionId::IGNARA);

        assert_eq!(
            combo.first_hit_damage,
            FIRST_COMBO_HIT_BASE_DAMAGE * CHARACTER_AUTO_ATTACK_DAMAGE_MULTIPLIER
        );
        assert_eq!(
            combo.last_hit_damage,
            LAST_COMBO_HIT_BASE_DAMAGE * CHARACTER_AUTO_ATTACK_DAMAGE_MULTIPLIER
        );
    }

    #[test]
    fn attack_speed_maps_to_cooldown() {
        let combo = auto_attack_combo(ChampionId::LIRA);
        assert_eq!(combo.cooldown_seconds(), 1.0);
    }
}
