use super::combat::{ServerCombatNumberEvents, apply_damage};
use super::geometry::{distance_to_segment_xz, horizontal_distance, point_in_oriented_rect_xz};
use super::lobby::{ConnectedPlayers, LoadingScreenReadyPlayers};
use bevy::prelude::*;
use game_shared::game::{
    auto_attack::AUTO_ATTACK_RANGE,
    lane::{
        LANE_HALF_WIDTH, LANE_PLAYER_COLLISION_RADIUS, LANE_SPAWN_Z, LANE_WAVE_INTERVAL_SECONDS,
        LaneUnitKind, lane_forward_direction, lane_forward_yaw, lane_spawn_position,
        lane_tower_position, lane_unit_stats,
    },
    lane_navigation::{
        LANE_NAVIGATION_CLEARANCE, LaneNavigationMesh, LaneNavigationObstacle,
        LaneNavigationPath as MeshNavigationPath, lane_navigation_obstacle_revision,
        resolve_circle_obstacle_collisions,
    },
    team::TeamSpec,
};
use game_shared::network::{
    LaneSnapshot, NetworkLaneUnit, NetworkTargetId, PlayerStateChannel,
    RangedMinionAutoAttackVisualEvent, ReliableCommandChannel,
};
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::*;
use std::collections::{HashMap, VecDeque};

const LANE_SNAPSHOT_INTERVAL_SECONDS: f32 = 1.0 / 20.0;
const LANE_WAVE_UNIT_SPAWN_INTERVAL_SECONDS: f32 = 0.4;
const TOWER_PROJECTILE_SPEED: f32 = 24.0;
const TOWER_PROJECTILE_MIN_TRAVEL_SECONDS: f32 = 0.1;
const TOWER_PROJECTILE_MAX_TRAVEL_SECONDS: f32 = 0.45;
const RANGED_MINION_PROJECTILE_SPEED: f32 = 16.0;
const RANGED_MINION_PROJECTILE_HEIGHT: f32 = 0.8;
const RANGED_MINION_PROJECTILE_MIN_TRAVEL_SECONDS: f32 = 0.075;
const RANGED_MINION_PROJECTILE_MAX_TRAVEL_SECONDS: f32 = 0.45;
const MINION_SEPARATION_MARGIN: f32 = 0.15;
// Retains a small gap at contact so floating-point roots cannot admit a full movement step.
const MINION_COLLISION_SKIN: f32 = 0.002;
// A minion acquires a new hostile when that hostile's collision edge is within this distance.
const MINION_TARGET_TRIGGER_RANGE: f32 = 4.0;
// Melee minions should not commit to the first enemy in a wave while it is still far ahead.
const MINION_MELEE_TARGET_LEASH_MARGIN: f32 = 1.25;
const MINION_RANGED_TARGET_LEASH_MARGIN: f32 = 4.0;
const MELEE_MINION_RETARGET_DISTANCE_ADVANTAGE: f32 = 0.5;
const MINION_WAVE_ASSIST_RADIUS: f32 = 4.5;
// Ranged minions begin their approach from the rear of the marching column once the melee
// frontline has engaged, so they can fan out instead of getting stuck directly behind it.
const RANGED_MINION_WAVE_ASSIST_RADIUS: f32 = 7.0;
const MINION_WAVE_TARGET_CLUSTER_RADIUS: f32 = 4.5;
// A following minion may join the opponent's active front, but never skip to a rear row.
const MINION_WAVE_FRONTLINE_DEPTH: f32 = 1.25;
const NAVIGATION_GOAL_REPLAN_DISTANCE: f32 = 0.35;
const NAVIGATION_WAYPOINT_REACHED_DISTANCE: f32 = 0.12;
// A projected mesh start must be reached before consuming the next route corner. Using the
// regular waypoint tolerance here can skip the short recovery leg and leave a mover stuck on a
// tower edge.
const NAVIGATION_RECOVERY_WAYPOINT_REACHED_DISTANCE: f32 = 0.001;
// Begin routing before a minion reaches a tower, while keeping the initial march centered.
const MINION_NAVIGATION_ACTIVATION_DISTANCE: f32 = 8.0;
// Dynamic minions still need a short local avoidance pass after following a static mesh route.
const MINION_BLOCKER_DETOUR_STRENGTH: f32 = 1.25;
const MINION_BLOCKER_DETOUR_PROGRESS_WEIGHT: f32 = 0.3;
const MELEE_COMBAT_SLOT_COLUMNS: usize = 3;
// Six melee minions fit around the edge of a target's attack ring without overlapping. Further
// attackers retain a rear queue until a ring position opens up.
const MELEE_COMBAT_RING_SLOTS: usize = 6;
const RANGED_COMBAT_SLOT_COLUMNS: usize = 5;
const MELEE_SINGLETON_COMBAT_SLOT_ANGLES: [f32; 3] = [0.0, -1.2, 1.2];
const RANGED_SINGLETON_COMBAT_SLOT_ANGLES: [f32; 5] = [0.0, -0.6, 0.6, -1.2, 1.2];

const MINION_WAVE: [LaneUnitKind; 7] = [
    LaneUnitKind::MeleeBox,
    LaneUnitKind::MeleeBox,
    LaneUnitKind::MeleeBox,
    LaneUnitKind::LargeRangedBox,
    LaneUnitKind::RangedOrb,
    LaneUnitKind::RangedOrb,
    LaneUnitKind::RangedOrb,
];

fn ranged_minion_projectile_travel_seconds(distance: f32) -> f32 {
    (distance / RANGED_MINION_PROJECTILE_SPEED).clamp(
        RANGED_MINION_PROJECTILE_MIN_TRAVEL_SECONDS,
        RANGED_MINION_PROJECTILE_MAX_TRAVEL_SECONDS,
    )
}

/// Stores server-authoritative state for the single-lane test map.
#[derive(Resource, Debug)]
pub(super) struct ServerLaneState {
    units: HashMap<u64, ServerLaneUnit>,
    next_unit_id: u64,
    wave_timer: Timer,
    wave_spawn_timer: Timer,
    pending_wave_units: VecDeque<LaneUnitKind>,
    snapshot_timer: Timer,
    started: bool,
    tower_projectiles: Vec<TowerProjectile>,
    ranged_minion_projectiles: Vec<RangedMinionProjectile>,
    ranged_minion_auto_attack_visuals: Vec<RangedMinionAutoAttackVisualEvent>,
    navigation: LaneNavigationCache,
}

#[derive(Debug, Clone)]
struct ServerLaneUnit {
    kind: LaneUnitKind,
    team: TeamSpec,
    position: Vec3,
    health: f32,
    attack_cooldown_seconds: f32,
    attack_target: Option<NetworkTargetId>,
    engagement_target: Option<NetworkTargetId>,
    forced_player_target: Option<u64>,
    navigation_path: LaneNavigationPath,
}

/// Stores a minion's current mesh route until its target or static obstacles change.
#[derive(Debug, Clone, Default)]
struct LaneNavigationPath {
    obstacle_revision: Option<u64>,
    goal: Option<Vec3>,
    waypoints: VecDeque<MinionNavigationWaypoint>,
    route_resolved: bool,
}

/// Stores one minion route point and whether it must be reached before advancing the route.
#[derive(Debug, Clone, Copy)]
struct MinionNavigationWaypoint {
    position: Vec3,
    requires_precise_arrival: bool,
}

#[derive(Debug, Clone, Copy)]
struct CombatSlotAssignment {
    angle: f32,
    queue_row: usize,
    uses_attack_ring: bool,
}

/// Caches one navigation mesh for each requested mover radius.
#[derive(Debug, Default)]
struct LaneNavigationCache {
    obstacle_revision: Option<u64>,
    meshes: Vec<(u32, LaneNavigationMesh)>,
}

#[derive(Debug, Clone, Copy)]
struct TowerProjectile {
    source_id: u64,
    target: NetworkTargetId,
    remaining_seconds: f32,
}

#[derive(Debug, Clone, Copy)]
struct RangedMinionProjectile {
    target: NetworkTargetId,
    remaining_seconds: f32,
    damage: f32,
}

#[derive(Debug, Clone, Copy)]
struct LaneDamageAction {
    target: NetworkTargetId,
    amount: f32,
}

impl Default for ServerLaneState {
    fn default() -> Self {
        Self {
            units: HashMap::new(),
            next_unit_id: 1,
            wave_timer: Timer::from_seconds(LANE_WAVE_INTERVAL_SECONDS, TimerMode::Repeating),
            wave_spawn_timer: Timer::from_seconds(
                LANE_WAVE_UNIT_SPAWN_INTERVAL_SECONDS,
                TimerMode::Repeating,
            ),
            pending_wave_units: VecDeque::new(),
            snapshot_timer: Timer::from_seconds(
                LANE_SNAPSHOT_INTERVAL_SECONDS,
                TimerMode::Repeating,
            ),
            started: false,
            tower_projectiles: Vec::new(),
            ranged_minion_projectiles: Vec::new(),
            ranged_minion_auto_attack_visuals: Vec::new(),
            navigation: LaneNavigationCache::default(),
        }
    }
}

impl LaneNavigationCache {
    fn refresh(&mut self, obstacles: &[LaneNavigationObstacle]) -> u64 {
        let obstacle_revision = lane_navigation_obstacle_revision(obstacles);
        if self.obstacle_revision != Some(obstacle_revision) {
            self.obstacle_revision = Some(obstacle_revision);
            self.meshes.clear();
        }

        obstacle_revision
    }

    fn find_path_with_projection(
        &mut self,
        agent_radius: f32,
        start: Vec3,
        goal: Vec3,
        obstacles: &[LaneNavigationObstacle],
    ) -> (u64, Option<MeshNavigationPath>) {
        let obstacle_revision = self.refresh(obstacles);
        let radius_key = agent_radius.max(0.0).to_bits();
        let mesh_index = match self
            .meshes
            .iter()
            .position(|(candidate_radius, _)| *candidate_radius == radius_key)
        {
            Some(index) => index,
            None => {
                self.meshes
                    .push((radius_key, LaneNavigationMesh::new(agent_radius, obstacles)));
                self.meshes.len() - 1
            }
        };

        (
            obstacle_revision,
            self.meshes[mesh_index]
                .1
                .find_path_with_projection(start, goal),
        )
    }

    fn obstacle_revision(&mut self, obstacles: &[LaneNavigationObstacle]) -> u64 {
        self.refresh(obstacles)
    }
}

impl ServerLaneState {
    /// Returns the mesh route including its projected safe start position when needed.
    pub(super) fn navigation_path_with_projection_for_mover(
        &mut self,
        start: Vec3,
        goal: Vec3,
        agent_radius: f32,
    ) -> (u64, Option<MeshNavigationPath>) {
        let obstacles = self.living_tower_navigation_obstacles();
        self.navigation
            .find_path_with_projection(agent_radius, start, goal, &obstacles)
    }

    /// Returns the revision of the current living-tower navigation layout.
    pub(super) fn navigation_obstacle_revision(&mut self) -> u64 {
        let obstacles = self.living_tower_navigation_obstacles();
        self.navigation.obstacle_revision(&obstacles)
    }

    fn living_tower_navigation_obstacles(&self) -> Vec<LaneNavigationObstacle> {
        self.units
            .values()
            .filter(|unit| unit.kind == LaneUnitKind::Tower && unit.health > 0.0)
            .map(|unit| {
                LaneNavigationObstacle::new(unit.position, lane_unit_stats(unit.kind).hit_radius)
            })
            .collect()
    }

    /// Returns the current position and targeting radius for a hostile lane unit.
    pub(super) fn target_for_player_auto_attack(
        &self,
        caster_team: TeamSpec,
        target_id: u64,
    ) -> Option<(Vec3, f32)> {
        let target = self.units.get(&target_id)?;
        if target.health <= 0.0 || target.team == caster_team || !caster_team.is_playable() {
            return None;
        }

        Some((target.position, lane_unit_stats(target.kind).hit_radius))
    }

    /// Resolves a player movement segment against every living lane tower.
    pub(super) fn resolve_player_tower_collision(
        &self,
        start: Vec3,
        desired: Vec3,
        player_radius: f32,
    ) -> Vec3 {
        let tower_obstacles = self.living_tower_navigation_obstacles();

        resolve_circle_obstacle_collisions(start, desired, player_radius, &tower_obstacles)
    }

    /// Validates and applies a player auto attack against a tower or minion.
    pub(super) fn apply_player_auto_attack(
        &mut self,
        caster_position: Vec3,
        caster_team: TeamSpec,
        target_id: u64,
        damage: f32,
    ) -> Option<(Vec3, f32)> {
        let (target_position, hit_radius) =
            self.target_for_player_auto_attack(caster_team, target_id)?;
        let distance = horizontal_distance(caster_position, target_position);
        if distance > AUTO_ATTACK_RANGE + hit_radius {
            return None;
        }

        self.apply_lane_unit_damage(target_id, damage);
        Some((target_position, hit_radius))
    }

    /// Applies hostile spell damage to enemy minions near an accepted spell impact.
    pub(super) fn apply_spell_damage(
        &mut self,
        caster_team: TeamSpec,
        center: Vec3,
        radius: f32,
        damage: f32,
    ) {
        if !caster_team.is_playable() || damage <= 0.0 {
            return;
        }

        let target_ids = self
            .units
            .iter()
            .filter(|(_, unit)| unit.kind != LaneUnitKind::Tower)
            .filter(|(_, unit)| unit.team != caster_team && unit.health > 0.0)
            .filter(|(_, unit)| {
                horizontal_distance(unit.position, center)
                    <= radius + lane_unit_stats(unit.kind).hit_radius
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();

        for target_id in target_ids {
            self.apply_lane_unit_damage(target_id, damage);
        }
    }

    /// Applies one projectile hit to each enemy minion crossed by a spell segment.
    pub(super) fn apply_spell_damage_on_segment(
        &mut self,
        caster_team: TeamSpec,
        segment_start: Vec3,
        segment_end: Vec3,
        projectile_radius: f32,
        damage: f32,
        hit_target_ids: &mut Vec<u64>,
    ) {
        if !caster_team.is_playable() || damage <= 0.0 {
            return;
        }

        let mut target_ids = self
            .units
            .iter()
            .filter(|(_, unit)| unit.kind != LaneUnitKind::Tower)
            .filter(|(_, unit)| unit.team != caster_team && unit.health > 0.0)
            .filter(|(id, _)| !hit_target_ids.contains(id))
            .filter(|(_, unit)| {
                distance_to_segment_xz(unit.position, segment_start, segment_end)
                    <= projectile_radius + lane_unit_stats(unit.kind).hit_radius
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        target_ids.sort_unstable();

        for target_id in target_ids {
            hit_target_ids.push(target_id);
            self.apply_lane_unit_damage(target_id, damage);
        }
    }

    /// Applies one spell damage tick to enemy minions inside an oriented ground-effect rectangle.
    pub(super) fn apply_spell_damage_in_oriented_rect(
        &mut self,
        caster_team: TeamSpec,
        start: Vec3,
        end: Vec3,
        width: f32,
        damage: f32,
    ) {
        if !caster_team.is_playable() || damage <= 0.0 {
            return;
        }

        let mut target_ids = self
            .units
            .iter()
            .filter(|(_, unit)| unit.kind != LaneUnitKind::Tower)
            .filter(|(_, unit)| unit.team != caster_team && unit.health > 0.0)
            .filter(|(_, unit)| point_in_oriented_rect_xz(unit.position, start, end, width))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        target_ids.sort_unstable();

        for target_id in target_ids {
            self.apply_lane_unit_damage(target_id, damage);
        }
    }

    /// Returns a living enemy minion that can be targeted by one server-authoritative spell.
    pub(super) fn spell_target(
        &self,
        caster_team: TeamSpec,
        target_id: u64,
    ) -> Option<(Vec3, f32)> {
        let target = self.units.get(&target_id)?;
        if !caster_team.is_playable()
            || target.kind == LaneUnitKind::Tower
            || target.team == caster_team
            || target.health <= 0.0
        {
            return None;
        }

        Some((target.position, lane_unit_stats(target.kind).hit_radius))
    }

    /// Applies spell damage to one living enemy minion selected by a spell target id.
    pub(super) fn apply_spell_damage_to_target(
        &mut self,
        caster_team: TeamSpec,
        target_id: u64,
        damage: f32,
    ) -> Option<(Vec3, f32)> {
        if damage <= 0.0 {
            return None;
        }

        let target = self.spell_target(caster_team, target_id)?;
        self.apply_lane_unit_damage(target_id, damage);
        Some(target)
    }

    /// Finds the closest enemy minion that is within a spell's allowed search radius.
    pub(super) fn nearest_enemy_minion_for_spell(
        &self,
        caster_team: TeamSpec,
        range_center: Vec3,
        range: f32,
        priority_origin: Vec3,
    ) -> Option<(u64, Vec3, f32)> {
        if !caster_team.is_playable() {
            return None;
        }

        self.units
            .iter()
            .filter(|(_, unit)| unit.kind != LaneUnitKind::Tower)
            .filter(|(_, unit)| unit.team != caster_team && unit.health > 0.0)
            .filter(|(_, unit)| horizontal_distance(range_center, unit.position) <= range)
            .map(|(id, unit)| {
                (
                    *id,
                    unit.position,
                    lane_unit_stats(unit.kind).hit_radius,
                    horizontal_distance(priority_origin, unit.position),
                )
            })
            .min_by(|left, right| {
                left.3
                    .partial_cmp(&right.3)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(left.0.cmp(&right.0))
            })
            .map(|(id, position, hit_radius, _)| (id, position, hit_radius))
    }

    /// Spawns a lane unit for a server spell timing test and returns its stable id.
    #[cfg(test)]
    pub(super) fn spawn_spell_test_unit(
        &mut self,
        kind: LaneUnitKind,
        team: TeamSpec,
        position: Vec3,
    ) -> u64 {
        let unit_id = self.next_unit_id;
        self.spawn_unit(kind, team, position);
        unit_id
    }

    /// Returns a lane unit's health for a server spell timing test.
    #[cfg(test)]
    pub(super) fn spell_test_unit_health(&self, unit_id: u64) -> Option<f32> {
        self.units.get(&unit_id).map(|unit| unit.health)
    }

    /// Prioritizes an enemy player who damages an allied player under a tower.
    pub(super) fn record_hostile_player_action(
        &mut self,
        attacker_id: u64,
        victim_id: u64,
        players: &ConnectedPlayers,
    ) {
        let Some(attacker) = players.states.get(&attacker_id) else {
            return;
        };
        let Some(victim) = players.states.get(&victim_id) else {
            return;
        };
        let attacker_team = TeamSpec::from(attacker.team);
        let victim_team = TeamSpec::from(victim.team);
        if attacker_team == victim_team || !attacker_team.is_playable() || victim.health <= 0.0 {
            return;
        }

        for unit in self.units.values_mut() {
            if unit.kind != LaneUnitKind::Tower || unit.team != victim_team || unit.health <= 0.0 {
                continue;
            }
            let tower_range = lane_unit_stats(LaneUnitKind::Tower).attack_range;
            if horizontal_distance(unit.position, victim.position) <= tower_range
                && horizontal_distance(unit.position, attacker.position) <= tower_range
            {
                unit.forced_player_target = Some(attacker_id);
            }
        }
    }

    fn start(&mut self) {
        self.started = true;
        self.spawn_tower(TeamSpec::Light);
        self.spawn_tower(TeamSpec::Dark);
        self.queue_wave();
        self.spawn_next_wave_pair();
    }

    fn reset(&mut self) {
        self.units.clear();
        self.next_unit_id = 1;
        self.wave_timer.reset();
        self.wave_spawn_timer.reset();
        self.pending_wave_units.clear();
        self.snapshot_timer.reset();
        self.started = false;
        self.tower_projectiles.clear();
        self.ranged_minion_projectiles.clear();
        self.ranged_minion_auto_attack_visuals.clear();
        self.navigation = LaneNavigationCache::default();
    }

    fn spawn_tower(&mut self, team: TeamSpec) {
        self.spawn_unit(LaneUnitKind::Tower, team, lane_tower_position(team));
    }

    fn queue_wave(&mut self) {
        self.pending_wave_units.extend(MINION_WAVE);
    }

    fn update_pending_wave_spawns(&mut self, delta: std::time::Duration) {
        if self.pending_wave_units.is_empty() {
            return;
        }
        if self.wave_spawn_timer.tick(delta).just_finished() {
            self.spawn_next_wave_pair();
        }
    }

    fn spawn_next_wave_pair(&mut self) {
        let Some(kind) = self.pending_wave_units.pop_front() else {
            return;
        };

        for team in [TeamSpec::Light, TeamSpec::Dark] {
            self.spawn_unit(kind, team, lane_spawn_position(team));
        }
        if self.pending_wave_units.is_empty() {
            self.wave_spawn_timer.reset();
        }
    }

    fn spawn_unit(&mut self, kind: LaneUnitKind, team: TeamSpec, position: Vec3) {
        let id = self.next_unit_id;
        self.next_unit_id = self.next_unit_id.saturating_add(1);
        self.units.insert(
            id,
            ServerLaneUnit {
                kind,
                team,
                position,
                health: lane_unit_stats(kind).max_health,
                attack_cooldown_seconds: 0.0,
                attack_target: None,
                engagement_target: None,
                forced_player_target: None,
                navigation_path: LaneNavigationPath::default(),
            },
        );
    }

    fn update(
        &mut self,
        players: &mut ConnectedPlayers,
        combat_events: &mut ServerCombatNumberEvents,
        delta_seconds: f32,
    ) {
        self.advance_tower_projectiles(players, combat_events, delta_seconds);
        self.advance_ranged_minion_projectiles(players, combat_events, delta_seconds);

        let mut unit_ids = self.units.keys().copied().collect::<Vec<_>>();
        unit_ids.sort_unstable();
        let minion_targets = self.select_minion_targets(&unit_ids, players);
        let mut damage_actions = Vec::new();
        let mut tower_projectiles = Vec::new();

        for unit_id in unit_ids {
            let Some(unit) = self.units.get(&unit_id) else {
                continue;
            };
            if unit.health <= 0.0 {
                continue;
            }

            let kind = unit.kind;
            let team = unit.team;
            let position = unit.position;
            let attack_cooldown = (unit.attack_cooldown_seconds - delta_seconds).max(0.0);

            if kind == LaneUnitKind::Tower {
                let target = self.select_tower_target(unit_id, players);
                let mut target_for_snapshot = target;
                if attack_cooldown <= 0.0
                    && let Some(target) = target
                {
                    let Some(target_position) = self.target_position(target, players) else {
                        continue;
                    };
                    let distance = horizontal_distance(position, target_position);
                    tower_projectiles.push(TowerProjectile {
                        source_id: unit_id,
                        target,
                        remaining_seconds: (distance / TOWER_PROJECTILE_SPEED).clamp(
                            TOWER_PROJECTILE_MIN_TRAVEL_SECONDS,
                            TOWER_PROJECTILE_MAX_TRAVEL_SECONDS,
                        ),
                    });
                    target_for_snapshot = Some(target);
                }

                if let Some(unit) = self.units.get_mut(&unit_id) {
                    unit.attack_cooldown_seconds = if tower_projectiles
                        .last()
                        .is_some_and(|projectile| projectile.source_id == unit_id)
                    {
                        lane_unit_stats(LaneUnitKind::Tower).attack_interval_seconds
                    } else {
                        attack_cooldown
                    };
                    unit.attack_target = target_for_snapshot;
                }
                continue;
            }

            let target = minion_targets.get(&unit_id).copied().flatten();
            let stats = lane_unit_stats(kind);
            let mut moved_position = position;
            let mut attack_target = None;
            let mut next_cooldown = attack_cooldown;
            let mut has_combat_target = false;

            if let Some(target) = target
                && let Some(target_position) = self.target_position(target, players)
            {
                let target_radius = self.target_hit_radius(target);
                has_combat_target = true;
                let target_distance = horizontal_distance(position, target_position);
                let in_attack_range = target_distance <= stats.attack_range + target_radius;
                if in_attack_range {
                    attack_target = Some(target);
                    if attack_cooldown <= 0.0 {
                        if Self::is_ranged_minion(kind) {
                            let travel_seconds =
                                ranged_minion_projectile_travel_seconds(target_distance);
                            self.ranged_minion_projectiles.push(RangedMinionProjectile {
                                target,
                                remaining_seconds: travel_seconds,
                                damage: stats.attack_damage,
                            });
                            self.ranged_minion_auto_attack_visuals.push(
                                RangedMinionAutoAttackVisualEvent {
                                    source_unit_id: unit_id,
                                    team,
                                    target,
                                    start: (position + Vec3::Y * RANGED_MINION_PROJECTILE_HEIGHT)
                                        .into(),
                                    end: (target_position
                                        + Vec3::Y * RANGED_MINION_PROJECTILE_HEIGHT)
                                        .into(),
                                    travel_seconds,
                                },
                            );
                        } else {
                            damage_actions.push(LaneDamageAction {
                                target,
                                amount: stats.attack_damage,
                            });
                        }
                        next_cooldown = stats.attack_interval_seconds;
                    }
                }

                let minimum_distance = stats.hit_radius + target_radius + MINION_SEPARATION_MARGIN;
                if !in_attack_range || target_distance < minimum_distance - 0.001 {
                    let combat_position = self.combat_slot_position(
                        unit_id,
                        team,
                        kind,
                        stats,
                        target,
                        target_position,
                        target_radius,
                        &minion_targets,
                    );
                    let navigation_agent_radius =
                        self.minion_navigation_agent_radius_for_target(stats, target);
                    let navigation_waypoint = self.navigation_waypoint_for_minion(
                        unit_id,
                        position,
                        combat_position,
                        navigation_agent_radius,
                    );
                    moved_position = self.move_minion_to_combat_slot(
                        unit_id,
                        position,
                        navigation_waypoint,
                        stats,
                        target,
                        target_position,
                        target_radius,
                        delta_seconds,
                    );
                } else if let Some(unit) = self.units.get_mut(&unit_id) {
                    // Attackers hold their combat slot instead of circling their target.
                    unit.navigation_path = LaneNavigationPath::default();
                }
            }

            if !has_combat_target {
                let direction = lane_forward_direction(team);
                let navigation_goal = Vec3::new(0.0, position.y, direction.z * LANE_SPAWN_Z);
                let navigation_waypoint = self.navigation_waypoint_for_minion(
                    unit_id,
                    position,
                    navigation_goal,
                    Self::minion_navigation_agent_radius(stats),
                );
                let follows_tower_navigation = self.minion_follows_navigation_route(unit_id);
                moved_position = self.move_minion_toward(
                    unit_id,
                    position,
                    navigation_waypoint,
                    stats,
                    delta_seconds,
                    follows_tower_navigation
                        || self.can_route_around_frontline(unit_id, &minion_targets, players),
                );
            }

            if let Some(unit) = self.units.get_mut(&unit_id) {
                unit.position = moved_position;
                unit.attack_cooldown_seconds = next_cooldown;
                unit.attack_target = attack_target;
                unit.engagement_target = target;
            }
        }

        self.tower_projectiles.extend(tower_projectiles);
        for action in damage_actions {
            self.apply_damage_action(action, players, combat_events);
        }
        self.units.retain(|_, unit| unit.health > 0.0);
    }

    fn is_ranged_minion(kind: LaneUnitKind) -> bool {
        matches!(kind, LaneUnitKind::LargeRangedBox | LaneUnitKind::RangedOrb)
    }

    fn take_ranged_minion_auto_attack_visuals(&mut self) -> Vec<RangedMinionAutoAttackVisualEvent> {
        std::mem::take(&mut self.ranged_minion_auto_attack_visuals)
    }

    fn advance_tower_projectiles(
        &mut self,
        players: &mut ConnectedPlayers,
        combat_events: &mut ServerCombatNumberEvents,
        delta_seconds: f32,
    ) {
        let mut impacts = Vec::new();
        self.tower_projectiles.retain_mut(|projectile| {
            projectile.remaining_seconds -= delta_seconds;
            if projectile.remaining_seconds > 0.0 {
                return true;
            }
            impacts.push(projectile.target);
            false
        });

        for target in impacts {
            self.apply_damage_action(
                LaneDamageAction {
                    target,
                    amount: lane_unit_stats(LaneUnitKind::Tower).attack_damage,
                },
                players,
                combat_events,
            );
        }
    }

    fn advance_ranged_minion_projectiles(
        &mut self,
        players: &mut ConnectedPlayers,
        combat_events: &mut ServerCombatNumberEvents,
        delta_seconds: f32,
    ) {
        let mut impacts = Vec::new();
        self.ranged_minion_projectiles.retain_mut(|projectile| {
            projectile.remaining_seconds -= delta_seconds;
            if projectile.remaining_seconds > 0.0 {
                return true;
            }
            impacts.push(LaneDamageAction {
                target: projectile.target,
                amount: projectile.damage,
            });
            false
        });

        for impact in impacts {
            self.apply_damage_action(impact, players, combat_events);
        }
    }

    fn apply_damage_action(
        &mut self,
        action: LaneDamageAction,
        players: &mut ConnectedPlayers,
        combat_events: &mut ServerCombatNumberEvents,
    ) {
        match action.target {
            NetworkTargetId::Player(player_id) => {
                if let Some(player) = players.states.get_mut(&player_id) {
                    apply_damage(
                        combat_events,
                        player_id,
                        player,
                        action.amount,
                        game_shared::network::NetworkCombatNumberKind::AutoAttack,
                    );
                }
            }
            NetworkTargetId::LaneUnit(unit_id) => {
                self.apply_lane_unit_damage(unit_id, action.amount)
            }
        }
    }

    fn apply_lane_unit_damage(&mut self, unit_id: u64, damage: f32) {
        if damage <= 0.0 {
            return;
        }
        if let Some(unit) = self.units.get_mut(&unit_id) {
            unit.health = (unit.health - damage).max(0.0);
        }
    }

    fn combat_slot_position(
        &self,
        unit_id: u64,
        team: TeamSpec,
        kind: LaneUnitKind,
        stats: game_shared::game::lane::LaneUnitStats,
        target: NetworkTargetId,
        target_position: Vec3,
        target_radius: f32,
        minion_targets: &HashMap<u64, Option<NetworkTargetId>>,
    ) -> Vec3 {
        let minimum_distance = stats.hit_radius + target_radius + MINION_SEPARATION_MARGIN;
        let maximum_distance = stats.attack_range + target_radius - 0.01;
        let preferred_distance = stats.attack_range + target_radius - 0.35;
        let front_row_distance = preferred_distance
            .max(minimum_distance)
            .min(maximum_distance.max(minimum_distance));
        let assignment = self.combat_slot_assignment(unit_id, team, kind, target, minion_targets);
        let queue_spacing = stats.hit_radius * 2.0 + MINION_SEPARATION_MARGIN;
        let maximum_engagement_distance =
            stats.attack_range + target_radius + Self::minion_target_leash_margin(kind) - 0.01;
        let distance = if assignment.uses_attack_ring {
            maximum_distance.max(minimum_distance)
        } else {
            (front_row_distance + assignment.queue_row as f32 * queue_spacing)
                .min(maximum_engagement_distance.max(front_row_distance))
        };
        let approach_side = -lane_forward_direction(team);
        let offset =
            (approach_side * assignment.angle.cos() + Vec3::X * assignment.angle.sin()) * distance;
        let x_limit = (LANE_HALF_WIDTH - stats.hit_radius).max(0.0);

        self.constrain_combat_slot_to_lane(
            target_position,
            offset,
            distance,
            approach_side,
            x_limit,
        )
    }

    fn constrain_combat_slot_to_lane(
        &self,
        target_position: Vec3,
        mut offset: Vec3,
        distance: f32,
        approach_side: Vec3,
        x_limit: f32,
    ) -> Vec3 {
        let desired_x = target_position.x + offset.x;
        if desired_x > x_limit || desired_x < -x_limit {
            let inward_sign = if target_position.x >= 0.0 { -1.0 } else { 1.0 };
            let required_inward_offset = (target_position.x.abs() - x_limit).max(0.0);
            let x_magnitude = offset.x.abs().max(required_inward_offset).min(distance);
            offset.x = inward_sign * x_magnitude;
            let z_magnitude = (distance.powi(2) - x_magnitude.powi(2)).max(0.0).sqrt();
            let z_sign = if offset.z.abs() <= f32::EPSILON {
                approach_side.z.signum()
            } else {
                offset.z.signum()
            };
            offset.z = z_sign * z_magnitude;
        }

        Vec3::new(
            (target_position.x + offset.x).clamp(-x_limit, x_limit),
            target_position.y,
            (target_position.z + offset.z).clamp(-LANE_SPAWN_Z, LANE_SPAWN_Z),
        )
    }

    fn combat_slot_assignment(
        &self,
        unit_id: u64,
        team: TeamSpec,
        kind: LaneUnitKind,
        target: NetworkTargetId,
        minion_targets: &HashMap<u64, Option<NetworkTargetId>>,
    ) -> CombatSlotAssignment {
        let mut attackers = minion_targets
            .iter()
            .filter_map(|(candidate_id, candidate_target)| {
                if *candidate_target != Some(target) {
                    return None;
                }
                let candidate = self.units.get(candidate_id)?;
                if candidate.health <= 0.0
                    || candidate.team != team
                    || candidate.kind == LaneUnitKind::Tower
                    || !Self::shares_combat_band(kind, candidate.kind)
                {
                    return None;
                }
                Some(*candidate_id)
            })
            .collect::<Vec<_>>();
        attackers.sort_unstable();
        let group_index = attackers
            .iter()
            .position(|candidate_id| *candidate_id == unit_id)
            .unwrap_or_default();
        let group_count = attackers.len();
        let team_sign = if team == TeamSpec::Dark { -1.0 } else { 1.0 };

        if group_count <= 1 {
            return CombatSlotAssignment {
                angle: team_sign * Self::singleton_combat_slot_angle(unit_id, kind),
                queue_row: 0,
                uses_attack_ring: false,
            };
        }

        if kind == LaneUnitKind::MeleeBox
            && group_count > MELEE_COMBAT_SLOT_COLUMNS
            && group_index < MELEE_COMBAT_RING_SLOTS
        {
            let ring_slot_count = group_count.min(MELEE_COMBAT_RING_SLOTS);
            return CombatSlotAssignment {
                angle: team_sign * Self::melee_combat_ring_angle(group_index, ring_slot_count),
                queue_row: 0,
                uses_attack_ring: true,
            };
        }

        let columns = Self::combat_slot_columns(kind);
        let queue_row = group_index / columns;
        let active_columns = group_count.min(columns);
        let slot_index = Self::combat_slot_column_for_rank(group_index % columns, active_columns);
        let progress = slot_index as f32 / (active_columns - 1) as f32;
        let angle = -1.2 + progress * 2.4;

        CombatSlotAssignment {
            angle: team_sign * angle,
            queue_row,
            uses_attack_ring: false,
        }
    }

    fn melee_combat_ring_angle(slot_index: usize, slot_count: usize) -> f32 {
        debug_assert!(slot_count > MELEE_COMBAT_SLOT_COLUMNS);
        debug_assert!(slot_count <= MELEE_COMBAT_RING_SLOTS);
        debug_assert!(slot_index < slot_count);

        if slot_index == 0 {
            return 0.0;
        }

        let angular_step = std::f32::consts::TAU / slot_count as f32;
        let offset = slot_index.div_ceil(2);
        let angle = offset as f32 * angular_step;
        if slot_index % 2 == 1 { -angle } else { angle }
    }

    fn shares_combat_band(left: LaneUnitKind, right: LaneUnitKind) -> bool {
        matches!(left, LaneUnitKind::MeleeBox) == matches!(right, LaneUnitKind::MeleeBox)
    }

    fn combat_slot_columns(kind: LaneUnitKind) -> usize {
        match kind {
            LaneUnitKind::MeleeBox => MELEE_COMBAT_SLOT_COLUMNS,
            LaneUnitKind::LargeRangedBox | LaneUnitKind::RangedOrb => RANGED_COMBAT_SLOT_COLUMNS,
            LaneUnitKind::Tower => 1,
        }
    }

    fn combat_slot_column_for_rank(rank: usize, column_count: usize) -> usize {
        debug_assert!(rank < column_count);
        debug_assert!(column_count > 0);

        if column_count.is_multiple_of(2) {
            let left_center = column_count / 2 - 1;
            let right_center = column_count / 2;
            let offset = rank / 2;
            return if rank.is_multiple_of(2) {
                left_center.saturating_sub(offset)
            } else {
                (right_center + offset).min(column_count - 1)
            };
        }

        let center = column_count / 2;
        if rank == 0 {
            return center;
        }

        let offset = rank.div_ceil(2);
        if rank % 2 == 1 {
            center.saturating_sub(offset)
        } else {
            (center + offset).min(column_count - 1)
        }
    }

    fn singleton_combat_slot_angle(unit_id: u64, kind: LaneUnitKind) -> f32 {
        let pair_index = unit_id.saturating_sub(1) / 2;
        let angles = match kind {
            LaneUnitKind::MeleeBox => &MELEE_SINGLETON_COMBAT_SLOT_ANGLES[..],
            LaneUnitKind::LargeRangedBox | LaneUnitKind::RangedOrb => {
                &RANGED_SINGLETON_COMBAT_SLOT_ANGLES[..]
            }
            LaneUnitKind::Tower => return 0.0,
        };

        angles[(pair_index % angles.len() as u64) as usize]
    }

    fn minion_target_leash_margin(kind: LaneUnitKind) -> f32 {
        match kind {
            LaneUnitKind::MeleeBox => MINION_MELEE_TARGET_LEASH_MARGIN,
            LaneUnitKind::LargeRangedBox | LaneUnitKind::RangedOrb => {
                MINION_RANGED_TARGET_LEASH_MARGIN
            }
            LaneUnitKind::Tower => 0.0,
        }
    }

    fn minion_wave_assist_radius(kind: LaneUnitKind) -> f32 {
        match kind {
            LaneUnitKind::MeleeBox => MINION_WAVE_ASSIST_RADIUS,
            LaneUnitKind::LargeRangedBox | LaneUnitKind::RangedOrb => {
                RANGED_MINION_WAVE_ASSIST_RADIUS
            }
            LaneUnitKind::Tower => 0.0,
        }
    }

    fn is_within_minion_trigger_range(
        minion_position: Vec3,
        target_position: Vec3,
        target_radius: f32,
    ) -> bool {
        horizontal_distance(minion_position, target_position)
            <= MINION_TARGET_TRIGGER_RANGE + target_radius
    }

    fn target_is_alive(&self, target: NetworkTargetId, players: &ConnectedPlayers) -> bool {
        match target {
            NetworkTargetId::Player(player_id) => players
                .states
                .get(&player_id)
                .is_some_and(|player| player.health > 0.0),
            NetworkTargetId::LaneUnit(unit_id) => self
                .units
                .get(&unit_id)
                .is_some_and(|unit| unit.health > 0.0),
        }
    }

    fn has_materially_closer_local_target(
        &self,
        unit_id: u64,
        current_target: NetworkTargetId,
        players: &ConnectedPlayers,
    ) -> bool {
        let NetworkTargetId::LaneUnit(current_target_id) = current_target else {
            return false;
        };
        if !self.target_is_alive(current_target, players) {
            return false;
        }
        let Some(unit) = self.units.get(&unit_id) else {
            return false;
        };
        if unit.kind != LaneUnitKind::MeleeBox {
            return false;
        }

        let Some(current_target_position) = self.target_position(current_target, players) else {
            return false;
        };
        let stats = lane_unit_stats(unit.kind);
        let current_target_distance = horizontal_distance(unit.position, current_target_position);
        if current_target_distance <= stats.attack_range + self.target_hit_radius(current_target) {
            return false;
        }

        let closest_player_distance = players
            .states
            .values()
            .filter(|player| {
                let player_team = TeamSpec::from(player.team);
                player.health > 0.0
                    && player_team != unit.team
                    && player_team.is_playable()
                    && Self::is_within_minion_trigger_range(
                        unit.position,
                        player.position,
                        LANE_PLAYER_COLLISION_RADIUS,
                    )
            })
            .map(|player| horizontal_distance(unit.position, player.position))
            .min_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        let closest_minion_distance = self
            .units
            .iter()
            .filter(|(target_id, target)| {
                **target_id != unit_id
                    && **target_id != current_target_id
                    && target.health > 0.0
                    && target.team != unit.team
                    && target.kind != LaneUnitKind::Tower
            })
            .filter(|(_, target)| {
                Self::is_within_minion_trigger_range(
                    unit.position,
                    target.position,
                    lane_unit_stats(target.kind).hit_radius,
                )
            })
            .map(|(_, target)| horizontal_distance(unit.position, target.position))
            .min_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));

        closest_player_distance
            .into_iter()
            .chain(closest_minion_distance)
            .min_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
            .is_some_and(|closest_distance| {
                closest_distance + MELEE_MINION_RETARGET_DISTANCE_ADVANTAGE
                    <= current_target_distance
            })
    }

    fn can_route_around_frontline(
        &self,
        unit_id: u64,
        minion_targets: &HashMap<u64, Option<NetworkTargetId>>,
        players: &ConnectedPlayers,
    ) -> bool {
        let Some(unit) = self.units.get(&unit_id) else {
            return false;
        };

        self.units.iter().any(|(ally_id, ally)| {
            *ally_id != unit_id
                && ally.team == unit.team
                && ally.kind != LaneUnitKind::Tower
                && ally.health > 0.0
                && horizontal_distance(unit.position, ally.position)
                    <= Self::minion_wave_assist_radius(unit.kind)
                && minion_targets
                    .get(ally_id)
                    .copied()
                    .flatten()
                    .is_some_and(|target| {
                        self.is_active_frontline_target(unit.team, target, players)
                    })
        })
    }

    /// Returns whether a minion is currently following a route around a living tower.
    fn minion_follows_navigation_route(&self, unit_id: u64) -> bool {
        self.units.get(&unit_id).is_some_and(|unit| {
            unit.navigation_path.route_resolved && !unit.navigation_path.waypoints.is_empty()
        })
    }

    fn is_active_frontline_target(
        &self,
        team: TeamSpec,
        target: NetworkTargetId,
        players: &ConnectedPlayers,
    ) -> bool {
        match target {
            NetworkTargetId::Player(player_id) => {
                players.states.get(&player_id).is_some_and(|player| {
                    player.health > 0.0
                        && TeamSpec::from(player.team).is_playable()
                        && TeamSpec::from(player.team) != team
                })
            }
            NetworkTargetId::LaneUnit(target_id) => self
                .units
                .get(&target_id)
                .is_some_and(|target| target.health > 0.0 && target.team != team),
        }
    }

    fn navigation_waypoint_for_minion(
        &mut self,
        unit_id: u64,
        position: Vec3,
        goal: Vec3,
        agent_radius: f32,
    ) -> Vec3 {
        // The mesh is only needed close to a tower. This keeps newly spawned waves in their
        // centered marching column instead of steering toward a far-away obstacle immediately.
        if !self.minion_route_needs_tower_navigation(position, goal, agent_radius) {
            if let Some(unit) = self.units.get_mut(&unit_id) {
                unit.navigation_path = LaneNavigationPath::default();
            }
            return goal;
        }

        let obstacle_revision = self.navigation_obstacle_revision();
        let needs_replan = self.units.get(&unit_id).is_none_or(|unit| {
            !unit.navigation_path.route_resolved
                || unit.navigation_path.obstacle_revision != Some(obstacle_revision)
                || unit.navigation_path.goal.is_none_or(|previous_goal| {
                    horizontal_distance(previous_goal, goal) > NAVIGATION_GOAL_REPLAN_DISTANCE
                })
        });

        if needs_replan {
            let via = self.minion_navigation_side_via(unit_id, position, goal, agent_radius);
            let (obstacle_revision, path) = if let Some(via) = via {
                let (obstacle_revision, approach) =
                    self.minion_navigation_path_for_mover(position, via, agent_radius);
                let (_, departure) = self.minion_navigation_path_for_mover(via, goal, agent_radius);
                let path = approach
                    .zip(departure)
                    .map(|(mut approach, departure)| {
                        approach.extend(departure);
                        approach
                    })
                    .or_else(|| {
                        self.minion_navigation_path_for_mover(position, goal, agent_radius)
                            .1
                    });
                (obstacle_revision, path)
            } else {
                self.minion_navigation_path_for_mover(position, goal, agent_radius)
            };
            if let Some(unit) = self.units.get_mut(&unit_id) {
                unit.navigation_path.obstacle_revision = Some(obstacle_revision);
                unit.navigation_path.goal = Some(goal);
                unit.navigation_path.route_resolved = path.is_some();
                unit.navigation_path.waypoints = path.unwrap_or_default().into();
            }
        }

        let Some(unit) = self.units.get_mut(&unit_id) else {
            return goal;
        };
        while unit
            .navigation_path
            .waypoints
            .front()
            .is_some_and(|waypoint| {
                horizontal_distance(position, waypoint.position)
                    <= if waypoint.requires_precise_arrival {
                        NAVIGATION_RECOVERY_WAYPOINT_REACHED_DISTANCE
                    } else {
                        NAVIGATION_WAYPOINT_REACHED_DISTANCE
                    }
            })
        {
            unit.navigation_path.waypoints.pop_front();
        }

        unit.navigation_path
            .waypoints
            .front()
            .map(|waypoint| waypoint.position)
            .unwrap_or(goal)
    }

    fn minion_navigation_agent_radius(stats: game_shared::game::lane::LaneUnitStats) -> f32 {
        stats.hit_radius + MINION_SEPARATION_MARGIN + MINION_COLLISION_SKIN
    }

    fn minion_navigation_agent_radius_for_target(
        &self,
        stats: game_shared::game::lane::LaneUnitStats,
        target: NetworkTargetId,
    ) -> f32 {
        let target_is_tower = matches!(target, NetworkTargetId::LaneUnit(target_id) if self
            .units
            .get(&target_id)
            .is_some_and(|unit| unit.kind == LaneUnitKind::Tower));

        if target_is_tower {
            // Target clearance is enforced separately by `move_minion_to_combat_slot`. Baking
            // its interpersonal separation margin into the mesh would project melee attackers
            // just outside their legal attack range.
            stats.hit_radius
        } else {
            Self::minion_navigation_agent_radius(stats)
        }
    }

    fn minion_navigation_path_for_mover(
        &mut self,
        start: Vec3,
        goal: Vec3,
        agent_radius: f32,
    ) -> (u64, Option<Vec<MinionNavigationWaypoint>>) {
        let (obstacle_revision, path) =
            self.navigation_path_with_projection_for_mover(start, goal, agent_radius);
        (
            obstacle_revision,
            path.map(|path| {
                let mut waypoints = Vec::with_capacity(path.waypoints.len() + 1);
                if horizontal_distance(start, path.start)
                    > NAVIGATION_RECOVERY_WAYPOINT_REACHED_DISTANCE
                {
                    waypoints.push(MinionNavigationWaypoint {
                        position: path.start,
                        requires_precise_arrival: true,
                    });
                }
                waypoints.extend(path.waypoints.into_iter().map(|position| {
                    MinionNavigationWaypoint {
                        position,
                        requires_precise_arrival: false,
                    }
                }));
                waypoints
            }),
        )
    }

    fn minion_route_needs_tower_navigation(
        &self,
        position: Vec3,
        goal: Vec3,
        agent_radius: f32,
    ) -> bool {
        let path = Vec3::new(goal.x - position.x, 0.0, goal.z - position.z);
        let path_length_squared = path.length_squared();
        if path_length_squared <= f32::EPSILON {
            return false;
        }

        self.units.values().any(|tower| {
            if tower.kind != LaneUnitKind::Tower || tower.health <= 0.0 {
                return false;
            }

            let from_start = Vec3::new(
                tower.position.x - position.x,
                0.0,
                tower.position.z - position.z,
            );
            let progress = (from_start.dot(path) / path_length_squared).clamp(0.0, 1.0);
            let nearest = position + path * progress;
            let tower_clearance = lane_unit_stats(LaneUnitKind::Tower).hit_radius + agent_radius;
            horizontal_distance(nearest, tower.position) <= tower_clearance
                && horizontal_distance(position, nearest)
                    <= MINION_NAVIGATION_ACTIVATION_DISTANCE + tower_clearance
        })
    }

    fn minion_navigation_side_via(
        &self,
        unit_id: u64,
        position: Vec3,
        goal: Vec3,
        agent_radius: f32,
    ) -> Option<Vec3> {
        let path = Vec3::new(goal.x - position.x, 0.0, goal.z - position.z);
        let path_length_squared = path.length_squared();
        if path_length_squared <= f32::EPSILON {
            return None;
        }
        let team = self.units.get(&unit_id)?.team;

        let tower = self
            .units
            .values()
            .filter(|tower| tower.kind == LaneUnitKind::Tower && tower.health > 0.0)
            .filter_map(|tower| {
                let from_start = Vec3::new(
                    tower.position.x - position.x,
                    0.0,
                    tower.position.z - position.z,
                );
                let progress = (from_start.dot(path) / path_length_squared).clamp(0.0, 1.0);
                let nearest = position + path * progress;
                let clearance = lane_unit_stats(LaneUnitKind::Tower).hit_radius + agent_radius;
                (horizontal_distance(nearest, tower.position) <= clearance
                    && horizontal_distance(position, nearest)
                        <= MINION_NAVIGATION_ACTIVATION_DISTANCE + clearance)
                    .then_some((progress, tower))
            })
            .min_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })?
            .1;

        let side_rank = self
            .units
            .iter()
            .filter(|(candidate_id, candidate)| {
                **candidate_id < unit_id
                    && candidate.team == team
                    && candidate.kind != LaneUnitKind::Tower
            })
            .count();
        let side = if side_rank % 2 == 0 { 1.0 } else { -1.0 };
        let clearance = lane_unit_stats(LaneUnitKind::Tower).hit_radius
            + agent_radius
            + LANE_NAVIGATION_CLEARANCE;
        let x_limit = (LANE_HALF_WIDTH - agent_radius - LANE_NAVIGATION_CLEARANCE).max(0.0);

        Some(Vec3::new(
            (tower.position.x + side * clearance).clamp(-x_limit, x_limit),
            tower.position.y,
            tower.position.z,
        ))
    }

    fn move_minion_toward(
        &self,
        unit_id: u64,
        position: Vec3,
        target_position: Vec3,
        stats: game_shared::game::lane::LaneUnitStats,
        delta_seconds: f32,
        allow_minion_detour: bool,
    ) -> Vec3 {
        let desired_position = Self::step_toward(
            position,
            target_position,
            stats.movement_speed * delta_seconds,
        );
        self.move_minion_around_blockers(
            unit_id,
            position,
            desired_position,
            stats.hit_radius,
            None,
            None,
            allow_minion_detour,
        )
    }

    fn move_minion_to_combat_slot(
        &self,
        unit_id: u64,
        position: Vec3,
        combat_position: Vec3,
        stats: game_shared::game::lane::LaneUnitStats,
        target: NetworkTargetId,
        target_position: Vec3,
        target_radius: f32,
        delta_seconds: f32,
    ) -> Vec3 {
        let target_offset = Vec3::new(
            position.x - target_position.x,
            0.0,
            position.z - target_position.z,
        );
        let current_distance = target_offset.length();
        let desired_offset = Vec3::new(
            combat_position.x - target_position.x,
            0.0,
            combat_position.z - target_position.z,
        );
        let minimum_distance = stats.hit_radius + target_radius + MINION_SEPARATION_MARGIN;
        let max_distance = stats.movement_speed * delta_seconds;
        let at_target_boundary = (current_distance - minimum_distance).abs() <= 0.001
            && current_distance > f32::EPSILON
            && desired_offset.length_squared() > f32::EPSILON;
        let ignored_unit_id = match target {
            NetworkTargetId::LaneUnit(target_id) => Some(target_id),
            NetworkTargetId::Player(_) => None,
        };

        let desired_position = if current_distance < minimum_distance - 0.001 {
            let escape_direction = if current_distance > f32::EPSILON {
                target_offset / current_distance
            } else {
                desired_offset.normalize_or_zero()
            };
            let escape_distance = (current_distance + max_distance).min(minimum_distance);
            target_position + escape_direction * escape_distance
        } else if at_target_boundary {
            let current_direction = target_offset / current_distance;
            let desired_direction = desired_offset.normalize();
            let turn_angle = (current_direction.x * desired_direction.z
                - current_direction.z * desired_direction.x)
                .atan2(current_direction.dot(desired_direction));
            let max_turn_angle = max_distance / current_distance;
            let step_angle = turn_angle.clamp(-max_turn_angle, max_turn_angle);
            let (sin, cos) = step_angle.sin_cos();
            let rotated_direction = Vec3::new(
                current_direction.x * cos - current_direction.z * sin,
                0.0,
                current_direction.x * sin + current_direction.z * cos,
            );
            target_position + rotated_direction * current_distance
        } else {
            let desired_position = Self::step_toward(position, combat_position, max_distance);
            Self::limit_movement_by_target_clearance(
                position,
                desired_position,
                target_position,
                minimum_distance,
            )
        };

        let moved_position = self.move_minion_around_blockers(
            unit_id,
            position,
            desired_position,
            stats.hit_radius,
            ignored_unit_id,
            Some((target_position, minimum_distance)),
            true,
        );
        if current_distance >= minimum_distance - 0.001
            && horizontal_distance(moved_position, target_position) < minimum_distance - 0.001
        {
            position
        } else {
            moved_position
        }
    }

    fn move_minion_around_blockers(
        &self,
        unit_id: u64,
        position: Vec3,
        desired_position: Vec3,
        hit_radius: f32,
        ignored_unit_id: Option<u64>,
        target_clearance: Option<(Vec3, f32)>,
        allow_minion_detour: bool,
    ) -> Vec3 {
        let direct_position = self.limit_minion_movement_by_spacing(
            unit_id,
            position,
            desired_position,
            hit_radius,
            ignored_unit_id,
        );
        if horizontal_distance(direct_position, desired_position) <= 0.001 {
            return direct_position;
        }

        if !allow_minion_detour
            && !self.tower_blocks_minion_movement(
                unit_id,
                position,
                desired_position,
                hit_radius,
                ignored_unit_id,
            )
        {
            return direct_position;
        }

        let desired_offset = Vec3::new(
            desired_position.x - position.x,
            0.0,
            desired_position.z - position.z,
        );
        let desired_distance = desired_offset.length();
        if desired_distance <= f32::EPSILON {
            return direct_position;
        }

        let forward = desired_offset / desired_distance;
        let lateral = Vec3::new(-forward.z, 0.0, forward.x);
        let preferred_side = self
            .detour_side_away_from_blocker(
                unit_id,
                position,
                desired_position,
                hit_radius,
                ignored_unit_id,
                lateral,
            )
            .unwrap_or_else(|| self.default_minion_detour_side(unit_id, position, lateral));
        let mut best_position = direct_position;
        let mut best_score = Self::blocker_detour_score(position, direct_position, forward);

        for strength in [
            preferred_side * MINION_BLOCKER_DETOUR_STRENGTH,
            -preferred_side * MINION_BLOCKER_DETOUR_STRENGTH,
            preferred_side * MINION_BLOCKER_DETOUR_STRENGTH * 2.0,
            -preferred_side * MINION_BLOCKER_DETOUR_STRENGTH * 2.0,
            preferred_side * MINION_BLOCKER_DETOUR_STRENGTH * 4.0,
            -preferred_side * MINION_BLOCKER_DETOUR_STRENGTH * 4.0,
        ] {
            let direction = (forward + lateral * strength).normalize_or_zero();
            if direction.length_squared() <= f32::EPSILON {
                continue;
            }
            let mut candidate_position =
                Self::clamp_minion_to_lane(position + direction * desired_distance, hit_radius);
            if let Some((target_position, minimum_distance)) = target_clearance {
                candidate_position = Self::limit_movement_by_target_clearance(
                    position,
                    candidate_position,
                    target_position,
                    minimum_distance,
                );
            }
            candidate_position = self.limit_minion_movement_by_spacing(
                unit_id,
                position,
                candidate_position,
                hit_radius,
                ignored_unit_id,
            );

            let candidate_score = Self::blocker_detour_score(position, candidate_position, forward);
            if candidate_score > best_score + 0.001 {
                best_position = candidate_position;
                best_score = candidate_score;
            }
        }

        // At the exact edge of a tower's clearance circle, every movement with a forward
        // component still points into the obstacle. Take a lateral step first, then resume the
        // forward-biased candidates on the next update.
        for direction in [lateral * preferred_side, -lateral * preferred_side] {
            let mut candidate_position =
                Self::clamp_minion_to_lane(position + direction * desired_distance, hit_radius);
            if let Some((target_position, minimum_distance)) = target_clearance {
                candidate_position = Self::limit_movement_by_target_clearance(
                    position,
                    candidate_position,
                    target_position,
                    minimum_distance,
                );
            }
            candidate_position = self.limit_minion_movement_by_spacing(
                unit_id,
                position,
                candidate_position,
                hit_radius,
                ignored_unit_id,
            );

            let candidate_score = Self::blocker_detour_score(position, candidate_position, forward);
            if candidate_score > best_score + 0.001 {
                best_position = candidate_position;
                best_score = candidate_score;
            }
        }

        if horizontal_distance(best_position, position) > 0.001 {
            return best_position;
        }

        // A rear minion can be wedged between two friendly units at their exact spacing
        // boundary. Step back out of that pocket before resuming the forward detour search.
        for direction in [
            (-forward + lateral * preferred_side).normalize_or_zero(),
            (-forward - lateral * preferred_side).normalize_or_zero(),
            -forward,
        ] {
            let mut candidate_position =
                Self::clamp_minion_to_lane(position + direction * desired_distance, hit_radius);
            if let Some((target_position, minimum_distance)) = target_clearance {
                candidate_position = Self::limit_movement_by_target_clearance(
                    position,
                    candidate_position,
                    target_position,
                    minimum_distance,
                );
            }
            candidate_position = self.limit_minion_movement_by_spacing(
                unit_id,
                position,
                candidate_position,
                hit_radius,
                ignored_unit_id,
            );

            if horizontal_distance(candidate_position, position) > 0.001 {
                return candidate_position;
            }
        }

        best_position
    }

    fn detour_side_away_from_blocker(
        &self,
        unit_id: u64,
        position: Vec3,
        desired_position: Vec3,
        hit_radius: f32,
        ignored_unit_id: Option<u64>,
        lateral: Vec3,
    ) -> Option<f32> {
        self.units
            .iter()
            .filter(|(other_id, other)| {
                **other_id != unit_id && Some(**other_id) != ignored_unit_id && other.health > 0.0
            })
            .filter_map(|(other_id, other)| {
                let minimum_distance = hit_radius
                    + lane_unit_stats(other.kind).hit_radius
                    + MINION_SEPARATION_MARGIN
                    + MINION_COLLISION_SKIN;
                let distance = distance_to_segment_xz(other.position, position, desired_position);
                (distance <= minimum_distance).then_some((
                    horizontal_distance(position, other.position),
                    *other_id,
                    Vec3::new(
                        position.x - other.position.x,
                        0.0,
                        position.z - other.position.z,
                    )
                    .dot(lateral),
                ))
            })
            .min_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.1.cmp(&right.1))
            })
            .and_then(|(_, _, lateral_clearance)| {
                (lateral_clearance.abs() > 0.01).then_some(lateral_clearance.signum())
            })
    }

    fn default_minion_detour_side(&self, unit_id: u64, position: Vec3, lateral: Vec3) -> f32 {
        if position.x.abs() > 0.05 && lateral.x.abs() > f32::EPSILON {
            return (position.x.signum() * lateral.x.signum()).signum();
        }

        let Some(unit) = self.units.get(&unit_id) else {
            return 1.0;
        };
        let team_rank = self
            .units
            .iter()
            .filter(|(candidate_id, candidate)| {
                **candidate_id < unit_id
                    && candidate.team == unit.team
                    && candidate.kind != LaneUnitKind::Tower
            })
            .count();

        if team_rank % 2 == 0 { 1.0 } else { -1.0 }
    }

    fn tower_blocks_minion_movement(
        &self,
        unit_id: u64,
        position: Vec3,
        desired_position: Vec3,
        hit_radius: f32,
        ignored_unit_id: Option<u64>,
    ) -> bool {
        let movement = Vec3::new(
            desired_position.x - position.x,
            0.0,
            desired_position.z - position.z,
        );
        if movement.length_squared() <= f32::EPSILON {
            return false;
        }

        self.units.iter().any(|(tower_id, tower)| {
            if *tower_id == unit_id
                || Some(*tower_id) == ignored_unit_id
                || tower.kind != LaneUnitKind::Tower
                || tower.health <= 0.0
            {
                return false;
            }

            let relative_position = Vec3::new(
                position.x - tower.position.x,
                0.0,
                position.z - tower.position.z,
            );
            let minimum_distance = hit_radius
                + lane_unit_stats(LaneUnitKind::Tower).hit_radius
                + MINION_SEPARATION_MARGIN;
            relative_position.dot(movement) < 0.0
                && distance_to_segment_xz(tower.position, position, desired_position)
                    <= minimum_distance
        })
    }

    fn blocker_detour_score(position: Vec3, candidate_position: Vec3, forward: Vec3) -> f32 {
        let movement = Vec3::new(
            candidate_position.x - position.x,
            0.0,
            candidate_position.z - position.z,
        );
        movement.dot(forward) + movement.length() * MINION_BLOCKER_DETOUR_PROGRESS_WEIGHT
    }

    fn clamp_minion_to_lane(position: Vec3, hit_radius: f32) -> Vec3 {
        let x_limit = (LANE_HALF_WIDTH - hit_radius).max(0.0);
        Vec3::new(
            position.x.clamp(-x_limit, x_limit),
            position.y,
            position.z.clamp(-LANE_SPAWN_Z, LANE_SPAWN_Z),
        )
    }

    fn step_toward(position: Vec3, target_position: Vec3, max_distance: f32) -> Vec3 {
        let offset = Vec3::new(
            target_position.x - position.x,
            0.0,
            target_position.z - position.z,
        );
        let distance = offset.length();
        if distance <= f32::EPSILON || distance <= max_distance {
            return target_position;
        }

        position + offset * (max_distance / distance)
    }

    fn limit_movement_by_target_clearance(
        position: Vec3,
        desired_position: Vec3,
        target_position: Vec3,
        minimum_distance: f32,
    ) -> Vec3 {
        let movement = Vec3::new(
            desired_position.x - position.x,
            0.0,
            desired_position.z - position.z,
        );
        let movement_length_squared = movement.length_squared();
        if movement_length_squared <= f32::EPSILON {
            return position;
        }

        let relative_position = Vec3::new(
            position.x - target_position.x,
            0.0,
            position.z - target_position.z,
        );
        let separation_squared = relative_position.length_squared() - minimum_distance.powi(2);
        let movement_toward_target = relative_position.dot(movement);
        if separation_squared <= 0.0 {
            return if movement_toward_target >= 0.0 {
                desired_position
            } else {
                position
            };
        }
        if movement_toward_target >= 0.0 {
            return desired_position;
        }

        let discriminant =
            movement_toward_target.powi(2) - movement_length_squared * separation_squared;
        if discriminant < 0.0 {
            return desired_position;
        }
        let collision_fraction =
            (-movement_toward_target - discriminant.sqrt()) / movement_length_squared;
        if (0.0..=1.0).contains(&collision_fraction) {
            position + movement * collision_fraction
        } else {
            desired_position
        }
    }

    fn limit_minion_movement_by_spacing(
        &self,
        unit_id: u64,
        position: Vec3,
        desired_position: Vec3,
        hit_radius: f32,
        ignored_unit_id: Option<u64>,
    ) -> Vec3 {
        let movement = Vec3::new(
            desired_position.x - position.x,
            0.0,
            desired_position.z - position.z,
        );
        let movement_length_squared = movement.length_squared();
        if movement_length_squared <= f32::EPSILON {
            return position;
        }

        let mut allowed_fraction: f32 = 1.0;
        for (other_id, other) in &self.units {
            if *other_id == unit_id || Some(*other_id) == ignored_unit_id || other.health <= 0.0 {
                continue;
            }

            let minimum_distance = hit_radius
                + lane_unit_stats(other.kind).hit_radius
                + MINION_SEPARATION_MARGIN
                + MINION_COLLISION_SKIN;
            let relative_position = Vec3::new(
                position.x - other.position.x,
                0.0,
                position.z - other.position.z,
            );
            let separation_squared = relative_position.length_squared() - minimum_distance.powi(2);
            let movement_toward_other = relative_position.dot(movement);

            if separation_squared <= 0.0 {
                if relative_position.length_squared() <= f32::EPSILON {
                    if unit_id > *other_id {
                        allowed_fraction = 0.0;
                    }
                } else if movement_toward_other < 0.0 {
                    allowed_fraction = 0.0;
                }
                continue;
            }
            if movement_toward_other >= 0.0 {
                continue;
            }

            let discriminant =
                movement_toward_other.powi(2) - movement_length_squared * separation_squared;
            if discriminant < 0.0 {
                continue;
            }

            let collision_fraction =
                (-movement_toward_other - discriminant.sqrt()) / movement_length_squared;
            if collision_fraction > 0.0 {
                allowed_fraction = allowed_fraction.min(collision_fraction);
            }
        }

        let resolved_position = position + movement * allowed_fraction;
        if self.minion_position_overlaps_blocker(
            unit_id,
            resolved_position,
            hit_radius,
            ignored_unit_id,
        ) {
            position
        } else {
            resolved_position
        }
    }

    fn minion_position_overlaps_blocker(
        &self,
        unit_id: u64,
        position: Vec3,
        hit_radius: f32,
        ignored_unit_id: Option<u64>,
    ) -> bool {
        self.units.iter().any(|(other_id, other)| {
            if *other_id == unit_id || Some(*other_id) == ignored_unit_id || other.health <= 0.0 {
                return false;
            }

            let minimum_distance =
                hit_radius + lane_unit_stats(other.kind).hit_radius + MINION_SEPARATION_MARGIN;
            horizontal_distance(position, other.position) + 0.0001 < minimum_distance
        })
    }

    fn select_minion_targets(
        &self,
        unit_ids: &[u64],
        players: &ConnectedPlayers,
    ) -> HashMap<u64, Option<NetworkTargetId>> {
        let mut planned_targets = HashMap::with_capacity(unit_ids.len());

        // Preserve valid engagements before assigning new targets so nearby minions can
        // distribute across the same opposing wave instead of all selecting one unit.
        for unit_id in unit_ids {
            let Some(unit) = self.units.get(unit_id) else {
                continue;
            };
            if unit.health <= 0.0 {
                planned_targets.insert(*unit_id, None);
                continue;
            }
            if let Some(target) = self.valid_engagement_target(*unit_id, players) {
                planned_targets.insert(*unit_id, Some(target));
            }
        }

        for unit_id in unit_ids {
            if planned_targets.contains_key(unit_id) {
                continue;
            }
            let Some(unit) = self.units.get(unit_id) else {
                continue;
            };
            if unit.health <= 0.0 {
                planned_targets.insert(*unit_id, None);
                continue;
            }
            let target = self.select_minion_target_with_plan(*unit_id, players, &planned_targets);
            planned_targets.insert(*unit_id, target);
        }

        planned_targets
    }

    #[cfg(test)]
    fn select_minion_target(
        &self,
        unit_id: u64,
        players: &ConnectedPlayers,
    ) -> Option<NetworkTargetId> {
        self.select_minion_target_with_plan(unit_id, players, &HashMap::new())
    }

    fn select_minion_target_with_plan(
        &self,
        unit_id: u64,
        players: &ConnectedPlayers,
        planned_targets: &HashMap<u64, Option<NetworkTargetId>>,
    ) -> Option<NetworkTargetId> {
        let unit = self.units.get(&unit_id)?;
        if unit.health <= 0.0 || unit.kind == LaneUnitKind::Tower {
            return None;
        }

        let needs_closer_local_target = unit.engagement_target.is_some_and(|target| {
            self.has_materially_closer_local_target(unit_id, target, players)
        });

        if let Some(target) = self.valid_engagement_target(unit_id, players) {
            return Some(target);
        }

        let player_target = players
            .states
            .iter()
            .filter(|(_, player)| {
                let player_team = TeamSpec::from(player.team);
                player.health > 0.0
                    && player_team != unit.team
                    && player_team.is_playable()
                    && Self::is_within_minion_trigger_range(
                        unit.position,
                        player.position,
                        LANE_PLAYER_COLLISION_RADIUS,
                    )
            })
            .map(|(player_id, player)| {
                (
                    *player_id,
                    horizontal_distance(unit.position, player.position),
                )
            })
            .min_by(Self::compare_distance_then_id);
        let minion_target = self
            .units
            .iter()
            .filter(|(target_id, target)| **target_id != unit_id && target.health > 0.0)
            .filter(|(_, target)| target.team != unit.team)
            .filter(|(_, target)| target.kind != LaneUnitKind::Tower)
            .filter(|(_, target)| {
                Self::is_within_minion_trigger_range(
                    unit.position,
                    target.position,
                    lane_unit_stats(target.kind).hit_radius,
                )
            })
            .map(|(target_id, target)| {
                (
                    *target_id,
                    horizontal_distance(unit.position, target.position),
                )
            })
            .min_by(Self::compare_distance_then_id);

        // A player that is the first hostile reached keeps priority over the wave plan.
        if let Some((player_id, player_distance)) = player_target
            && minion_target.is_none_or(|(_, minion_distance)| player_distance <= minion_distance)
        {
            return Some(NetworkTargetId::Player(player_id));
        }

        if needs_closer_local_target && let Some((minion_id, _)) = minion_target {
            return Some(NetworkTargetId::LaneUnit(minion_id));
        }

        if let Some(target) = self.select_wave_assist_target(unit_id, planned_targets) {
            return Some(target);
        }

        if let Some((minion_id, _)) = minion_target {
            return Some(NetworkTargetId::LaneUnit(minion_id));
        }

        self.units
            .iter()
            .filter(|(target_id, target)| **target_id != unit_id && target.health > 0.0)
            .filter(|(_, target)| target.team != unit.team)
            .filter(|(_, target)| target.kind == LaneUnitKind::Tower)
            .filter(|(_, target)| {
                Self::is_within_minion_trigger_range(
                    unit.position,
                    target.position,
                    lane_unit_stats(target.kind).hit_radius,
                )
            })
            .map(|(target_id, target)| {
                (
                    *target_id,
                    horizontal_distance(unit.position, target.position),
                )
            })
            .min_by(Self::compare_distance_then_id)
            .map(|(target_id, _)| NetworkTargetId::LaneUnit(target_id))
    }

    fn valid_engagement_target(
        &self,
        unit_id: u64,
        players: &ConnectedPlayers,
    ) -> Option<NetworkTargetId> {
        let unit = self.units.get(&unit_id)?;
        if unit.health <= 0.0 || unit.kind == LaneUnitKind::Tower {
            return None;
        }
        let target = unit.engagement_target?;
        if !self.is_valid_minion_target(unit_id, target, players)
            || self.has_materially_closer_local_target(unit_id, target, players)
        {
            return None;
        }

        Some(target)
    }

    fn select_wave_assist_target(
        &self,
        unit_id: u64,
        planned_targets: &HashMap<u64, Option<NetworkTargetId>>,
    ) -> Option<NetworkTargetId> {
        let unit = self.units.get(&unit_id)?;
        let (anchor_target_id, _) = self
            .units
            .iter()
            .filter(|(ally_id, ally)| {
                **ally_id != unit_id
                    && ally.team == unit.team
                    && ally.kind != LaneUnitKind::Tower
                    && ally.health > 0.0
                    && horizontal_distance(unit.position, ally.position)
                        <= Self::minion_wave_assist_radius(unit.kind)
            })
            .filter_map(|(ally_id, ally)| {
                let target = match planned_targets.get(ally_id) {
                    Some(target) => *target,
                    None => ally.engagement_target,
                }?;
                let NetworkTargetId::LaneUnit(target_id) = target else {
                    return None;
                };
                let target = self.units.get(&target_id)?;
                (target.health > 0.0
                    && target.team != unit.team
                    && target.kind != LaneUnitKind::Tower)
                    .then_some((target_id, horizontal_distance(unit.position, ally.position)))
            })
            .min_by(|left, right| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.0.cmp(&right.0))
            })?;
        let anchor = self.units.get(&anchor_target_id)?;
        let anchor_position = anchor.position;
        let target_forward = lane_forward_direction(anchor.team);
        self.units
            .iter()
            .filter(|(target_id, target)| **target_id != unit_id && target.health > 0.0)
            .filter(|(_, target)| target.team != unit.team)
            .filter(|(_, target)| target.kind != LaneUnitKind::Tower)
            .filter(|(_, target)| {
                horizontal_distance(target.position, anchor_position)
                    <= MINION_WAVE_TARGET_CLUSTER_RADIUS
            })
            .filter(|(_, target)| {
                let relative_position = Vec3::new(
                    target.position.x - anchor_position.x,
                    0.0,
                    target.position.z - anchor_position.z,
                );
                relative_position.dot(target_forward) >= -MINION_WAVE_FRONTLINE_DEPTH
            })
            .filter(|(_, target)| {
                Self::is_within_minion_trigger_range(
                    unit.position,
                    target.position,
                    lane_unit_stats(target.kind).hit_radius,
                )
            })
            .map(|(target_id, target)| {
                let assigned_attackers = planned_targets
                    .iter()
                    .filter(|(attacker_id, planned_target)| {
                        **planned_target == Some(NetworkTargetId::LaneUnit(*target_id))
                            && self.units.get(attacker_id).is_some_and(|attacker| {
                                attacker.health > 0.0 && attacker.team == unit.team
                            })
                    })
                    .count();
                (
                    *target_id,
                    assigned_attackers,
                    horizontal_distance(unit.position, target.position),
                )
            })
            .min_by(|left, right| {
                left.1
                    .cmp(&right.1)
                    .then_with(|| {
                        left.2
                            .partial_cmp(&right.2)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| left.0.cmp(&right.0))
            })
            .map(|(target_id, _, _)| NetworkTargetId::LaneUnit(target_id))
    }

    fn is_valid_minion_target(
        &self,
        unit_id: u64,
        target: NetworkTargetId,
        players: &ConnectedPlayers,
    ) -> bool {
        let Some(unit) = self.units.get(&unit_id) else {
            return false;
        };

        match target {
            NetworkTargetId::Player(player_id) => {
                let Some(player) = players.states.get(&player_id) else {
                    return false;
                };
                let player_team = TeamSpec::from(player.team);
                player.health > 0.0
                    && player_team != unit.team
                    && player_team.is_playable()
                    && Self::is_within_minion_trigger_range(
                        unit.position,
                        player.position,
                        LANE_PLAYER_COLLISION_RADIUS,
                    )
            }
            NetworkTargetId::LaneUnit(target_id) => {
                let Some(candidate) = self.units.get(&target_id) else {
                    return false;
                };
                candidate.health > 0.0
                    && target_id != unit_id
                    && candidate.team != unit.team
                    && Self::is_within_minion_trigger_range(
                        unit.position,
                        candidate.position,
                        lane_unit_stats(candidate.kind).hit_radius,
                    )
            }
        }
    }

    fn compare_distance_then_id(left: &(u64, f32), right: &(u64, f32)) -> std::cmp::Ordering {
        left.1
            .partial_cmp(&right.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    }

    fn select_tower_target(
        &mut self,
        tower_id: u64,
        players: &ConnectedPlayers,
    ) -> Option<NetworkTargetId> {
        let tower = self.units.get(&tower_id)?;
        let tower_position = tower.position;
        let tower_team = tower.team;
        let tower_range = lane_unit_stats(LaneUnitKind::Tower).attack_range;
        let forced_target = tower.forced_player_target;

        if let Some(player_id) = forced_target {
            if self.player_is_valid_tower_target(
                player_id,
                tower_position,
                tower_team,
                tower_range,
                players,
            ) {
                return Some(NetworkTargetId::Player(player_id));
            }
            if let Some(tower) = self.units.get_mut(&tower_id) {
                tower.forced_player_target = None;
            }
        }

        let minion_target = self
            .units
            .iter()
            .filter(|(id, unit)| **id != tower_id && unit.health > 0.0)
            .filter(|(_, unit)| unit.team != tower_team && unit.kind != LaneUnitKind::Tower)
            .filter(|(_, unit)| horizontal_distance(tower_position, unit.position) <= tower_range)
            .map(|(unit_id, unit)| (*unit_id, horizontal_distance(tower_position, unit.position)))
            .min_by(Self::compare_distance_then_id)
            .map(|(unit_id, _)| NetworkTargetId::LaneUnit(unit_id));
        if minion_target.is_some() {
            return minion_target;
        }

        players
            .states
            .iter()
            .filter(|(player_id, _)| {
                self.player_is_valid_tower_target(
                    **player_id,
                    tower_position,
                    tower_team,
                    tower_range,
                    players,
                )
            })
            .min_by(|(left_id, _), (right_id, _)| {
                let left = players
                    .states
                    .get(left_id)
                    .map(|player| player.position)
                    .unwrap_or(Vec3::ZERO);
                let right = players
                    .states
                    .get(right_id)
                    .map(|player| player.position)
                    .unwrap_or(Vec3::ZERO);
                let left_distance = horizontal_distance(tower_position, left);
                let right_distance = horizontal_distance(tower_position, right);
                left_distance
                    .partial_cmp(&right_distance)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left_id.cmp(right_id))
            })
            .map(|(player_id, _)| NetworkTargetId::Player(*player_id))
    }

    fn player_is_valid_tower_target(
        &self,
        player_id: u64,
        tower_position: Vec3,
        tower_team: TeamSpec,
        tower_range: f32,
        players: &ConnectedPlayers,
    ) -> bool {
        let Some(player) = players.states.get(&player_id) else {
            return false;
        };
        player.health > 0.0
            && TeamSpec::from(player.team) != tower_team
            && TeamSpec::from(player.team).is_playable()
            && horizontal_distance(tower_position, player.position) <= tower_range
    }

    fn target_position(&self, target: NetworkTargetId, players: &ConnectedPlayers) -> Option<Vec3> {
        match target {
            NetworkTargetId::Player(player_id) => {
                players.states.get(&player_id).map(|player| player.position)
            }
            NetworkTargetId::LaneUnit(unit_id) => {
                self.units.get(&unit_id).map(|unit| unit.position)
            }
        }
    }

    fn target_hit_radius(&self, target: NetworkTargetId) -> f32 {
        match target {
            NetworkTargetId::Player(_) => LANE_PLAYER_COLLISION_RADIUS,
            NetworkTargetId::LaneUnit(unit_id) => self
                .units
                .get(&unit_id)
                .map(|unit| lane_unit_stats(unit.kind).hit_radius)
                .unwrap_or(0.0),
        }
    }

    fn snapshot(&self) -> LaneSnapshot {
        let mut units = self
            .units
            .iter()
            .map(|(id, unit)| NetworkLaneUnit {
                id: *id,
                kind: unit.kind,
                team: unit.team,
                position: unit.position.into(),
                yaw: lane_forward_yaw(unit.team),
                health: unit.health,
                max_health: lane_unit_stats(unit.kind).max_health,
                hit_radius: lane_unit_stats(unit.kind).hit_radius,
                attack_target: self.attack_target_for_snapshot(*id, unit),
            })
            .collect::<Vec<_>>();
        units.sort_by_key(|unit| unit.id);
        LaneSnapshot { units }
    }

    fn attack_target_for_snapshot(
        &self,
        unit_id: u64,
        unit: &ServerLaneUnit,
    ) -> Option<NetworkTargetId> {
        self.tower_projectiles
            .iter()
            .find(|projectile| projectile.source_id == unit_id)
            .map(|projectile| projectile.target)
            .or(unit.attack_target)
    }
}

/// Advances the server-authoritative lane only after at least one client finished loading.
pub(super) fn update_server_lane(
    time: Res<Time>,
    mut lane: ResMut<ServerLaneState>,
    mut players: ResMut<ConnectedPlayers>,
    ready_players: Res<LoadingScreenReadyPlayers>,
    mut combat_events: ResMut<ServerCombatNumberEvents>,
) {
    if !ready_players.has_ready_players() {
        if lane.started {
            lane.reset();
        }
        return;
    }

    if !lane.started {
        lane.start();
    }

    if lane.wave_timer.tick(time.delta()).just_finished() {
        lane.queue_wave();
    }
    lane.update_pending_wave_spawns(time.delta());
    lane.update(&mut players, &mut combat_events, time.delta_secs());
}

/// Broadcasts lane snapshots and reliable ranged-minion attack visuals to connected clients.
pub(super) fn broadcast_lane_snapshots(
    time: Res<Time>,
    mut lane: ResMut<ServerLaneState>,
    mut clients: Query<
        (
            &mut MessageSender<LaneSnapshot>,
            &mut MessageSender<RangedMinionAutoAttackVisualEvent>,
        ),
        (With<ClientOf>, With<Connected>),
    >,
) {
    let should_broadcast_snapshot = lane.snapshot_timer.tick(time.delta()).just_finished();
    let ranged_minion_auto_attack_visuals = lane.take_ranged_minion_auto_attack_visuals();
    if !should_broadcast_snapshot && ranged_minion_auto_attack_visuals.is_empty() {
        return;
    }

    let snapshot = should_broadcast_snapshot.then(|| lane.snapshot());
    for (mut snapshot_sender, mut visual_sender) in &mut clients {
        if let Some(snapshot) = &snapshot {
            snapshot_sender.send::<PlayerStateChannel>(snapshot.clone());
        }
        for visual in &ranged_minion_auto_attack_visuals {
            visual_sender.send::<ReliableCommandChannel>(*visual);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::lobby::{ConnectedPlayerState, DevelopmentTeam};
    use super::*;
    use std::time::Duration;

    #[test]
    fn initial_wave_spawns_as_a_centered_marching_column() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        assert_eq!(LANE_WAVE_UNIT_SPAWN_INTERVAL_SECONDS, 0.4);
        lane.start();

        assert_eq!(
            minion_kinds(&lane, TeamSpec::Light),
            vec![LaneUnitKind::MeleeBox]
        );
        assert_eq!(
            minion_kinds(&lane, TeamSpec::Dark),
            vec![LaneUnitKind::MeleeBox]
        );

        lane.update_pending_wave_spawns(Duration::from_secs_f32(
            LANE_WAVE_UNIT_SPAWN_INTERVAL_SECONDS - 0.01,
        ));
        assert_eq!(minion_kinds(&lane, TeamSpec::Light).len(), 1);
        assert_eq!(minion_kinds(&lane, TeamSpec::Dark).len(), 1);

        for expected_count in 2..=MINION_WAVE.len() {
            let spawn_delta = if expected_count == 2 {
                Duration::from_secs_f32(0.01)
            } else {
                Duration::from_secs_f32(LANE_WAVE_UNIT_SPAWN_INTERVAL_SECONDS)
            };
            lane.update_pending_wave_spawns(spawn_delta);
            assert_eq!(minion_kinds(&lane, TeamSpec::Light).len(), expected_count);
            assert_eq!(minion_kinds(&lane, TeamSpec::Dark).len(), expected_count);
            lane.update(
                &mut players,
                &mut combat_events,
                LANE_WAVE_UNIT_SPAWN_INTERVAL_SECONDS,
            );
        }

        assert_eq!(minion_kinds(&lane, TeamSpec::Light), MINION_WAVE);
        assert_eq!(minion_kinds(&lane, TeamSpec::Dark), MINION_WAVE);
        assert_marching_column(&lane, TeamSpec::Light);
        assert_marching_column(&lane, TeamSpec::Dark);
    }

    #[test]
    fn initial_wave_routes_past_its_own_tower() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.start();

        let delta_seconds = 1.0 / 60.0;
        for _ in 0..720 {
            lane.update_pending_wave_spawns(Duration::from_secs_f32(delta_seconds));
            lane.update(&mut players, &mut combat_events, delta_seconds);
        }

        for team in [TeamSpec::Light, TeamSpec::Dark] {
            let tower_z = lane_tower_position(team).z;
            let direction = lane_forward_direction(team).z;
            for (kind, position) in minion_positions(&lane, team) {
                assert!(
                    (position.z - tower_z) * direction > 2.0,
                    "{team:?} {kind:?} remains at its own tower: {position:?}"
                );
            }
        }
    }

    #[test]
    fn minion_mesh_routes_keep_the_tower_clearance_used_by_collision() {
        let mut lane = ServerLaneState::default();
        let tower_position = Vec3::ZERO;
        let minion_position = Vec3::new(0.0, 0.0, -4.0);
        let goal = Vec3::new(0.0, 0.0, 10.0);
        let stats = lane_unit_stats(LaneUnitKind::MeleeBox);
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Light, tower_position);

        let (_, route) = lane.minion_navigation_path_for_mover(
            minion_position,
            goal,
            ServerLaneState::minion_navigation_agent_radius(stats),
        );
        let route = route.expect("a route around the solid tower");
        let tower_clearance = lane_unit_stats(LaneUnitKind::Tower).hit_radius
            + stats.hit_radius
            + MINION_SEPARATION_MARGIN
            + MINION_COLLISION_SKIN;
        let mut previous = minion_position;
        for waypoint in route {
            assert!(
                distance_to_segment_xz(tower_position, previous, waypoint.position) + 0.001
                    >= tower_clearance,
                "route segment enters the tower collision clearance: {previous:?} -> {:?}",
                waypoint.position
            );
            previous = waypoint.position;
        }
    }

    #[test]
    fn minion_route_keeps_projected_start_recovery_before_a_side_via() {
        let mut lane = ServerLaneState::default();
        let stats = lane_unit_stats(LaneUnitKind::MeleeBox);
        let start = Vec3::new(LANE_HALF_WIDTH - stats.hit_radius, 0.0, -4.0);
        let goal = Vec3::new(-5.0, 0.0, 4.0);
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, start);

        let agent_radius = ServerLaneState::minion_navigation_agent_radius(stats);
        assert!(
            lane.minion_navigation_side_via(2, start, goal, agent_radius)
                .is_some()
        );
        let recovery = lane.navigation_waypoint_for_minion(
            2,
            start,
            goal,
            ServerLaneState::minion_navigation_agent_radius(stats),
        );
        let path = &lane.units[&2].navigation_path;
        assert!(
            path.waypoints
                .front()
                .is_some_and(|waypoint| waypoint.requires_precise_arrival)
        );
        assert!(
            horizontal_distance(start, recovery) > NAVIGATION_WAYPOINT_REACHED_DISTANCE,
            "the mesh projection must not be consumed by normal waypoint tolerance"
        );

        let partially_recovered = lane.move_minion_toward(2, start, recovery, stats, 0.1, true);
        lane.units.get_mut(&2).unwrap().position = partially_recovered;
        let continued_recovery = lane.navigation_waypoint_for_minion(
            2,
            partially_recovered,
            goal,
            ServerLaneState::minion_navigation_agent_radius(stats),
        );
        assert_eq!(continued_recovery, recovery);

        let recovered =
            lane.move_minion_toward(2, partially_recovered, continued_recovery, stats, 0.1, true);
        assert!(
            horizontal_distance(recovered, recovery)
                <= NAVIGATION_RECOVERY_WAYPOINT_REACHED_DISTANCE,
            "recovery waypoint was blocked: start={start:?}, recovery={recovery:?}, recovered={recovered:?}"
        );
        lane.units.get_mut(&2).unwrap().position = recovered;

        let next_waypoint = lane.navigation_waypoint_for_minion(
            2,
            recovered,
            goal,
            ServerLaneState::minion_navigation_agent_radius(stats),
        );
        assert!(horizontal_distance(next_waypoint, recovery) > 0.01);
    }

    #[test]
    fn failed_minion_navigation_route_retries_with_the_same_goal_and_towers() {
        let mut lane = ServerLaneState::default();
        let tower_position = Vec3::ZERO;
        let minion_position = Vec3::new(0.0, 0.0, -4.0);
        let goal = Vec3::new(0.0, 0.0, LANE_SPAWN_Z);
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Light, tower_position);
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, minion_position);

        let obstacle_revision = lane.navigation_obstacle_revision();
        lane.units.get_mut(&2).unwrap().navigation_path = LaneNavigationPath {
            obstacle_revision: Some(obstacle_revision),
            goal: Some(goal),
            waypoints: VecDeque::new(),
            route_resolved: false,
        };

        let waypoint = lane.navigation_waypoint_for_minion(
            2,
            minion_position,
            goal,
            ServerLaneState::minion_navigation_agent_radius(lane_unit_stats(
                LaneUnitKind::MeleeBox,
            )),
        );
        let path = &lane.units[&2].navigation_path;

        assert!(path.route_resolved);
        assert!(!path.waypoints.is_empty());
        assert_ne!(waypoint, goal);
    }

    fn minion_kinds(lane: &ServerLaneState, team: TeamSpec) -> Vec<LaneUnitKind> {
        let mut units = lane
            .units
            .iter()
            .filter(|(_, unit)| unit.team == team && unit.kind != LaneUnitKind::Tower)
            .map(|(id, unit)| (*id, unit.kind))
            .collect::<Vec<_>>();
        units.sort_by_key(|(id, _)| *id);
        units.into_iter().map(|(_, kind)| kind).collect()
    }

    fn minion_positions(lane: &ServerLaneState, team: TeamSpec) -> Vec<(LaneUnitKind, Vec3)> {
        let mut units = lane
            .units
            .iter()
            .filter(|(_, unit)| unit.team == team && unit.kind != LaneUnitKind::Tower)
            .map(|(id, unit)| (*id, unit.kind, unit.position))
            .collect::<Vec<_>>();
        units.sort_by_key(|(id, _, _)| *id);
        units
            .into_iter()
            .map(|(_, kind, position)| (kind, position))
            .collect()
    }

    fn all_minion_positions(lane: &ServerLaneState) -> Vec<(u64, LaneUnitKind, Vec3)> {
        let mut units = lane
            .units
            .iter()
            .filter(|(_, unit)| unit.kind != LaneUnitKind::Tower)
            .map(|(id, unit)| (*id, unit.kind, unit.position))
            .collect::<Vec<_>>();
        units.sort_by_key(|(id, _, _)| *id);
        units
    }

    fn assert_minion_spacing(lane: &ServerLaneState, team: TeamSpec) {
        let minions = minion_positions(lane, team);
        for (index, (left_kind, left_position)) in minions.iter().enumerate() {
            for (right_kind, right_position) in minions.iter().skip(index + 1) {
                let minimum_distance = lane_unit_stats(*left_kind).hit_radius
                    + lane_unit_stats(*right_kind).hit_radius
                    + MINION_SEPARATION_MARGIN;
                assert!(
                    horizontal_distance(*left_position, *right_position) + 0.001
                        >= minimum_distance,
                    "{left_kind:?} and {right_kind:?} overlap: {left_position:?}, {right_position:?}"
                );
            }
        }
    }

    fn assert_all_minion_spacing(lane: &ServerLaneState) {
        let minions = all_minion_positions(lane);
        for (index, (_, left_kind, left_position)) in minions.iter().enumerate() {
            for (_, right_kind, right_position) in minions.iter().skip(index + 1) {
                let minimum_distance = lane_unit_stats(*left_kind).hit_radius
                    + lane_unit_stats(*right_kind).hit_radius
                    + MINION_SEPARATION_MARGIN;
                assert!(
                    horizontal_distance(*left_position, *right_position) + 0.001
                        >= minimum_distance,
                    "{left_kind:?} and {right_kind:?} overlap: {left_position:?}, {right_position:?}"
                );
            }
        }
    }

    fn assert_minion_clearance_from_lane_unit(lane: &ServerLaneState, target_id: u64) {
        let target = &lane.units[&target_id];
        let target_radius = lane_unit_stats(target.kind).hit_radius;
        for (unit_id, unit) in &lane.units {
            if *unit_id == target_id || unit.kind == LaneUnitKind::Tower {
                continue;
            }
            let minimum_distance =
                lane_unit_stats(unit.kind).hit_radius + target_radius + MINION_SEPARATION_MARGIN;
            assert!(
                horizontal_distance(unit.position, target.position) + 0.001 >= minimum_distance,
                "unit {unit_id} overlaps target {target_id}: {:?}, {:?}",
                unit.position,
                target.position
            );
        }
    }

    fn assert_marching_column(lane: &ServerLaneState, team: TeamSpec) {
        let minions = minion_positions(lane, team);
        assert!(minions.iter().all(|(_, position)| position.x.abs() < 0.001));

        let direction = lane_forward_direction(team);
        for [(_, front), (_, following)] in minions.array_windows() {
            assert!(
                (front.z - following.z) * direction.z > 0.0,
                "{team:?} minions are not ordered in a marching column: {front:?}, {following:?}"
            );
        }
        assert_minion_spacing(lane, team);
    }

    #[test]
    fn spell_damage_only_hits_enemy_minions() {
        let mut lane = ServerLaneState::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Dark, Vec3::ZERO);
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, Vec3::ZERO);

        lane.apply_spell_damage(TeamSpec::Light, Vec3::ZERO, 1.0, 20.0);

        assert_eq!(lane.units[&1].health, 350.0);
        assert_eq!(lane.units[&2].health, 330.0);
        assert_eq!(lane.units[&3].health, 5500.0);
    }

    #[test]
    fn projectile_spell_damage_waits_until_the_minion_is_crossed() {
        let mut lane = ServerLaneState::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 2.0),
        );
        let mut hit_target_ids = Vec::new();

        lane.apply_spell_damage_on_segment(
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -2.0),
            Vec3::new(0.0, 0.0, 1.0),
            0.2,
            10.0,
            &mut hit_target_ids,
        );
        assert_eq!(lane.units[&2].health, 350.0);
        assert!(hit_target_ids.is_empty());

        lane.apply_spell_damage_on_segment(
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 2.0),
            0.2,
            10.0,
            &mut hit_target_ids,
        );
        assert_eq!(lane.units[&2].health, 340.0);
        assert_eq!(hit_target_ids, vec![2]);

        lane.apply_spell_damage_on_segment(
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 2.0),
            0.2,
            10.0,
            &mut hit_target_ids,
        );
        assert_eq!(lane.units[&2].health, 340.0);
    }

    #[test]
    fn opposing_minions_stop_after_reaching_attack_range() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(
            LaneUnitKind::Tower,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -20.0),
        );
        lane.spawn_unit(
            LaneUnitKind::Tower,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 20.0),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -0.65),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 0.65),
        );
        let initial_light_position = lane.units[&3].position;
        let initial_dark_position = lane.units[&4].position;

        lane.update(&mut players, &mut combat_events, 0.05);
        assert_all_minion_spacing(&lane);

        assert_eq!(
            lane.units[&3].engagement_target,
            Some(NetworkTargetId::LaneUnit(4))
        );
        assert_eq!(
            lane.units[&4].engagement_target,
            Some(NetworkTargetId::LaneUnit(3))
        );
        assert_eq!(lane.units[&3].position, initial_light_position);
        assert_eq!(lane.units[&4].position, initial_dark_position);

        lane.update(&mut players, &mut combat_events, 0.05);
        assert_eq!(lane.units[&3].position, initial_light_position);
        assert_eq!(lane.units[&4].position, initial_dark_position);
    }

    #[test]
    fn minion_trigger_range_limits_acquisition_and_retained_engagements() {
        let mut lane = ServerLaneState::default();
        let players = ConnectedPlayers::default();
        let target_radius = lane_unit_stats(LaneUnitKind::MeleeBox).hit_radius;
        let trigger_edge = MINION_TARGET_TRIGGER_RANGE + target_radius;
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, trigger_edge - 0.01),
        );

        assert_eq!(
            lane.select_minion_target(1, &players),
            Some(NetworkTargetId::LaneUnit(2))
        );
        lane.units.get_mut(&1).unwrap().engagement_target = Some(NetworkTargetId::LaneUnit(2));
        assert_eq!(
            lane.valid_engagement_target(1, &players),
            Some(NetworkTargetId::LaneUnit(2))
        );

        lane.units.get_mut(&2).unwrap().position.z = trigger_edge + 0.01;
        assert_eq!(lane.select_minion_target(1, &players), None);
        assert_eq!(lane.valid_engagement_target(1, &players), None);
    }

    #[test]
    fn minion_trigger_range_applies_to_players_and_towers() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        players.states.insert(
            20,
            test_player_state(
                DevelopmentTeam::Dark,
                Vec3::new(
                    0.0,
                    0.0,
                    MINION_TARGET_TRIGGER_RANGE + LANE_PLAYER_COLLISION_RADIUS - 0.01,
                ),
            ),
        );

        assert_eq!(
            lane.select_minion_target(1, &players),
            Some(NetworkTargetId::Player(20))
        );

        players.states.get_mut(&20).unwrap().position.z =
            MINION_TARGET_TRIGGER_RANGE + LANE_PLAYER_COLLISION_RADIUS + 0.01;
        let tower_trigger_edge =
            MINION_TARGET_TRIGGER_RANGE + lane_unit_stats(LaneUnitKind::Tower).hit_radius;
        lane.spawn_unit(
            LaneUnitKind::Tower,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, tower_trigger_edge - 0.01),
        );

        assert_eq!(
            lane.select_minion_target(1, &players),
            Some(NetworkTargetId::LaneUnit(2))
        );
        lane.units.get_mut(&1).unwrap().engagement_target = Some(NetworkTargetId::LaneUnit(2));
        assert_eq!(
            lane.valid_engagement_target(1, &players),
            Some(NetworkTargetId::LaneUnit(2))
        );

        lane.units.get_mut(&2).unwrap().position.z = tower_trigger_edge + 0.01;
        assert_eq!(lane.valid_engagement_target(1, &players), None);
    }

    #[test]
    fn wave_assist_balances_targets_along_the_same_frontline() {
        let mut lane = ServerLaneState::default();
        let players = ConnectedPlayers::default();
        for (team, position) in [
            (TeamSpec::Light, Vec3::new(0.0, 0.0, 0.0)),
            (TeamSpec::Dark, Vec3::new(-0.4, 0.0, 1.05)),
            (TeamSpec::Light, Vec3::new(-1.5, 0.0, -0.5)),
            (TeamSpec::Dark, Vec3::new(0.4, 0.0, 1.05)),
            (TeamSpec::Light, Vec3::new(0.0, 0.0, -1.25)),
        ] {
            lane.spawn_unit(LaneUnitKind::MeleeBox, team, position);
        }
        let unit_ids = [1, 2, 3, 4, 5];

        let targets = lane.select_minion_targets(&unit_ids, &players);

        assert_eq!(targets[&1], Some(NetworkTargetId::LaneUnit(2)));
        assert_eq!(targets[&3], Some(NetworkTargetId::LaneUnit(4)));
        assert_eq!(targets[&5], Some(NetworkTargetId::LaneUnit(2)));
    }

    #[test]
    fn combat_slots_keep_friendly_minions_separated_around_a_target() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(-0.7, 0.0, -0.7),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(0.7, 0.0, -0.7),
        );
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, Vec3::ZERO);

        lane.update(&mut players, &mut combat_events, 0.5);

        assert_eq!(
            lane.units[&1].attack_target,
            Some(NetworkTargetId::LaneUnit(3))
        );
        assert_eq!(
            lane.units[&2].attack_target,
            Some(NetworkTargetId::LaneUnit(3))
        );
        assert!(lane.units[&1].position.x < lane.units[&2].position.x - 0.25);
        assert_minion_spacing(&lane, TeamSpec::Light);
    }

    #[test]
    fn combat_slots_distribute_a_second_wave_that_focuses_one_target() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, Vec3::ZERO);
        for position in [
            Vec3::new(-1.6, 0.0, -1.0),
            Vec3::new(0.0, 0.0, -1.8),
            Vec3::new(1.6, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 1.8),
        ] {
            lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, position);
        }

        for _ in 0..10 {
            lane.update(&mut players, &mut combat_events, 0.1);
            assert_all_minion_spacing(&lane);
            assert_minion_clearance_from_lane_unit(&lane, 1);
        }

        for minion_id in 2..=5 {
            assert_eq!(
                lane.units[&minion_id].engagement_target,
                Some(NetworkTargetId::LaneUnit(1))
            );
        }
        let distinct_x_positions = lane
            .units
            .values()
            .filter(|unit| unit.team == TeamSpec::Light)
            .map(|unit| (unit.position.x * 10.0).round() as i32)
            .collect::<std::collections::HashSet<_>>();
        assert!(distinct_x_positions.len() >= 3);
    }

    #[test]
    fn melee_overflow_slots_reposition_into_the_attack_ring_without_overlapping() {
        let mut lane = ServerLaneState::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Dark, Vec3::ZERO);
        for position in [
            Vec3::new(-2.5, 0.0, -1.0),
            Vec3::new(-0.8, 0.0, -2.5),
            Vec3::new(0.8, 0.0, -2.5),
            Vec3::new(2.5, 0.0, -1.0),
            Vec3::new(-2.5, 0.0, 1.0),
            Vec3::new(2.5, 0.0, 1.0),
        ] {
            lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, position);
        }

        let target = NetworkTargetId::LaneUnit(1);
        let targets = (2..=7)
            .map(|unit_id| (unit_id, Some(target)))
            .collect::<HashMap<_, _>>();
        let stats = lane_unit_stats(LaneUnitKind::MeleeBox);
        let target_radius = lane_unit_stats(LaneUnitKind::MeleeBox).hit_radius;
        let slots = (2..=7)
            .map(|unit_id| {
                lane.combat_slot_position(
                    unit_id,
                    TeamSpec::Light,
                    LaneUnitKind::MeleeBox,
                    stats,
                    target,
                    Vec3::ZERO,
                    target_radius,
                    &targets,
                )
            })
            .collect::<Vec<_>>();
        let minimum_target_distance = stats.hit_radius + target_radius + MINION_SEPARATION_MARGIN;
        let maximum_attack_distance = stats.attack_range + target_radius;
        let minimum_minion_distance =
            stats.hit_radius * 2.0 + MINION_SEPARATION_MARGIN + MINION_COLLISION_SKIN;

        for slot in &slots {
            let distance = horizontal_distance(*slot, Vec3::ZERO);
            assert!(
                distance + 0.001 >= minimum_target_distance,
                "melee slot overlaps its target: {slot:?}"
            );
            assert!(
                distance <= maximum_attack_distance,
                "melee slot is queued outside its attack ring: {slot:?}"
            );
        }
        for (index, slot) in slots.iter().enumerate() {
            for other_slot in slots.iter().skip(index + 1) {
                assert!(
                    horizontal_distance(*slot, *other_slot) + 0.001 >= minimum_minion_distance,
                    "melee attack-ring slots overlap: {slot:?}, {other_slot:?}"
                );
            }
        }
    }

    #[test]
    fn four_to_six_melee_minions_reach_separate_attack_positions_around_one_target() {
        for minion_count in (MELEE_COMBAT_SLOT_COLUMNS + 1)..=MELEE_COMBAT_RING_SLOTS {
            let mut lane = ServerLaneState::default();
            let mut players = ConnectedPlayers::default();
            let mut combat_events = ServerCombatNumberEvents::default();
            lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, Vec3::ZERO);

            let approach_side = -lane_forward_direction(TeamSpec::Light);
            for slot_index in 0..minion_count {
                let angle = ServerLaneState::melee_combat_ring_angle(slot_index, minion_count);
                let direction = approach_side * angle.cos() + Vec3::X * angle.sin();
                lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, direction * 4.0);
            }

            for _ in 0..10 {
                lane.update(&mut players, &mut combat_events, 0.1);
                assert_all_minion_spacing(&lane);
                assert_minion_clearance_from_lane_unit(&lane, 1);
            }

            let maximum_attack_distance = lane_unit_stats(LaneUnitKind::MeleeBox).attack_range
                + lane_unit_stats(LaneUnitKind::Tower).hit_radius;
            for unit_id in 2..=minion_count as u64 + 1 {
                let minion = &lane.units[&unit_id];
                assert_eq!(
                    minion.attack_target,
                    Some(NetworkTargetId::LaneUnit(1)),
                    "melee minion {unit_id} remained outside its attack position for {minion_count} attackers"
                );
                assert!(
                    horizontal_distance(minion.position, Vec3::ZERO) <= maximum_attack_distance,
                    "melee minion {unit_id} stopped outside attack range for {minion_count} attackers: {:?}",
                    minion.position
                );
            }
        }
    }

    #[test]
    fn overflow_melee_attackers_form_separate_rows_behind_the_front_fan() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, Vec3::ZERO);
        for position in [
            Vec3::new(-1.7, 0.0, -1.2),
            Vec3::new(0.0, 0.0, -2.0),
            Vec3::new(1.7, 0.0, -1.2),
            Vec3::new(-3.0, 0.0, -2.2),
            Vec3::new(0.0, 0.0, -3.5),
            Vec3::new(3.0, 0.0, -2.2),
            Vec3::new(0.0, 0.0, -5.0),
        ] {
            lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, position);
        }
        for minion_id in 2..=8 {
            lane.units.get_mut(&minion_id).unwrap().engagement_target =
                Some(NetworkTargetId::LaneUnit(1));
        }

        for _ in 0..20 {
            lane.update(&mut players, &mut combat_events, 0.1);
            assert_all_minion_spacing(&lane);
            assert_minion_clearance_from_lane_unit(&lane, 1);
        }

        let radial_rows = (2..=8)
            .map(|minion_id| {
                (horizontal_distance(lane.units[&minion_id].position, Vec3::ZERO) * 10.0).round()
                    as i32
            })
            .collect::<std::collections::HashSet<_>>();
        assert!(radial_rows.len() >= 3);
    }

    #[test]
    fn ranged_minion_group_stays_in_a_front_fan_around_its_target() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, Vec3::ZERO);
        for position in [
            Vec3::new(-1.7, 0.0, -1.0),
            Vec3::new(-0.55, 0.0, -1.9),
            Vec3::new(0.55, 0.0, -1.9),
            Vec3::new(1.7, 0.0, -1.0),
        ] {
            lane.spawn_unit(LaneUnitKind::RangedOrb, TeamSpec::Light, position);
        }

        for _ in 0..10 {
            lane.update(&mut players, &mut combat_events, 0.1);
            assert_all_minion_spacing(&lane);
            assert_minion_clearance_from_lane_unit(&lane, 1);
        }

        for minion_id in 2..=5 {
            assert_eq!(
                lane.units[&minion_id].engagement_target,
                Some(NetworkTargetId::LaneUnit(1))
            );
            assert!(lane.units[&minion_id].position.z < 0.0);
        }
    }

    #[test]
    fn following_minions_stop_before_overlapping_a_friendly_minion() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, 0.0),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -1.4),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 0.5),
        );

        lane.update(&mut players, &mut combat_events, 0.5);

        let minimum_distance =
            lane_unit_stats(LaneUnitKind::MeleeBox).hit_radius * 2.0 + MINION_SEPARATION_MARGIN;
        assert!(
            horizontal_distance(lane.units[&1].position, lane.units[&2].position) + 0.001
                >= minimum_distance
        );
        assert!(lane.units[&2].position.z < 0.0);
    }

    #[test]
    fn minion_attacks_a_closer_enemy_player_before_an_enemy_minion() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 1.0),
        );
        players.states.insert(
            20,
            test_player_state(DevelopmentTeam::Dark, Vec3::new(0.0, 0.0, 0.5)),
        );

        lane.update(&mut players, &mut combat_events, 0.1);

        assert_eq!(
            lane.units[&1].attack_target,
            Some(NetworkTargetId::Player(20))
        );
        assert_eq!(players.states[&20].health, 89.0);
    }

    #[test]
    fn ranged_minion_attack_emits_a_visual_and_damages_on_projectile_impact() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::RangedOrb, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 2.0),
        );

        lane.update(&mut players, &mut combat_events, 0.1);

        let visuals = lane.take_ranged_minion_auto_attack_visuals();
        assert_eq!(visuals.len(), 1);
        let visual = visuals[0];
        assert_eq!(visual.source_unit_id, 1);
        assert_eq!(visual.team, TeamSpec::Light);
        assert_eq!(visual.target, NetworkTargetId::LaneUnit(2));
        assert_eq!(
            visual.start,
            (Vec3::Y * RANGED_MINION_PROJECTILE_HEIGHT).into()
        );
        assert_eq!(
            visual.end,
            (Vec3::new(0.0, RANGED_MINION_PROJECTILE_HEIGHT, 2.0)).into()
        );
        assert_eq!(
            visual.travel_seconds,
            ranged_minion_projectile_travel_seconds(2.0)
        );
        assert_eq!(lane.ranged_minion_projectiles.len(), 1);
        assert_eq!(
            lane.units[&2].health,
            lane_unit_stats(LaneUnitKind::MeleeBox).max_health
        );

        lane.units.get_mut(&2).unwrap().position = Vec3::new(0.0, 0.0, 30.0);
        lane.update(
            &mut players,
            &mut combat_events,
            visual.travel_seconds + 0.01,
        );
        assert!(lane.ranged_minion_projectiles.is_empty());
        assert_eq!(
            lane.units[&2].health,
            lane_unit_stats(LaneUnitKind::MeleeBox).max_health
                - lane_unit_stats(LaneUnitKind::RangedOrb).attack_damage
        );
        assert!(lane.take_ranged_minion_auto_attack_visuals().is_empty());
    }

    #[test]
    fn minion_keeps_its_first_player_target_inside_the_leash() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        players.states.insert(
            20,
            test_player_state(DevelopmentTeam::Dark, Vec3::new(0.0, 0.0, 0.5)),
        );

        lane.update(&mut players, &mut combat_events, 0.1);
        assert_eq!(
            lane.units[&1].engagement_target,
            Some(NetworkTargetId::Player(20))
        );

        players.states.get_mut(&20).unwrap().position = Vec3::new(0.0, 0.0, 2.0);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(1.27, 0.0, -0.48),
        );
        lane.update(&mut players, &mut combat_events, 0.1);

        assert_eq!(
            lane.units[&1].engagement_target,
            Some(NetworkTargetId::Player(20))
        );
        assert!(lane.units[&1].position.z > -0.48);
    }

    #[test]
    fn melee_minion_retargets_a_live_distant_anchor_to_a_closer_enemy() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 2.4),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 1.2),
        );
        lane.units.get_mut(&1).unwrap().engagement_target = Some(NetworkTargetId::LaneUnit(2));

        lane.update(&mut players, &mut combat_events, 0.1);

        assert_eq!(
            lane.units[&1].engagement_target,
            Some(NetworkTargetId::LaneUnit(3))
        );
        assert_eq!(
            lane.units[&1].attack_target,
            Some(NetworkTargetId::LaneUnit(3))
        );
        assert_eq!(lane.units[&3].health, 339.0);
    }

    #[test]
    fn melee_minion_prefers_a_closer_local_target_over_wave_balancing() {
        let mut lane = ServerLaneState::default();
        let players = ConnectedPlayers::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 2.4),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 1.2),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(-0.2, 0.0, 0.0),
        );
        lane.units.get_mut(&1).unwrap().engagement_target = Some(NetworkTargetId::LaneUnit(2));
        let planned_targets = HashMap::from([(4, Some(NetworkTargetId::LaneUnit(3)))]);

        assert_eq!(
            lane.select_minion_target_with_plan(1, &players, &planned_targets),
            Some(NetworkTargetId::LaneUnit(3))
        );
    }

    #[test]
    fn minion_retargets_a_nearby_enemy_behind_or_beside_it_after_a_kill() {
        for replacement_position in [Vec3::new(0.0, 0.0, -3.0), Vec3::new(3.0, 0.0, 0.0)] {
            let mut lane = ServerLaneState::default();
            let mut players = ConnectedPlayers::default();
            let mut combat_events = ServerCombatNumberEvents::default();
            lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
            lane.spawn_unit(
                LaneUnitKind::MeleeBox,
                TeamSpec::Dark,
                Vec3::new(0.0, 0.0, 1.0),
            );
            lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Dark, replacement_position);
            lane.units.get_mut(&1).unwrap().engagement_target = Some(NetworkTargetId::LaneUnit(2));
            lane.apply_lane_unit_damage(2, 1_000.0);

            lane.update(&mut players, &mut combat_events, 0.1);

            assert_eq!(
                lane.units[&1].engagement_target,
                Some(NetworkTargetId::LaneUnit(3))
            );
            let movement = lane.units[&1].position;
            assert!(
                movement.dot(replacement_position.normalize()) > 0.1,
                "minion did not turn toward {replacement_position:?}: {movement:?}"
            );
            assert_all_minion_spacing(&lane);
        }
    }

    #[test]
    fn marching_minion_engages_a_side_enemy_inside_trigger_range() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(3.0, 0.0, 0.0),
        );

        lane.update(&mut players, &mut combat_events, 0.1);

        assert_eq!(
            lane.units[&1].engagement_target,
            Some(NetworkTargetId::LaneUnit(2))
        );
        assert!(lane.units[&1].position.x > 0.0);
    }

    #[test]
    fn ranged_minions_keep_advancing_after_the_melee_front_engages() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        let wave_positions = [
            (LaneUnitKind::MeleeBox, -0.65),
            (LaneUnitKind::MeleeBox, -2.57),
            (LaneUnitKind::MeleeBox, -4.49),
            (LaneUnitKind::LargeRangedBox, -6.41),
            (LaneUnitKind::RangedOrb, -8.33),
            (LaneUnitKind::RangedOrb, -10.25),
            (LaneUnitKind::RangedOrb, -12.17),
        ];

        for (kind, z) in wave_positions {
            lane.spawn_unit(kind, TeamSpec::Light, Vec3::new(0.0, 0.0, z));
        }
        for (kind, z) in wave_positions {
            lane.spawn_unit(kind, TeamSpec::Dark, Vec3::new(0.0, 0.0, -z));
        }

        let initial_positions = lane
            .units
            .iter()
            .filter(|(_, unit)| ServerLaneState::is_ranged_minion(unit.kind))
            .map(|(id, unit)| (*id, (unit.team, unit.position)))
            .collect::<HashMap<_, _>>();
        let mut minions_that_engaged = std::collections::HashSet::new();

        for _ in 0..50 {
            lane.update(&mut players, &mut combat_events, 0.1);
            minions_that_engaged.extend(
                lane.units
                    .iter()
                    .filter(|(_, unit)| {
                        ServerLaneState::is_ranged_minion(unit.kind)
                            && unit.engagement_target.is_some()
                    })
                    .map(|(unit_id, _)| *unit_id),
            );
            if minions_that_engaged.len() == initial_positions.len() {
                break;
            }
        }

        for (unit_id, (team, initial_position)) in initial_positions {
            let unit = &lane.units[&unit_id];
            assert!(
                (unit.position.z - initial_position.z) * lane_forward_direction(team).z > 1.0,
                "ranged minion {unit_id} did not advance from {initial_position:?} to {:?}",
                unit.position
            );
            assert!(
                minions_that_engaged.contains(&unit_id),
                "ranged minion {unit_id} never entered its trigger range"
            );
        }
    }

    #[test]
    fn rear_minions_find_targets_and_route_before_the_frontline_dies() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        let wave_positions = [
            (LaneUnitKind::MeleeBox, -0.65),
            (LaneUnitKind::MeleeBox, -2.57),
            (LaneUnitKind::MeleeBox, -4.49),
            (LaneUnitKind::LargeRangedBox, -6.41),
            (LaneUnitKind::RangedOrb, -8.33),
            (LaneUnitKind::RangedOrb, -10.25),
            (LaneUnitKind::RangedOrb, -12.17),
        ];

        for (kind, z) in wave_positions {
            lane.spawn_unit(kind, TeamSpec::Light, Vec3::new(0.0, 0.0, z));
        }
        for (kind, z) in wave_positions {
            lane.spawn_unit(kind, TeamSpec::Dark, Vec3::new(0.0, 0.0, -z));
        }

        let rear_melee_start = lane.units[&2].position;
        let ranged_start = lane.units[&4].position;
        for _ in 0..20 {
            lane.update(&mut players, &mut combat_events, 0.1);
            assert_all_minion_spacing(&lane);
        }

        assert!(
            lane.units[&8].health > 0.0,
            "the initial melee target died before rear minions joined the fight"
        );
        assert!(lane.units[&2].engagement_target.is_some());
        assert!(lane.units[&4].engagement_target.is_some());
        assert!(lane.units[&2].position.z > rear_melee_start.z + 0.5);
        assert!(lane.units[&4].position.z > ranged_start.z + 0.5);
        assert!(
            lane.units[&4].position.x.abs() > 0.1,
            "the ranged minion remained queued directly behind the melee frontline"
        );
    }

    #[test]
    fn minions_bypass_their_own_solid_tower_on_opposite_sides() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        let tower_position = lane_tower_position(TeamSpec::Light);
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Light, tower_position);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            tower_position - Vec3::Z * 4.0,
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            tower_position - Vec3::Z * 5.4,
        );

        for _ in 0..60 {
            lane.update(&mut players, &mut combat_events, 0.1);
            assert_minion_clearance_from_lane_unit(&lane, 1);
            assert_all_minion_spacing(&lane);
        }

        let first_minion = lane.units[&2].position;
        let second_minion = lane.units[&3].position;
        assert!(first_minion.z > tower_position.z + 2.0);
        assert!(second_minion.z > tower_position.z + 2.0);
        assert!(
            first_minion.x * second_minion.x < -0.01,
            "minions did not split around their tower: {first_minion:?}, {second_minion:?}"
        );
    }

    #[test]
    fn tower_navigation_routes_keep_a_three_minion_queue_advancing() {
        let mut lane = ServerLaneState::default();
        let tower_position = Vec3::ZERO;
        let minion_position = Vec3::new(0.0, 0.0, -4.0);
        let stats = lane_unit_stats(LaneUnitKind::MeleeBox);
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, tower_position);
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, minion_position);

        let waypoint = lane.navigation_waypoint_for_minion(
            2,
            minion_position,
            Vec3::new(0.0, 0.0, LANE_SPAWN_Z),
            ServerLaneState::minion_navigation_agent_radius(stats),
        );
        assert!(lane.minion_follows_navigation_route(2));

        let direction = (waypoint - minion_position).normalize_or_zero();
        assert!(direction.length_squared() > f32::EPSILON);
        let blocker_distance =
            stats.hit_radius * 2.0 + MINION_SEPARATION_MARGIN + MINION_COLLISION_SKIN + 0.01;
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            minion_position + direction * blocker_distance,
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            minion_position - direction * blocker_distance,
        );

        let blocked_position =
            lane.move_minion_toward(2, minion_position, waypoint, stats, 0.1, false);
        let detoured_position = lane.move_minion_toward(
            2,
            minion_position,
            waypoint,
            stats,
            0.1,
            lane.minion_follows_navigation_route(2),
        );

        assert!(horizontal_distance(blocked_position, minion_position) < 0.02);
        assert!(
            horizontal_distance(detoured_position, minion_position) > 0.01,
            "a minion following a tower route did not sidestep waiting lane traffic"
        );
        assert!(
            horizontal_distance(detoured_position, tower_position)
                >= lane_unit_stats(LaneUnitKind::Tower).hit_radius + stats.hit_radius
        );
        lane.units.get_mut(&2).unwrap().position = detoured_position;
        assert_all_minion_spacing(&lane);
    }

    #[test]
    fn minions_stop_outside_an_enemy_tower_and_attack_it() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        let tower_position = lane_tower_position(TeamSpec::Dark);
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, tower_position);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            tower_position - Vec3::Z * 1.95,
        );

        lane.update(&mut players, &mut combat_events, 0.1);

        assert_eq!(
            lane.units[&2].attack_target,
            Some(NetworkTargetId::LaneUnit(1))
        );
        assert!(lane.units[&1].health < lane_unit_stats(LaneUnitKind::Tower).max_health);
        assert!(lane.units[&2].position.z < tower_position.z);
        assert_minion_clearance_from_lane_unit(&lane, 1);
    }

    #[test]
    fn melee_minion_reaches_an_enemy_tower_from_outside_attack_range() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -3.0),
        );

        for _ in 0..10 {
            lane.update(&mut players, &mut combat_events, 0.1);
        }

        let minion = &lane.units[&2];
        let tower = &lane.units[&1];
        assert_eq!(minion.engagement_target, Some(NetworkTargetId::LaneUnit(1)));
        assert_eq!(minion.attack_target, Some(NetworkTargetId::LaneUnit(1)));
        assert!(
            tower.health < lane_unit_stats(LaneUnitKind::Tower).max_health,
            "the minion did not reach its tower attack ring"
        );
        assert_minion_clearance_from_lane_unit(&lane, 1);
    }

    #[test]
    fn player_tower_collision_ignores_a_destroyed_tower() {
        let mut lane = ServerLaneState::default();
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, Vec3::ZERO);
        let start = Vec3::new(0.0, 0.0, -5.0);
        let desired = Vec3::new(0.0, 0.0, 5.0);

        let blocked = lane.resolve_player_tower_collision(
            start,
            desired,
            game_shared::game::lane::LANE_PLAYER_COLLISION_RADIUS,
        );
        assert!(blocked.z < 0.0);

        lane.apply_lane_unit_damage(1, 10_000.0);
        let unblocked = lane.resolve_player_tower_collision(
            start,
            desired,
            game_shared::game::lane::LANE_PLAYER_COLLISION_RADIUS,
        );
        assert_eq!(unblocked, desired);
    }

    #[test]
    fn followers_route_around_a_frontline_attacking_an_enemy_player() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -1.4),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -2.8),
        );
        let mut player = test_player_state(DevelopmentTeam::Dark, Vec3::new(0.0, 0.0, 0.5));
        player.health = 10_000.0;
        players.states.insert(20, player);

        let first_follower_start = lane.units[&2].position;
        let second_follower_start = lane.units[&3].position;
        for _ in 0..15 {
            lane.update(&mut players, &mut combat_events, 0.1);
            assert_all_minion_spacing(&lane);
        }

        assert_eq!(
            lane.units[&1].attack_target,
            Some(NetworkTargetId::Player(20))
        );
        assert_eq!(
            lane.units[&2].attack_target,
            Some(NetworkTargetId::Player(20))
        );
        assert!(lane.units[&2].position.z > first_follower_start.z + 0.4);
        assert!(lane.units[&3].position.z > second_follower_start.z + 0.5);
        assert!(
            lane.units[&2].position.x * lane.units[&3].position.x < -0.01,
            "followers did not split around the player-focused frontline: {:?}, {:?}",
            lane.units[&2].position,
            lane.units[&3].position
        );
    }

    #[test]
    fn followers_route_around_a_frontline_attacking_an_enemy_tower() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Dark, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -1.95),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -3.35),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -4.75),
        );

        let first_follower_start = lane.units[&3].position;
        let second_follower_start = lane.units[&4].position;
        for _ in 0..12 {
            lane.update(&mut players, &mut combat_events, 0.1);
            assert_minion_clearance_from_lane_unit(&lane, 1);
            assert_all_minion_spacing(&lane);
        }

        assert_eq!(
            lane.units[&2].attack_target,
            Some(NetworkTargetId::LaneUnit(1))
        );
        assert!(lane.units[&3].position.z > first_follower_start.z + 0.5);
        assert!(lane.units[&4].position.z > second_follower_start.z + 0.5);
        assert!(
            lane.units[&3].position.x * lane.units[&4].position.x < -0.01,
            "followers did not split around the tower-focused frontline: {:?}, {:?}",
            lane.units[&3].position,
            lane.units[&4].position
        );
    }

    #[test]
    fn ranged_minion_keeps_marching_until_an_enemy_enters_trigger_range() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 3.0),
        );
        lane.spawn_unit(
            LaneUnitKind::LargeRangedBox,
            TeamSpec::Light,
            Vec3::new(0.0, 0.0, -5.76),
        );

        lane.update(&mut players, &mut combat_events, 0.1);

        assert_eq!(
            lane.units[&1].engagement_target,
            Some(NetworkTargetId::LaneUnit(2))
        );
        assert_eq!(lane.units[&3].engagement_target, None);
        assert!(lane.units[&3].position.z > -5.76);
    }

    #[test]
    fn minion_target_selection_breaks_equal_distance_ties_by_id() {
        let mut lane = ServerLaneState::default();
        let players = ConnectedPlayers::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(-0.5, 0.0, 1.0),
        );
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.5, 0.0, 1.0),
        );

        assert_eq!(
            lane.select_minion_target(1, &players),
            Some(NetworkTargetId::LaneUnit(2))
        );
    }

    #[test]
    fn combat_slot_at_the_lane_edge_keeps_a_minion_outside_player_collision_radius() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Light,
            Vec3::new(4.4, 0.0, 0.0),
        );
        players.states.insert(
            20,
            test_player_state(DevelopmentTeam::Dark, Vec3::new(5.8, 0.0, 0.0)),
        );

        lane.update(&mut players, &mut combat_events, 0.5);

        let minion = &lane.units[&1];
        let minimum_distance = lane_unit_stats(LaneUnitKind::MeleeBox).hit_radius
            + LANE_PLAYER_COLLISION_RADIUS
            + MINION_SEPARATION_MARGIN;
        assert!(
            horizontal_distance(minion.position, players.states[&20].position) + 0.001
                >= minimum_distance
        );
        assert!(
            minion.position.x <= LANE_HALF_WIDTH - lane_unit_stats(minion.kind).hit_radius + 0.001
        );
    }

    #[test]
    fn tower_prioritizes_an_attacker_and_resolves_an_in_flight_shot() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::Tower, TeamSpec::Light, Vec3::ZERO);
        players.states.insert(
            10,
            test_player_state(DevelopmentTeam::Light, Vec3::new(0.0, 0.0, 2.0)),
        );
        players.states.insert(
            20,
            test_player_state(DevelopmentTeam::Dark, Vec3::new(0.0, 0.0, 3.0)),
        );

        lane.record_hostile_player_action(20, 10, &players);
        lane.update(&mut players, &mut combat_events, 0.01);
        assert_eq!(
            lane.units[&1].attack_target,
            Some(NetworkTargetId::Player(20))
        );

        players.states.get_mut(&20).unwrap().position = Vec3::new(0.0, 0.0, 20.0);
        lane.update(&mut players, &mut combat_events, 0.5);

        assert_eq!(players.states[&20].health, 10.0);
        assert!(lane.units[&1].forced_player_target.is_none());
    }

    fn test_player_state(team: DevelopmentTeam, position: Vec3) -> ConnectedPlayerState {
        ConnectedPlayerState {
            position,
            position_correction_generation: 0,
            yaw: 0.0,
            moving: false,
            health: 100.0,
            champion: game_shared::network::ChampionId::LIRA,
            lira_q_cooldown: 0.0,
            lira_w_cooldown: 0.0,
            lira_e_cooldown: 0.0,
            auto_attack_cooldown: 0.0,
            auto_attack_combo_stage: 0,
            auto_attack_combo_target: None,
            auto_attack_combo_reset_timer: 0.0,
            ignara_q_cooldown: 0.0,
            ignara_w_cooldown: 0.0,
            ignara_e_cooldown: 0.0,
            yuna_q_cooldown: 0.0,
            yuna_w_cooldown: 0.0,
            yuna_e_cooldown: 0.0,
            sophia_q_cooldown: 0.0,
            sophia_w_cooldown: 0.0,
            sophia_e_cooldown: 0.0,
            sophia_damage_buff_timer: 0.0,
            sophia_speed_buff_timer: 0.0,
            sophia_damage_amp_available: false,
            slow_timer: 0.0,
            slow_multiplier: 1.0,
            stun_timer: 0.0,
            team,
            respawn_timer: None,
            respawn_generation: 0,
            respawn_input_grace: 0.0,
        }
    }
}
