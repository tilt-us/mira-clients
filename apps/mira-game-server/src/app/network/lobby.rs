use super::super::content::{ServerAbilityDefinition, ServerChampionCatalog};
use super::super::match_manifest::{ServerMatchManifest, ServerMatchPlayer};
use super::combat::{ServerCombatNumberEvents, apply_area_damage, apply_damage, apply_heal};
use super::geometry::{
    clamp_cast_target, distance_to_segment_xz, horizontal_distance, point_in_oriented_rect_xz,
};
use super::lane::ServerLaneState;
use bevy::prelude::*;
use game_shared::game::{
    auto_attack::{
        AUTO_ATTACK_COMBO_RESET_SECONDS, AUTO_ATTACK_RANGE, auto_attack_combo,
        auto_attack_projectile_travel_seconds,
    },
    lane::{
        LANE_HALF_WIDTH, LANE_PLAYER_BASE_MOVEMENT_SPEED, LANE_PLAYER_COLLISION_RADIUS,
        LANE_SPAWN_Z, lane_forward_yaw, lane_spawn_position,
    },
    player::DEFAULT_PLAYER_HEALTH_REGENERATION_PER_SECOND,
    team::TeamSpec,
};
use game_shared::network::{
    AbilitySlot, AbilityVisualEvent, AbilityVisualTuning, AutoAttackVisualEvent,
    ChampionCatalogUpdate, ChampionId, ClientLeave, DisplayReady, LoadingScreenPlayer,
    LoadingScreenStatus, MatchSnapshot, NetworkCombatNumberKind, NetworkPlayer, NetworkTargetId,
    PlayerCommand, PlayerStateUpdate, ReliableCommandChannel, WorldPosition,
};
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

#[cfg(test)]
use game_shared::game::lane::{LaneUnitKind, lane_unit_stats};

const DEFAULT_DEVELOPMENT_TEAM: DevelopmentTeam = DevelopmentTeam::Light;
pub(super) const DEVELOPMENT_PLAYER_HIT_RADIUS: f32 = LANE_PLAYER_COLLISION_RADIUS;
const MATCH_SNAPSHOT_INTERVAL_SECONDS: f32 = 0.05;
const LOADING_SCREEN_STATUS_INTERVAL_SECONDS: f32 = 0.1;
pub(super) const RESPAWN_SECONDS: f32 = 5.0;
const RESPAWN_INPUT_GRACE_SECONDS: f32 = 0.25;
const AUTO_ATTACK_INPUT_BUFFER_SECONDS: f32 = 0.15;
const AUTO_ATTACK_PROJECTILE_HEIGHT: f32 = 0.8;
const SERVER_PLAYER_NAVIGATION_SPEED: f32 = LANE_PLAYER_BASE_MOVEMENT_SPEED;
const PLAYER_NAVIGATION_WAYPOINT_REACHED_DISTANCE: f32 = 0.08;
const PLAYER_NAVIGATION_RECOVERY_WAYPOINT_REACHED_DISTANCE: f32 = 0.001;
const PLAYER_NAVIGATION_COLLISION_REPLAN_DISTANCE: f32 = 0.001;
const ATTACK_MOVE_TARGET_UPDATE_DISTANCE: f32 = 0.08;
const ATTACK_MOVE_INNER_RANGE_MARGIN: f32 = PLAYER_NAVIGATION_WAYPOINT_REACHED_DISTANCE + 0.04;
const EFFECT_TICK_INTERVAL_SECONDS: f32 = 1.0;
const DEFAULT_MOVEMENT_SPEED_MULTIPLIER: f32 = 1.0;
const DEFAULT_DAMAGE_MULTIPLIER: f32 = 1.0;
const MAX_MOVEMENT_SPEED_MULTIPLIER: f32 = 2.0;
const SOPHIA_SPEED_BUFF_MULTIPLIER: f32 = 1.2;
const DEFAULT_SOPHIA_DAMAGE_BUFF_MULTIPLIER: f32 = 1.2;
const IGNARA_E_COLLISION_RADIUS_WIDTH_FACTOR: f32 = 0.28;
const IGNARA_E_COLLISION_RADIUS_PROGRESS_FACTOR: f32 = 1.85;

/// Enumerates Development Team states or variants used by the dedicated server lobby simulation system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DevelopmentTeam {
    Neutral,
    Light,
    Dark,
}
/// Stores the latest known visual state for one connected development player.
///
/// - `position`: Latest known world-space position.
/// - `position_correction_generation`: Monotonic counter for rejected client positions.
/// - `yaw`: Latest known facing angle around the Y axis.
/// - `moving`: Whether the player is currently moving.
/// - `health`: Current synchronized health value.
/// - `max_health`: Authoritative maximum health used by server combat calculations.
/// - `lira_q_cooldown`: Remaining authoritative Lira Q cooldown in seconds.
/// - `lira_w_cooldown`: Remaining authoritative Lira W cooldown in seconds.
/// - `lira_e_cooldown`: Remaining authoritative Lira E cooldown in seconds.
/// - `auto_attack_cooldown`: Remaining authoritative auto-attack cooldown in seconds.
/// - `respawn_timer`: Remaining respawn time when the player is dead.
/// - `respawn_generation`: Monotonic counter incremented each time the player respawns.
/// - `respawn_input_grace`: Short duration that rejects stale position updates after respawn.
#[derive(Debug, Clone, Copy)]
pub(super) struct ConnectedPlayerState {
    pub(super) position: Vec3,
    pub(super) position_correction_generation: u32,
    pub(super) yaw: f32,
    pub(super) moving: bool,
    pub(super) health: f32,
    pub(super) max_health: f32,
    pub(super) champion: ChampionId,
    pub(super) lira_q_cooldown: f32,
    pub(super) lira_w_cooldown: f32,
    pub(super) lira_e_cooldown: f32,
    pub(super) auto_attack_cooldown: f32,
    pub(super) auto_attack_combo_stage: usize,
    pub(super) auto_attack_combo_target: Option<NetworkTargetId>,
    pub(super) auto_attack_combo_reset_timer: f32,
    pub(super) ignara_q_cooldown: f32,
    pub(super) ignara_w_cooldown: f32,
    pub(super) ignara_e_cooldown: f32,
    pub(super) yuna_q_cooldown: f32,
    pub(super) yuna_w_cooldown: f32,
    pub(super) yuna_e_cooldown: f32,
    pub(super) sophia_q_cooldown: f32,
    pub(super) sophia_w_cooldown: f32,
    pub(super) sophia_e_cooldown: f32,
    pub(super) sophia_damage_buff_timer: f32,
    pub(super) sophia_speed_buff_timer: f32,
    pub(super) sophia_damage_amp_available: bool,
    pub(super) slow_timer: f32,
    pub(super) slow_multiplier: f32,
    pub(super) stun_timer: f32,
    pub(super) team: DevelopmentTeam,
    pub(super) respawn_timer: Option<f32>,
    pub(super) respawn_generation: u32,
    pub(super) respawn_input_grace: f32,
}
/// Stores the latest known server-side state for connected development players.
///
/// - `states`: Latest known player states by player id.
#[derive(Resource, Debug, Default)]
pub(super) struct ConnectedPlayers {
    pub(super) states: HashMap<u64, ConnectedPlayerState>,
}

/// Stores server-authoritative routes requested by connected players.
#[derive(Resource, Debug, Default)]
pub(super) struct ServerPlayerNavigation {
    paths: HashMap<u64, ServerPlayerNavigationPath>,
}

#[derive(Debug, Clone)]
struct ServerPlayerNavigationPath {
    requested_target: Vec3,
    target: Vec3,
    attack_target: Option<NetworkTargetId>,
    attack_target_position: Option<Vec3>,
    obstacle_revision: Option<u64>,
    waypoints: VecDeque<Vec3>,
    recovery_waypoint: Option<Vec3>,
    planned: bool,
}

impl ServerPlayerNavigation {
    fn request_move(&mut self, player_id: u64, target: Vec3) {
        self.paths.insert(
            player_id,
            ServerPlayerNavigationPath {
                requested_target: target,
                target,
                attack_target: None,
                attack_target_position: None,
                obstacle_revision: None,
                waypoints: VecDeque::new(),
                recovery_waypoint: None,
                planned: false,
            },
        );
    }

    fn request_attack_move(&mut self, player_id: u64, target: NetworkTargetId) {
        self.paths.insert(
            player_id,
            ServerPlayerNavigationPath {
                requested_target: Vec3::ZERO,
                target: Vec3::ZERO,
                attack_target: Some(target),
                attack_target_position: None,
                obstacle_revision: None,
                waypoints: VecDeque::new(),
                recovery_waypoint: None,
                planned: false,
            },
        );
    }

    fn clear(&mut self, player_id: u64) {
        self.paths.remove(&player_id);
    }
}
/// Stores active server-authoritative ability simulations.
///
/// - `auto_attack_projectiles`: Active player auto-attack projectiles.
/// - `q_projectiles`: Active Lira Q projectiles.
/// - `w_projectiles`: Active Lira W arcing projectiles.
/// - `e_missiles`: Active Lira E contact missiles.
#[derive(Resource, Debug, Default)]
pub(super) struct ActiveServerAbilities {
    auto_attack_projectiles: Vec<ServerAutoAttackProjectile>,
    q_projectiles: Vec<ServerQProjectile>,
    w_projectiles: Vec<ServerWProjectile>,
    e_missiles: Vec<ServerEMissile>,
    ignara_q_zones: Vec<ServerIgnaraQZone>,
    ignara_w_fireballs: Vec<ServerIgnaraWFireball>,
    ignara_e_snowballs: Vec<ServerIgnaraESnowball>,
    yuna_q_orbs: Vec<ServerYunaQOrb>,
    yuna_w_fields: Vec<ServerYunaWField>,
    sophia_q_orbs: Vec<ServerSophiaQOrb>,
    sophia_minions: Vec<ServerSophiaMinion>,
}

/// Stores one launched player auto attack until its projectile reaches the target.
///
/// - `caster_team`: Team used to validate that the target remains hostile on impact.
/// - `target`: Player or lane unit selected when the attack was launched.
/// - `remaining_seconds`: Server-authoritative flight time still remaining.
/// - `damage`: Accepted combo damage applied on impact.
#[derive(Debug, Clone, Copy)]
struct ServerAutoAttackProjectile {
    caster_team: TeamSpec,
    target: NetworkTargetId,
    remaining_seconds: f32,
    damage: f32,
}

/// Stores one active server-authoritative Lira Q projectile.
///
/// - `caster_player_id`: Player id that owns the projectile.
/// - `start`: Projectile start position.
/// - `end`: Projectile end position.
/// - `elapsed`: Elapsed projectile lifetime in seconds.
/// - `travel_seconds`: Server-authoritative travel duration.
/// - `projectile_radius`: Server-authoritative hit radius.
/// - `explosion_radius`: Server-authoritative terminal explosion radius.
/// - `direct_hit_damage`: Server-authoritative damage applied by pass-through hits.
/// - `area_damage`: Server-authoritative damage applied by the terminal explosion.
/// - `hit_targets`: Player ids already hit by the pass-through projectile.
/// - `hit_lane_unit_ids`: Lane unit ids already hit by the pass-through projectile.
#[derive(Debug, Clone)]
struct ServerQProjectile {
    caster_player_id: u64,
    start: Vec3,
    end: Vec3,
    elapsed: f32,
    travel_seconds: f32,
    projectile_radius: f32,
    explosion_radius: f32,
    direct_hit_damage: f32,
    area_damage: f32,
    hit_targets: Vec<u64>,
    hit_lane_unit_ids: Vec<u64>,
}
/// Stores one active server-authoritative Lira W projectile.
///
/// - `caster_player_id`: Player id that owns the projectile.
/// - `end`: Projectile landing position.
/// - `elapsed`: Elapsed projectile lifetime in seconds.
/// - `travel_seconds`: Server-authoritative travel duration.
/// - `explosion_radius`: Server-authoritative landing explosion radius.
/// - `area_damage`: Server-authoritative damage applied by the landing explosion.
#[derive(Debug, Clone, Copy)]
struct ServerWProjectile {
    caster_player_id: u64,
    end: Vec3,
    elapsed: f32,
    travel_seconds: f32,
    explosion_radius: f32,
    area_damage: f32,
}
/// Stores one active server-authoritative Lira E missile.
///
/// - `caster_player_id`: Player id that owns the missile.
/// - `position`: Current missile position.
/// - `phase`: Orbit phase offset.
/// - `elapsed`: Elapsed missile lifetime in seconds.
/// - `damage`: Server-authoritative damage applied by missile contact.
/// - `lifetime_seconds`: Server-authoritative missile lifetime.
/// - `search_radius`: Server-authoritative target search radius.
/// - `orbit_radius`: Server-authoritative orbit radius.
/// - `orbit_height`: Server-authoritative orbit height.
/// - `orbit_speed`: Server-authoritative orbit speed.
/// - `chase_speed`: Server-authoritative chase speed.
/// - `missile_radius`: Server-authoritative hit radius.
/// - `mode`: Current missile behavior mode.
#[derive(Debug, Clone, Copy)]
struct ServerEMissile {
    caster_player_id: u64,
    position: Vec3,
    phase: f32,
    elapsed: f32,
    damage: f32,
    lifetime_seconds: f32,
    search_radius: f32,
    orbit_radius: f32,
    orbit_height: f32,
    orbit_speed: f32,
    chase_speed: f32,
    missile_radius: f32,
    mode: ServerEMissileMode,
}
/// Defines the server-side behavior mode for one Lira E missile.
///
/// - `Orbiting`: Missile is orbiting the caster and searching for a target.
/// - `Chasing`: Missile is chasing the stored player or lane-minion target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerEMissileMode {
    Orbiting,
    Chasing(NetworkTargetId),
}
/// Stores one server-authoritative Ignara Q burning ground zone.
#[derive(Debug, Clone)]
struct ServerIgnaraQZone {
    caster_player_id: u64,
    start: Vec3,
    end: Vec3,
    elapsed: f32,
    lifetime_seconds: f32,
    width: f32,
    damage_per_second: f32,
}
/// Stores one server-authoritative Ignara W fireball.
#[derive(Debug, Clone, Copy)]
struct ServerIgnaraWFireball {
    caster_player_id: u64,
    target: NetworkTargetId,
    elapsed: f32,
    travel_seconds: f32,
    damage: f32,
}
/// Stores one server-authoritative Ignara E rolling snowball.
#[derive(Debug, Clone)]
struct ServerIgnaraESnowball {
    caster_player_id: u64,
    start: Vec3,
    end: Vec3,
    elapsed: f32,
    travel_seconds: f32,
    range: f32,
    width: f32,
    small_distance: f32,
    medium_distance: f32,
    small_damage: f32,
    medium_damage: f32,
    large_damage: f32,
    hit_targets: Vec<u64>,
    hit_lane_unit_ids: Vec<u64>,
}
/// Stores one server-authoritative Yuna Q gravity field.
#[derive(Debug, Clone)]
struct ServerYunaQOrb {
    caster_player_id: u64,
    position: Vec3,
    elapsed: f32,
    travel_seconds: f32,
    lifetime_seconds: f32,
    radius: f32,
    damage_per_second: f32,
    pull_speed: f32,
    move_speed_multiplier: f32,
}
/// Stores one server-authoritative Yuna W healing field.
#[derive(Debug, Clone)]
struct ServerYunaWField {
    caster_player_id: u64,
    elapsed: f32,
    tick_elapsed: f32,
    lifetime_seconds: f32,
    radius: f32,
    heal: f32,
}
/// Stores one server-authoritative Sophia Q damage orb attached to an enemy target.
#[derive(Debug, Clone)]
struct ServerSophiaQOrb {
    caster_player_id: u64,
    target: NetworkTargetId,
    elapsed: f32,
    tick_elapsed: f32,
    lifetime_seconds: f32,
    damage_per_second: f32,
}
/// Stores one server-authoritative Sophia W minion.
#[derive(Debug, Clone)]
struct ServerSophiaMinion {
    caster_player_id: u64,
    position: Vec3,
    phase: f32,
    elapsed: f32,
    lifetime_seconds: f32,
    search_radius: f32,
    chase_speed: f32,
    radius: f32,
    damage: f32,
    slow_seconds: f32,
    slow_multiplier: f32,
    target: Option<NetworkTargetId>,
}
/// Limits how often the development server broadcasts match roster snapshots.
///
/// - `0`: Repeating timer for snapshot broadcasts.
#[derive(Resource, Debug)]
pub(super) struct MatchSnapshotBroadcastTimer(Timer);

/// Limits loading-screen status broadcasts so they cannot crowd out gameplay state.
#[derive(Resource, Debug)]
pub(super) struct LoadingScreenStatusBroadcastTimer(Timer);
/// Tracks connected clients that already received the current champion catalog.
///
/// - `0`: Netcode player ids that have received the catalog update.
#[derive(Resource, Debug, Default)]
pub(super) struct SentChampionCatalogClients(HashSet<u64>);
/// Tracks clients whose display has finished loading local match visuals.
#[derive(Resource, Debug, Default)]
pub(super) struct LoadingScreenReadyPlayers {
    ready_player_ids: HashSet<u64>,
}
/// Tracks players that intentionally left but whose transport connection may not have timed out yet.
#[derive(Resource, Debug, Default)]
pub(super) struct LeavingPlayers {
    player_ids: HashSet<u64>,
}

impl Default for MatchSnapshotBroadcastTimer {
    /// Returns the default configuration used by the dedicated server lobby simulation system.
    fn default() -> Self {
        Self(Timer::from_seconds(
            MATCH_SNAPSHOT_INTERVAL_SECONDS,
            TimerMode::Repeating,
        ))
    }
}

impl Default for LoadingScreenStatusBroadcastTimer {
    /// Returns the default loading-screen status broadcast interval.
    fn default() -> Self {
        Self(Timer::from_seconds(
            LOADING_SCREEN_STATUS_INTERVAL_SECONDS,
            TimerMode::Repeating,
        ))
    }
}

impl LoadingScreenReadyPlayers {
    pub(super) fn has_ready_players(&self) -> bool {
        !self.ready_player_ids.is_empty()
    }
}
/// Receives explicit leave messages so players disappear before the transport timeout.
pub(super) fn receive_client_leave(
    mut clients: Query<
        (&RemoteId, &mut MessageReceiver<ClientLeave>),
        (With<ClientOf>, With<Connected>),
    >,
    mut players: ResMut<ConnectedPlayers>,
    mut player_navigation: ResMut<ServerPlayerNavigation>,
    mut ready_players: ResMut<LoadingScreenReadyPlayers>,
    mut leaving_players: ResMut<LeavingPlayers>,
) {
    for (remote_id, mut receiver) in &mut clients {
        let Some(player_id) = netcode_player_id(*remote_id) else {
            continue;
        };

        if receiver.receive().next().is_none() {
            continue;
        }

        mark_player_departed(
            player_id,
            &mut players,
            &mut player_navigation,
            &mut ready_players,
            &mut leaving_players,
        );
    }
}

/// Removes a disconnected player's runtime state from the active match.
pub(super) fn handle_client_disconnection(
    trigger: On<Add, Disconnected>,
    remote_ids: Query<&RemoteId, With<ClientOf>>,
    mut players: ResMut<ConnectedPlayers>,
    mut player_navigation: ResMut<ServerPlayerNavigation>,
    mut ready_players: ResMut<LoadingScreenReadyPlayers>,
    mut leaving_players: ResMut<LeavingPlayers>,
    mut sent_catalog_clients: ResMut<SentChampionCatalogClients>,
) {
    let Ok(remote_id) = remote_ids.get(trigger.entity) else {
        return;
    };
    let Some(player_id) = netcode_player_id(*remote_id) else {
        return;
    };

    cleanup_disconnected_player(
        player_id,
        &mut players,
        &mut player_navigation,
        &mut ready_players,
        &mut leaving_players,
        &mut sent_catalog_clients,
    );
}

/// Marks a player as departed and clears all gameplay state owned by that player.
fn mark_player_departed(
    player_id: u64,
    players: &mut ConnectedPlayers,
    player_navigation: &mut ServerPlayerNavigation,
    ready_players: &mut LoadingScreenReadyPlayers,
    leaving_players: &mut LeavingPlayers,
) {
    leaving_players.player_ids.insert(player_id);
    ready_players.ready_player_ids.remove(&player_id);
    players.states.remove(&player_id);
    player_navigation.clear(player_id);
}

/// Clears state that belongs to a transport client after its link disconnects.
fn cleanup_disconnected_player(
    player_id: u64,
    players: &mut ConnectedPlayers,
    player_navigation: &mut ServerPlayerNavigation,
    ready_players: &mut LoadingScreenReadyPlayers,
    leaving_players: &mut LeavingPlayers,
    sent_catalog_clients: &mut SentChampionCatalogClients,
) {
    mark_player_departed(
        player_id,
        players,
        player_navigation,
        ready_players,
        leaving_players,
    );
    sent_catalog_clients.0.remove(&player_id);
}
/// Receives client display-ready signals after each client has loaded local visuals.
pub(super) fn receive_display_ready(
    mut clients: Query<
        (&RemoteId, &mut MessageReceiver<DisplayReady>),
        (With<ClientOf>, With<Connected>),
    >,
    mut ready_players: ResMut<LoadingScreenReadyPlayers>,
    mut leaving_players: ResMut<LeavingPlayers>,
    manifest: Res<ServerMatchManifest>,
) {
    let connected_player_ids = clients
        .iter()
        .filter_map(|(remote_id, _)| netcode_player_id(*remote_id))
        .filter(|player_id| !leaving_players.player_ids.contains(player_id))
        .collect::<HashSet<_>>();
    ready_players
        .ready_player_ids
        .retain(|player_id| connected_player_ids.contains(player_id));

    for (remote_id, mut receiver) in &mut clients {
        let Some(player_id) = netcode_player_id(*remote_id) else {
            continue;
        };
        if manifest.is_enforced() && manifest.player(player_id).is_none() {
            continue;
        }

        for _ in receiver.receive() {
            leaving_players.player_ids.remove(&player_id);
            ready_players.ready_player_ids.insert(player_id);
        }
    }
}
/// Broadcasts current loading-screen readiness without competing with gameplay state snapshots.
pub(super) fn broadcast_loading_screen_status(
    mut clients: Query<
        (&RemoteId, &mut MessageSender<LoadingScreenStatus>),
        (With<ClientOf>, With<Connected>),
    >,
    mut ready_players: ResMut<LoadingScreenReadyPlayers>,
    leaving_players: Res<LeavingPlayers>,
    manifest: Res<ServerMatchManifest>,
    players: Res<ConnectedPlayers>,
    mut timer: ResMut<LoadingScreenStatusBroadcastTimer>,
    time: Res<Time>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let connected_player_ids = clients
        .iter()
        .filter_map(|(remote_id, _)| netcode_player_id(*remote_id))
        .filter(|player_id| !leaving_players.player_ids.contains(player_id))
        .collect::<HashSet<_>>();
    ready_players
        .ready_player_ids
        .retain(|player_id| connected_player_ids.contains(player_id));

    let expected_player_ids = expected_loading_player_ids(&manifest, &connected_player_ids);
    let total_players = expected_player_ids
        .len()
        .max(connected_player_ids.len())
        .max(1);
    let mut ready_player_ids = ready_players
        .ready_player_ids
        .iter()
        .copied()
        .filter(|player_id| {
            expected_player_ids.is_empty() || expected_player_ids.contains(player_id)
        })
        .collect::<Vec<_>>();
    ready_player_ids.sort_unstable();
    let ready_count = ready_player_ids.len();
    let can_close = ready_count >= total_players;
    let loading_players = loading_screen_players(
        &manifest,
        &players,
        &connected_player_ids,
        &ready_player_ids,
    );

    for (_, mut sender) in &mut clients {
        sender.send::<ReliableCommandChannel>(LoadingScreenStatus {
            ready_players: ready_count,
            total_players,
            ready_player_ids: ready_player_ids.clone(),
            players: loading_players.clone(),
            can_close,
        });
    }
}
/// Sends the loaded server champion catalog once to each connected client.
///
/// - `clients`: Connected client links that can receive the champion catalog.
/// - `catalog`: Server-authoritative champion catalog loaded from the champion API.
/// - `sent_clients`: Tracks which client ids already received this catalog.
pub(super) fn send_champion_catalogs(
    mut clients: Query<
        (&RemoteId, &mut MessageSender<ChampionCatalogUpdate>),
        (With<ClientOf>, With<Connected>),
    >,
    catalog: Res<ServerChampionCatalog>,
    mut sent_clients: ResMut<SentChampionCatalogClients>,
) {
    let connected_player_ids = clients
        .iter()
        .filter_map(|(remote_id, _)| netcode_player_id(*remote_id))
        .collect::<HashSet<_>>();
    sent_clients
        .0
        .retain(|player_id| connected_player_ids.contains(player_id));

    for (remote_id, mut sender) in &mut clients {
        let Some(player_id) = netcode_player_id(*remote_id) else {
            continue;
        };
        if sent_clients.0.contains(&player_id) {
            continue;
        }

        sender.send::<ReliableCommandChannel>(catalog.catalog_update());
        sent_clients.0.insert(player_id);
    }
}
/// Receives local player state updates sent by connected clients.
///
/// - `clients`: Connected client links that may contain player state update messages.
/// - `players`: Server-side development player state cache.
/// - `catalog`: Server-authoritative champion content catalog.
pub(super) fn receive_player_state_updates(
    mut clients: Query<
        (&RemoteId, &mut MessageReceiver<PlayerStateUpdate>),
        (With<ClientOf>, With<Connected>),
    >,
    mut players: ResMut<ConnectedPlayers>,
    catalog: Res<ServerChampionCatalog>,
    leaving_players: Res<LeavingPlayers>,
    manifest: Res<ServerMatchManifest>,
) {
    for (remote_id, mut receiver) in &mut clients {
        let Some(player_id) = netcode_player_id(*remote_id) else {
            continue;
        };
        if leaving_players.player_ids.contains(&player_id) {
            continue;
        }
        let Some(match_player) = authorized_match_player(&manifest, player_id) else {
            continue;
        };

        for update in receiver.receive() {
            let champion = match_player
                .as_ref()
                .map_or(update.champion, |player| player.champion);
            let team = match_player
                .as_ref()
                .map_or(update.team, |player| player.team);
            let max_health = development_champion_max_health(&catalog, champion);
            players
                .states
                .entry(player_id)
                .and_modify(|state| {
                    state.max_health = max_health;
                    if state.champion != champion {
                        state.champion = champion;
                        state.health = max_health;
                        state.auto_attack_combo_stage = 0;
                        state.auto_attack_combo_target = None;
                        state.auto_attack_combo_reset_timer = 0.0;
                    }
                    state.team = team.into();
                    if update.yaw.is_finite() {
                        state.yaw = update.yaw;
                    }
                    if state.health <= 0.0
                        || state.respawn_input_grace > 0.0
                        || state.stun_timer > 0.0
                    {
                        state.moving = false;
                    }
                })
                .or_insert(ConnectedPlayerState {
                    position: lane_spawn_position(team),
                    position_correction_generation: 0,
                    yaw: if update.yaw.is_finite() {
                        update.yaw
                    } else {
                        lane_forward_yaw(team)
                    },
                    moving: false,
                    health: max_health,
                    max_health,
                    champion,
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
                    slow_multiplier: DEFAULT_MOVEMENT_SPEED_MULTIPLIER,
                    stun_timer: 0.0,
                    team: team.into(),
                    respawn_timer: None,
                    respawn_generation: 0,
                    respawn_input_grace: RESPAWN_INPUT_GRACE_SECONDS,
                });
        }
    }
}
/// Receives authoritative player commands and resolves supported server-side abilities.
///
/// - `clients`: Connected client links with command receivers and ability visual senders.
/// - `players`: Server-side development player state cache.
/// - `abilities`: Active server-side ability simulations.
/// - `catalog`: Server-authoritative champion content catalog.
/// - `time`: Bevy time resource used to advance ability cooldowns.
pub(super) fn receive_player_commands(
    mut clients: ParamSet<(
        Query<(&RemoteId, &mut MessageReceiver<PlayerCommand>), (With<ClientOf>, With<Connected>)>,
        Query<
            (&RemoteId, &mut MessageSender<AbilityVisualEvent>),
            (With<ClientOf>, With<Connected>),
        >,
        Query<
            (&RemoteId, &mut MessageSender<AutoAttackVisualEvent>),
            (With<ClientOf>, With<Connected>),
        >,
    )>,
    mut players: ResMut<ConnectedPlayers>,
    mut player_navigation: ResMut<ServerPlayerNavigation>,
    mut abilities: ResMut<ActiveServerAbilities>,
    mut lane: ResMut<ServerLaneState>,
    catalog: Res<ServerChampionCatalog>,
    leaving_players: Res<LeavingPlayers>,
    manifest: Res<ServerMatchManifest>,
    time: Res<Time>,
) {
    tick_ability_cooldowns(&mut players, time.delta_secs());

    let mut visual_events = Vec::new();
    let mut auto_attack_visual_events = Vec::new();

    {
        let mut receivers = clients.p0();
        for (remote_id, mut receiver) in &mut receivers {
            let Some(caster_player_id) = netcode_player_id(*remote_id) else {
                continue;
            };
            if leaving_players.player_ids.contains(&caster_player_id) {
                continue;
            }

            for command in receiver.receive() {
                if let PlayerCommand::CastAbility { champion, .. } = command
                    && !authorized_champion(&manifest, caster_player_id, champion)
                {
                    continue;
                }

                match command {
                    PlayerCommand::MoveTo(target) => {
                        let target = Vec3::from(target);
                        if target.x.is_finite()
                            && target.y.is_finite()
                            && target.z.is_finite()
                            && players.states.get(&caster_player_id).is_some_and(|state| {
                                state.health > 0.0
                                    && state.respawn_input_grace <= 0.0
                                    && state.stun_timer <= 0.0
                            })
                        {
                            player_navigation.request_move(
                                caster_player_id,
                                clamp_player_position_to_lane(target),
                            );
                        }
                    }
                    PlayerCommand::AttackMove { target }
                        if players.states.get(&caster_player_id).is_some_and(|state| {
                            state.health > 0.0
                                && state.respawn_input_grace <= 0.0
                                && state.stun_timer <= 0.0
                        }) =>
                    {
                        player_navigation.request_attack_move(caster_player_id, target);
                    }
                    PlayerCommand::CastAbility {
                        champion,
                        slot,
                        target,
                    } if champion == ChampionId::LIRA && slot == AbilitySlot::Q => {
                        if let Some(event) = accept_lira_q_cast(
                            caster_player_id,
                            target.position,
                            &mut players,
                            &mut abilities,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility {
                        champion,
                        slot,
                        target,
                    } if champion == ChampionId::LIRA && slot == AbilitySlot::W => {
                        if let Some(event) = accept_lira_w_cast(
                            caster_player_id,
                            target.position,
                            &mut players,
                            &mut abilities,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility { champion, slot, .. }
                        if champion == ChampionId::LIRA && slot == AbilitySlot::E =>
                    {
                        if let Some(event) = accept_lira_e_cast(
                            caster_player_id,
                            &mut players,
                            &mut abilities,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility {
                        champion,
                        slot,
                        target,
                    } if champion == ChampionId::IGNARA && slot == AbilitySlot::Q => {
                        if let Some(event) = accept_ignara_q_cast(
                            caster_player_id,
                            target.position,
                            &mut players,
                            &mut abilities,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility {
                        champion,
                        slot,
                        target,
                    } if champion == ChampionId::IGNARA && slot == AbilitySlot::W => {
                        if let Some(event) = accept_ignara_w_cast(
                            caster_player_id,
                            target.position,
                            &mut players,
                            &lane,
                            &mut abilities,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility {
                        champion,
                        slot,
                        target,
                    } if champion == ChampionId::IGNARA && slot == AbilitySlot::E => {
                        if let Some(event) = accept_ignara_e_cast(
                            caster_player_id,
                            target.position,
                            &mut players,
                            &mut abilities,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility {
                        champion,
                        slot,
                        target,
                    } if champion == ChampionId::YUNA && slot == AbilitySlot::Q => {
                        if let Some(event) = accept_yuna_q_cast(
                            caster_player_id,
                            target.position,
                            &mut players,
                            &mut abilities,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility {
                        champion,
                        slot,
                        target,
                    } if champion == ChampionId::YUNA && slot == AbilitySlot::W => {
                        if let Some(event) = accept_yuna_w_cast(
                            caster_player_id,
                            target.position,
                            &mut players,
                            &mut abilities,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility {
                        champion,
                        slot,
                        target,
                    } if champion == ChampionId::YUNA && slot == AbilitySlot::E => {
                        if let Some(event) = accept_yuna_e_cast(
                            caster_player_id,
                            target.position,
                            &mut players,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility {
                        champion,
                        slot,
                        target,
                    } if champion == ChampionId::SOPHIA && slot == AbilitySlot::Q => {
                        if let Some(event) = accept_sophia_q_cast(
                            caster_player_id,
                            target.position,
                            &mut players,
                            &lane,
                            &mut abilities,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility { champion, slot, .. }
                        if champion == ChampionId::SOPHIA && slot == AbilitySlot::W =>
                    {
                        if let Some(event) = accept_sophia_w_cast(
                            caster_player_id,
                            &mut players,
                            &mut abilities,
                            &catalog,
                        ) {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::CastAbility { champion, slot, .. }
                        if champion == ChampionId::SOPHIA && slot == AbilitySlot::E =>
                    {
                        if let Some(event) =
                            accept_sophia_e_cast(caster_player_id, &mut players, &catalog)
                        {
                            visual_events.push(event);
                        }
                    }
                    PlayerCommand::AutoAttack { target } => {
                        if let Some(event) = accept_auto_attack_target(
                            caster_player_id,
                            target,
                            &mut players,
                            Some(&mut lane),
                            &mut abilities,
                            &catalog,
                        ) {
                            auto_attack_visual_events.push(event);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if visual_events.is_empty() && auto_attack_visual_events.is_empty() {
        return;
    }

    if !visual_events.is_empty() {
        let mut senders = clients.p1();
        for event in visual_events {
            for (remote_id, mut sender) in &mut senders {
                if netcode_player_id(*remote_id) == Some(event.caster_player_id) {
                    continue;
                }

                sender.send::<ReliableCommandChannel>(event);
            }
        }
    }

    if !auto_attack_visual_events.is_empty() {
        let mut senders = clients.p2();
        for event in auto_attack_visual_events {
            for (remote_id, mut sender) in &mut senders {
                if netcode_player_id(*remote_id) == Some(event.caster_player_id) {
                    continue;
                }

                sender.send::<ReliableCommandChannel>(event);
            }
        }
    }
}

/// Advances server-authoritative player movement along navigation-mesh waypoints.
pub(super) fn update_server_player_navigation(
    time: Res<Time>,
    mut navigation: ResMut<ServerPlayerNavigation>,
    mut players: ResMut<ConnectedPlayers>,
    mut lane: ResMut<ServerLaneState>,
    abilities: Res<ActiveServerAbilities>,
) {
    advance_server_player_navigation(
        &mut navigation,
        &mut players,
        &mut lane,
        &abilities,
        time.delta_secs(),
    );
}

fn advance_server_player_navigation(
    navigation: &mut ServerPlayerNavigation,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    abilities: &ActiveServerAbilities,
    delta_seconds: f32,
) {
    let obstacle_revision = lane.navigation_obstacle_revision();
    let teams_by_player_id = players
        .states
        .iter()
        .map(|(player_id, state)| (*player_id, state.team))
        .collect::<HashMap<_, _>>();
    let mut player_ids = navigation.paths.keys().copied().collect::<Vec<_>>();
    player_ids.sort_unstable();

    for player_id in player_ids {
        let Some(player) = players.states.get(&player_id).copied() else {
            navigation.clear(player_id);
            continue;
        };
        if player.health <= 0.0 || player.respawn_input_grace > 0.0 || player.stun_timer > 0.0 {
            if let Some(player) = players.states.get_mut(&player_id) {
                player.moving = false;
            }
            navigation.clear(player_id);
            continue;
        }

        // Recover an invalid authoritative position before planning. This can happen when a
        // structure appears around a player between snapshots or after an external displacement.
        let recovered_position = lane.resolve_structure_collision(
            player.position,
            player.position,
            DEVELOPMENT_PLAYER_HIT_RADIUS,
        );
        let player = if horizontal_distance(recovered_position, player.position)
            > PLAYER_NAVIGATION_COLLISION_REPLAN_DISTANCE
        {
            if let Some(state) = players.states.get_mut(&player_id) {
                state.position = recovered_position;
                state.moving = false;
            }
            if let Some(route) = navigation.paths.get_mut(&player_id) {
                route.planned = false;
            }
            players.states.get(&player_id).copied().unwrap_or(player)
        } else {
            player
        };

        let attack_target = navigation
            .paths
            .get(&player_id)
            .and_then(|route| route.attack_target);
        let resolved_attack_target = attack_target.and_then(|target| {
            attack_move_target_position(player_id, player, target, players, lane)
        });
        if attack_target.is_some() && resolved_attack_target.is_none() {
            if let Some(player) = players.states.get_mut(&player_id) {
                player.moving = false;
            }
            navigation.clear(player_id);
            continue;
        }

        if let Some((target_position, target_radius)) = resolved_attack_target {
            let target_in_attack_range = horizontal_distance(player.position, target_position)
                <= AUTO_ATTACK_RANGE + target_radius;
            let Some(route) = navigation.paths.get_mut(&player_id) else {
                continue;
            };
            let target_moved = route
                .attack_target_position
                .is_none_or(|previous_position| {
                    horizontal_distance(previous_position, target_position)
                        > ATTACK_MOVE_TARGET_UPDATE_DISTANCE
                });
            if target_moved {
                route.requested_target = clamp_player_position_to_lane(
                    route
                        .attack_target_position
                        .map(|previous_position| {
                            route.requested_target
                                + Vec3::new(
                                    target_position.x - previous_position.x,
                                    0.0,
                                    target_position.z - previous_position.z,
                                )
                        })
                        .unwrap_or_else(|| {
                            attack_move_approach_goal(
                                player.position,
                                target_position,
                                AUTO_ATTACK_RANGE + target_radius,
                            )
                        }),
                );
                route.attack_target_position = Some(target_position);
                route.planned = false;
            }
            if target_in_attack_range {
                route.waypoints.clear();
                route.recovery_waypoint = None;
                route.planned = true;
                if let Some(player) = players.states.get_mut(&player_id) {
                    player.moving = false;
                }
                continue;
            }
            if route.waypoints.is_empty() {
                route.planned = false;
            }
        }

        let should_replan = navigation.paths.get(&player_id).is_some_and(|route| {
            !route.planned || route.obstacle_revision != Some(obstacle_revision)
        });
        if should_replan {
            let target = navigation
                .paths
                .get(&player_id)
                .map(|route| route.requested_target)
                .unwrap_or(player.position);
            let (route_revision, path) = lane.navigation_path_with_projection_for_mover(
                player.position,
                target,
                DEVELOPMENT_PLAYER_HIT_RADIUS,
            );
            let (reachable_target, recovery_waypoint, waypoints) = match path {
                Some(mut path) => {
                    let has_recovery_waypoint =
                        path.prepend_start_recovery_waypoint(player.position);
                    let reachable_target = path.waypoints.last().copied().unwrap_or(target);
                    (
                        reachable_target,
                        has_recovery_waypoint.then_some(path.start),
                        path.waypoints.into(),
                    )
                }
                None => (target, None, VecDeque::new()),
            };
            if let Some(route) = navigation.paths.get_mut(&player_id) {
                route.obstacle_revision = Some(route_revision);
                route.waypoints = waypoints;
                route.recovery_waypoint = recovery_waypoint;
                route.target = reachable_target;
                route.planned = true;
            }
        }

        let Some(route) = navigation.paths.get_mut(&player_id) else {
            continue;
        };
        while let Some(waypoint) = route.waypoints.front().copied() {
            let reached_distance = if route.recovery_waypoint == Some(waypoint) {
                PLAYER_NAVIGATION_RECOVERY_WAYPOINT_REACHED_DISTANCE
            } else {
                PLAYER_NAVIGATION_WAYPOINT_REACHED_DISTANCE
            };
            if horizontal_distance(player.position, waypoint) > reached_distance {
                break;
            }
            route.waypoints.pop_front();
            if route.recovery_waypoint == Some(waypoint) {
                route.recovery_waypoint = None;
            }
        }
        let target = route.target;
        let waypoint = route.waypoints.front().copied();
        let is_attack_move = route.attack_target.is_some();
        let reached_target = waypoint.is_none()
            && horizontal_distance(player.position, target)
                <= PLAYER_NAVIGATION_WAYPOINT_REACHED_DISTANCE;
        let _ = route;

        let Some(waypoint) = waypoint else {
            if let Some(player) = players.states.get_mut(&player_id) {
                player.moving = false;
            }
            if reached_target && !is_attack_move {
                navigation.clear(player_id);
            }
            continue;
        };

        let desired_position = step_toward_player_navigation(
            player.position,
            waypoint,
            SERVER_PLAYER_NAVIGATION_SPEED
                * movement_speed_multiplier(
                    &player,
                    false,
                    yuna_pull_center_for_player(abilities, &teams_by_player_id, player_id, &player),
                )
                * delta_seconds.max(0.0),
        );
        let resolved_position = lane.resolve_structure_collision(
            player.position,
            desired_position,
            DEVELOPMENT_PLAYER_HIT_RADIUS,
        );
        let movement = Vec3::new(
            resolved_position.x - player.position.x,
            0.0,
            resolved_position.z - player.position.z,
        );
        let route_was_blocked = horizontal_distance(resolved_position, desired_position)
            > PLAYER_NAVIGATION_COLLISION_REPLAN_DISTANCE;
        if let Some(player) = players.states.get_mut(&player_id) {
            player.position = resolved_position;
            player.moving = movement.length_squared() > f32::EPSILON;
            if player.moving {
                player.yaw = movement.x.atan2(movement.z);
            }
        }
        if route_was_blocked && let Some(route) = navigation.paths.get_mut(&player_id) {
            route.planned = false;
        }
    }

    navigation
        .paths
        .retain(|player_id, _| players.states.contains_key(player_id));
}

/// Resolves the current hostile target tracked by a server-authoritative attack-move order.
fn attack_move_target_position(
    caster_player_id: u64,
    caster: ConnectedPlayerState,
    target: NetworkTargetId,
    players: &ConnectedPlayers,
    lane: &ServerLaneState,
) -> Option<(Vec3, f32)> {
    let caster_team = TeamSpec::from(caster.team);
    if !caster_team.is_playable() {
        return None;
    }

    match target {
        NetworkTargetId::Player(target_player_id) => {
            if target_player_id == caster_player_id {
                return None;
            }
            let target_player = players.states.get(&target_player_id)?;
            (target_player.health > 0.0
                && target_player.team != caster.team
                && TeamSpec::from(target_player.team).is_playable())
            .then_some((target_player.position, DEVELOPMENT_PLAYER_HIT_RADIUS))
        }
        NetworkTargetId::LaneUnit(target_unit_id) => {
            lane.target_for_player_auto_attack(caster_team, target_unit_id)
        }
    }
}

/// Returns an approach point that remains inside legal basic-attack range after route arrival.
fn attack_move_approach_goal(
    player_position: Vec3,
    target_position: Vec3,
    attack_range: f32,
) -> Vec3 {
    let offset = Vec3::new(
        player_position.x - target_position.x,
        0.0,
        player_position.z - target_position.z,
    );
    let direction = if offset.length_squared() <= f32::EPSILON {
        Vec3::Z
    } else {
        offset.normalize()
    };
    target_position + direction * (attack_range - ATTACK_MOVE_INNER_RANGE_MARGIN).max(0.0)
}

fn step_toward_player_navigation(position: Vec3, target: Vec3, max_distance: f32) -> Vec3 {
    let offset = Vec3::new(target.x - position.x, 0.0, target.z - position.z);
    let distance = offset.length();
    if distance <= f32::EPSILON || distance <= max_distance {
        target
    } else {
        position + offset * (max_distance / distance)
    }
}

/// Regenerates health for living players while the match has ready clients.
pub(super) fn update_player_health_regeneration(
    mut players: ResMut<ConnectedPlayers>,
    ready_players: Res<LoadingScreenReadyPlayers>,
    time: Res<Time>,
) {
    if !ready_players.has_ready_players() {
        return;
    }

    regenerate_player_health(&mut players, time.delta_secs());
}

fn regenerate_player_health(players: &mut ConnectedPlayers, delta_seconds: f32) {
    let health_regeneration =
        DEFAULT_PLAYER_HEALTH_REGENERATION_PER_SECOND * delta_seconds.max(0.0);

    for player in players.states.values_mut() {
        if player.health <= 0.0 {
            continue;
        }

        player.health = (player.health + health_regeneration).min(player.max_health);
    }
}

/// Advances launched player auto-attack projectiles and applies their impact damage.
///
/// This system runs before player commands are received, so an attack accepted in the current
/// update cannot consume the current frame's delta time before its projectile is visible.
pub(super) fn update_server_auto_attack_projectiles(
    mut abilities: ResMut<ActiveServerAbilities>,
    mut players: ResMut<ConnectedPlayers>,
    mut lane: ResMut<ServerLaneState>,
    mut combat_events: ResMut<ServerCombatNumberEvents>,
    ready_players: Res<LoadingScreenReadyPlayers>,
    time: Res<Time>,
) {
    reset_active_abilities_without_ready_players(&mut abilities, &ready_players);
    if !ready_players.has_ready_players() {
        return;
    }

    advance_server_auto_attack_projectiles(
        &mut abilities,
        &mut players,
        &mut lane,
        &mut combat_events,
        time.delta_secs(),
    );
}

fn advance_server_auto_attack_projectiles(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    delta_seconds: f32,
) {
    let mut impacts = Vec::new();
    abilities.auto_attack_projectiles.retain_mut(|projectile| {
        projectile.remaining_seconds -= delta_seconds.max(0.0);
        if projectile.remaining_seconds > 0.0 {
            return true;
        }

        impacts.push(*projectile);
        false
    });

    for impact in impacts {
        match impact.target {
            NetworkTargetId::Player(target_player_id) => {
                let Some(target) = players.states.get_mut(&target_player_id) else {
                    continue;
                };
                let target_team = TeamSpec::from(target.team);
                if target.health <= 0.0
                    || !target_team.is_playable()
                    || target_team == impact.caster_team
                {
                    continue;
                }

                apply_damage(
                    combat_events,
                    target_player_id,
                    target,
                    impact.damage,
                    NetworkCombatNumberKind::AutoAttack,
                );
            }
            NetworkTargetId::LaneUnit(target_unit_id) => {
                lane.apply_player_auto_attack_impact(
                    impact.caster_team,
                    target_unit_id,
                    impact.damage,
                );
            }
        }
    }
}

/// Advances active server-authoritative ability simulations and applies contact damage.
///
/// - `abilities`: Active server-side ability simulations.
/// - `players`: Server-side development player state cache.
/// - `lane`: Server-side lane state that receives projectile impacts.
/// - `ready_players`: Clients whose loaded game state keeps the lane active.
/// - `time`: Bevy time resource used to advance projectile and missile movement.
pub(super) fn update_server_abilities(
    mut abilities: ResMut<ActiveServerAbilities>,
    mut players: ResMut<ConnectedPlayers>,
    mut lane: ResMut<ServerLaneState>,
    mut combat_events: ResMut<ServerCombatNumberEvents>,
    catalog: Res<ServerChampionCatalog>,
    ready_players: Res<LoadingScreenReadyPlayers>,
    time: Res<Time>,
) {
    reset_active_abilities_without_ready_players(&mut abilities, &ready_players);
    if !ready_players.has_ready_players() {
        return;
    }

    let delta_seconds = time.delta_secs();
    update_lira_q_projectiles(
        &mut abilities,
        &mut players,
        &mut lane,
        &mut combat_events,
        delta_seconds,
    );
    update_lira_w_projectiles(
        &mut abilities,
        &mut players,
        &mut lane,
        &mut combat_events,
        delta_seconds,
    );
    update_lira_e_missiles(
        &mut abilities,
        &mut players,
        &mut lane,
        &mut combat_events,
        delta_seconds,
    );
    update_ignara_q_zones(
        &mut abilities,
        &mut players,
        &mut lane,
        &mut combat_events,
        delta_seconds,
    );
    update_ignara_w_fireballs(
        &mut abilities,
        &mut players,
        &mut lane,
        &mut combat_events,
        delta_seconds,
    );
    update_ignara_e_snowballs(
        &mut abilities,
        &mut players,
        &mut lane,
        &mut combat_events,
        delta_seconds,
    );
    update_yuna_q_orbs(
        &mut abilities,
        &mut players,
        &mut lane,
        &mut combat_events,
        delta_seconds,
    );
    update_yuna_w_fields(
        &mut abilities,
        &mut players,
        &mut combat_events,
        &catalog,
        delta_seconds,
    );
    update_sophia_q_orbs(
        &mut abilities,
        &mut players,
        &mut lane,
        &mut combat_events,
        delta_seconds,
    );
    update_sophia_minions(
        &mut abilities,
        &mut players,
        &mut lane,
        &mut combat_events,
        delta_seconds,
    );
}

/// Clears active ability simulations while the lane has no ready clients.
fn reset_active_abilities_without_ready_players(
    abilities: &mut ActiveServerAbilities,
    ready_players: &LoadingScreenReadyPlayers,
) {
    if !ready_players.has_ready_players() {
        *abilities = ActiveServerAbilities::default();
    }
}
/// Advances player death timers and respawns players when their timer expires.
///
/// - `players`: Server-side development player state cache.
/// - `catalog`: Server-authoritative champion content catalog.
/// - `time`: Bevy time resource used to advance respawn timers.
pub(super) fn update_player_death_and_respawn(
    mut players: ResMut<ConnectedPlayers>,
    mut player_navigation: ResMut<ServerPlayerNavigation>,
    catalog: Res<ServerChampionCatalog>,
    time: Res<Time>,
) {
    let mut player_ids = players.states.keys().copied().collect::<Vec<_>>();
    player_ids.sort_unstable();

    for player_id in player_ids {
        let Some(state) = players.states.get_mut(&player_id) else {
            continue;
        };
        if state.health <= 0.0 {
            state.moving = false;
            player_navigation.clear(player_id);
        }
        let Some(respawn_timer) = state.respawn_timer.as_mut() else {
            continue;
        };

        *respawn_timer -= time.delta_secs();
        if *respawn_timer > 0.0 {
            continue;
        }

        state.max_health = development_champion_max_health(&catalog, state.champion);
        state.health = state.max_health;
        let team = TeamSpec::from(state.team);
        state.position = lane_spawn_position(team);
        state.yaw = lane_forward_yaw(team);
        state.moving = false;
        state.lira_q_cooldown = 0.0;
        state.lira_w_cooldown = 0.0;
        state.lira_e_cooldown = 0.0;
        state.auto_attack_cooldown = 0.0;
        state.auto_attack_combo_stage = 0;
        state.auto_attack_combo_target = None;
        state.auto_attack_combo_reset_timer = 0.0;
        state.ignara_q_cooldown = 0.0;
        state.ignara_w_cooldown = 0.0;
        state.ignara_e_cooldown = 0.0;
        state.yuna_q_cooldown = 0.0;
        state.yuna_w_cooldown = 0.0;
        state.yuna_e_cooldown = 0.0;
        state.sophia_q_cooldown = 0.0;
        state.sophia_w_cooldown = 0.0;
        state.sophia_e_cooldown = 0.0;
        state.sophia_damage_buff_timer = 0.0;
        state.sophia_speed_buff_timer = 0.0;
        state.sophia_damage_amp_available = false;
        state.slow_timer = 0.0;
        state.slow_multiplier = DEFAULT_MOVEMENT_SPEED_MULTIPLIER;
        state.stun_timer = 0.0;
        state.respawn_timer = None;
        state.respawn_generation = state.respawn_generation.saturating_add(1);
        state.respawn_input_grace = RESPAWN_INPUT_GRACE_SECONDS;
        player_navigation.clear(player_id);
    }
}
/// Broadcasts ability visual events from one client to all other connected clients.
///
/// - `clients`: Connected client links with ability visual receivers and senders.
pub(super) fn rebroadcast_ability_visuals(
    mut clients: Query<
        (
            &RemoteId,
            &mut MessageReceiver<AbilityVisualEvent>,
            &mut MessageSender<AbilityVisualEvent>,
        ),
        (With<ClientOf>, With<Connected>),
    >,
) {
    let mut events = Vec::new();
    for (remote_id, mut receiver, _) in &mut clients {
        let Some(caster_player_id) = netcode_player_id(*remote_id) else {
            continue;
        };

        for mut event in receiver.receive() {
            event.caster_player_id = caster_player_id;
            events.push(event);
        }
    }

    for event in events {
        for (remote_id, _, mut sender) in &mut clients {
            if netcode_player_id(*remote_id) == Some(event.caster_player_id) {
                continue;
            }

            sender.send::<ReliableCommandChannel>(event);
        }
    }
}
/// Sends a lightweight match roster to every connected development client.
///
/// - `clients`: Connected Lightyear client links that can receive match snapshots.
/// - `players`: Latest known server-side development player state.
/// - `manifest`: Optional match manifest used as authoritative player assignment.
/// - `catalog`: Server-authoritative champion content catalog.
/// - `timer`: Broadcast timer used to avoid sending reliable snapshots every frame.
/// - `time`: Bevy time resource used to advance the broadcast timer.
pub(super) fn broadcast_match_snapshots(
    mut clients: Query<
        (&RemoteId, &mut MessageSender<MatchSnapshot>),
        (With<ClientOf>, With<Connected>),
    >,
    mut players: ResMut<ConnectedPlayers>,
    abilities: Res<ActiveServerAbilities>,
    catalog: Res<ServerChampionCatalog>,
    manifest: Res<ServerMatchManifest>,
    leaving_players: Res<LeavingPlayers>,
    mut timer: ResMut<MatchSnapshotBroadcastTimer>,
    time: Res<Time>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let mut player_ids = clients
        .iter()
        .filter_map(|(remote_id, _)| netcode_player_id(*remote_id))
        .filter(|player_id| !leaving_players.player_ids.contains(player_id))
        .collect::<Vec<_>>();
    player_ids.sort_unstable();

    if player_ids.is_empty() {
        players.states.clear();
        return;
    }

    players
        .states
        .retain(|player_id, _| player_ids.contains(player_id));

    let teams_by_player_id = players
        .states
        .iter()
        .map(|(player_id, state)| (*player_id, state.team))
        .collect::<HashMap<_, _>>();

    let players = player_ids
        .iter()
        .map(|player_id| {
            let manifest_player = manifest.player(*player_id);
            let fallback_champion = manifest_player
                .as_ref()
                .map(|player| player.champion)
                .unwrap_or(ChampionId::LIRA);
            let fallback_team = manifest_player
                .as_ref()
                .map(|player| DevelopmentTeam::from(player.team))
                .unwrap_or(DEFAULT_DEVELOPMENT_TEAM);
            let fallback_max_health = development_champion_max_health(&catalog, fallback_champion);
            let state = players
                .states
                .entry(*player_id)
                .or_insert_with(|| ConnectedPlayerState {
                    position: lane_spawn_position(TeamSpec::from(fallback_team)),
                    position_correction_generation: 0,
                    yaw: lane_forward_yaw(TeamSpec::from(fallback_team)),
                    moving: false,
                    health: fallback_max_health,
                    max_health: fallback_max_health,
                    champion: fallback_champion,
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
                    slow_multiplier: DEFAULT_MOVEMENT_SPEED_MULTIPLIER,
                    stun_timer: 0.0,
                    team: fallback_team,
                    respawn_timer: None,
                    respawn_generation: 0,
                    respawn_input_grace: 0.0,
                });
            let champion = state.champion;
            let team = state.team;
            let max_health = development_champion_max_health(&catalog, champion);
            state.max_health = max_health;
            if (state.health - max_health).abs() > f32::EPSILON && state.health > max_health {
                state.health = max_health;
            }
            let pull_effect =
                yuna_pull_center_for_player(&abilities, &teams_by_player_id, *player_id, state);
            let stunned = state.stun_timer > 0.0;

            NetworkPlayer {
                player_id: *player_id,
                champion,
                team: team.into(),
                position: WorldPosition::from(state.position),
                position_correction_generation: state.position_correction_generation,
                yaw: state.yaw,
                moving: state.moving,
                health: state.health,
                max_health: state.max_health,
                alive: state.health > 0.0,
                stunned,
                control_locked: stunned,
                move_speed_multiplier: movement_speed_multiplier(state, stunned, pull_effect),
                pull_center: pull_effect.map(|(center, _)| WorldPosition::from(center)),
                respawn_generation: state.respawn_generation,
                respawn_seconds: state.respawn_timer.unwrap_or(0.0),
            }
        })
        .collect::<Vec<_>>();

    for (remote_id, mut sender) in &mut clients {
        let Some(local_player_id) = netcode_player_id(*remote_id) else {
            continue;
        };

        sender.send::<ReliableCommandChannel>(MatchSnapshot {
            local_player_id,
            players: players.clone(),
        });
    }
}
/// Extracts the numeric Netcode player id from a remote peer id.
///
/// - `remote_id`: Remote peer id stored on a Lightyear client link.
///
/// - Numeric player id for Netcode clients.
fn netcode_player_id(remote_id: RemoteId) -> Option<u64> {
    match remote_id.0 {
        PeerId::Netcode(player_id) => Some(player_id),
        _ => None,
    }
}
fn expected_loading_player_ids(
    manifest: &ServerMatchManifest,
    connected_player_ids: &HashSet<u64>,
) -> HashSet<u64> {
    if manifest.is_enforced() {
        return manifest.player_ids().into_iter().collect();
    }

    connected_player_ids.clone()
}
fn loading_screen_players(
    manifest: &ServerMatchManifest,
    players: &ConnectedPlayers,
    connected_player_ids: &HashSet<u64>,
    ready_player_ids: &[u64],
) -> Vec<LoadingScreenPlayer> {
    let ready_player_ids = ready_player_ids.iter().copied().collect::<HashSet<_>>();
    let mut loading_players = if manifest.is_enforced() {
        manifest
            .players()
            .into_iter()
            .map(|(player_id, player)| LoadingScreenPlayer {
                player_id,
                display_name: player.display_name,
                avatar_url: player.avatar_url,
                champion: player.champion,
                team: player.team,
                ready: ready_player_ids.contains(&player_id),
            })
            .collect::<Vec<_>>()
    } else {
        connected_player_ids
            .iter()
            .copied()
            .map(|player_id| {
                let state = players.states.get(&player_id);
                LoadingScreenPlayer {
                    player_id,
                    display_name: None,
                    avatar_url: None,
                    champion: state
                        .map(|state| state.champion)
                        .unwrap_or(ChampionId::LIRA),
                    team: state
                        .map(|state| state.team.into())
                        .unwrap_or(TeamSpec::Light),
                    ready: ready_player_ids.contains(&player_id),
                }
            })
            .collect::<Vec<_>>()
    };

    loading_players.sort_by_key(|player| {
        let team_rank = match player.team {
            TeamSpec::Light => 0,
            TeamSpec::Dark => 1,
            TeamSpec::Neutral => 2,
        };
        (team_rank, player.player_id)
    });
    loading_players
}
fn authorized_match_player(
    manifest: &ServerMatchManifest,
    player_id: u64,
) -> Option<Option<ServerMatchPlayer>> {
    if let Some(player) = manifest.player(player_id) {
        return Some(Some(player));
    }
    if manifest.is_enforced() {
        warn!(
            "Rejected player {} because they are not part of match manifest {:?}.",
            player_id, manifest.match_id
        );
        return None;
    }
    Some(None)
}
fn authorized_champion(
    manifest: &ServerMatchManifest,
    player_id: u64,
    champion: ChampionId,
) -> bool {
    let Some(match_player) = authorized_match_player(manifest, player_id) else {
        return false;
    };
    match_player
        .map(|player| player.champion == champion)
        .unwrap_or(true)
}

/// Clamps a client-reported player position to the playable single-lane map bounds.
fn clamp_player_position_to_lane(mut position: Vec3) -> Vec3 {
    position.x = position.x.clamp(-LANE_HALF_WIDTH, LANE_HALF_WIDTH);
    position.y = 0.0;
    position.z = position.z.clamp(-LANE_SPAWN_Z, LANE_SPAWN_Z);
    position
}

impl From<DevelopmentTeam> for TeamSpec {
    fn from(value: DevelopmentTeam) -> Self {
        match value {
            DevelopmentTeam::Neutral => TeamSpec::Neutral,
            DevelopmentTeam::Light => TeamSpec::Light,
            DevelopmentTeam::Dark => TeamSpec::Dark,
        }
    }
}

impl From<TeamSpec> for DevelopmentTeam {
    fn from(value: TeamSpec) -> Self {
        match value {
            TeamSpec::Neutral => DevelopmentTeam::Neutral,
            TeamSpec::Light => DevelopmentTeam::Light,
            TeamSpec::Dark => DevelopmentTeam::Dark,
        }
    }
}
/// Returns the active Yuna Q pull center affecting a player, if any.
fn yuna_pull_center_for_player(
    abilities: &ActiveServerAbilities,
    teams_by_player_id: &HashMap<u64, DevelopmentTeam>,
    player_id: u64,
    state: &ConnectedPlayerState,
) -> Option<(Vec3, f32)> {
    if state.health <= 0.0 {
        return None;
    }

    abilities
        .yuna_q_orbs
        .iter()
        .filter(|orb| {
            orb.caster_player_id != player_id
                && teams_by_player_id
                    .get(&orb.caster_player_id)
                    .is_some_and(|caster_team| *caster_team != state.team)
                && orb.elapsed >= orb.travel_seconds.max(0.0)
                && orb.elapsed
                    < orb.travel_seconds.max(0.0) + orb.lifetime_seconds.max(f32::EPSILON)
                && horizontal_distance(state.position, orb.position)
                    <= orb.radius + DEVELOPMENT_PLAYER_HIT_RADIUS
        })
        .map(|orb| (orb.position, orb.move_speed_multiplier))
        .next()
}
/// Computes the outgoing movement multiplier after control effects and buffs.
fn movement_speed_multiplier(
    state: &ConnectedPlayerState,
    stunned: bool,
    pull_effect: Option<(Vec3, f32)>,
) -> f32 {
    if stunned {
        return 0.0;
    }

    let mut multiplier = DEFAULT_MOVEMENT_SPEED_MULTIPLIER;
    if let Some((_, pull_multiplier)) = pull_effect {
        multiplier *= pull_multiplier;
    }
    if state.slow_timer > 0.0 {
        multiplier *= state.slow_multiplier;
    }
    if state.sophia_speed_buff_timer > 0.0 {
        multiplier *= SOPHIA_SPEED_BUFF_MULTIPLIER;
    }

    multiplier.clamp(0.0, MAX_MOVEMENT_SPEED_MULTIPLIER)
}
/// Returns the max health configured for the current development champion.
///
/// - `catalog`: Server-authoritative champion content catalog.
///
/// - Max health value loaded from the server champion content file.
fn development_champion_max_health(catalog: &ServerChampionCatalog, champion: ChampionId) -> f32 {
    catalog
        .champion(champion)
        .or_else(|| catalog.champion(ChampionId::LIRA))
        .unwrap_or_else(|| panic!("Missing server champion content for {}", champion.0))
        .base_stats
        .max_health
}
/// Returns max health for a connected development player.
fn development_player_max_health(
    catalog: &ServerChampionCatalog,
    players: &ConnectedPlayers,
    player_id: u64,
) -> f32 {
    let champion = players
        .states
        .get(&player_id)
        .map(|state| state.champion)
        .unwrap_or(ChampionId::LIRA);

    development_champion_max_health(catalog, champion)
}
/// Returns the tuning configured for the current development champion ability.
///
/// - `catalog`: Server-authoritative champion content catalog.
/// - `slot`: Ability slot whose tuning should be read.
///
/// - Ability tuning loaded from the server champion content file.
fn development_ability(
    catalog: &ServerChampionCatalog,
    slot: AbilitySlot,
) -> ServerAbilityDefinition {
    champion_ability(catalog, ChampionId::LIRA, slot)
}
/// Returns the tuning configured for a champion ability.
fn champion_ability(
    catalog: &ServerChampionCatalog,
    champion: ChampionId,
    slot: AbilitySlot,
) -> ServerAbilityDefinition {
    catalog.ability(champion, slot).cloned().unwrap_or_else(|| {
        panic!(
            "Missing server ability content for champion {} slot {:?}",
            champion.0, slot
        )
    })
}
/// Consumes Sophia's next-ability damage buff and returns the active multiplier.
fn consume_sophia_damage_multiplier(
    players: &mut ConnectedPlayers,
    caster_player_id: u64,
    ability: &ServerAbilityDefinition,
) -> f32 {
    let Some(caster) = players.states.get_mut(&caster_player_id) else {
        return DEFAULT_DAMAGE_MULTIPLIER;
    };
    if caster.sophia_damage_buff_timer <= 0.0 || !caster.sophia_damage_amp_available {
        return DEFAULT_DAMAGE_MULTIPLIER;
    }

    caster.sophia_damage_amp_available = false;
    caster.sophia_damage_buff_timer = 0.0;
    positive_or(
        ability.damage_multiplier,
        DEFAULT_SOPHIA_DAMAGE_BUFF_MULTIPLIER,
    )
}
/// Accepts a basic auto attack and starts its server-authoritative projectile.
#[cfg(test)]
fn accept_auto_attack(
    caster_player_id: u64,
    target_player_id: u64,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
) -> Option<AutoAttackVisualEvent> {
    let catalog = ServerChampionCatalog::embedded_test_catalog();
    accept_auto_attack_target(
        caster_player_id,
        NetworkTargetId::Player(target_player_id),
        players,
        None,
        abilities,
        &catalog,
    )
}

/// Validates and starts a player auto attack against an enemy player or lane unit.
fn accept_auto_attack_target(
    caster_player_id: u64,
    target: NetworkTargetId,
    players: &mut ConnectedPlayers,
    mut lane: Option<&mut ServerLaneState>,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AutoAttackVisualEvent> {
    let caster = *players.states.get(&caster_player_id)?;
    if caster.health <= 0.0
        || caster.stun_timer > 0.0
        || caster.auto_attack_cooldown > AUTO_ATTACK_INPUT_BUFFER_SECONDS
        || caster.team == DevelopmentTeam::Neutral
    {
        return None;
    }

    let caster_team = TeamSpec::from(caster.team);
    let (target_position, target_radius) = match target {
        NetworkTargetId::Player(target_player_id) => {
            if caster_player_id == target_player_id {
                return None;
            }
            let target_state = *players.states.get(&target_player_id)?;
            if target_state.health <= 0.0
                || target_state.team == caster.team
                || target_state.team == DevelopmentTeam::Neutral
            {
                return None;
            }
            (target_state.position, DEVELOPMENT_PLAYER_HIT_RADIUS)
        }
        NetworkTargetId::LaneUnit(target_unit_id) => lane
            .as_deref()
            .and_then(|lane| lane.target_for_player_auto_attack(caster_team, target_unit_id))?,
    };

    let distance = horizontal_distance(caster.position, target_position);
    if distance > AUTO_ATTACK_RANGE + target_radius {
        return None;
    }

    let combo = catalog
        .auto_attack_combo(caster.champion)
        .unwrap_or_else(|| auto_attack_combo(caster.champion));
    let combo_stage = if caster.auto_attack_combo_target == Some(target) {
        caster
            .auto_attack_combo_stage
            .min(combo.combo_length.saturating_sub(1))
    } else {
        0
    };
    let damage = combo.damage_for_stage(combo_stage);
    let next_combo_stage = (combo_stage + 1) % combo.combo_length.max(1);
    let travel_seconds = auto_attack_projectile_travel_seconds(distance);

    match target {
        NetworkTargetId::Player(target_player_id) => {
            if let Some(lane) = lane.as_deref_mut() {
                lane.record_hostile_player_action(caster_player_id, target_player_id, players);
            }
        }
        NetworkTargetId::LaneUnit(_) => {}
    }

    abilities
        .auto_attack_projectiles
        .push(ServerAutoAttackProjectile {
            caster_team,
            target,
            remaining_seconds: travel_seconds,
            damage,
        });

    let caster = players.states.get_mut(&caster_player_id)?;
    caster.auto_attack_cooldown = combo.cooldown_seconds();
    caster.auto_attack_combo_stage = next_combo_stage;
    caster.auto_attack_combo_target = Some(target);
    caster.auto_attack_combo_reset_timer =
        AUTO_ATTACK_COMBO_RESET_SECONDS + combo.cooldown_seconds();

    Some(AutoAttackVisualEvent {
        caster_player_id,
        target,
        start: (caster.position + Vec3::Y * AUTO_ATTACK_PROJECTILE_HEIGHT).into(),
        end: (target_position + Vec3::Y * AUTO_ATTACK_PROJECTILE_HEIGHT).into(),
        travel_seconds,
    })
}

/// Returns living enemy players inside one spell's impact radius.
fn enemy_players_in_spell_area(
    players: &ConnectedPlayers,
    caster_player_id: u64,
    caster_team: TeamSpec,
    center: Vec3,
    radius: f32,
) -> Vec<u64> {
    players
        .states
        .iter()
        .filter(|(player_id, player)| {
            **player_id != caster_player_id
                && TeamSpec::from(player.team) != caster_team
                && player.health > 0.0
                && horizontal_distance(player.position, center)
                    <= radius + DEVELOPMENT_PLAYER_HIT_RADIUS
        })
        .map(|(player_id, _)| *player_id)
        .collect()
}

/// Resolves the current position and collision radius for one enemy spell target.
fn spell_target_position(
    players: &ConnectedPlayers,
    lane: &ServerLaneState,
    caster_team: TeamSpec,
    target: NetworkTargetId,
) -> Option<(Vec3, f32)> {
    match target {
        NetworkTargetId::Player(player_id) => {
            let player = players.states.get(&player_id)?;
            let player_team = TeamSpec::from(player.team);
            if player.health <= 0.0
                || player_team == caster_team
                || !player_team.is_playable()
                || !caster_team.is_playable()
            {
                return None;
            }
            Some((player.position, DEVELOPMENT_PLAYER_HIT_RADIUS))
        }
        NetworkTargetId::LaneUnit(unit_id) => lane.spell_target(caster_team, unit_id),
    }
}

/// Applies one spell hit to a validated enemy player or lane minion.
fn apply_spell_damage_to_target(
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    caster_player_id: u64,
    caster_team: TeamSpec,
    target: NetworkTargetId,
    damage: f32,
) -> Option<(Vec3, f32)> {
    if damage <= 0.0 {
        return None;
    }

    let target_info = spell_target_position(players, lane, caster_team, target)?;
    match target {
        NetworkTargetId::Player(target_player_id) => {
            lane.record_hostile_player_action(caster_player_id, target_player_id, players);
            let target = players.states.get_mut(&target_player_id)?;
            apply_damage(
                combat_events,
                target_player_id,
                target,
                damage,
                NetworkCombatNumberKind::Spell,
            );
        }
        NetworkTargetId::LaneUnit(target_unit_id) => {
            lane.apply_spell_damage_to_target(caster_team, target_unit_id, damage)?;
        }
    }

    Some(target_info)
}

/// Finds the nearest valid player or lane-minion target around a point-click spell location.
fn find_nearest_enemy_spell_target_around_point(
    players: &ConnectedPlayers,
    lane: &ServerLaneState,
    caster_player_id: u64,
    point: Vec3,
    radius: f32,
) -> Option<NetworkTargetId> {
    let caster_team = TeamSpec::from(players.states.get(&caster_player_id)?.team);
    if !caster_team.is_playable() {
        return None;
    }

    let player_target = players
        .states
        .iter()
        .filter(|(target_player_id, target_state)| {
            **target_player_id != caster_player_id
                && TeamSpec::from(target_state.team) != caster_team
                && TeamSpec::from(target_state.team).is_playable()
                && target_state.health > 0.0
                && horizontal_distance(target_state.position, point) <= radius
        })
        .map(|(target_player_id, target_state)| {
            (
                NetworkTargetId::Player(*target_player_id),
                horizontal_distance(target_state.position, point),
                *target_player_id,
            )
        })
        .min_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(left.2.cmp(&right.2))
        });
    let lane_target = lane
        .nearest_enemy_minion_for_spell(caster_team, point, radius, point)
        .map(|(unit_id, position, _)| {
            (
                NetworkTargetId::LaneUnit(unit_id),
                horizontal_distance(position, point),
                unit_id,
            )
        });

    match (player_target, lane_target) {
        (Some((target, player_distance, _)), Some((lane_target, lane_distance, _))) => {
            if player_distance <= lane_distance {
                Some(target)
            } else {
                Some(lane_target)
            }
        }
        (Some((target, _, _)), None) => Some(target),
        (None, Some((target, _, _))) => Some(target),
        (None, None) => None,
    }
}
/// Accepts a Lira Q cast and starts its server-side projectile simulation.
///
/// - `caster_player_id`: Player id that requested the Q cast.
/// - `target_position`: Optional world-space aim point sent by the client.
/// - `players`: Server-side development player state cache.
/// - `abilities`: Active server-side ability simulations.
/// - `catalog`: Server-authoritative champion content catalog.
///
/// - Ability visual event to broadcast when the cast was accepted.
fn accept_lira_q_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = development_ability(catalog, AbilitySlot::Q);
    let caster = players.states.get_mut(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.lira_q_cooldown > 0.0 {
        return None;
    }
    caster.lira_q_cooldown = ability.cooldown_seconds;
    let caster = *caster;

    let origin_ground = caster.position;
    let direction = target_position
        .map(Vec3::from)
        .map(|target| Vec3::new(target.x - origin_ground.x, 0.0, target.z - origin_ground.z))
        .filter(|delta| delta.length_squared() > f32::EPSILON)
        .map(|delta| delta.normalize())
        .unwrap_or_else(|| Quat::from_rotation_y(caster.yaw) * Vec3::Z);

    let start = origin_ground + Vec3::Y * ability.projectile_height;
    let end = origin_ground + direction * ability.range + Vec3::Y * ability.projectile_height;
    abilities.q_projectiles.push(ServerQProjectile {
        caster_player_id,
        start,
        end,
        elapsed: 0.0,
        travel_seconds: ability.travel_seconds,
        projectile_radius: ability.projectile_radius,
        explosion_radius: ability.explosion_radius,
        direct_hit_damage: ability.damage.direct_hit,
        area_damage: ability.damage.area,
        hit_targets: Vec::new(),
        hit_lane_unit_ids: Vec::new(),
    });

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::LIRA,
        slot: AbilitySlot::Q,
        start: WorldPosition::from(start),
        end: Some(WorldPosition::from(end)),
        visual: ability_visual_tuning(&ability),
    })
}
/// Accepts a Lira W cast and starts its server-side projectile simulation.
///
/// - `caster_player_id`: Player id that requested the W cast.
/// - `target_position`: Optional world-space aim point sent by the client.
/// - `players`: Server-side development player state cache.
/// - `abilities`: Active server-side ability simulations.
/// - `catalog`: Server-authoritative champion content catalog.
///
/// - Ability visual event to broadcast when the cast was accepted.
fn accept_lira_w_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = development_ability(catalog, AbilitySlot::W);
    let caster = players.states.get_mut(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.lira_w_cooldown > 0.0 {
        return None;
    }
    let target_position = target_position.map(Vec3::from)?;
    caster.lira_w_cooldown = ability.cooldown_seconds;
    let caster = *caster;

    let origin_ground = caster.position;
    let target_ground = clamp_cast_target(origin_ground, target_position, ability.range);
    let start = origin_ground + Vec3::Y * ability.projectile_height;
    let end = target_ground + Vec3::Y * ability.target_height;

    abilities.w_projectiles.push(ServerWProjectile {
        caster_player_id,
        end,
        elapsed: 0.0,
        travel_seconds: ability.travel_seconds,
        explosion_radius: ability.explosion_radius,
        area_damage: ability.damage.area,
    });

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::LIRA,
        slot: AbilitySlot::W,
        start: WorldPosition::from(start),
        end: Some(WorldPosition::from(end)),
        visual: ability_visual_tuning(&ability),
    })
}
/// Accepts a Lira E cast and starts its server-side missile simulations.
///
/// - `caster_player_id`: Player id that requested the E cast.
/// - `players`: Server-side development player state cache.
/// - `abilities`: Active server-side ability simulations.
/// - `catalog`: Server-authoritative champion content catalog.
///
/// - Ability visual event to broadcast when the cast was accepted.
fn accept_lira_e_cast(
    caster_player_id: u64,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = development_ability(catalog, AbilitySlot::E);
    let caster = players.states.get_mut(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.lira_e_cooldown > 0.0 {
        return None;
    }
    caster.lira_e_cooldown = ability.cooldown_seconds;
    let caster = *caster;

    let missile_count = ability.missile_count.max(1);
    for index in 0..missile_count {
        let phase = index as f32 / missile_count as f32 * std::f32::consts::TAU;
        let offset = Vec3::new(phase.cos(), 0.0, phase.sin()) * ability.missile_orbit_radius
            + Vec3::Y * ability.missile_orbit_height;
        abilities.e_missiles.push(ServerEMissile {
            caster_player_id,
            position: caster.position + offset,
            phase,
            elapsed: 0.0,
            damage: ability.damage.missile,
            lifetime_seconds: ability.missile_lifetime_seconds,
            search_radius: ability.missile_search_radius,
            orbit_radius: ability.missile_orbit_radius,
            orbit_height: ability.missile_orbit_height,
            orbit_speed: ability.missile_orbit_speed,
            chase_speed: ability.missile_chase_speed,
            missile_radius: ability.missile_radius,
            mode: ServerEMissileMode::Orbiting,
        });
    }

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::LIRA,
        slot: AbilitySlot::E,
        start: WorldPosition::from(caster.position),
        end: None,
        visual: ability_visual_tuning(&ability),
    })
}
/// Accepts an Ignara Q cast and starts its server-side burning ground simulation.
fn accept_ignara_q_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, ChampionId::IGNARA, AbilitySlot::Q);
    let caster = players.states.get_mut(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.ignara_q_cooldown > 0.0 {
        return None;
    }
    caster.ignara_q_cooldown = ability.cooldown_seconds;
    let caster = *caster;

    let direction = target_position
        .map(Vec3::from)
        .map(|target| {
            Vec3::new(
                target.x - caster.position.x,
                0.0,
                target.z - caster.position.z,
            )
        })
        .filter(|delta| delta.length_squared() > f32::EPSILON)
        .map(|delta| delta.normalize())
        .unwrap_or_else(|| Quat::from_rotation_y(caster.yaw) * Vec3::Z);
    let end = caster.position + direction * ability.range;

    abilities.ignara_q_zones.push(ServerIgnaraQZone {
        caster_player_id,
        start: caster.position,
        end,
        elapsed: 0.0,
        lifetime_seconds: positive_or(ability.lifetime_seconds, ability.travel_seconds),
        width: ability.width,
        damage_per_second: ability.damage_per_second,
    });

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::IGNARA,
        slot: AbilitySlot::Q,
        start: WorldPosition::from(caster.position),
        end: Some(WorldPosition::from(end)),
        visual: AbilityVisualTuning {
            travel_seconds: positive_or(ability.lifetime_seconds, ability.travel_seconds),
            projectile_radius: ability.width * 0.5,
            explosion_radius: ability.range,
            ..default()
        },
    })
}
/// Accepts an Ignara W point-click fireball and stores the selected target.
fn accept_ignara_w_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    lane: &ServerLaneState,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, ChampionId::IGNARA, AbilitySlot::W);
    let target_position = target_position.map(Vec3::from)?;
    let caster = *players.states.get(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.ignara_w_cooldown > 0.0 {
        return None;
    }
    let caster_position = caster.position;
    let caster_team = TeamSpec::from(caster.team);
    let target_point = clamp_cast_target(caster_position, target_position, ability.range);
    let target = find_nearest_enemy_spell_target_around_point(
        players,
        lane,
        caster_player_id,
        target_point,
        ability.target_radius,
    )?;

    if let Some(caster) = players.states.get_mut(&caster_player_id) {
        caster.ignara_w_cooldown = ability.cooldown_seconds;
    }
    abilities.ignara_w_fireballs.push(ServerIgnaraWFireball {
        caster_player_id,
        target,
        elapsed: 0.0,
        travel_seconds: ability.travel_seconds,
        damage: ability.damage.direct_hit,
    });

    let end = spell_target_position(players, lane, caster_team, target)
        .map(|(position, _)| position)
        .unwrap_or(target_point);

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::IGNARA,
        slot: AbilitySlot::W,
        start: WorldPosition::from(caster_position + Vec3::Y * 0.75),
        end: Some(WorldPosition::from(end + Vec3::Y * 0.75)),
        visual: AbilityVisualTuning {
            travel_seconds: ability.travel_seconds,
            projectile_radius: ability.projectile_radius,
            explosion_radius: ability.target_radius,
            ..default()
        },
    })
}
/// Accepts an Ignara E cast and starts its server-side rolling snowball simulation.
fn accept_ignara_e_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, ChampionId::IGNARA, AbilitySlot::E);
    let caster = players.states.get_mut(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.ignara_e_cooldown > 0.0 {
        return None;
    }
    caster.ignara_e_cooldown = ability.cooldown_seconds;
    let caster = *caster;

    let direction = target_position
        .map(Vec3::from)
        .map(|target| {
            Vec3::new(
                target.x - caster.position.x,
                0.0,
                target.z - caster.position.z,
            )
        })
        .filter(|delta| delta.length_squared() > f32::EPSILON)
        .map(|delta| delta.normalize())
        .unwrap_or_else(|| Quat::from_rotation_y(caster.yaw) * Vec3::Z);
    let end = caster.position + direction * ability.range;

    abilities.ignara_e_snowballs.push(ServerIgnaraESnowball {
        caster_player_id,
        start: caster.position,
        end,
        elapsed: 0.0,
        travel_seconds: ability.travel_seconds,
        range: ability.range,
        width: ability.width,
        small_distance: ability.small_distance,
        medium_distance: ability.medium_distance,
        small_damage: ability.small_damage,
        medium_damage: ability.medium_damage,
        large_damage: ability.large_damage,
        hit_targets: Vec::new(),
        hit_lane_unit_ids: Vec::new(),
    });

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::IGNARA,
        slot: AbilitySlot::E,
        start: WorldPosition::from(caster.position + Vec3::Y * 0.45),
        end: Some(WorldPosition::from(end + Vec3::Y * 0.45)),
        visual: AbilityVisualTuning {
            travel_seconds: ability.travel_seconds,
            projectile_radius: ability.width * 0.5,
            explosion_radius: ability.range,
            ..default()
        },
    })
}
/// Accepts a Yuna Q cast and starts its server-side gravity field simulation.
fn accept_yuna_q_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, ChampionId::YUNA, AbilitySlot::Q);
    let caster = players.states.get_mut(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.yuna_q_cooldown > 0.0 {
        return None;
    }
    let target_position = target_position.map(Vec3::from)?;
    caster.yuna_q_cooldown = ability.cooldown_seconds;
    let caster = *caster;

    let target = clamp_cast_target(caster.position, target_position, ability.range);
    abilities.yuna_q_orbs.push(ServerYunaQOrb {
        caster_player_id,
        position: target,
        elapsed: 0.0,
        travel_seconds: ability.travel_seconds,
        lifetime_seconds: ability.lifetime_seconds,
        radius: ability.explosion_radius,
        damage_per_second: ability.damage_per_second,
        pull_speed: ability.pull_speed,
        move_speed_multiplier: ability.move_speed_multiplier,
    });

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::YUNA,
        slot: AbilitySlot::Q,
        start: WorldPosition::from(caster.position + Vec3::Y * 0.8),
        end: Some(WorldPosition::from(target + Vec3::Y * 0.55)),
        visual: AbilityVisualTuning {
            travel_seconds: ability.travel_seconds,
            projectile_radius: ability.projectile_radius,
            explosion_radius: ability.explosion_radius,
            ..default()
        },
    })
}
/// Accepts a Yuna W cast and starts its server-side once-per-ally healing field.
fn accept_yuna_w_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, ChampionId::YUNA, AbilitySlot::W);
    let caster = players.states.get_mut(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.yuna_w_cooldown > 0.0 {
        return None;
    }
    caster.yuna_w_cooldown = ability.cooldown_seconds;
    let caster = *caster;
    let center = target_position.map(Vec3::from).unwrap_or(caster.position);

    abilities.yuna_w_fields.push(ServerYunaWField {
        caster_player_id,
        elapsed: 0.0,
        tick_elapsed: 0.0,
        lifetime_seconds: ability.lifetime_seconds,
        radius: ability.explosion_radius,
        heal: ability.heal,
    });

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::YUNA,
        slot: AbilitySlot::W,
        start: WorldPosition::from(center),
        end: None,
        visual: AbilityVisualTuning {
            travel_seconds: ability.lifetime_seconds,
            explosion_radius: ability.explosion_radius,
            ..default()
        },
    })
}
/// Accepts a Yuna E point-click stun and applies the immobilize immediately.
fn accept_yuna_e_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, ChampionId::YUNA, AbilitySlot::E);
    let target_position = target_position.map(Vec3::from)?;
    let caster = *players.states.get(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.yuna_e_cooldown > 0.0 {
        return None;
    }

    let target_point = clamp_cast_target(caster.position, target_position, ability.range);
    let target_player_id = find_nearest_enemy_target_around_point(
        players,
        caster_player_id,
        target_point,
        ability.target_radius,
    )?;
    let target_position = players
        .states
        .get(&target_player_id)
        .map(|target| target.position)
        .unwrap_or(target_point);

    if let Some(caster) = players.states.get_mut(&caster_player_id) {
        caster.yuna_e_cooldown = ability.cooldown_seconds;
    }
    if let Some(target) = players.states.get_mut(&target_player_id) {
        target.stun_timer = target.stun_timer.max(ability.stun_seconds);
        target.moving = false;
    }

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::YUNA,
        slot: AbilitySlot::E,
        start: WorldPosition::from(caster.position + Vec3::Y * 0.85),
        end: Some(WorldPosition::from(target_position + Vec3::Y * 0.85)),
        visual: AbilityVisualTuning {
            travel_seconds: ability.travel_seconds,
            projectile_radius: ability.projectile_radius,
            explosion_radius: ability.target_radius,
            ..default()
        },
    })
}
/// Accepts a Sophia Q point-click orb and starts its damage-over-time simulation.
fn accept_sophia_q_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    lane: &ServerLaneState,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, ChampionId::SOPHIA, AbilitySlot::Q);
    let target_position = target_position.map(Vec3::from)?;
    let caster = *players.states.get(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.sophia_q_cooldown > 0.0 {
        return None;
    }

    let target_point = clamp_cast_target(caster.position, target_position, ability.range);
    let target = find_nearest_enemy_spell_target_around_point(
        players,
        lane,
        caster_player_id,
        target_point,
        ability.target_radius,
    )?;
    let caster_team = TeamSpec::from(caster.team);
    let damage_multiplier = consume_sophia_damage_multiplier(players, caster_player_id, &ability);
    if let Some(caster) = players.states.get_mut(&caster_player_id) {
        caster.sophia_q_cooldown = ability.cooldown_seconds;
    }

    abilities.sophia_q_orbs.push(ServerSophiaQOrb {
        caster_player_id,
        target,
        elapsed: 0.0,
        tick_elapsed: 0.0,
        lifetime_seconds: ability.lifetime_seconds,
        damage_per_second: ability.damage_per_second * damage_multiplier,
    });

    let target_position = spell_target_position(players, lane, caster_team, target)
        .map(|(position, _)| position)
        .unwrap_or(target_point);

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::SOPHIA,
        slot: AbilitySlot::Q,
        start: WorldPosition::from(caster.position + Vec3::Y * 1.0),
        end: Some(WorldPosition::from(target_position + Vec3::Y * 1.75)),
        visual: AbilityVisualTuning {
            travel_seconds: ability.lifetime_seconds,
            projectile_radius: ability.projectile_radius,
            explosion_radius: ability.target_radius,
            ..default()
        },
    })
}
/// Accepts a Sophia W cast and summons two chasing minions.
fn accept_sophia_w_cast(
    caster_player_id: u64,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, ChampionId::SOPHIA, AbilitySlot::W);
    let caster = players.states.get_mut(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.sophia_w_cooldown > 0.0 {
        return None;
    }
    caster.sophia_w_cooldown = ability.cooldown_seconds;
    let caster = *caster;
    let damage_multiplier = consume_sophia_damage_multiplier(players, caster_player_id, &ability);

    let minion_count = ability.missile_count.max(1);
    for index in 0..minion_count {
        let phase = index as f32 / minion_count as f32 * std::f32::consts::TAU;
        let offset = Vec3::new(phase.cos(), 0.0, phase.sin()) * ability.missile_orbit_radius
            + Vec3::Y * 0.35;
        abilities.sophia_minions.push(ServerSophiaMinion {
            caster_player_id,
            position: caster.position + offset,
            phase,
            elapsed: 0.0,
            lifetime_seconds: ability.missile_lifetime_seconds,
            search_radius: ability.missile_search_radius,
            chase_speed: ability.missile_chase_speed,
            radius: ability.missile_radius,
            damage: ability.damage.missile * damage_multiplier,
            slow_seconds: ability.slow_seconds,
            slow_multiplier: ability.move_speed_multiplier,
            target: None,
        });
    }

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::SOPHIA,
        slot: AbilitySlot::W,
        start: WorldPosition::from(caster.position),
        end: None,
        visual: ability_visual_tuning(&ability),
    })
}
/// Accepts a Sophia E self-buff.
fn accept_sophia_e_cast(
    caster_player_id: u64,
    players: &mut ConnectedPlayers,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, ChampionId::SOPHIA, AbilitySlot::E);
    let caster = players.states.get_mut(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.sophia_e_cooldown > 0.0 {
        return None;
    }

    caster.sophia_e_cooldown = ability.cooldown_seconds;
    caster.sophia_damage_buff_timer = ability.lifetime_seconds;
    caster.sophia_speed_buff_timer = ability.speed_seconds;
    caster.sophia_damage_amp_available = true;

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: ChampionId::SOPHIA,
        slot: AbilitySlot::E,
        start: WorldPosition::from(caster.position),
        end: None,
        visual: AbilityVisualTuning {
            travel_seconds: ability.lifetime_seconds,
            ..default()
        },
    })
}
/// Converts server-authoritative ability data into network visual tuning.
///
/// - `ability`: Server-authoritative ability definition accepted for a cast.
///
/// - Visual tuning sent to clients for deterministic ability rendering.
fn ability_visual_tuning(ability: &ServerAbilityDefinition) -> AbilityVisualTuning {
    AbilityVisualTuning {
        travel_seconds: ability.travel_seconds,
        projectile_radius: ability.projectile_radius,
        explosion_radius: ability.explosion_radius,
        missile_count: ability.missile_count.min(u16::MAX as usize) as u16,
        missile_lifetime_seconds: ability.missile_lifetime_seconds,
        missile_search_radius: ability.missile_search_radius,
        missile_orbit_radius: ability.missile_orbit_radius,
        missile_orbit_height: ability.missile_orbit_height,
        missile_orbit_speed: ability.missile_orbit_speed,
        missile_chase_speed: ability.missile_chase_speed,
        missile_radius: ability.missile_radius,
    }
}
/// Advances server-side ability cooldowns for connected players.
///
/// - `players`: Server-side development player state cache.
/// - `delta_seconds`: Elapsed time since the previous update.
fn tick_ability_cooldowns(players: &mut ConnectedPlayers, delta_seconds: f32) {
    for state in players.states.values_mut() {
        state.lira_q_cooldown = (state.lira_q_cooldown - delta_seconds).max(0.0);
        state.lira_w_cooldown = (state.lira_w_cooldown - delta_seconds).max(0.0);
        state.lira_e_cooldown = (state.lira_e_cooldown - delta_seconds).max(0.0);
        state.auto_attack_cooldown = (state.auto_attack_cooldown - delta_seconds).max(0.0);
        if state.auto_attack_combo_reset_timer > 0.0 {
            state.auto_attack_combo_reset_timer =
                (state.auto_attack_combo_reset_timer - delta_seconds).max(0.0);
            if state.auto_attack_combo_reset_timer <= 0.0 {
                state.auto_attack_combo_stage = 0;
                state.auto_attack_combo_target = None;
            }
        }
        state.ignara_q_cooldown = (state.ignara_q_cooldown - delta_seconds).max(0.0);
        state.ignara_w_cooldown = (state.ignara_w_cooldown - delta_seconds).max(0.0);
        state.ignara_e_cooldown = (state.ignara_e_cooldown - delta_seconds).max(0.0);
        state.yuna_q_cooldown = (state.yuna_q_cooldown - delta_seconds).max(0.0);
        state.yuna_w_cooldown = (state.yuna_w_cooldown - delta_seconds).max(0.0);
        state.yuna_e_cooldown = (state.yuna_e_cooldown - delta_seconds).max(0.0);
        state.sophia_q_cooldown = (state.sophia_q_cooldown - delta_seconds).max(0.0);
        state.sophia_w_cooldown = (state.sophia_w_cooldown - delta_seconds).max(0.0);
        state.sophia_e_cooldown = (state.sophia_e_cooldown - delta_seconds).max(0.0);
        state.sophia_damage_buff_timer = (state.sophia_damage_buff_timer - delta_seconds).max(0.0);
        state.sophia_speed_buff_timer = (state.sophia_speed_buff_timer - delta_seconds).max(0.0);
        if state.sophia_damage_buff_timer <= 0.0 {
            state.sophia_damage_amp_available = false;
        }
        state.slow_timer = (state.slow_timer - delta_seconds).max(0.0);
        if state.slow_timer <= 0.0 {
            state.slow_multiplier = 1.0;
        }
        state.stun_timer = (state.stun_timer - delta_seconds).max(0.0);
        state.respawn_input_grace = (state.respawn_input_grace - delta_seconds).max(0.0);
    }
}
/// Advances active Lira Q projectiles and applies direct and explosion contact damage.
///
/// - `abilities`: Active server-side ability simulations.
/// - `players`: Server-side development player state cache.
/// - `lane`: Server-side lane state that receives projectile contacts.
/// - `delta_seconds`: Elapsed time since the previous update.
fn update_lira_q_projectiles(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    delta_seconds: f32,
) {
    let mut finished_projectiles = Vec::new();

    for projectile in &mut abilities.q_projectiles {
        let caster_team = players
            .states
            .get(&projectile.caster_player_id)
            .map(|caster| caster.team);
        let lane_caster_team = caster_team
            .map(TeamSpec::from)
            .filter(|team| team.is_playable());
        let travel_seconds = projectile.travel_seconds.max(f32::EPSILON);
        let previous_progress = (projectile.elapsed / travel_seconds).clamp(0.0, 1.0);
        let previous_position = projectile.start.lerp(projectile.end, previous_progress);

        projectile.elapsed += delta_seconds;
        let progress = (projectile.elapsed / travel_seconds).clamp(0.0, 1.0);
        let current_position = projectile.start.lerp(projectile.end, progress);
        let mut direct_player_hit_ids = Vec::new();

        for (target_player_id, target_state) in &mut players.states {
            if *target_player_id == projectile.caster_player_id
                || Some(target_state.team) == caster_team
                || target_state.health <= 0.0
                || projectile.hit_targets.contains(target_player_id)
            {
                continue;
            }

            if distance_to_segment_xz(target_state.position, previous_position, current_position)
                <= projectile.projectile_radius + DEVELOPMENT_PLAYER_HIT_RADIUS
            {
                projectile.hit_targets.push(*target_player_id);
                direct_player_hit_ids.push(*target_player_id);
                apply_damage(
                    combat_events,
                    *target_player_id,
                    target_state,
                    projectile.direct_hit_damage,
                    NetworkCombatNumberKind::Spell,
                );
            }
        }

        if lane_caster_team.is_some() && projectile.direct_hit_damage > 0.0 {
            for target_player_id in direct_player_hit_ids {
                lane.record_hostile_player_action(
                    projectile.caster_player_id,
                    target_player_id,
                    players,
                );
            }
        }

        if let Some(caster_team) = lane_caster_team {
            lane.apply_spell_damage_on_segment(
                caster_team,
                previous_position,
                current_position,
                projectile.projectile_radius,
                projectile.direct_hit_damage,
                &mut projectile.hit_lane_unit_ids,
            );
        }

        if projectile.elapsed >= travel_seconds {
            finished_projectiles.push((
                projectile.caster_player_id,
                lane_caster_team,
                projectile.end,
                projectile.explosion_radius,
                projectile.area_damage,
            ));
        }
    }

    abilities
        .q_projectiles
        .retain(|projectile| projectile.elapsed < projectile.travel_seconds.max(f32::EPSILON));

    for (caster_player_id, lane_caster_team, end, explosion_radius, area_damage) in
        finished_projectiles
    {
        if let Some(caster_team) = lane_caster_team
            && area_damage > 0.0
        {
            for target_player_id in enemy_players_in_spell_area(
                players,
                caster_player_id,
                caster_team,
                end,
                explosion_radius,
            ) {
                lane.record_hostile_player_action(caster_player_id, target_player_id, players);
            }
        }
        apply_area_damage(
            combat_events,
            players,
            caster_player_id,
            end,
            explosion_radius,
            area_damage,
        );
        if let Some(caster_team) = lane_caster_team {
            lane.apply_spell_damage(caster_team, end, explosion_radius, area_damage);
        }
    }
}
/// Advances active Lira W projectiles and applies landing explosion contact damage.
///
/// - `abilities`: Active server-side ability simulations.
/// - `players`: Server-side development player state cache.
/// - `lane`: Server-side lane state that receives landing impacts.
/// - `delta_seconds`: Elapsed time since the previous update.
fn update_lira_w_projectiles(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    delta_seconds: f32,
) {
    let mut finished_projectiles = Vec::new();

    for projectile in &mut abilities.w_projectiles {
        projectile.elapsed += delta_seconds;
        if projectile.elapsed >= projectile.travel_seconds.max(f32::EPSILON) {
            finished_projectiles.push((
                projectile.caster_player_id,
                projectile.end,
                projectile.explosion_radius,
                projectile.area_damage,
            ));
        }
    }

    abilities
        .w_projectiles
        .retain(|projectile| projectile.elapsed < projectile.travel_seconds.max(f32::EPSILON));

    for (caster_player_id, end, explosion_radius, area_damage) in finished_projectiles {
        let lane_caster_team = players
            .states
            .get(&caster_player_id)
            .map(|caster| TeamSpec::from(caster.team))
            .filter(|team| team.is_playable());
        if let Some(caster_team) = lane_caster_team
            && area_damage > 0.0
        {
            for target_player_id in enemy_players_in_spell_area(
                players,
                caster_player_id,
                caster_team,
                end,
                explosion_radius,
            ) {
                lane.record_hostile_player_action(caster_player_id, target_player_id, players);
            }
        }
        apply_area_damage(
            combat_events,
            players,
            caster_player_id,
            end,
            explosion_radius,
            area_damage,
        );
        if let Some(caster_team) = lane_caster_team {
            lane.apply_spell_damage(caster_team, end, explosion_radius, area_damage);
        }
    }
}
/// Advances active Lira E missiles and applies contact damage.
///
/// - `abilities`: Active server-side ability simulations.
/// - `players`: Server-side development player state cache.
/// - `delta_seconds`: Elapsed time since the previous update.
fn update_lira_e_missiles(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    delta_seconds: f32,
) {
    let mut spent_missiles = Vec::new();

    for (missile_index, missile) in abilities.e_missiles.iter_mut().enumerate() {
        missile.elapsed += delta_seconds;
        if missile.elapsed >= missile.lifetime_seconds.max(f32::EPSILON) {
            spent_missiles.push(missile_index);
            continue;
        }

        let Some(caster) = players.states.get(&missile.caster_player_id).copied() else {
            spent_missiles.push(missile_index);
            continue;
        };

        if caster.health <= 0.0 {
            spent_missiles.push(missile_index);
            continue;
        }

        if missile.mode == ServerEMissileMode::Orbiting
            && let Some(target) = find_lira_e_target(
                players,
                lane,
                missile.caster_player_id,
                caster.position,
                missile.position,
                missile.search_radius,
            )
        {
            missile.mode = ServerEMissileMode::Chasing(target);
        }

        match missile.mode {
            ServerEMissileMode::Orbiting => {
                let angle = missile.phase + missile.elapsed * missile.orbit_speed;
                let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * missile.orbit_radius
                    + Vec3::Y * missile.orbit_height;
                missile.position = caster.position + offset;
            }
            ServerEMissileMode::Chasing(target) => {
                let caster_team = TeamSpec::from(caster.team);
                let Some((target_position, target_radius)) =
                    spell_target_position(players, lane, caster_team, target)
                else {
                    spent_missiles.push(missile_index);
                    continue;
                };

                let visual_target_position = target_position + Vec3::Y * 0.7;
                let to_target = visual_target_position - missile.position;
                let distance = to_target.length();

                if distance <= missile.missile_radius + target_radius {
                    apply_spell_damage_to_target(
                        players,
                        lane,
                        combat_events,
                        missile.caster_player_id,
                        caster_team,
                        target,
                        missile.damage,
                    );
                    spent_missiles.push(missile_index);
                    continue;
                }

                if distance > f32::EPSILON {
                    let step = missile.chase_speed * delta_seconds;
                    missile.position += to_target.normalize() * step.min(distance);
                }
            }
        }
    }

    spent_missiles.sort_unstable();
    spent_missiles.dedup();
    for missile_index in spent_missiles.into_iter().rev() {
        abilities.e_missiles.swap_remove(missile_index);
    }
}
/// Advances Ignara Q burning zones and applies burn damage over time.
fn update_ignara_q_zones(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    delta_seconds: f32,
) {
    for zone in &mut abilities.ignara_q_zones {
        zone.elapsed += delta_seconds;
        let caster_team = players
            .states
            .get(&zone.caster_player_id)
            .map(|caster| caster.team);
        let lane_caster_team = caster_team
            .map(TeamSpec::from)
            .filter(|team| team.is_playable());
        let damage = zone.damage_per_second * delta_seconds;
        let mut hit_player_ids = Vec::new();

        for (target_player_id, target_state) in &mut players.states {
            if *target_player_id == zone.caster_player_id
                || Some(target_state.team) == caster_team
                || target_state.health <= 0.0
            {
                continue;
            }

            if point_in_oriented_rect_xz(target_state.position, zone.start, zone.end, zone.width) {
                hit_player_ids.push(*target_player_id);
                apply_damage(
                    combat_events,
                    *target_player_id,
                    target_state,
                    damage,
                    NetworkCombatNumberKind::Spell,
                );
            }
        }

        if let Some(caster_team) = lane_caster_team {
            if damage > 0.0 {
                for target_player_id in hit_player_ids {
                    lane.record_hostile_player_action(
                        zone.caster_player_id,
                        target_player_id,
                        players,
                    );
                }
            }
            lane.apply_spell_damage_in_oriented_rect(
                caster_team,
                zone.start,
                zone.end,
                zone.width,
                damage,
            );
        }
    }

    abilities
        .ignara_q_zones
        .retain(|zone| zone.elapsed < zone.lifetime_seconds.max(f32::EPSILON));
}
/// Advances Ignara W fireballs and applies direct target damage on arrival.
fn update_ignara_w_fireballs(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    delta_seconds: f32,
) {
    let mut finished_fireballs = Vec::new();

    for (index, fireball) in abilities.ignara_w_fireballs.iter_mut().enumerate() {
        fireball.elapsed += delta_seconds;
        if fireball.elapsed >= fireball.travel_seconds.max(f32::EPSILON) {
            finished_fireballs.push(index);
            let Some(caster_team) = players
                .states
                .get(&fireball.caster_player_id)
                .map(|caster| TeamSpec::from(caster.team))
                .filter(|team| team.is_playable())
            else {
                continue;
            };
            apply_spell_damage_to_target(
                players,
                lane,
                combat_events,
                fireball.caster_player_id,
                caster_team,
                fireball.target,
                fireball.damage,
            );
        }
    }

    for index in finished_fireballs.into_iter().rev() {
        abilities.ignara_w_fireballs.swap_remove(index);
    }
}
/// Advances Ignara E rolling snowballs and applies distance-tiered contact damage.
fn update_ignara_e_snowballs(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    delta_seconds: f32,
) {
    for snowball in &mut abilities.ignara_e_snowballs {
        let caster_team = players
            .states
            .get(&snowball.caster_player_id)
            .map(|caster| caster.team);
        let lane_caster_team = caster_team
            .map(TeamSpec::from)
            .filter(|team| team.is_playable());
        let travel_seconds = snowball.travel_seconds.max(f32::EPSILON);
        let previous_progress = (snowball.elapsed / travel_seconds).clamp(0.0, 1.0);
        let previous_position = snowball.start.lerp(snowball.end, previous_progress);

        snowball.elapsed += delta_seconds;
        let progress = (snowball.elapsed / travel_seconds).clamp(0.0, 1.0);
        let current_position = snowball.start.lerp(snowball.end, progress);
        let travelled = snowball.start.distance(current_position);
        let radius = ignara_e_radius_for_distance(travelled, snowball.width, snowball.range);
        let damage = ignara_e_damage_for_distance(
            travelled,
            snowball.small_distance,
            snowball.medium_distance,
            snowball.small_damage,
            snowball.medium_damage,
            snowball.large_damage,
        );
        let mut hit_player_ids = Vec::new();

        for (target_player_id, target_state) in &mut players.states {
            if *target_player_id == snowball.caster_player_id
                || Some(target_state.team) == caster_team
                || target_state.health <= 0.0
                || snowball.hit_targets.contains(target_player_id)
            {
                continue;
            }

            if distance_to_segment_xz(target_state.position, previous_position, current_position)
                <= radius + DEVELOPMENT_PLAYER_HIT_RADIUS
            {
                snowball.hit_targets.push(*target_player_id);
                hit_player_ids.push(*target_player_id);
                apply_damage(
                    combat_events,
                    *target_player_id,
                    target_state,
                    damage,
                    NetworkCombatNumberKind::Spell,
                );
            }
        }

        if let Some(caster_team) = lane_caster_team {
            if damage > 0.0 {
                for target_player_id in hit_player_ids {
                    lane.record_hostile_player_action(
                        snowball.caster_player_id,
                        target_player_id,
                        players,
                    );
                }
            }
            lane.apply_spell_damage_on_segment(
                caster_team,
                previous_position,
                current_position,
                radius,
                damage,
                &mut snowball.hit_lane_unit_ids,
            );
        }
    }

    abilities
        .ignara_e_snowballs
        .retain(|snowball| snowball.elapsed < snowball.travel_seconds.max(f32::EPSILON));
}
/// Advances Yuna Q gravity fields, pulling and damaging enemy players inside the area.
fn update_yuna_q_orbs(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    delta_seconds: f32,
) {
    for orb in &mut abilities.yuna_q_orbs {
        orb.elapsed += delta_seconds;
        if orb.elapsed < orb.travel_seconds.max(0.0) {
            continue;
        }

        let caster_team = players
            .states
            .get(&orb.caster_player_id)
            .map(|caster| caster.team);
        let lane_caster_team = caster_team
            .map(TeamSpec::from)
            .filter(|team| team.is_playable());
        let damage = orb.damage_per_second * delta_seconds;
        let mut hit_player_ids = Vec::new();

        for (target_player_id, target_state) in &mut players.states {
            if Some(target_state.team) == caster_team
                || *target_player_id == orb.caster_player_id
                || target_state.health <= 0.0
            {
                continue;
            }

            let distance = horizontal_distance(target_state.position, orb.position);
            if distance > orb.radius + DEVELOPMENT_PLAYER_HIT_RADIUS {
                continue;
            }

            hit_player_ids.push(*target_player_id);
            apply_damage(
                combat_events,
                *target_player_id,
                target_state,
                damage,
                NetworkCombatNumberKind::Spell,
            );
            let pull_delta = Vec3::new(
                orb.position.x - target_state.position.x,
                0.0,
                orb.position.z - target_state.position.z,
            );
            let pull_distance = pull_delta.length();
            if pull_distance > 0.08 {
                let step = (orb.pull_speed * delta_seconds).min(pull_distance);
                let pulled_position = target_state.position + pull_delta.normalize() * step;
                let resolved_position = lane.resolve_structure_collision(
                    target_state.position,
                    pulled_position,
                    DEVELOPMENT_PLAYER_HIT_RADIUS,
                );
                if horizontal_distance(resolved_position, pulled_position) > 0.001 {
                    target_state.position_correction_generation =
                        target_state.position_correction_generation.wrapping_add(1);
                }
                target_state.position = resolved_position;
                target_state.moving = false;
            }
        }

        if let Some(caster_team) = lane_caster_team {
            if damage > 0.0 {
                for target_player_id in hit_player_ids {
                    lane.record_hostile_player_action(
                        orb.caster_player_id,
                        target_player_id,
                        players,
                    );
                }
            }
            lane.apply_spell_damage(caster_team, orb.position, orb.radius, damage);
        }
    }

    abilities.yuna_q_orbs.retain(|orb| {
        orb.elapsed < orb.travel_seconds.max(0.0) + orb.lifetime_seconds.max(f32::EPSILON)
    });
}
/// Advances Yuna W healing fields and heals allied players once per second while inside.
fn update_yuna_w_fields(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    combat_events: &mut ServerCombatNumberEvents,
    catalog: &ServerChampionCatalog,
    delta_seconds: f32,
) {
    for field in &mut abilities.yuna_w_fields {
        field.elapsed += delta_seconds;
        field.tick_elapsed += delta_seconds;
        let Some((caster_team, field_position)) = players
            .states
            .get(&field.caster_player_id)
            .map(|caster| (caster.team, caster.position))
        else {
            continue;
        };

        let heal_tick_count = consume_elapsed_ticks(&mut field.tick_elapsed);
        if heal_tick_count == 0 {
            continue;
        }

        let heal_amount = field.heal * heal_tick_count as f32;

        let player_ids = players.states.keys().copied().collect::<Vec<_>>();
        for target_player_id in player_ids {
            let Some(target_state) = players.states.get(&target_player_id) else {
                continue;
            };
            if target_state.team != caster_team
                || target_state.health <= 0.0
                || horizontal_distance(target_state.position, field_position)
                    > field.radius + DEVELOPMENT_PLAYER_HIT_RADIUS
            {
                continue;
            }

            let max_health = development_player_max_health(catalog, players, target_player_id);
            if let Some(target_state) = players.states.get_mut(&target_player_id) {
                apply_heal(
                    combat_events,
                    target_player_id,
                    target_state,
                    heal_amount,
                    max_health,
                );
            }
        }
    }

    abilities
        .yuna_w_fields
        .retain(|field| field.elapsed < field.lifetime_seconds.max(f32::EPSILON));
}
/// Advances Sophia Q orbs and applies one damage tick per second.
fn update_sophia_q_orbs(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    delta_seconds: f32,
) {
    for orb in &mut abilities.sophia_q_orbs {
        orb.elapsed += delta_seconds;
        orb.tick_elapsed += delta_seconds;
        for _ in 0..consume_elapsed_ticks(&mut orb.tick_elapsed) {
            let Some(caster_team) = players
                .states
                .get(&orb.caster_player_id)
                .map(|caster| TeamSpec::from(caster.team))
                .filter(|team| team.is_playable())
            else {
                continue;
            };
            apply_spell_damage_to_target(
                players,
                lane,
                combat_events,
                orb.caster_player_id,
                caster_team,
                orb.target,
                orb.damage_per_second,
            );
        }
    }

    abilities
        .sophia_q_orbs
        .retain(|orb| orb.elapsed < orb.lifetime_seconds.max(f32::EPSILON));
}
/// Advances Sophia W minions, target acquisition, chase movement, contact damage, and slow.
fn update_sophia_minions(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    lane: &mut ServerLaneState,
    combat_events: &mut ServerCombatNumberEvents,
    delta_seconds: f32,
) {
    let mut spent_minions = Vec::new();

    for (minion_index, minion) in abilities.sophia_minions.iter_mut().enumerate() {
        minion.elapsed += delta_seconds;
        if minion.elapsed >= minion.lifetime_seconds.max(f32::EPSILON) {
            spent_minions.push(minion_index);
            continue;
        }

        let Some(caster) = players.states.get(&minion.caster_player_id).copied() else {
            spent_minions.push(minion_index);
            continue;
        };
        if caster.health <= 0.0 {
            spent_minions.push(minion_index);
            continue;
        }

        if minion.target.is_none() {
            let angle = minion.phase + minion.elapsed * 1.8;
            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 1.0 + Vec3::Y * 0.35;
            resolve_sophia_minion_movement(lane, minion, caster.position + offset);
            minion.target = find_sophia_minion_target(
                players,
                lane,
                minion.caster_player_id,
                minion.position,
                minion.search_radius,
            );
        }

        let Some(target) = minion.target else {
            continue;
        };
        let caster_team = TeamSpec::from(caster.team);
        let Some((target_position, target_radius)) =
            spell_target_position(players, lane, caster_team, target)
        else {
            spent_minions.push(minion_index);
            continue;
        };

        let target_position = match target {
            NetworkTargetId::Player(target_player_id) => players
                .states
                .get(&target_player_id)
                .map(|target| {
                    target.position - (Quat::from_rotation_y(target.yaw) * Vec3::Z) * 0.75
                        + Vec3::Y * 0.35
                })
                .unwrap_or(target_position),
            NetworkTargetId::LaneUnit(_) => target_position + Vec3::Y * 0.35,
        };
        let to_target = target_position - minion.position;
        let distance = to_target.length();
        if distance <= minion.radius + target_radius {
            let hit = apply_spell_damage_to_target(
                players,
                lane,
                combat_events,
                minion.caster_player_id,
                caster_team,
                target,
                minion.damage,
            );
            if hit.is_some() {
                if let NetworkTargetId::Player(target_player_id) = target
                    && let Some(target) = players.states.get_mut(&target_player_id)
                {
                    target.slow_timer = target.slow_timer.max(minion.slow_seconds);
                    target.slow_multiplier = target.slow_multiplier.min(minion.slow_multiplier);
                    target.moving = false;
                }
                spent_minions.push(minion_index);
            }
            continue;
        }

        if distance > f32::EPSILON {
            let step = minion.chase_speed * delta_seconds;
            let desired_position = minion.position + to_target.normalize() * step.min(distance);
            resolve_sophia_minion_movement(lane, minion, desired_position);
        }
    }

    spent_minions.sort_unstable();
    spent_minions.dedup();
    for minion_index in spent_minions.into_iter().rev() {
        abilities.sophia_minions.swap_remove(minion_index);
    }
}

/// Resolves a Sophia W minion's requested position against living lane structures.
fn resolve_sophia_minion_movement(
    lane: &ServerLaneState,
    minion: &mut ServerSophiaMinion,
    desired_position: Vec3,
) {
    minion.position =
        lane.resolve_structure_collision(minion.position, desired_position, minion.radius);
}

/// Finds the nearest valid Lira E missile target in search radius.
///
/// - `players`: Server-side development player state cache.
/// - `lane`: Server-side lane state that can supply enemy minion targets.
/// - `caster_player_id`: Player id that owns the missile.
/// - `caster_position`: Current caster position used for search range.
/// - `missile_position`: Current missile position used to pick the nearest target.
/// - `search_radius`: Server-authoritative missile search radius.
///
/// - Stable id of the nearest valid player or lane-minion target.
fn find_lira_e_target(
    players: &ConnectedPlayers,
    lane: &ServerLaneState,
    caster_player_id: u64,
    caster_position: Vec3,
    missile_position: Vec3,
    search_radius: f32,
) -> Option<NetworkTargetId> {
    let caster_team = TeamSpec::from(players.states.get(&caster_player_id)?.team);
    if !caster_team.is_playable() {
        return None;
    }

    let player_target = players
        .states
        .iter()
        .filter(|(target_player_id, target_state)| {
            **target_player_id != caster_player_id
                && TeamSpec::from(target_state.team) != caster_team
                && TeamSpec::from(target_state.team).is_playable()
                && target_state.health > 0.0
                && horizontal_distance(caster_position, target_state.position) <= search_radius
        })
        .map(|(target_player_id, target_state)| {
            (
                NetworkTargetId::Player(*target_player_id),
                horizontal_distance(missile_position, target_state.position),
                *target_player_id,
            )
        })
        .min_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(left.2.cmp(&right.2))
        });
    let lane_target = lane
        .nearest_enemy_minion_for_spell(
            caster_team,
            caster_position,
            search_radius,
            missile_position,
        )
        .map(|(unit_id, position, _)| {
            (
                NetworkTargetId::LaneUnit(unit_id),
                horizontal_distance(missile_position, position),
                unit_id,
            )
        });

    match (player_target, lane_target) {
        (Some((player_target, player_distance, _)), Some((lane_target, lane_distance, _))) => {
            if player_distance <= lane_distance {
                Some(player_target)
            } else {
                Some(lane_target)
            }
        }
        (Some((target, _, _)), None) | (None, Some((target, _, _))) => Some(target),
        (None, None) => None,
    }
}
/// Finds the nearest living player or lane minion for a Sophia W minion.
fn find_sophia_minion_target(
    players: &ConnectedPlayers,
    lane: &ServerLaneState,
    caster_player_id: u64,
    minion_position: Vec3,
    search_radius: f32,
) -> Option<NetworkTargetId> {
    let caster_team = TeamSpec::from(players.states.get(&caster_player_id)?.team);
    if !caster_team.is_playable() {
        return None;
    }

    let player_target = players
        .states
        .iter()
        .filter(|(target_player_id, target_state)| {
            **target_player_id != caster_player_id
                && TeamSpec::from(target_state.team) != caster_team
                && TeamSpec::from(target_state.team).is_playable()
                && target_state.health > 0.0
                && horizontal_distance(minion_position, target_state.position) <= search_radius
        })
        .map(|(target_player_id, target_state)| {
            (
                NetworkTargetId::Player(*target_player_id),
                horizontal_distance(minion_position, target_state.position),
                *target_player_id,
            )
        })
        .min_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(left.2.cmp(&right.2))
        });
    let lane_target = lane
        .nearest_enemy_minion_for_spell(
            caster_team,
            minion_position,
            search_radius,
            minion_position,
        )
        .map(|(unit_id, position, _)| {
            (
                NetworkTargetId::LaneUnit(unit_id),
                horizontal_distance(minion_position, position),
                unit_id,
            )
        });

    match (player_target, lane_target) {
        (Some((player_target, player_distance, _)), Some((lane_target, lane_distance, _))) => {
            if player_distance <= lane_distance {
                Some(player_target)
            } else {
                Some(lane_target)
            }
        }
        (Some((target, _, _)), None) | (None, Some((target, _, _))) => Some(target),
        (None, None) => None,
    }
}
/// Finds the nearest living enemy around a clicked point for point-click spells.
fn find_nearest_enemy_target_around_point(
    players: &ConnectedPlayers,
    caster_player_id: u64,
    point: Vec3,
    radius: f32,
) -> Option<u64> {
    let caster_team = players.states.get(&caster_player_id)?.team;
    players
        .states
        .iter()
        .filter(|(target_player_id, target_state)| {
            **target_player_id != caster_player_id
                && target_state.team != caster_team
                && target_state.health > 0.0
                && horizontal_distance(target_state.position, point) <= radius
        })
        .min_by(|(_, left), (_, right)| {
            horizontal_distance(left.position, point)
                .partial_cmp(&horizontal_distance(right.position, point))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(target_player_id, _)| *target_player_id)
}
/// Returns Ignara E damage for the travelled projectile distance.
fn ignara_e_damage_for_distance(
    distance: f32,
    small_distance: f32,
    medium_distance: f32,
    small_damage: f32,
    medium_damage: f32,
    large_damage: f32,
) -> f32 {
    if distance < small_distance {
        small_damage
    } else if distance < medium_distance {
        medium_damage
    } else {
        large_damage
    }
}
/// Returns Ignara E collision radius for the travelled projectile distance.
fn ignara_e_radius_for_distance(distance: f32, width: f32, range: f32) -> f32 {
    let base_radius = width * IGNARA_E_COLLISION_RADIUS_WIDTH_FACTOR;
    let progress = (distance / range.max(f32::EPSILON)).clamp(0.0, 1.0);
    base_radius * (1.0 + progress * IGNARA_E_COLLISION_RADIUS_PROGRESS_FACTOR)
}
fn consume_elapsed_ticks(tick_elapsed: &mut f32) -> u32 {
    let mut tick_count = 0;
    while *tick_elapsed >= EFFECT_TICK_INTERVAL_SECONDS {
        *tick_elapsed -= EFFECT_TICK_INTERVAL_SECONDS;
        tick_count += 1;
    }
    tick_count
}
fn positive_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_player_cleanup_clears_runtime_and_catalog_state() {
        const PLAYER_ID: u64 = 42;
        let mut players = ConnectedPlayers::default();
        let mut navigation = ServerPlayerNavigation::default();
        let mut ready_players = LoadingScreenReadyPlayers::default();
        let mut leaving_players = LeavingPlayers::default();
        let mut sent_catalog_clients = SentChampionCatalogClients::default();
        players.states.insert(
            PLAYER_ID,
            test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        navigation.request_move(PLAYER_ID, Vec3::X);
        ready_players.ready_player_ids.insert(PLAYER_ID);
        sent_catalog_clients.0.insert(PLAYER_ID);

        cleanup_disconnected_player(
            PLAYER_ID,
            &mut players,
            &mut navigation,
            &mut ready_players,
            &mut leaving_players,
            &mut sent_catalog_clients,
        );

        assert!(!players.states.contains_key(&PLAYER_ID));
        assert!(!navigation.paths.contains_key(&PLAYER_ID));
        assert!(!ready_players.ready_player_ids.contains(&PLAYER_ID));
        assert!(leaving_players.player_ids.contains(&PLAYER_ID));
        assert!(!sent_catalog_clients.0.contains(&PLAYER_ID));
    }

    #[test]
    fn lira_q_damages_minions_when_the_projectile_reaches_them() {
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let minion_id = lane.spawn_spell_test_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 2.0),
        );
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        players.states.insert(
            1,
            test_player_state(
                ChampionId::LIRA,
                DevelopmentTeam::Light,
                Vec3::new(0.0, 0.0, -4.0),
            ),
        );
        abilities.q_projectiles.push(ServerQProjectile {
            caster_player_id: 1,
            start: Vec3::new(0.0, 0.8, -4.0),
            end: Vec3::new(0.0, 0.8, 4.0),
            elapsed: 0.0,
            travel_seconds: 1.0,
            projectile_radius: 0.2,
            explosion_radius: 0.0,
            direct_hit_damage: 28.0,
            area_damage: 0.0,
            hit_targets: Vec::new(),
            hit_lane_unit_ids: Vec::new(),
        });

        update_lira_q_projectiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.55,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 350.0);

        update_lira_q_projectiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.2,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 322.0);
    }
    #[test]
    fn lira_w_damages_minions_when_the_projectile_lands() {
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let minion_id =
            lane.spawn_spell_test_unit(LaneUnitKind::MeleeBox, TeamSpec::Dark, Vec3::ZERO);
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        abilities.w_projectiles.push(ServerWProjectile {
            caster_player_id: 1,
            end: Vec3::ZERO,
            elapsed: 0.0,
            travel_seconds: 0.5,
            explosion_radius: 2.0,
            area_damage: 48.0,
        });

        update_lira_w_projectiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.49,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 350.0);

        update_lira_w_projectiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.02,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 302.0);
    }
    #[test]
    fn lira_e_damages_minions_on_missile_contact() {
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let minion_id = lane.spawn_spell_test_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 3.0),
        );
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        abilities.e_missiles.push(ServerEMissile {
            caster_player_id: 1,
            position: Vec3::new(0.0, 0.7, 0.0),
            phase: 0.0,
            elapsed: 0.0,
            damage: 33.0,
            lifetime_seconds: 5.0,
            search_radius: 8.0,
            orbit_radius: 1.0,
            orbit_height: 0.7,
            orbit_speed: 1.0,
            chase_speed: 10.0,
            missile_radius: 0.2,
            mode: ServerEMissileMode::Orbiting,
        });

        update_lira_e_missiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.23,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 350.0);

        update_lira_e_missiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.01,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 317.0);
    }
    #[test]
    fn ignara_w_damages_minions_when_the_fireball_arrives() {
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let minion_id =
            lane.spawn_spell_test_unit(LaneUnitKind::MeleeBox, TeamSpec::Dark, Vec3::ZERO);
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::IGNARA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        abilities.ignara_w_fireballs.push(ServerIgnaraWFireball {
            caster_player_id: 1,
            target: NetworkTargetId::LaneUnit(minion_id),
            elapsed: 0.0,
            travel_seconds: 0.5,
            damage: 35.0,
        });

        update_ignara_w_fireballs(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.49,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 350.0);

        update_ignara_w_fireballs(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.02,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 315.0);
    }
    #[test]
    fn yuna_q_damages_minions_only_after_the_field_arrives() {
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let minion_id =
            lane.spawn_spell_test_unit(LaneUnitKind::MeleeBox, TeamSpec::Dark, Vec3::ZERO);
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::YUNA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        abilities.yuna_q_orbs.push(ServerYunaQOrb {
            caster_player_id: 1,
            position: Vec3::ZERO,
            elapsed: 0.0,
            travel_seconds: 0.5,
            lifetime_seconds: 1.0,
            radius: 2.0,
            damage_per_second: 50.0,
            pull_speed: 0.0,
            move_speed_multiplier: 1.0,
        });

        update_yuna_q_orbs(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.49,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 350.0);

        update_yuna_q_orbs(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.02,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 349.0);
    }
    #[test]
    fn sophia_q_damages_a_minion_on_its_damage_tick() {
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let minion_id =
            lane.spawn_spell_test_unit(LaneUnitKind::MeleeBox, TeamSpec::Dark, Vec3::ZERO);
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::SOPHIA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        abilities.sophia_q_orbs.push(ServerSophiaQOrb {
            caster_player_id: 1,
            target: NetworkTargetId::LaneUnit(minion_id),
            elapsed: 0.0,
            tick_elapsed: 0.0,
            lifetime_seconds: 4.0,
            damage_per_second: 24.0,
        });

        update_sophia_q_orbs(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.99,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 350.0);

        update_sophia_q_orbs(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.02,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 326.0);
    }
    #[test]
    fn sophia_w_damages_a_minion_on_contact() {
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let minion_id = lane.spawn_spell_test_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 1.0),
        );
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::SOPHIA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        assert_eq!(
            find_sophia_minion_target(&players, &lane, 1, Vec3::new(0.0, 0.35, 0.0), 4.5,),
            Some(NetworkTargetId::LaneUnit(minion_id))
        );
        abilities.sophia_minions.push(ServerSophiaMinion {
            caster_player_id: 1,
            position: Vec3::new(0.0, 0.35, 0.0),
            phase: 0.0,
            elapsed: 0.0,
            lifetime_seconds: 4.0,
            search_radius: 4.5,
            chase_speed: 10.0,
            radius: 0.34,
            damage: 30.0,
            slow_seconds: 0.0,
            slow_multiplier: 1.0,
            target: Some(NetworkTargetId::LaneUnit(minion_id)),
        });

        update_sophia_minions(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.03,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 350.0);

        update_sophia_minions(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            0.01,
        );
        assert_approx_eq(lane.spell_test_unit_health(minion_id).unwrap(), 320.0);
    }

    #[test]
    fn sophia_w_minions_cannot_cross_a_nexus_or_hit_a_target_behind_it() {
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let nexus_position = Vec3::ZERO;
        lane.spawn_spell_test_unit(LaneUnitKind::Nexus, TeamSpec::Dark, nexus_position);
        let target_id = lane.spawn_spell_test_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 4.0),
        );
        let initial_target_health = lane
            .spell_test_unit_health(target_id)
            .expect("the target minion is alive");
        let mut players = ConnectedPlayers::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        let sophia_minion_radius = 0.34;
        players.states.insert(
            1,
            test_player_state(
                ChampionId::SOPHIA,
                DevelopmentTeam::Light,
                Vec3::new(0.0, 0.0, -4.0),
            ),
        );
        abilities.sophia_minions.push(ServerSophiaMinion {
            caster_player_id: 1,
            position: Vec3::new(0.0, 0.35, -4.0),
            phase: 0.0,
            elapsed: 0.0,
            lifetime_seconds: 4.0,
            search_radius: 10.0,
            chase_speed: 10.0,
            radius: sophia_minion_radius,
            damage: 30.0,
            slow_seconds: 0.0,
            slow_multiplier: 1.0,
            target: Some(NetworkTargetId::LaneUnit(target_id)),
        });

        update_sophia_minions(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            1.0,
        );

        let sophia_minion = abilities
            .sophia_minions
            .first()
            .expect("the blocked Sophia minion remains active");
        let required_clearance =
            lane_unit_stats(LaneUnitKind::Nexus).hit_radius + sophia_minion_radius;
        assert!(horizontal_distance(sophia_minion.position, nexus_position) >= required_clearance);
        assert!(sophia_minion.position.z < nexus_position.z);
        assert_eq!(
            lane.spell_test_unit_health(target_id),
            Some(initial_target_health)
        );
    }

    #[test]
    fn active_abilities_are_cleared_without_ready_clients() {
        let mut abilities = ActiveServerAbilities::default();
        abilities
            .auto_attack_projectiles
            .push(ServerAutoAttackProjectile {
                caster_team: TeamSpec::Light,
                target: NetworkTargetId::Player(2),
                remaining_seconds: 0.2,
                damage: 45.0,
            });
        abilities.w_projectiles.push(ServerWProjectile {
            caster_player_id: 1,
            end: Vec3::ZERO,
            elapsed: 0.2,
            travel_seconds: 0.5,
            explosion_radius: 2.0,
            area_damage: 48.0,
        });

        reset_active_abilities_without_ready_players(
            &mut abilities,
            &LoadingScreenReadyPlayers::default(),
        );

        assert!(abilities.auto_attack_projectiles.is_empty());
        assert!(abilities.w_projectiles.is_empty());
    }

    #[test]
    fn player_health_regeneration_restores_living_players_without_exceeding_maximum() {
        let mut players = ConnectedPlayers::default();
        let elapsed_seconds = 2.0;
        let initial_damaged_health = 70.0;
        let max_health = 100.0;
        let mut damaged_player =
            test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO);
        damaged_player.health = initial_damaged_health;
        damaged_player.max_health = max_health;
        let mut capped_player =
            test_player_state(ChampionId::YUNA, DevelopmentTeam::Light, Vec3::ZERO);
        capped_player.health = max_health - 1.0;
        capped_player.max_health = max_health;
        let mut dead_player =
            test_player_state(DARK_TARGET_CHAMPION, DevelopmentTeam::Dark, Vec3::ZERO);
        dead_player.health = 0.0;
        dead_player.max_health = max_health;
        players.states.insert(1, damaged_player);
        players.states.insert(2, capped_player);
        players.states.insert(3, dead_player);

        regenerate_player_health(&mut players, elapsed_seconds);

        assert_approx_eq(
            players.states[&1].health,
            initial_damaged_health
                + DEFAULT_PLAYER_HEALTH_REGENERATION_PER_SECOND * elapsed_seconds,
        );
        assert_eq!(players.states[&2].health, max_health);
        assert_eq!(players.states[&3].health, 0.0);
    }

    #[test]
    fn accepted_auto_attacks_apply_champion_combo_damage_on_projectile_impact() {
        let mut players = ConnectedPlayers::default();
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        let combo_target_max_health = 400.0;
        players.states.insert(
            1,
            test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        let mut combo_target = test_player_state(
            DARK_TARGET_CHAMPION,
            DevelopmentTeam::Dark,
            Vec3::new(3.0, 0.0, 0.0),
        );
        combo_target.health = combo_target_max_health;
        combo_target.max_health = combo_target_max_health;
        players.states.insert(2, combo_target);

        let combo = auto_attack_combo(ChampionId::LIRA);
        let expected_damages = (0..combo.combo_length)
            .map(|stage| combo.damage_for_stage(stage))
            .collect::<Vec<_>>();
        let mut expected_health = combo_target_max_health;

        for expected_damage in expected_damages {
            let combat_event_count = combat_events.events.len();
            let event = accept_auto_attack(1, 2, &mut players, &mut abilities)
                .expect("the valid auto attack starts a projectile");
            assert_approx_eq(players.states.get(&2).unwrap().health, expected_health);
            assert_eq!(combat_events.events.len(), combat_event_count);

            advance_server_auto_attack_projectiles(
                &mut abilities,
                &mut players,
                &mut lane,
                &mut combat_events,
                event.travel_seconds,
            );
            expected_health -= expected_damage;
            assert_approx_eq(players.states.get(&2).unwrap().health, expected_health);
            players.states.get_mut(&1).unwrap().auto_attack_cooldown = 0.0;
        }

        let combat_event_count = combat_events.events.len();
        let event = accept_auto_attack(1, 2, &mut players, &mut abilities)
            .expect("the valid auto attack starts a projectile");
        assert_approx_eq(players.states.get(&2).unwrap().health, expected_health);
        assert_eq!(combat_events.events.len(), combat_event_count);
        advance_server_auto_attack_projectiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            event.travel_seconds,
        );
        expected_health -= combo.damage_for_stage(0);
        assert_approx_eq(players.states.get(&2).unwrap().health, expected_health);
        assert!(
            combat_events
                .events
                .iter()
                .all(|event| event.kind == NetworkCombatNumberKind::AutoAttack)
        );
    }

    #[test]
    fn player_auto_attack_damage_waits_for_minion_tower_and_nexus_projectile_impact() {
        let catalog = ServerChampionCatalog::embedded_test_catalog();
        let damage = catalog
            .auto_attack_combo(ChampionId::LIRA)
            .unwrap()
            .damage_for_stage(0);

        for kind in [
            LaneUnitKind::MeleeBox,
            LaneUnitKind::Tower,
            LaneUnitKind::Nexus,
        ] {
            let mut players = ConnectedPlayers::default();
            let mut abilities = ActiveServerAbilities::default();
            let mut lane = ServerLaneState::default();
            let mut combat_events = ServerCombatNumberEvents::default();
            players.states.insert(
                1,
                test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO),
            );
            let target_id =
                lane.spawn_spell_test_unit(kind, TeamSpec::Dark, Vec3::new(3.0, 0.0, 0.0));
            let initial_health = lane
                .spell_test_unit_health(target_id)
                .expect("the lane target exists");

            let event = accept_auto_attack_target(
                1,
                NetworkTargetId::LaneUnit(target_id),
                &mut players,
                Some(&mut lane),
                &mut abilities,
                &catalog,
            )
            .expect("the valid auto attack starts a projectile");

            assert_eq!(lane.spell_test_unit_health(target_id), Some(initial_health));
            advance_server_auto_attack_projectiles(
                &mut abilities,
                &mut players,
                &mut lane,
                &mut combat_events,
                event.travel_seconds * 0.5,
            );
            assert_eq!(lane.spell_test_unit_health(target_id), Some(initial_health));

            advance_server_auto_attack_projectiles(
                &mut abilities,
                &mut players,
                &mut lane,
                &mut combat_events,
                event.travel_seconds,
            );
            assert_approx_eq(
                lane.spell_test_unit_health(target_id)
                    .expect("the lane target remains after one attack"),
                initial_health - damage,
            );
        }
    }

    #[test]
    fn player_auto_attack_projectile_ignores_a_despawned_lane_target() {
        let catalog = ServerChampionCatalog::embedded_test_catalog();
        let mut players = ConnectedPlayers::default();
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        let target_id = lane.spawn_spell_test_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(3.0, 0.0, 0.0),
        );
        let event = accept_auto_attack_target(
            1,
            NetworkTargetId::LaneUnit(target_id),
            &mut players,
            Some(&mut lane),
            &mut abilities,
            &catalog,
        )
        .expect("the valid auto attack starts a projectile");

        lane.despawn_test_unit(target_id);
        assert_eq!(lane.spell_test_unit_health(target_id), None);

        advance_server_auto_attack_projectiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            event.travel_seconds,
        );

        assert!(abilities.auto_attack_projectiles.is_empty());
        assert_eq!(lane.spell_test_unit_health(target_id), None);
    }

    #[test]
    fn auto_attack_combo_resets_when_target_changes() {
        let mut players = ConnectedPlayers::default();
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::YUNA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        players.states.insert(
            2,
            test_player_state(
                DARK_TARGET_CHAMPION,
                DevelopmentTeam::Dark,
                Vec3::new(3.0, 0.0, 0.0),
            ),
        );
        players.states.insert(
            3,
            test_player_state(
                DARK_TARGET_CHAMPION,
                DevelopmentTeam::Dark,
                Vec3::new(4.0, 0.0, 0.0),
            ),
        );

        let first_hit_damage = auto_attack_combo(ChampionId::YUNA).damage_for_stage(0);

        let first_event = accept_auto_attack(1, 2, &mut players, &mut abilities)
            .expect("the first valid auto attack starts a projectile");
        players.states.get_mut(&1).unwrap().auto_attack_cooldown = 0.0;
        let second_event = accept_auto_attack(1, 3, &mut players, &mut abilities)
            .expect("the target change starts a new projectile");

        assert_eq!(players.states.get(&3).unwrap().health, 100.0);
        advance_server_auto_attack_projectiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            first_event.travel_seconds.max(second_event.travel_seconds),
        );

        assert_approx_eq(
            players.states.get(&3).unwrap().health,
            100.0 - first_hit_damage,
        );
    }
    #[test]
    fn auto_attack_combo_resets_after_idle_timeout() {
        let mut players = ConnectedPlayers::default();
        let mut abilities = ActiveServerAbilities::default();
        let mut lane = ServerLaneState::default();
        let mut combat_events = ServerCombatNumberEvents::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::YUNA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        players.states.insert(
            2,
            test_player_state(
                DARK_TARGET_CHAMPION,
                DevelopmentTeam::Dark,
                Vec3::new(3.0, 0.0, 0.0),
            ),
        );

        let first_hit_damage = auto_attack_combo(ChampionId::YUNA).damage_for_stage(0);

        let first_event = accept_auto_attack(1, 2, &mut players, &mut abilities)
            .expect("the first valid auto attack starts a projectile");
        advance_server_auto_attack_projectiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            first_event.travel_seconds,
        );
        players.states.get_mut(&1).unwrap().auto_attack_cooldown = 0.0;
        tick_ability_cooldowns(
            &mut players,
            AUTO_ATTACK_COMBO_RESET_SECONDS
                + auto_attack_combo(ChampionId::YUNA).cooldown_seconds(),
        );
        let second_event = accept_auto_attack(1, 2, &mut players, &mut abilities)
            .expect("the idle-reset auto attack starts a projectile");
        advance_server_auto_attack_projectiles(
            &mut abilities,
            &mut players,
            &mut lane,
            &mut combat_events,
            second_event.travel_seconds,
        );

        assert_approx_eq(
            players.states.get(&2).unwrap().health,
            100.0 - first_hit_damage * 2.0,
        );
    }

    #[test]
    fn player_navigation_moves_at_server_speed_without_teleporting() {
        let mut navigation = ServerPlayerNavigation::default();
        let mut players = ConnectedPlayers::default();
        let mut lane = ServerLaneState::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        navigation.request_move(1, Vec3::new(0.0, 0.0, 20.0));

        advance_server_player_navigation(
            &mut navigation,
            &mut players,
            &mut lane,
            &ActiveServerAbilities::default(),
            0.1,
        );

        let player = &players.states[&1];
        assert_approx_eq(player.position.z, SERVER_PLAYER_NAVIGATION_SPEED * 0.1);
        assert!(player.position.z < 20.0);
        assert!(player.moving);
    }

    #[test]
    fn player_navigation_uses_authoritative_speed_modifiers() {
        let mut navigation = ServerPlayerNavigation::default();
        let mut players = ConnectedPlayers::default();
        let mut lane = ServerLaneState::default();
        let mut player = test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO);
        player.slow_timer = 1.0;
        player.slow_multiplier = 0.5;
        players.states.insert(1, player);
        navigation.request_move(1, Vec3::new(0.0, 0.0, 20.0));

        advance_server_player_navigation(
            &mut navigation,
            &mut players,
            &mut lane,
            &ActiveServerAbilities::default(),
            0.1,
        );

        assert_approx_eq(
            players.states[&1].position.z,
            SERVER_PLAYER_NAVIGATION_SPEED * 0.5 * 0.1,
        );
    }

    #[test]
    fn player_attack_move_reaches_an_out_of_range_lane_target_inside_attack_range() {
        let mut navigation = ServerPlayerNavigation::default();
        let mut players = ConnectedPlayers::default();
        let mut lane = ServerLaneState::default();
        let target_id = lane.spawn_spell_test_unit(
            LaneUnitKind::MeleeBox,
            TeamSpec::Dark,
            Vec3::new(0.0, 0.0, 15.0),
        );
        players.states.insert(
            1,
            test_player_state(
                ChampionId::LIRA,
                DevelopmentTeam::Light,
                Vec3::new(0.0, 0.0, -15.0),
            ),
        );
        navigation.request_attack_move(1, NetworkTargetId::LaneUnit(target_id));

        advance_server_player_navigation(
            &mut navigation,
            &mut players,
            &mut lane,
            &ActiveServerAbilities::default(),
            0.1,
        );
        assert_approx_eq(
            players.states[&1].position.z,
            -15.0 + SERVER_PLAYER_NAVIGATION_SPEED * 0.1,
        );
        assert!(players.states[&1].moving);

        for _ in 0..160 {
            advance_server_player_navigation(
                &mut navigation,
                &mut players,
                &mut lane,
                &ActiveServerAbilities::default(),
                0.1,
            );
        }

        let (target_position, target_radius) = lane
            .target_for_player_auto_attack(TeamSpec::Light, target_id)
            .expect("a living enemy minion");
        assert!(
            horizontal_distance(players.states[&1].position, target_position)
                <= AUTO_ATTACK_RANGE + target_radius,
            "attack move stopped outside legal attack range"
        );
        assert!(!players.states[&1].moving);
        assert_eq!(
            navigation.paths[&1].attack_target,
            Some(NetworkTargetId::LaneUnit(target_id))
        );
        assert_approx_eq(
            lane.spell_test_unit_health(target_id)
                .expect("the attack-move target remains alive"),
            lane_unit_stats(LaneUnitKind::MeleeBox).max_health,
        );
    }

    #[test]
    fn player_attack_move_routes_around_towers_without_entering_their_clearance() {
        let mut navigation = ServerPlayerNavigation::default();
        let mut players = ConnectedPlayers::default();
        let mut lane = ServerLaneState::default();
        let blocking_tower_position = Vec3::ZERO;
        lane.spawn_spell_test_unit(LaneUnitKind::Tower, TeamSpec::Dark, blocking_tower_position);
        let target_tower_position = Vec3::new(0.0, 0.0, 10.0);
        let target_tower_id =
            lane.spawn_spell_test_unit(LaneUnitKind::Tower, TeamSpec::Dark, target_tower_position);
        players.states.insert(
            1,
            test_player_state(
                ChampionId::LIRA,
                DevelopmentTeam::Light,
                Vec3::new(0.0, 0.0, -10.0),
            ),
        );
        navigation.request_attack_move(1, NetworkTargetId::LaneUnit(target_tower_id));

        let tower_clearance =
            lane_unit_stats(LaneUnitKind::Tower).hit_radius + DEVELOPMENT_PLAYER_HIT_RADIUS;
        let mut maximum_side_offset = 0.0_f32;
        for _ in 0..160 {
            advance_server_player_navigation(
                &mut navigation,
                &mut players,
                &mut lane,
                &ActiveServerAbilities::default(),
                0.1,
            );
            let position = players.states[&1].position;
            maximum_side_offset = maximum_side_offset.max(position.x.abs());
            assert!(
                horizontal_distance(position, blocking_tower_position) + 0.001 >= tower_clearance
            );
            assert!(
                horizontal_distance(position, target_tower_position) + 0.001 >= tower_clearance
            );
        }

        let (_, target_radius) = lane
            .target_for_player_auto_attack(TeamSpec::Light, target_tower_id)
            .expect("a living target tower");
        assert!(
            maximum_side_offset > 1.0,
            "attack move never routed around the blocker"
        );
        assert!(
            horizontal_distance(players.states[&1].position, target_tower_position)
                <= AUTO_ATTACK_RANGE + target_radius
        );
    }

    #[test]
    fn player_attack_move_replans_when_its_player_target_moves() {
        let mut navigation = ServerPlayerNavigation::default();
        let mut players = ConnectedPlayers::default();
        let mut lane = ServerLaneState::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        players.states.insert(
            2,
            test_player_state(
                DARK_TARGET_CHAMPION,
                DevelopmentTeam::Dark,
                Vec3::new(0.0, 0.0, 12.0),
            ),
        );
        navigation.request_attack_move(1, NetworkTargetId::Player(2));

        advance_server_player_navigation(
            &mut navigation,
            &mut players,
            &mut lane,
            &ActiveServerAbilities::default(),
            0.1,
        );
        let initial_goal = navigation.paths[&1].requested_target;
        let moved_target_position = Vec3::new(2.0, 0.0, 15.0);
        players.states.get_mut(&2).unwrap().position = moved_target_position;

        advance_server_player_navigation(
            &mut navigation,
            &mut players,
            &mut lane,
            &ActiveServerAbilities::default(),
            0.1,
        );

        let route = &navigation.paths[&1];
        assert_eq!(route.attack_target_position, Some(moved_target_position));
        assert_ne!(route.requested_target, initial_goal);
        assert!(players.states[&1].moving);
    }

    #[test]
    fn move_to_replaces_an_active_player_attack_move_order() {
        let mut navigation = ServerPlayerNavigation::default();
        navigation.request_attack_move(1, NetworkTargetId::Player(2));
        navigation.request_move(1, Vec3::new(1.0, 0.0, 2.0));

        let route = &navigation.paths[&1];
        assert_eq!(route.attack_target, None);
        assert_eq!(route.requested_target, Vec3::new(1.0, 0.0, 2.0));
    }

    #[test]
    fn player_attack_move_stops_when_its_target_is_invalid() {
        let mut navigation = ServerPlayerNavigation::default();
        let mut players = ConnectedPlayers::default();
        let mut lane = ServerLaneState::default();
        players.states.insert(
            1,
            test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, Vec3::ZERO),
        );
        navigation.request_attack_move(1, NetworkTargetId::Player(99));

        advance_server_player_navigation(
            &mut navigation,
            &mut players,
            &mut lane,
            &ActiveServerAbilities::default(),
            0.1,
        );

        assert!(!navigation.paths.contains_key(&1));
        assert!(!players.states[&1].moving);
    }

    #[test]
    fn player_navigation_routes_around_living_structures() {
        for structure_kind in [LaneUnitKind::Tower, LaneUnitKind::Nexus] {
            let mut navigation = ServerPlayerNavigation::default();
            let mut players = ConnectedPlayers::default();
            let mut lane = ServerLaneState::default();
            let structure_position = Vec3::ZERO;
            lane.spawn_spell_test_unit(structure_kind, TeamSpec::Dark, structure_position);
            players.states.insert(
                1,
                test_player_state(
                    ChampionId::LIRA,
                    DevelopmentTeam::Light,
                    Vec3::new(0.0, 0.0, -10.0),
                ),
            );
            navigation.request_move(1, Vec3::new(0.0, 0.0, 10.0));

            let mut maximum_side_offset = 0.0_f32;
            let minimum_clearance =
                lane_unit_stats(structure_kind).hit_radius + DEVELOPMENT_PLAYER_HIT_RADIUS;
            for _ in 0..80 {
                advance_server_player_navigation(
                    &mut navigation,
                    &mut players,
                    &mut lane,
                    &ActiveServerAbilities::default(),
                    0.1,
                );
                let position = players.states[&1].position;
                maximum_side_offset = maximum_side_offset.max(position.x.abs());
                assert!(
                    horizontal_distance(position, structure_position) + 0.001 >= minimum_clearance,
                    "player crossed the {structure_kind:?} at {position:?}"
                );
            }

            assert!(maximum_side_offset > 1.0);
            assert!(players.states[&1].position.z > 9.5);
        }
    }

    #[test]
    fn player_navigation_escapes_a_tower_collision_edge_before_following_the_route() {
        let mut navigation = ServerPlayerNavigation::default();
        let mut players = ConnectedPlayers::default();
        let mut lane = ServerLaneState::default();
        let tower_position = Vec3::ZERO;
        lane.spawn_spell_test_unit(LaneUnitKind::Tower, TeamSpec::Dark, tower_position);
        let collision_radius =
            lane_unit_stats(LaneUnitKind::Tower).hit_radius + DEVELOPMENT_PLAYER_HIT_RADIUS;
        let physical_edge = Vec3::new(0.0, 0.0, -collision_radius);
        players.states.insert(
            1,
            test_player_state(ChampionId::LIRA, DevelopmentTeam::Light, physical_edge),
        );
        navigation.request_move(1, Vec3::new(0.0, 0.0, 10.0));

        advance_server_player_navigation(
            &mut navigation,
            &mut players,
            &mut lane,
            &ActiveServerAbilities::default(),
            0.1,
        );
        assert!(navigation.paths[&1].recovery_waypoint.is_some());

        let mut farthest_distance = horizontal_distance(players.states[&1].position, physical_edge);
        for _ in 0..79 {
            advance_server_player_navigation(
                &mut navigation,
                &mut players,
                &mut lane,
                &ActiveServerAbilities::default(),
                0.1,
            );
            let position = players.states[&1].position;
            farthest_distance = farthest_distance.max(horizontal_distance(position, physical_edge));
            assert!(
                horizontal_distance(position, tower_position) + 0.001 >= collision_radius,
                "player crossed the tower at {position:?}"
            );
        }

        assert!(
            farthest_distance > 1.0,
            "player remained at the collision edge"
        );
        assert!(players.states[&1].position.z > 9.5);
    }

    #[test]
    fn player_navigation_replans_to_the_original_goal_after_tower_destruction() {
        let mut navigation = ServerPlayerNavigation::default();
        let mut players = ConnectedPlayers::default();
        let mut lane = ServerLaneState::default();
        lane.spawn_spell_test_unit(LaneUnitKind::Tower, TeamSpec::Dark, Vec3::ZERO);
        players.states.insert(
            1,
            test_player_state(
                ChampionId::LIRA,
                DevelopmentTeam::Light,
                Vec3::new(0.0, 0.0, -10.0),
            ),
        );
        navigation.request_move(1, Vec3::ZERO);

        advance_server_player_navigation(
            &mut navigation,
            &mut players,
            &mut lane,
            &ActiveServerAbilities::default(),
            0.1,
        );
        let projected_target = navigation.paths[&1].target;
        assert_ne!(projected_target, Vec3::ZERO);

        lane.despawn_test_unit(1);
        advance_server_player_navigation(
            &mut navigation,
            &mut players,
            &mut lane,
            &ActiveServerAbilities::default(),
            0.1,
        );

        assert_eq!(navigation.paths[&1].requested_target, Vec3::ZERO);
        assert_eq!(navigation.paths[&1].target, Vec3::ZERO);
    }

    const DARK_TARGET_CHAMPION: ChampionId = ChampionId::IGNARA;
    fn test_player_state(
        champion: ChampionId,
        team: DevelopmentTeam,
        position: Vec3,
    ) -> ConnectedPlayerState {
        ConnectedPlayerState {
            position,
            position_correction_generation: 0,
            yaw: 0.0,
            moving: false,
            health: 100.0,
            max_health: 100.0,
            champion,
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
    fn assert_approx_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.001,
            "expected {expected}, got {actual}"
        );
    }
}
