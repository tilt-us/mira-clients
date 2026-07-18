use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::team::TeamSpec;

/// Half-width of the single-lane corridor in world units.
pub const LANE_HALF_WIDTH: f32 = 6.0;

/// Z coordinate used for the outer spawn point of each playable team.
pub const LANE_SPAWN_Z: f32 = 50.0;

/// Z coordinate used for the tower protecting each team's side of the lane.
pub const LANE_TOWER_Z: f32 = 22.0;

/// Base player movement speed used to derive minion movement speed.
pub const LANE_PLAYER_BASE_MOVEMENT_SPEED: f32 = 6.0;

/// Movement speed assigned to all lane minions.
pub const LANE_MINION_MOVEMENT_SPEED: f32 = LANE_PLAYER_BASE_MOVEMENT_SPEED * 0.8;

/// Seconds between consecutive minion waves after the initial wave.
pub const LANE_WAVE_INTERVAL_SECONDS: f32 = 60.0;

/// Radius in which a tower can select and attack enemies.
pub const TOWER_ATTACK_RANGE: f32 = 6.0;

/// Describes the visual and combat role of a replicated lane unit.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LaneUnitKind {
    /// A small melee minion rendered as a cube.
    MeleeBox,
    /// A larger ranged minion rendered as a cube.
    LargeRangedBox,
    /// A ranged minion rendered as a sphere.
    RangedOrb,
    /// A stationary defensive tower.
    Tower,
}

/// Stores the authoritative combat and presentation values for one lane unit kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneUnitStats {
    /// Maximum health assigned when the unit is spawned.
    pub max_health: f32,
    /// Damage dealt by one accepted attack.
    pub attack_damage: f32,
    /// Maximum horizontal distance at which an attack can be started.
    pub attack_range: f32,
    /// Seconds between attacks.
    pub attack_interval_seconds: f32,
    /// Horizontal movement speed in world units per second.
    pub movement_speed: f32,
    /// Horizontal hit radius used by targeting and range checks.
    pub hit_radius: f32,
}

/// Returns the authoritative stats configured for a lane unit kind.
pub fn lane_unit_stats(kind: LaneUnitKind) -> LaneUnitStats {
    match kind {
        LaneUnitKind::MeleeBox => LaneUnitStats {
            max_health: 350.0,
            attack_damage: 11.0,
            attack_range: 0.75,
            attack_interval_seconds: 0.6,
            movement_speed: LANE_MINION_MOVEMENT_SPEED,
            hit_radius: 0.55,
        },
        LaneUnitKind::LargeRangedBox => LaneUnitStats {
            max_health: 560.0,
            attack_damage: 16.0,
            attack_range: 3.0,
            attack_interval_seconds: 1.1,
            movement_speed: LANE_MINION_MOVEMENT_SPEED,
            hit_radius: 0.9,
        },
        LaneUnitKind::RangedOrb => LaneUnitStats {
            max_health: 200.0,
            attack_damage: 7.0,
            attack_range: 3.0,
            attack_interval_seconds: 0.9,
            movement_speed: LANE_MINION_MOVEMENT_SPEED,
            hit_radius: 0.45,
        },
        LaneUnitKind::Tower => LaneUnitStats {
            max_health: 5500.0,
            attack_damage: 90.0,
            attack_range: TOWER_ATTACK_RANGE,
            attack_interval_seconds: 1.2,
            movement_speed: 0.0,
            hit_radius: 1.25,
        },
    }
}

/// Returns the spawn position for a playable team on the single lane.
pub fn lane_spawn_position(team: TeamSpec) -> Vec3 {
    match team {
        TeamSpec::Light => Vec3::new(0.0, 0.0, -LANE_SPAWN_Z),
        TeamSpec::Dark => Vec3::new(0.0, 0.0, LANE_SPAWN_Z),
        TeamSpec::Neutral => Vec3::ZERO,
    }
}

/// Returns the defensive tower position for a playable team on the single lane.
pub fn lane_tower_position(team: TeamSpec) -> Vec3 {
    match team {
        TeamSpec::Light => Vec3::new(0.0, 0.0, -LANE_TOWER_Z),
        TeamSpec::Dark => Vec3::new(0.0, 0.0, LANE_TOWER_Z),
        TeamSpec::Neutral => Vec3::ZERO,
    }
}

/// Returns the direction in which a team's minions advance along the lane.
pub fn lane_forward_direction(team: TeamSpec) -> Vec3 {
    match team {
        TeamSpec::Light => Vec3::Z,
        TeamSpec::Dark => Vec3::NEG_Z,
        TeamSpec::Neutral => Vec3::ZERO,
    }
}

/// Returns the yaw angle that faces a team's forward lane direction.
pub fn lane_forward_yaw(team: TeamSpec) -> f32 {
    match team {
        TeamSpec::Light => 0.0,
        TeamSpec::Dark => std::f32::consts::PI,
        TeamSpec::Neutral => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_playable_teams_at_opposite_lane_ends() {
        assert_eq!(lane_spawn_position(TeamSpec::Light).z, -LANE_SPAWN_Z);
        assert_eq!(lane_spawn_position(TeamSpec::Dark).z, LANE_SPAWN_Z);
        assert_eq!(lane_forward_direction(TeamSpec::Light), Vec3::Z);
        assert_eq!(lane_forward_direction(TeamSpec::Dark), Vec3::NEG_Z);
    }

    #[test]
    fn configures_requested_lane_unit_stats() {
        let melee = lane_unit_stats(LaneUnitKind::MeleeBox);
        let tower = lane_unit_stats(LaneUnitKind::Tower);

        assert_eq!(melee.max_health, 350.0);
        assert_eq!(melee.attack_damage, 11.0);
        assert_eq!(melee.attack_interval_seconds, 0.6);
        assert_eq!(tower.max_health, 5500.0);
        assert_eq!(tower.attack_damage, 90.0);
        assert_eq!(tower.attack_range, TOWER_ATTACK_RANGE);
    }
}
