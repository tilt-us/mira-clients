use super::combat::{ServerCombatNumberEvents, apply_damage};
use super::geometry::horizontal_distance;
use super::lobby::{ConnectedPlayers, LoadingScreenReadyPlayers};
use bevy::prelude::*;
use game_shared::game::{
    lane::{
        LANE_SPAWN_Z, LANE_WAVE_INTERVAL_SECONDS, LaneUnitKind, lane_forward_direction,
        lane_forward_yaw, lane_spawn_position, lane_tower_position, lane_unit_stats,
    },
    team::TeamSpec,
};
use game_shared::network::{LaneSnapshot, NetworkLaneUnit, NetworkTargetId, PlayerStateChannel};
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::*;
use std::collections::{HashMap, VecDeque};

const LANE_SNAPSHOT_INTERVAL_SECONDS: f32 = 1.0 / 20.0;
const LANE_WAVE_UNIT_SPAWN_INTERVAL_SECONDS: f32 = 0.2;
const PLAYER_AUTO_ATTACK_RANGE: f32 = 5.0;
const TOWER_PROJECTILE_SPEED: f32 = 24.0;
const TOWER_PROJECTILE_MIN_TRAVEL_SECONDS: f32 = 0.1;
const TOWER_PROJECTILE_MAX_TRAVEL_SECONDS: f32 = 0.45;

const MINION_WAVE: [LaneUnitKind; 7] = [
    LaneUnitKind::MeleeBox,
    LaneUnitKind::MeleeBox,
    LaneUnitKind::MeleeBox,
    LaneUnitKind::LargeRangedBox,
    LaneUnitKind::RangedOrb,
    LaneUnitKind::RangedOrb,
    LaneUnitKind::RangedOrb,
];

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
}

#[derive(Debug, Clone)]
struct ServerLaneUnit {
    kind: LaneUnitKind,
    team: TeamSpec,
    position: Vec3,
    health: f32,
    attack_cooldown_seconds: f32,
    attack_target: Option<NetworkTargetId>,
    forced_player_target: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct TowerProjectile {
    source_id: u64,
    target: NetworkTargetId,
    remaining_seconds: f32,
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
        }
    }
}

impl ServerLaneState {
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
        if distance > PLAYER_AUTO_ATTACK_RANGE + hit_radius {
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
                forced_player_target: None,
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

        let unit_ids = self.units.keys().copied().collect::<Vec<_>>();
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
                if attack_cooldown <= 0.0 {
                    if let Some(target) = target {
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

            let target = self.select_minion_target(unit_id);
            let stats = lane_unit_stats(kind);
            let mut moved_position = position;
            let mut attack_target = None;
            let mut next_cooldown = attack_cooldown;

            if let Some(target) = target {
                if let Some(target_position) = self.target_position(target, players) {
                    let target_radius = self.target_hit_radius(target);
                    if horizontal_distance(position, target_position)
                        <= stats.attack_range + target_radius
                    {
                        attack_target = Some(target);
                        if attack_cooldown <= 0.0 {
                            damage_actions.push(LaneDamageAction {
                                target,
                                amount: stats.attack_damage,
                            });
                            next_cooldown = stats.attack_interval_seconds;
                        }
                    }
                }
            }

            if attack_target.is_none() {
                let direction = lane_forward_direction(team);
                moved_position += direction * stats.movement_speed * delta_seconds;
                moved_position.z = moved_position.z.clamp(-LANE_SPAWN_Z, LANE_SPAWN_Z);
            }

            if let Some(unit) = self.units.get_mut(&unit_id) {
                unit.position = moved_position;
                unit.attack_cooldown_seconds = next_cooldown;
                unit.attack_target = attack_target;
            }
        }

        self.tower_projectiles.extend(tower_projectiles);
        for action in damage_actions {
            self.apply_damage_action(action, players, combat_events);
        }
        self.units.retain(|_, unit| unit.health > 0.0);
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

    fn select_minion_target(&self, unit_id: u64) -> Option<NetworkTargetId> {
        let unit = self.units.get(&unit_id)?;
        let stats = lane_unit_stats(unit.kind);
        self.units
            .iter()
            .filter(|(target_id, target)| **target_id != unit_id && target.health > 0.0)
            .filter(|(_, target)| target.team != unit.team)
            .filter(|(_, target)| target.kind != LaneUnitKind::Tower)
            .filter(|(_, target)| {
                horizontal_distance(unit.position, target.position)
                    <= stats.attack_range + lane_unit_stats(target.kind).hit_radius
            })
            .min_by(|(_, left), (_, right)| {
                horizontal_distance(unit.position, left.position)
                    .partial_cmp(&horizontal_distance(unit.position, right.position))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(target_id, _)| NetworkTargetId::LaneUnit(*target_id))
            .or_else(|| {
                self.units
                    .iter()
                    .filter(|(target_id, target)| **target_id != unit_id && target.health > 0.0)
                    .filter(|(_, target)| target.team != unit.team)
                    .filter(|(_, target)| target.kind == LaneUnitKind::Tower)
                    .filter(|(_, target)| {
                        horizontal_distance(unit.position, target.position)
                            <= stats.attack_range + lane_unit_stats(target.kind).hit_radius
                    })
                    .map(|(target_id, _)| NetworkTargetId::LaneUnit(*target_id))
                    .next()
            })
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
            .min_by(|(_, left), (_, right)| {
                horizontal_distance(tower_position, left.position)
                    .partial_cmp(&horizontal_distance(tower_position, right.position))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| NetworkTargetId::LaneUnit(*id));
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
                horizontal_distance(tower_position, left)
                    .partial_cmp(&horizontal_distance(tower_position, right))
                    .unwrap_or(std::cmp::Ordering::Equal)
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
            NetworkTargetId::Player(_) => 0.9,
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

/// Broadcasts the latest lane state on the latest-only player-state channel.
pub(super) fn broadcast_lane_snapshots(
    time: Res<Time>,
    mut lane: ResMut<ServerLaneState>,
    mut clients: Query<&mut MessageSender<LaneSnapshot>, (With<ClientOf>, With<Connected>)>,
) {
    if !lane.snapshot_timer.tick(time.delta()).just_finished() {
        return;
    }

    let snapshot = lane.snapshot();
    for mut sender in &mut clients {
        sender.send::<PlayerStateChannel>(snapshot.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::super::lobby::{ConnectedPlayerState, DevelopmentTeam};
    use super::*;
    use std::time::Duration;

    #[test]
    fn initial_wave_spawns_each_pair_in_formation_order() {
        let mut lane = ServerLaneState::default();
        lane.start();

        assert_eq!(
            minion_kinds(&lane, TeamSpec::Light),
            vec![LaneUnitKind::MeleeBox]
        );
        assert_eq!(
            minion_kinds(&lane, TeamSpec::Dark),
            vec![LaneUnitKind::MeleeBox]
        );

        for expected_count in 2..=MINION_WAVE.len() {
            lane.update_pending_wave_spawns(Duration::from_secs_f32(
                LANE_WAVE_UNIT_SPAWN_INTERVAL_SECONDS,
            ));
            assert_eq!(minion_kinds(&lane, TeamSpec::Light).len(), expected_count);
            assert_eq!(minion_kinds(&lane, TeamSpec::Dark).len(), expected_count);
        }

        assert_eq!(minion_kinds(&lane, TeamSpec::Light), MINION_WAVE);
        assert_eq!(minion_kinds(&lane, TeamSpec::Dark), MINION_WAVE);
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
    fn opposing_minions_stop_and_attack_when_they_meet() {
        let mut lane = ServerLaneState::default();
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Light, Vec3::ZERO);
        lane.spawn_unit(LaneUnitKind::MeleeBox, TeamSpec::Dark, Vec3::ZERO);

        lane.update(&mut players, &mut combat_events, 0.1);

        assert_eq!(lane.units[&1].health, 339.0);
        assert_eq!(lane.units[&2].health, 339.0);
        assert_eq!(
            lane.units[&1].attack_target,
            Some(NetworkTargetId::LaneUnit(2))
        );
        assert_eq!(
            lane.units[&2].attack_target,
            Some(NetworkTargetId::LaneUnit(1))
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
            yaw: 0.0,
            moving: false,
            health: 100.0,
            champion: game_shared::network::ChampionId(6606),
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
