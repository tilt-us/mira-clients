use super::super::content::{ServerAbilityDefinition, ServerChampionCatalog};
use super::super::match_manifest::{ServerMatchManifest, ServerMatchPlayer};
use bevy::prelude::*;
use game_shared::game::team::TeamSpec;
use game_shared::network::{
    AbilitySlot, AbilityVisualEvent, AbilityVisualTuning, ChampionCatalogUpdate, ChampionId,
    DisplayReady, LoadingScreenPlayer, LoadingScreenStatus, MatchSnapshot, NetworkPlayer,
    PlayerCommand, PlayerStateChannel, PlayerStateUpdate, ReliableCommandChannel, WorldPosition,
};
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::*;
use std::collections::{HashMap, HashSet};

const LIRA_CHAMPION_ID: ChampionId = ChampionId(6606);
const IGNARA_CHAMPION_ID: ChampionId = ChampionId(6607);
const YUNA_CHAMPION_ID: ChampionId = ChampionId(6608);
const SOPHIA_CHAMPION_ID: ChampionId = ChampionId(6609);
const DEFAULT_DEVELOPMENT_TEAM: DevelopmentTeam = DevelopmentTeam::Light;
const DEVELOPMENT_PLAYER_HIT_RADIUS: f32 = 0.9;
const DEVELOPMENT_PLAYER_SPACING: f32 = 4.5;
const MATCH_SNAPSHOT_INTERVAL_SECONDS: f32 = 0.05;
const RESPAWN_SECONDS: f32 = 5.0;
const RESPAWN_INPUT_GRACE_SECONDS: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DevelopmentTeam {
    Neutral,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy)]
/// Description:
/// Stores the latest known visual state for one connected development player.
///
/// Fields:
/// - `position`: Latest known world-space position.
/// - `yaw`: Latest known facing angle around the Y axis.
/// - `moving`: Whether the player is currently moving.
/// - `health`: Current synchronized health value.
/// - `lira_q_cooldown`: Remaining authoritative Lira Q cooldown in seconds.
/// - `lira_w_cooldown`: Remaining authoritative Lira W cooldown in seconds.
/// - `lira_e_cooldown`: Remaining authoritative Lira E cooldown in seconds.
/// - `respawn_timer`: Remaining respawn time when the player is dead.
/// - `respawn_generation`: Monotonic counter incremented each time the player respawns.
/// - `respawn_input_grace`: Short duration that rejects stale position updates after respawn.
struct ConnectedPlayerState {
    position: Vec3,
    yaw: f32,
    moving: bool,
    health: f32,
    champion: ChampionId,
    lira_q_cooldown: f32,
    lira_w_cooldown: f32,
    lira_e_cooldown: f32,
    ignara_q_cooldown: f32,
    ignara_w_cooldown: f32,
    ignara_e_cooldown: f32,
    yuna_q_cooldown: f32,
    yuna_w_cooldown: f32,
    yuna_e_cooldown: f32,
    sophia_q_cooldown: f32,
    sophia_w_cooldown: f32,
    sophia_e_cooldown: f32,
    sophia_damage_buff_timer: f32,
    sophia_speed_buff_timer: f32,
    sophia_damage_amp_available: bool,
    slow_timer: f32,
    slow_multiplier: f32,
    stun_timer: f32,
    team: DevelopmentTeam,
    respawn_timer: Option<f32>,
    respawn_generation: u32,
    respawn_input_grace: f32,
}

#[derive(Resource, Debug, Default)]
/// Description:
/// Stores the latest known server-side state for connected development players.
///
/// Fields:
/// - `states`: Latest known player states by player id.
pub(super) struct ConnectedPlayers {
    states: HashMap<u64, ConnectedPlayerState>,
}

#[derive(Resource, Debug, Default)]
/// Description:
/// Stores active server-authoritative ability simulations.
///
/// Fields:
/// - `q_projectiles`: Active Lira Q projectiles.
/// - `w_projectiles`: Active Lira W arcing projectiles.
/// - `e_missiles`: Active Lira E contact missiles.
pub(super) struct ActiveServerAbilities {
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

#[derive(Debug, Clone)]
/// Description:
/// Stores one active server-authoritative Lira Q projectile.
///
/// Fields:
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
}

#[derive(Debug, Clone, Copy)]
/// Description:
/// Stores one active server-authoritative Lira W projectile.
///
/// Fields:
/// - `caster_player_id`: Player id that owns the projectile.
/// - `end`: Projectile landing position.
/// - `elapsed`: Elapsed projectile lifetime in seconds.
/// - `travel_seconds`: Server-authoritative travel duration.
/// - `explosion_radius`: Server-authoritative landing explosion radius.
/// - `area_damage`: Server-authoritative damage applied by the landing explosion.
struct ServerWProjectile {
    caster_player_id: u64,
    end: Vec3,
    elapsed: f32,
    travel_seconds: f32,
    explosion_radius: f32,
    area_damage: f32,
}

#[derive(Debug, Clone, Copy)]
/// Description:
/// Stores one active server-authoritative Lira E missile.
///
/// Fields:
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Description:
/// Defines the server-side behavior mode for one Lira E missile.
///
/// Fields:
/// - `Orbiting`: Missile is orbiting the caster and searching for a target.
/// - `Chasing`: Missile is chasing the stored player id.
enum ServerEMissileMode {
    Orbiting,
    Chasing(u64),
}

#[derive(Debug, Clone)]
/// Description:
/// Stores one server-authoritative Ignara Q burning ground zone.
struct ServerIgnaraQZone {
    caster_player_id: u64,
    start: Vec3,
    end: Vec3,
    elapsed: f32,
    lifetime_seconds: f32,
    width: f32,
    damage_per_second: f32,
}

#[derive(Debug, Clone, Copy)]
/// Description:
/// Stores one server-authoritative Ignara W fireball.
struct ServerIgnaraWFireball {
    target_player_id: u64,
    elapsed: f32,
    travel_seconds: f32,
    damage: f32,
}

#[derive(Debug, Clone)]
/// Description:
/// Stores one server-authoritative Ignara E rolling snowball.
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
}

#[derive(Debug, Clone)]
/// Description:
/// Stores one server-authoritative Yuna Q gravity field.
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

#[derive(Debug, Clone)]
/// Description:
/// Stores one server-authoritative Yuna W healing field.
struct ServerYunaWField {
    caster_player_id: u64,
    elapsed: f32,
    tick_elapsed: f32,
    lifetime_seconds: f32,
    radius: f32,
    heal: f32,
}

#[derive(Debug, Clone)]
/// Description:
/// Stores one server-authoritative Sophia Q damage orb attached to an enemy player.
struct ServerSophiaQOrb {
    caster_player_id: u64,
    target_player_id: u64,
    elapsed: f32,
    tick_elapsed: f32,
    lifetime_seconds: f32,
    damage_per_second: f32,
}

#[derive(Debug, Clone)]
/// Description:
/// Stores one server-authoritative Sophia W minion.
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
    target_player_id: Option<u64>,
}

#[derive(Resource, Debug)]
/// Description:
/// Limits how often the development server broadcasts match roster snapshots.
///
/// Fields:
/// - `0`: Repeating timer for snapshot broadcasts.
pub(super) struct MatchSnapshotBroadcastTimer(Timer);

#[derive(Resource, Debug, Default)]
/// Description:
/// Tracks connected clients that already received the current champion catalog.
///
/// Fields:
/// - `0`: Netcode player ids that have received the catalog update.
pub(super) struct SentChampionCatalogClients(HashSet<u64>);

#[derive(Resource, Debug, Default)]
/// Description:
/// Tracks clients whose display has finished loading local match visuals.
pub(super) struct LoadingScreenReadyPlayers {
    ready_player_ids: HashSet<u64>,
}

#[derive(Resource, Debug, Default)]
/// Description:
/// Caches the last logged loading-screen status so server diagnostics do not spam every tick.
pub(super) struct LoadingScreenStatusLogCache {
    last_status: Option<(usize, usize, Vec<u64>, bool)>,
}

impl Default for MatchSnapshotBroadcastTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            MATCH_SNAPSHOT_INTERVAL_SECONDS,
            TimerMode::Repeating,
        ))
    }
}

/// Description:
/// Receives client display-ready signals after each client has loaded local visuals.
pub(super) fn receive_display_ready(
    mut clients: Query<
        (&RemoteId, &mut MessageReceiver<DisplayReady>),
        (With<ClientOf>, With<Connected>),
    >,
    mut ready_players: ResMut<LoadingScreenReadyPlayers>,
    manifest: Res<ServerMatchManifest>,
) {
    let connected_player_ids = clients
        .iter()
        .filter_map(|(remote_id, _)| netcode_player_id(*remote_id))
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
            ready_players.ready_player_ids.insert(player_id);
        }
    }
}

/// Description:
/// Broadcasts current loading-screen readiness to every connected client.
pub(super) fn broadcast_loading_screen_status(
    mut clients: Query<
        (&RemoteId, &mut MessageSender<LoadingScreenStatus>),
        (With<ClientOf>, With<Connected>),
    >,
    mut ready_players: ResMut<LoadingScreenReadyPlayers>,
    mut log_cache: ResMut<LoadingScreenStatusLogCache>,
    manifest: Res<ServerMatchManifest>,
    players: Res<ConnectedPlayers>,
) {
    let connected_player_ids = clients
        .iter()
        .filter_map(|(remote_id, _)| netcode_player_id(*remote_id))
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
    let status_signature = (
        ready_count,
        total_players,
        ready_player_ids.clone(),
        can_close,
    );
    if log_cache.last_status.as_ref() != Some(&status_signature) {
        info!(
            "Loading screen status: ready={}/{} ids={:?} can_close={}",
            ready_count, total_players, ready_player_ids, can_close
        );
        log_cache.last_status = Some(status_signature);
    }

    for (_, mut sender) in &mut clients {
        sender.send::<PlayerStateChannel>(LoadingScreenStatus {
            ready_players: ready_count,
            total_players,
            ready_player_ids: ready_player_ids.clone(),
            players: loading_players.clone(),
            can_close,
        });
    }
}

/// Description:
/// Sends the loaded server champion catalog once to each connected client.
///
/// Params:
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

/// Description:
/// Receives local player state updates sent by connected clients.
///
/// Params:
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
    manifest: Res<ServerMatchManifest>,
) {
    for (remote_id, mut receiver) in &mut clients {
        let Some(player_id) = netcode_player_id(*remote_id) else {
            continue;
        };
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
                    if state.champion != champion {
                        state.champion = champion;
                        state.health = max_health;
                    }
                    state.team = team.into();
                    if state.health <= 0.0
                        || state.respawn_input_grace > 0.0
                        || state.stun_timer > 0.0
                    {
                        state.moving = false;
                    } else {
                        state.position = Vec3::from(update.position);
                        state.yaw = update.yaw;
                        state.moving = update.moving;
                    }
                })
                .or_insert(ConnectedPlayerState {
                    position: Vec3::from(update.position),
                    yaw: update.yaw,
                    moving: update.moving,
                    health: max_health,
                    champion,
                    lira_q_cooldown: 0.0,
                    lira_w_cooldown: 0.0,
                    lira_e_cooldown: 0.0,
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
                    team: team.into(),
                    respawn_timer: None,
                    respawn_generation: 0,
                    respawn_input_grace: 0.0,
                });
        }
    }
}

/// Description:
/// Receives authoritative player commands and resolves supported server-side abilities.
///
/// Params:
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
    )>,
    mut players: ResMut<ConnectedPlayers>,
    mut abilities: ResMut<ActiveServerAbilities>,
    catalog: Res<ServerChampionCatalog>,
    manifest: Res<ServerMatchManifest>,
    time: Res<Time>,
) {
    tick_ability_cooldowns(&mut players, time.delta_secs());

    let mut visual_events = Vec::new();

    {
        let mut receivers = clients.p0();
        for (remote_id, mut receiver) in &mut receivers {
            let Some(caster_player_id) = netcode_player_id(*remote_id) else {
                continue;
            };

            for command in receiver.receive() {
                if let PlayerCommand::CastAbility { champion, .. } = command {
                    if !authorized_champion(&manifest, caster_player_id, champion) {
                        continue;
                    }
                }

                match command {
                    PlayerCommand::CastAbility {
                        champion,
                        slot,
                        target,
                    } if champion == LIRA_CHAMPION_ID && slot == AbilitySlot::Q => {
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
                    } if champion == LIRA_CHAMPION_ID && slot == AbilitySlot::W => {
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
                        if champion == LIRA_CHAMPION_ID && slot == AbilitySlot::E =>
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
                    } if champion == IGNARA_CHAMPION_ID && slot == AbilitySlot::Q => {
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
                    } if champion == IGNARA_CHAMPION_ID && slot == AbilitySlot::W => {
                        if let Some(event) = accept_ignara_w_cast(
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
                    } if champion == IGNARA_CHAMPION_ID && slot == AbilitySlot::E => {
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
                    } if champion == YUNA_CHAMPION_ID && slot == AbilitySlot::Q => {
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
                    } if champion == YUNA_CHAMPION_ID && slot == AbilitySlot::W => {
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
                    } if champion == YUNA_CHAMPION_ID && slot == AbilitySlot::E => {
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
                    } if champion == SOPHIA_CHAMPION_ID && slot == AbilitySlot::Q => {
                        if let Some(event) = accept_sophia_q_cast(
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
                        if champion == SOPHIA_CHAMPION_ID && slot == AbilitySlot::W =>
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
                        if champion == SOPHIA_CHAMPION_ID && slot == AbilitySlot::E =>
                    {
                        if let Some(event) =
                            accept_sophia_e_cast(caster_player_id, &mut players, &catalog)
                        {
                            visual_events.push(event);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if visual_events.is_empty() {
        return;
    }

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

/// Description:
/// Advances active server-authoritative ability simulations and applies contact damage.
///
/// Params:
/// - `abilities`: Active server-side ability simulations.
/// - `players`: Server-side development player state cache.
/// - `time`: Bevy time resource used to advance projectile and missile movement.
pub(super) fn update_server_abilities(
    mut abilities: ResMut<ActiveServerAbilities>,
    mut players: ResMut<ConnectedPlayers>,
    catalog: Res<ServerChampionCatalog>,
    time: Res<Time>,
) {
    let delta_seconds = time.delta_secs();
    update_lira_q_projectiles(&mut abilities, &mut players, delta_seconds);
    update_lira_w_projectiles(&mut abilities, &mut players, delta_seconds);
    update_lira_e_missiles(&mut abilities, &mut players, delta_seconds);
    update_ignara_q_zones(&mut abilities, &mut players, delta_seconds);
    update_ignara_w_fireballs(&mut abilities, &mut players, delta_seconds);
    update_ignara_e_snowballs(&mut abilities, &mut players, delta_seconds);
    update_yuna_q_orbs(&mut abilities, &mut players, delta_seconds);
    update_yuna_w_fields(&mut abilities, &mut players, &catalog, delta_seconds);
    update_sophia_q_orbs(&mut abilities, &mut players, delta_seconds);
    update_sophia_minions(&mut abilities, &mut players, delta_seconds);
}

/// Description:
/// Advances player death timers and respawns players when their timer expires.
///
/// Params:
/// - `players`: Server-side development player state cache.
/// - `catalog`: Server-authoritative champion content catalog.
/// - `time`: Bevy time resource used to advance respawn timers.
pub(super) fn update_player_death_and_respawn(
    mut players: ResMut<ConnectedPlayers>,
    catalog: Res<ServerChampionCatalog>,
    time: Res<Time>,
) {
    let mut player_ids = players.states.keys().copied().collect::<Vec<_>>();
    player_ids.sort_unstable();

    let player_count = player_ids.len();
    for (index, player_id) in player_ids.into_iter().enumerate() {
        let Some(state) = players.states.get_mut(&player_id) else {
            continue;
        };
        let Some(respawn_timer) = state.respawn_timer.as_mut() else {
            continue;
        };

        *respawn_timer -= time.delta_secs();
        if *respawn_timer > 0.0 {
            continue;
        }

        state.health = development_champion_max_health(&catalog, state.champion);
        state.position = development_spawn_position(index, player_count);
        state.yaw = 0.0;
        state.moving = false;
        state.lira_q_cooldown = 0.0;
        state.lira_w_cooldown = 0.0;
        state.lira_e_cooldown = 0.0;
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
        state.slow_multiplier = 1.0;
        state.stun_timer = 0.0;
        state.respawn_timer = None;
        state.respawn_generation = state.respawn_generation.saturating_add(1);
        state.respawn_input_grace = RESPAWN_INPUT_GRACE_SECONDS;
    }
}

/// Description:
/// Broadcasts ability visual events from one client to all other connected clients.
///
/// Params:
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

/// Description:
/// Sends a lightweight match roster to every connected development client.
///
/// Params:
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
    mut timer: ResMut<MatchSnapshotBroadcastTimer>,
    time: Res<Time>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let mut player_ids = clients
        .iter()
        .filter_map(|(remote_id, _)| netcode_player_id(*remote_id))
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
        .enumerate()
        .map(|(index, player_id)| {
            let manifest_player = manifest.player(*player_id);
            let fallback_champion = manifest_player
                .as_ref()
                .map(|player| player.champion)
                .unwrap_or(LIRA_CHAMPION_ID);
            let fallback_team = manifest_player
                .as_ref()
                .map(|player| DevelopmentTeam::from(player.team))
                .unwrap_or(DEFAULT_DEVELOPMENT_TEAM);
            let fallback_max_health = development_champion_max_health(&catalog, fallback_champion);
            let state = players.states.entry(*player_id).or_insert_with(|| {
                info!(
                    "Initialized server player {} from {}: champion={:?} team={:?} max_health={}",
                    player_id,
                    if manifest_player.is_some() {
                        "match manifest"
                    } else {
                        "development fallback"
                    },
                    fallback_champion,
                    fallback_team,
                    fallback_max_health
                );
                ConnectedPlayerState {
                    position: development_spawn_position(index, player_ids.len()),
                    yaw: 0.0,
                    moving: false,
                    health: fallback_max_health,
                    champion: fallback_champion,
                    lira_q_cooldown: 0.0,
                    lira_w_cooldown: 0.0,
                    lira_e_cooldown: 0.0,
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
                    team: fallback_team,
                    respawn_timer: None,
                    respawn_generation: 0,
                    respawn_input_grace: 0.0,
                }
            });
            let champion = state.champion;
            let team = state.team;
            let max_health = development_champion_max_health(&catalog, champion);
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
                yaw: state.yaw,
                moving: state.moving,
                health: state.health,
                max_health,
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
    debug!(
        "Broadcasting match snapshot players={}",
        players
            .iter()
            .map(|player| format!(
                "{}:{:?}:{:?}:{}/{}",
                player.player_id, player.champion, player.team, player.health, player.max_health
            ))
            .collect::<Vec<_>>()
            .join(",")
    );

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

/// Description:
/// Extracts the numeric Netcode player id from a remote peer id.
///
/// Params:
/// - `remote_id`: Remote peer id stored on a Lightyear client link.
///
/// Return:
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
                        .unwrap_or(LIRA_CHAMPION_ID),
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

/// Description:
/// Computes a centered development spawn position for a connected player.
///
/// Params:
/// - `index`: Sorted player index.
/// - `player_count`: Total number of connected players.
///
/// Return:
/// - World-space spawn position for the player.
fn development_spawn_position(index: usize, player_count: usize) -> Vec3 {
    let centered_index = index as f32 - (player_count.saturating_sub(1) as f32 * 0.5);
    Vec3::new(centered_index * DEVELOPMENT_PLAYER_SPACING, 0.0, 0.0)
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

/// Description:
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

/// Description:
/// Computes the outgoing movement multiplier after control effects and buffs.
fn movement_speed_multiplier(
    state: &ConnectedPlayerState,
    stunned: bool,
    pull_effect: Option<(Vec3, f32)>,
) -> f32 {
    if stunned {
        return 0.0;
    }

    let mut multiplier = 1.0;
    if let Some((_, pull_multiplier)) = pull_effect {
        multiplier *= pull_multiplier;
    }
    if state.slow_timer > 0.0 {
        multiplier *= state.slow_multiplier;
    }
    if state.sophia_speed_buff_timer > 0.0 {
        multiplier *= 1.2;
    }

    multiplier.clamp(0.0, 2.0)
}

/// Description:
/// Returns the max health configured for the current development champion.
///
/// Params:
/// - `catalog`: Server-authoritative champion content catalog.
///
/// Returns:
/// - Max health value loaded from the server champion content file.
fn development_champion_max_health(catalog: &ServerChampionCatalog, champion: ChampionId) -> f32 {
    catalog
        .champion(champion)
        .or_else(|| catalog.champion(LIRA_CHAMPION_ID))
        .unwrap_or_else(|| panic!("Missing server champion content for {}", champion.0))
        .base_stats
        .max_health
}

/// Description:
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
        .unwrap_or(LIRA_CHAMPION_ID);

    development_champion_max_health(catalog, champion)
}

/// Description:
/// Returns the tuning configured for the current development champion ability.
///
/// Params:
/// - `catalog`: Server-authoritative champion content catalog.
/// - `slot`: Ability slot whose tuning should be read.
///
/// Returns:
/// - Ability tuning loaded from the server champion content file.
fn development_ability(
    catalog: &ServerChampionCatalog,
    slot: AbilitySlot,
) -> ServerAbilityDefinition {
    champion_ability(catalog, LIRA_CHAMPION_ID, slot)
}

/// Description:
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

/// Description:
/// Consumes Sophia's next-ability damage buff and returns the active multiplier.
fn consume_sophia_damage_multiplier(
    players: &mut ConnectedPlayers,
    caster_player_id: u64,
    ability: &ServerAbilityDefinition,
) -> f32 {
    let Some(caster) = players.states.get_mut(&caster_player_id) else {
        return 1.0;
    };
    if caster.sophia_damage_buff_timer <= 0.0 || !caster.sophia_damage_amp_available {
        return 1.0;
    }

    caster.sophia_damage_amp_available = false;
    caster.sophia_damage_buff_timer = 0.0;
    positive_or(ability.damage_multiplier, 1.2)
}

/// Description:
/// Accepts a Lira Q cast and starts its server-side projectile simulation.
///
/// Params:
/// - `caster_player_id`: Player id that requested the Q cast.
/// - `target_position`: Optional world-space aim point sent by the client.
/// - `players`: Server-side development player state cache.
/// - `abilities`: Active server-side ability simulations.
/// - `catalog`: Server-authoritative champion content catalog.
///
/// Returns:
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
    });

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: LIRA_CHAMPION_ID,
        slot: AbilitySlot::Q,
        start: WorldPosition::from(start),
        end: Some(WorldPosition::from(end)),
        visual: ability_visual_tuning(&ability),
    })
}

/// Description:
/// Accepts a Lira W cast and starts its server-side projectile simulation.
///
/// Params:
/// - `caster_player_id`: Player id that requested the W cast.
/// - `target_position`: Optional world-space aim point sent by the client.
/// - `players`: Server-side development player state cache.
/// - `abilities`: Active server-side ability simulations.
/// - `catalog`: Server-authoritative champion content catalog.
///
/// Returns:
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
        champion: LIRA_CHAMPION_ID,
        slot: AbilitySlot::W,
        start: WorldPosition::from(start),
        end: Some(WorldPosition::from(end)),
        visual: ability_visual_tuning(&ability),
    })
}

/// Description:
/// Accepts a Lira E cast and starts its server-side missile simulations.
///
/// Params:
/// - `caster_player_id`: Player id that requested the E cast.
/// - `players`: Server-side development player state cache.
/// - `abilities`: Active server-side ability simulations.
/// - `catalog`: Server-authoritative champion content catalog.
///
/// Returns:
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
        champion: LIRA_CHAMPION_ID,
        slot: AbilitySlot::E,
        start: WorldPosition::from(caster.position),
        end: None,
        visual: ability_visual_tuning(&ability),
    })
}

/// Description:
/// Accepts an Ignara Q cast and starts its server-side burning ground simulation.
fn accept_ignara_q_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, IGNARA_CHAMPION_ID, AbilitySlot::Q);
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
        champion: IGNARA_CHAMPION_ID,
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

/// Description:
/// Accepts an Ignara W point-click fireball and stores the selected target.
fn accept_ignara_w_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, IGNARA_CHAMPION_ID, AbilitySlot::W);
    let target_position = target_position.map(Vec3::from)?;
    let caster = *players.states.get(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.ignara_w_cooldown > 0.0 {
        return None;
    }
    let caster_position = caster.position;
    let target_point = clamp_cast_target(caster_position, target_position, ability.range);
    let target_player_id = find_nearest_enemy_target_around_point(
        players,
        caster_player_id,
        target_point,
        ability.target_radius,
    )?;

    if let Some(caster) = players.states.get_mut(&caster_player_id) {
        caster.ignara_w_cooldown = ability.cooldown_seconds;
    }
    abilities.ignara_w_fireballs.push(ServerIgnaraWFireball {
        target_player_id,
        elapsed: 0.0,
        travel_seconds: ability.travel_seconds,
        damage: ability.damage.direct_hit,
    });

    let end = players
        .states
        .get(&target_player_id)
        .map(|target| target.position)
        .unwrap_or(target_point);

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: IGNARA_CHAMPION_ID,
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

/// Description:
/// Accepts an Ignara E cast and starts its server-side rolling snowball simulation.
fn accept_ignara_e_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, IGNARA_CHAMPION_ID, AbilitySlot::E);
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
    });

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: IGNARA_CHAMPION_ID,
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

/// Description:
/// Accepts a Yuna Q cast and starts its server-side gravity field simulation.
fn accept_yuna_q_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, YUNA_CHAMPION_ID, AbilitySlot::Q);
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
        champion: YUNA_CHAMPION_ID,
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

/// Description:
/// Accepts a Yuna W cast and starts its server-side once-per-ally healing field.
fn accept_yuna_w_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, YUNA_CHAMPION_ID, AbilitySlot::W);
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
        champion: YUNA_CHAMPION_ID,
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

/// Description:
/// Accepts a Yuna E point-click stun and applies the immobilize immediately.
fn accept_yuna_e_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, YUNA_CHAMPION_ID, AbilitySlot::E);
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
        champion: YUNA_CHAMPION_ID,
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

/// Description:
/// Accepts a Sophia Q point-click orb and starts its damage-over-time simulation.
fn accept_sophia_q_cast(
    caster_player_id: u64,
    target_position: Option<WorldPosition>,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, SOPHIA_CHAMPION_ID, AbilitySlot::Q);
    let target_position = target_position.map(Vec3::from)?;
    let caster = *players.states.get(&caster_player_id)?;
    if caster.health <= 0.0 || caster.stun_timer > 0.0 || caster.sophia_q_cooldown > 0.0 {
        return None;
    }

    let target_point = clamp_cast_target(caster.position, target_position, ability.range);
    let target_player_id = find_nearest_enemy_target_around_point(
        players,
        caster_player_id,
        target_point,
        ability.target_radius,
    )?;
    let damage_multiplier = consume_sophia_damage_multiplier(players, caster_player_id, &ability);
    if let Some(caster) = players.states.get_mut(&caster_player_id) {
        caster.sophia_q_cooldown = ability.cooldown_seconds;
    }

    abilities.sophia_q_orbs.push(ServerSophiaQOrb {
        caster_player_id,
        target_player_id,
        elapsed: 0.0,
        tick_elapsed: 0.0,
        lifetime_seconds: ability.lifetime_seconds,
        damage_per_second: ability.damage_per_second * damage_multiplier,
    });

    let target_position = players
        .states
        .get(&target_player_id)
        .map(|target| target.position)
        .unwrap_or(target_point);

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: SOPHIA_CHAMPION_ID,
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

/// Description:
/// Accepts a Sophia W cast and summons two chasing minions.
fn accept_sophia_w_cast(
    caster_player_id: u64,
    players: &mut ConnectedPlayers,
    abilities: &mut ActiveServerAbilities,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, SOPHIA_CHAMPION_ID, AbilitySlot::W);
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
            target_player_id: None,
        });
    }

    Some(AbilityVisualEvent {
        caster_player_id,
        champion: SOPHIA_CHAMPION_ID,
        slot: AbilitySlot::W,
        start: WorldPosition::from(caster.position),
        end: None,
        visual: ability_visual_tuning(&ability),
    })
}

/// Description:
/// Accepts a Sophia E self-buff.
fn accept_sophia_e_cast(
    caster_player_id: u64,
    players: &mut ConnectedPlayers,
    catalog: &ServerChampionCatalog,
) -> Option<AbilityVisualEvent> {
    let ability = champion_ability(catalog, SOPHIA_CHAMPION_ID, AbilitySlot::E);
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
        champion: SOPHIA_CHAMPION_ID,
        slot: AbilitySlot::E,
        start: WorldPosition::from(caster.position),
        end: None,
        visual: AbilityVisualTuning {
            travel_seconds: ability.lifetime_seconds,
            ..default()
        },
    })
}

/// Description:
/// Converts server-authoritative ability data into network visual tuning.
///
/// Params:
/// - `ability`: Server-authoritative ability definition accepted for a cast.
///
/// Returns:
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

/// Description:
/// Advances server-side ability cooldowns for connected players.
///
/// Params:
/// - `players`: Server-side development player state cache.
/// - `delta_seconds`: Elapsed time since the previous update.
fn tick_ability_cooldowns(players: &mut ConnectedPlayers, delta_seconds: f32) {
    for state in players.states.values_mut() {
        state.lira_q_cooldown = (state.lira_q_cooldown - delta_seconds).max(0.0);
        state.lira_w_cooldown = (state.lira_w_cooldown - delta_seconds).max(0.0);
        state.lira_e_cooldown = (state.lira_e_cooldown - delta_seconds).max(0.0);
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

/// Description:
/// Advances active Lira Q projectiles and applies direct and explosion contact damage.
///
/// Params:
/// - `abilities`: Active server-side ability simulations.
/// - `players`: Server-side development player state cache.
/// - `delta_seconds`: Elapsed time since the previous update.
fn update_lira_q_projectiles(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    delta_seconds: f32,
) {
    let mut finished_projectiles = Vec::new();

    for projectile in &mut abilities.q_projectiles {
        let caster_team = players
            .states
            .get(&projectile.caster_player_id)
            .map(|caster| caster.team);
        let travel_seconds = projectile.travel_seconds.max(f32::EPSILON);
        let previous_progress = (projectile.elapsed / travel_seconds).clamp(0.0, 1.0);
        let previous_position = projectile.start.lerp(projectile.end, previous_progress);

        projectile.elapsed += delta_seconds;
        let progress = (projectile.elapsed / travel_seconds).clamp(0.0, 1.0);
        let current_position = projectile.start.lerp(projectile.end, progress);

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
                apply_damage(target_state, projectile.direct_hit_damage);
            }
        }

        if projectile.elapsed >= travel_seconds {
            finished_projectiles.push((
                projectile.caster_player_id,
                projectile.end,
                projectile.explosion_radius,
                projectile.area_damage,
            ));
        }
    }

    abilities
        .q_projectiles
        .retain(|projectile| projectile.elapsed < projectile.travel_seconds.max(f32::EPSILON));

    for (caster_player_id, end, explosion_radius, area_damage) in finished_projectiles {
        apply_area_damage(
            players,
            caster_player_id,
            end,
            explosion_radius,
            area_damage,
        );
    }
}

/// Description:
/// Advances active Lira W projectiles and applies landing explosion contact damage.
///
/// Params:
/// - `abilities`: Active server-side ability simulations.
/// - `players`: Server-side development player state cache.
/// - `delta_seconds`: Elapsed time since the previous update.
fn update_lira_w_projectiles(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
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
        apply_area_damage(
            players,
            caster_player_id,
            end,
            explosion_radius,
            area_damage,
        );
    }
}

/// Description:
/// Advances active Lira E missiles and applies contact damage.
///
/// Params:
/// - `abilities`: Active server-side ability simulations.
/// - `players`: Server-side development player state cache.
/// - `delta_seconds`: Elapsed time since the previous update.
fn update_lira_e_missiles(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
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

        if missile.mode == ServerEMissileMode::Orbiting {
            if let Some(target_player_id) = find_lira_e_target(
                players,
                missile.caster_player_id,
                caster.position,
                missile.position,
                missile.search_radius,
            ) {
                missile.mode = ServerEMissileMode::Chasing(target_player_id);
            }
        }

        match missile.mode {
            ServerEMissileMode::Orbiting => {
                let angle = missile.phase + missile.elapsed * missile.orbit_speed;
                let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * missile.orbit_radius
                    + Vec3::Y * missile.orbit_height;
                missile.position = caster.position + offset;
            }
            ServerEMissileMode::Chasing(target_player_id) => {
                let Some(target) = players.states.get_mut(&target_player_id) else {
                    spent_missiles.push(missile_index);
                    continue;
                };

                if target.health <= 0.0 {
                    spent_missiles.push(missile_index);
                    continue;
                }

                let target_position = target.position + Vec3::Y * 0.7;
                let to_target = target_position - missile.position;
                let distance = to_target.length();

                if distance <= missile.missile_radius + DEVELOPMENT_PLAYER_HIT_RADIUS {
                    apply_damage(target, missile.damage);
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

/// Description:
/// Advances Ignara Q burning zones and applies burn damage over time.
fn update_ignara_q_zones(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    delta_seconds: f32,
) {
    for zone in &mut abilities.ignara_q_zones {
        zone.elapsed += delta_seconds;
        let caster_team = players
            .states
            .get(&zone.caster_player_id)
            .map(|caster| caster.team);
        let damage = zone.damage_per_second * delta_seconds;

        for (target_player_id, target_state) in &mut players.states {
            if *target_player_id == zone.caster_player_id
                || Some(target_state.team) == caster_team
                || target_state.health <= 0.0
            {
                continue;
            }

            if point_in_oriented_rect_xz(target_state.position, zone.start, zone.end, zone.width) {
                apply_damage(target_state, damage);
            }
        }
    }

    abilities
        .ignara_q_zones
        .retain(|zone| zone.elapsed < zone.lifetime_seconds.max(f32::EPSILON));
}

/// Description:
/// Advances Ignara W fireballs and applies direct target damage on arrival.
fn update_ignara_w_fireballs(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    delta_seconds: f32,
) {
    let mut finished_fireballs = Vec::new();

    for (index, fireball) in abilities.ignara_w_fireballs.iter_mut().enumerate() {
        fireball.elapsed += delta_seconds;
        if fireball.elapsed >= fireball.travel_seconds.max(f32::EPSILON) {
            finished_fireballs.push(index);
            if let Some(target) = players.states.get_mut(&fireball.target_player_id) {
                apply_damage(target, fireball.damage);
            }
        }
    }

    for index in finished_fireballs.into_iter().rev() {
        abilities.ignara_w_fireballs.swap_remove(index);
    }
}

/// Description:
/// Advances Ignara E rolling snowballs and applies distance-tiered contact damage.
fn update_ignara_e_snowballs(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    delta_seconds: f32,
) {
    for snowball in &mut abilities.ignara_e_snowballs {
        let caster_team = players
            .states
            .get(&snowball.caster_player_id)
            .map(|caster| caster.team);
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
                apply_damage(target_state, damage);
            }
        }
    }

    abilities
        .ignara_e_snowballs
        .retain(|snowball| snowball.elapsed < snowball.travel_seconds.max(f32::EPSILON));
}

/// Description:
/// Advances Yuna Q gravity fields, pulling and damaging enemy players inside the area.
fn update_yuna_q_orbs(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
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
        let damage = orb.damage_per_second * delta_seconds;

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

            apply_damage(target_state, damage);
            let pull_delta = Vec3::new(
                orb.position.x - target_state.position.x,
                0.0,
                orb.position.z - target_state.position.z,
            );
            let pull_distance = pull_delta.length();
            if pull_distance > 0.08 {
                let step = (orb.pull_speed * delta_seconds).min(pull_distance);
                target_state.position += pull_delta.normalize() * step;
                target_state.moving = false;
            }
        }
    }

    abilities.yuna_q_orbs.retain(|orb| {
        orb.elapsed < orb.travel_seconds.max(0.0) + orb.lifetime_seconds.max(f32::EPSILON)
    });
}

/// Description:
/// Advances Yuna W healing fields and heals allied players once per second while inside.
fn update_yuna_w_fields(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
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

        if field.tick_elapsed < 1.0 {
            continue;
        }

        let mut heal_ticks = 0;
        while field.tick_elapsed >= 1.0 {
            field.tick_elapsed -= 1.0;
            heal_ticks += 1;
        }
        let heal_amount = field.heal * heal_ticks as f32;

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
                apply_heal(target_state, heal_amount, max_health);
            }
        }
    }

    abilities
        .yuna_w_fields
        .retain(|field| field.elapsed < field.lifetime_seconds.max(f32::EPSILON));
}

/// Description:
/// Advances Sophia Q orbs and applies one damage tick per second.
fn update_sophia_q_orbs(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
    delta_seconds: f32,
) {
    for orb in &mut abilities.sophia_q_orbs {
        orb.elapsed += delta_seconds;
        orb.tick_elapsed += delta_seconds;
        while orb.tick_elapsed >= 1.0 {
            orb.tick_elapsed -= 1.0;
            let caster_team = players
                .states
                .get(&orb.caster_player_id)
                .map(|caster| caster.team);
            let Some(target) = players.states.get_mut(&orb.target_player_id) else {
                continue;
            };
            if Some(target.team) != caster_team && target.health > 0.0 {
                apply_damage(target, orb.damage_per_second);
            }
        }
    }

    abilities
        .sophia_q_orbs
        .retain(|orb| orb.elapsed < orb.lifetime_seconds.max(f32::EPSILON));
}

/// Description:
/// Advances Sophia W minions, target acquisition, chase movement, contact damage, and slow.
fn update_sophia_minions(
    abilities: &mut ActiveServerAbilities,
    players: &mut ConnectedPlayers,
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

        if minion.target_player_id.is_none() {
            let angle = minion.phase + minion.elapsed * 1.8;
            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 1.0 + Vec3::Y * 0.35;
            minion.position = caster.position + offset;
            minion.target_player_id = find_sophia_minion_target(
                players,
                minion.caster_player_id,
                minion.position,
                minion.search_radius,
            );
        }

        let Some(target_player_id) = minion.target_player_id else {
            continue;
        };
        let Some(target) = players.states.get_mut(&target_player_id) else {
            spent_minions.push(minion_index);
            continue;
        };
        if target.health <= 0.0 {
            spent_minions.push(minion_index);
            continue;
        }

        let target_forward = Quat::from_rotation_y(target.yaw) * Vec3::Z;
        let target_back = target.position - target_forward * 0.75 + Vec3::Y * 0.35;
        let to_target = target_back - minion.position;
        let distance = to_target.length();
        if distance <= minion.radius + DEVELOPMENT_PLAYER_HIT_RADIUS {
            apply_damage(target, minion.damage);
            target.slow_timer = target.slow_timer.max(minion.slow_seconds);
            target.slow_multiplier = target.slow_multiplier.min(minion.slow_multiplier);
            target.moving = false;
            spent_minions.push(minion_index);
            continue;
        }

        if distance > f32::EPSILON {
            let step = minion.chase_speed * delta_seconds;
            minion.position += to_target.normalize() * step.min(distance);
        }
    }

    spent_minions.sort_unstable();
    spent_minions.dedup();
    for minion_index in spent_minions.into_iter().rev() {
        abilities.sophia_minions.swap_remove(minion_index);
    }
}

/// Description:
/// Finds the nearest valid Lira E missile target in search radius.
///
/// Params:
/// - `players`: Server-side development player state cache.
/// - `caster_player_id`: Player id that owns the missile.
/// - `caster_position`: Current caster position used for search range.
/// - `missile_position`: Current missile position used to pick the nearest target.
/// - `search_radius`: Server-authoritative missile search radius.
///
/// Returns:
/// - Player id of the nearest valid target.
fn find_lira_e_target(
    players: &ConnectedPlayers,
    caster_player_id: u64,
    caster_position: Vec3,
    missile_position: Vec3,
    search_radius: f32,
) -> Option<u64> {
    let caster_team = players.states.get(&caster_player_id)?.team;
    players
        .states
        .iter()
        .filter(|(target_player_id, target_state)| {
            **target_player_id != caster_player_id
                && target_state.team != caster_team
                && target_state.health > 0.0
                && horizontal_distance(caster_position, target_state.position) <= search_radius
        })
        .min_by(|(_, left), (_, right)| {
            horizontal_distance(missile_position, left.position)
                .partial_cmp(&horizontal_distance(missile_position, right.position))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(target_player_id, _)| *target_player_id)
}

/// Description:
/// Finds the nearest living enemy for a Sophia W minion.
fn find_sophia_minion_target(
    players: &ConnectedPlayers,
    caster_player_id: u64,
    minion_position: Vec3,
    search_radius: f32,
) -> Option<u64> {
    let caster_team = players.states.get(&caster_player_id)?.team;
    players
        .states
        .iter()
        .filter(|(target_player_id, target_state)| {
            **target_player_id != caster_player_id
                && target_state.team != caster_team
                && target_state.health > 0.0
                && horizontal_distance(minion_position, target_state.position) <= search_radius
        })
        .min_by(|(_, left), (_, right)| {
            horizontal_distance(minion_position, left.position)
                .partial_cmp(&horizontal_distance(minion_position, right.position))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(target_player_id, _)| *target_player_id)
}

/// Description:
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

/// Description:
/// Checks whether a world-space point lies inside an oriented XZ rectangle.
fn point_in_oriented_rect_xz(point: Vec3, start: Vec3, end: Vec3, width: f32) -> bool {
    let start_2d = Vec2::new(start.x, start.z);
    let end_2d = Vec2::new(end.x, end.z);
    let point_2d = Vec2::new(point.x, point.z);
    let axis = end_2d - start_2d;
    let length = axis.length();
    if length <= f32::EPSILON {
        return false;
    }

    let forward = axis / length;
    let right = Vec2::new(forward.y, -forward.x);
    let local = point_2d - start_2d;
    let forward_distance = local.dot(forward);
    let side_distance = local.dot(right).abs();

    forward_distance >= 0.0 && forward_distance <= length && side_distance <= width * 0.5
}

/// Description:
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

/// Description:
/// Returns Ignara E collision radius for the travelled projectile distance.
fn ignara_e_radius_for_distance(distance: f32, width: f32, range: f32) -> f32 {
    let base_radius = width * 0.28;
    let progress = (distance / range.max(f32::EPSILON)).clamp(0.0, 1.0);
    base_radius * (1.0 + progress * 1.85)
}

fn positive_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

/// Description:
/// Applies area damage to all valid enemy players in radius.
///
/// Params:
/// - `players`: Server-side development player state cache.
/// - `caster_player_id`: Player id that owns the damage source.
/// - `center`: Area center position.
/// - `radius`: Damage radius.
/// - `amount`: Damage amount to apply.
fn apply_area_damage(
    players: &mut ConnectedPlayers,
    caster_player_id: u64,
    center: Vec3,
    radius: f32,
    amount: f32,
) {
    let caster_team = players
        .states
        .get(&caster_player_id)
        .map(|caster| caster.team);
    for (target_player_id, target_state) in &mut players.states {
        if *target_player_id == caster_player_id
            || Some(target_state.team) == caster_team
            || target_state.health <= 0.0
        {
            continue;
        }

        if horizontal_distance(target_state.position, center)
            <= radius + DEVELOPMENT_PLAYER_HIT_RADIUS
        {
            apply_damage(target_state, amount);
        }
    }
}

/// Description:
/// Applies damage to one server-side player state.
///
/// Params:
/// - `target`: Player state to damage.
/// - `amount`: Damage amount to apply.
fn apply_damage(target: &mut ConnectedPlayerState, amount: f32) {
    if target.health <= 0.0 {
        return;
    }

    target.health = (target.health - amount).max(0.0);
    if target.health <= 0.0 {
        target.moving = false;
        target.respawn_timer = Some(RESPAWN_SECONDS);
    }
}

/// Description:
/// Applies capped healing to one server-side player state.
fn apply_heal(target: &mut ConnectedPlayerState, amount: f32, max_health: f32) {
    if target.health <= 0.0 {
        return;
    }

    target.health = (target.health + amount).min(max_health.max(1.0));
}

/// Description:
/// Clamps a cast target to a maximum range from an origin.
///
/// Params:
/// - `origin`: Cast origin position.
/// - `target`: Requested target position.
/// - `range`: Maximum allowed cast range.
///
/// Return:
/// - Clamped target position.
fn clamp_cast_target(origin: Vec3, target: Vec3, range: f32) -> Vec3 {
    let delta = Vec3::new(target.x - origin.x, 0.0, target.z - origin.z);
    if delta.length_squared() <= range * range {
        return Vec3::new(target.x, origin.y, target.z);
    }

    origin + delta.normalize_or_zero() * range
}

/// Description:
/// Computes the horizontal distance from a point to a segment.
///
/// Params:
/// - `point`: World-space point to test.
/// - `segment_start`: Segment start position.
/// - `segment_end`: Segment end position.
///
/// Return:
/// - Shortest horizontal distance from the point to the segment.
fn distance_to_segment_xz(point: Vec3, segment_start: Vec3, segment_end: Vec3) -> f32 {
    let point = Vec2::new(point.x, point.z);
    let segment_start = Vec2::new(segment_start.x, segment_start.z);
    let segment_end = Vec2::new(segment_end.x, segment_end.z);
    let segment = segment_end - segment_start;
    let segment_length_squared = segment.length_squared();

    if segment_length_squared <= f32::EPSILON {
        return point.distance(segment_start);
    }

    let t = ((point - segment_start).dot(segment) / segment_length_squared).clamp(0.0, 1.0);
    point.distance(segment_start + segment * t)
}

/// Description:
/// Computes horizontal distance between two world-space positions.
///
/// Params:
/// - `left`: First world-space position.
/// - `right`: Second world-space position.
///
/// Return:
/// - XZ-plane distance between both positions.
fn horizontal_distance(left: Vec3, right: Vec3) -> f32 {
    Vec2::new(left.x, left.z).distance(Vec2::new(right.x, right.z))
}
