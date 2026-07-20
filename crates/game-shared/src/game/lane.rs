use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{player::DEFAULT_PLAYER_MOVEMENT_SPEED, team::TeamSpec};

/// Half-width of the single-lane corridor in world units.
pub const LANE_HALF_WIDTH: f32 = 6.0;

/// Z coordinate used for the outer spawn point of each playable team.
pub const LANE_SPAWN_Z: f32 = 50.0;

/// Z coordinate used for the tower protecting each team's side of the lane.
pub const LANE_TOWER_Z: f32 = 22.0;

/// Base player movement speed used to derive minion movement speed.
pub const LANE_PLAYER_BASE_MOVEMENT_SPEED: f32 = DEFAULT_PLAYER_MOVEMENT_SPEED;

/// Horizontal collision radius used for players on the lane.
pub const LANE_PLAYER_COLLISION_RADIUS: f32 = 0.9;

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

const MELEE_BOX_STATS: LaneUnitStats = LaneUnitStats {
    max_health: 350.0,
    attack_damage: 11.0,
    attack_range: 0.75,
    attack_interval_seconds: 0.6,
    movement_speed: LANE_MINION_MOVEMENT_SPEED,
    hit_radius: 0.55,
};

const LARGE_RANGED_BOX_STATS: LaneUnitStats = LaneUnitStats {
    max_health: 560.0,
    attack_damage: 16.0,
    attack_range: 3.0,
    attack_interval_seconds: 1.1,
    movement_speed: LANE_MINION_MOVEMENT_SPEED,
    hit_radius: 0.9,
};

const RANGED_ORB_STATS: LaneUnitStats = LaneUnitStats {
    max_health: 200.0,
    attack_damage: 7.0,
    attack_range: 3.0,
    attack_interval_seconds: 0.9,
    movement_speed: LANE_MINION_MOVEMENT_SPEED,
    hit_radius: 0.45,
};

const TOWER_STATS: LaneUnitStats = LaneUnitStats {
    max_health: 5500.0,
    attack_damage: 90.0,
    attack_range: TOWER_ATTACK_RANGE,
    attack_interval_seconds: 1.2,
    movement_speed: 0.0,
    hit_radius: 1.25,
};

/// Returns the authoritative stats configured for a lane unit kind.
pub fn lane_unit_stats(kind: LaneUnitKind) -> LaneUnitStats {
    match kind {
        LaneUnitKind::MeleeBox => MELEE_BOX_STATS,
        LaneUnitKind::LargeRangedBox => LARGE_RANGED_BOX_STATS,
        LaneUnitKind::RangedOrb => RANGED_ORB_STATS,
        LaneUnitKind::Tower => TOWER_STATS,
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
    use crate::game::lane_navigation::{
        LaneNavigationObstacle, resolve_circle_obstacle_collisions,
    };

    #[test]
    fn places_playable_teams_at_opposite_lane_ends() {
        assert_eq!(lane_spawn_position(TeamSpec::Light).z, -LANE_SPAWN_Z);
        assert_eq!(lane_spawn_position(TeamSpec::Dark).z, LANE_SPAWN_Z);
        assert_eq!(lane_forward_direction(TeamSpec::Light), Vec3::Z);
        assert_eq!(lane_forward_direction(TeamSpec::Dark), Vec3::NEG_Z);
    }

    #[test]
    fn configures_requested_lane_unit_stats() {
        assert_eq!(lane_unit_stats(LaneUnitKind::MeleeBox), MELEE_BOX_STATS);
        assert_eq!(
            lane_unit_stats(LaneUnitKind::LargeRangedBox),
            LARGE_RANGED_BOX_STATS
        );
        assert_eq!(lane_unit_stats(LaneUnitKind::RangedOrb), RANGED_ORB_STATS);
        assert_eq!(lane_unit_stats(LaneUnitKind::Tower), TOWER_STATS);
    }

    #[test]
    fn tower_collision_blocks_a_player_from_crossing_the_lane() {
        let tower = lane_tower_position(TeamSpec::Light);
        let collision_radius =
            lane_unit_stats(LaneUnitKind::Tower).hit_radius + LANE_PLAYER_COLLISION_RADIUS;
        let start = tower - Vec3::Z * 5.0;
        let desired = tower + Vec3::Z * 5.0;

        let resolved = resolve_circle_obstacle_collisions(
            start,
            desired,
            LANE_PLAYER_COLLISION_RADIUS,
            &[LaneNavigationObstacle::new(
                tower,
                lane_unit_stats(LaneUnitKind::Tower).hit_radius,
            )],
        );

        assert!(resolved.z < tower.z);
        assert!(
            Vec2::new(resolved.x - tower.x, resolved.z - tower.z).length()
                >= collision_radius - 0.001
        );
    }

    #[test]
    fn tower_collision_preserves_an_unobstructed_path_alongside_the_tower() {
        let tower = lane_tower_position(TeamSpec::Dark);
        let start = tower + Vec3::new(-4.0, 0.0, -3.0);
        let desired = tower + Vec3::new(-4.0, 0.0, 3.0);

        let resolved = resolve_circle_obstacle_collisions(
            start,
            desired,
            LANE_PLAYER_COLLISION_RADIUS,
            &[LaneNavigationObstacle::new(
                tower,
                lane_unit_stats(LaneUnitKind::Tower).hit_radius,
            )],
        );

        assert_eq!(resolved, desired);
    }

    #[test]
    fn tower_collision_recovers_an_invalid_position_without_passing_through() {
        let tower = lane_tower_position(TeamSpec::Light);
        let start = tower;
        let desired = tower + Vec3::Z * 4.0;

        let resolved = resolve_circle_obstacle_collisions(
            start,
            desired,
            LANE_PLAYER_COLLISION_RADIUS,
            &[LaneNavigationObstacle::new(
                tower,
                lane_unit_stats(LaneUnitKind::Tower).hit_radius,
            )],
        );

        assert!(resolved.z < tower.z);
    }

    #[test]
    fn collision_uses_only_the_obstacles_supplied_by_the_live_lane() {
        let tower = lane_tower_position(TeamSpec::Light);
        let start = tower - Vec3::Z * 5.0;
        let desired = tower + Vec3::Z * 5.0;

        let resolved =
            resolve_circle_obstacle_collisions(start, desired, LANE_PLAYER_COLLISION_RADIUS, &[]);

        assert_eq!(resolved, desired);
    }
}
