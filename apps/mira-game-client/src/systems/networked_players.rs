use super::{
    CurrentChampionVisual, LocalChampionAnimations, TrainingDummy, healthbar, hierarchy_root,
    ui_state::MiraHudState,
};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use lightyear::prelude::*;
use mira_game_api::game::{
    player::{
        DEFAULT_PLAYER_MANA, DEFAULT_PLAYER_MOVEMENT_SPEED, Health, Mana, MoveSpeed, Player,
        PlayerControlled, PlayerId, PlayerProfile,
    },
    team::{Team, TeamSpec},
};
use mira_game_api::network::{
    ChampionId, MatchSnapshot, NetworkPlayer, PlayerStateChannel, PlayerStateUpdate,
};
use std::time::Duration;

const REMOTE_PLAYER_HIT_RADIUS: f32 = 0.9;
const LIRA_MODEL_PATH: &str = "game/champions/lira/model.glb";
const IGNARA_MODEL_PATH: &str = "game/champions/ignara/model.glb";
const YUNA_MODEL_PATH: &str = "game/champions/yuna/model.glb";
const SOPHIA_MODEL_PATH: &str = "game/champions/sophia/model.glb";
const PLAYER_STATE_UPDATE_INTERVAL_SECONDS: f32 = 1.0 / 30.0;
const REMOTE_POSITION_SMOOTHING: f32 = 24.0;
const REMOTE_ROTATION_SMOOTHING: f32 = 18.0;
const LOCAL_AUTHORITATIVE_POSITION_SMOOTHING: f32 = 18.0;
const LOCAL_AUTHORITATIVE_SNAP_DISTANCE: f32 = 12.0;
/// Marks a remote player stand-in spawned from server match snapshots.
///
/// - `player_id`: Network player id represented by this stand-in.
/// - `champion`: Champion id whose model is currently attached.
/// - `health_bar`: Health bar entity following the stand-in.
/// - `model_root`: Child entity that owns the spawned champion scene.
/// - `target_position`: Latest server position target used by interpolation.
/// - `target_rotation`: Latest server rotation target used by interpolation.
/// - `moving`: Latest server movement state used for animation.
/// - `respawn_generation`: Latest server respawn generation applied to this stand-in.
#[derive(Component, Debug, Clone, Copy)]
pub(super) struct RemotePlayerStandIn {
    player_id: u64,
    champion: ChampionId,
    team: TeamSpec,
    is_enemy: bool,
    health_bar: Entity,
    model_root: Entity,
    target_position: Vec3,
    target_rotation: Quat,
    moving: bool,
    respawn_generation: u32,
}

/// Stores the latest server transform and locomotion state for the local player.
#[derive(Component, Debug, Clone, Copy)]
pub(super) struct LocalAuthoritativeTransform {
    target_position: Vec3,
    target_rotation: Quat,
    pub(super) moving: bool,
}
/// Tracks whether the local player was moved to its server-assigned spawn.
///
/// - `player_id`: Player id whose spawn position was already applied.
/// - `player_count`: Roster size whose spawn layout was already applied.
/// - `respawn_generation`: Last local respawn generation applied to the transform.
/// - `position_correction_generation`: Last server position correction applied to the transform.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub(super) struct AppliedLocalNetworkSpawn {
    player_id: Option<u64>,
    player_count: usize,
    respawn_generation: u32,
    position_correction_generation: u32,
}
/// Limits how often the client sends local player state updates.
///
/// - `0`: Repeating timer for local player state update messages.
#[derive(Resource, Debug)]
pub(super) struct PlayerStateUpdateTimer(Timer);
/// Stores the local player's requested development champion and team.
#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct LocalPlayerSelection {
    champion: ChampionId,
    team: TeamSpec,
}

impl Default for PlayerStateUpdateTimer {
    /// Returns the default configuration used by the networked player synchronization system.
    fn default() -> Self {
        Self(Timer::from_seconds(
            PLAYER_STATE_UPDATE_INTERVAL_SECONDS,
            TimerMode::Repeating,
        ))
    }
}

impl Default for LocalPlayerSelection {
    /// Returns the default configuration used by the networked player synchronization system.
    fn default() -> Self {
        Self::from_args(std::env::args().skip(1))
    }
}

impl LocalPlayerSelection {
    /// Parses `--champion`, `--char`, and `--team` process args into a local selection.
    fn from_args(args: impl IntoIterator<Item = String>) -> Self {
        let mut champion = ChampionId::LIRA;
        let mut team = TeamSpec::Light;
        let mut pending_key = None::<String>;

        for arg in args {
            if let Some(key) = pending_key.take() {
                apply_selection_arg(&key, &arg, &mut champion, &mut team);
                continue;
            }

            if let Some((key, value)) = arg.split_once('=') {
                apply_selection_arg(key, value, &mut champion, &mut team);
                continue;
            }

            match arg.as_str() {
                "--champion" | "--char" | "-c" | "--team" | "-t" => {
                    pending_key = Some(arg);
                }
                _ => {}
            }
        }

        Self { champion, team }
    }
}
fn apply_selection_arg(key: &str, value: &str, champion: &mut ChampionId, team: &mut TeamSpec) {
    let key = key.trim_start_matches('-');

    match key {
        "champion" | "char" | "c" => {
            if let Some(parsed) = parse_champion(value) {
                *champion = parsed;
            } else {
                warn!("Ignoring unknown --{} value `{}`", key, value);
            }
        }
        "team" | "t" => {
            if let Some(parsed) = parse_team(value) {
                *team = parsed;
            } else {
                warn!("Ignoring unknown --team value `{}`", value);
            }
        }
        _ => {}
    }
}

fn parse_champion(value: &str) -> Option<ChampionId> {
    ChampionId::from_selector(value)
}
fn parse_team(value: &str) -> Option<TeamSpec> {
    match value.trim().to_ascii_lowercase().as_str() {
        "0" | "neutral" | "none" => Some(TeamSpec::Neutral),
        "1" | "light" | "team1" | "team_1" => Some(TeamSpec::Light),
        "2" | "dark" | "team2" | "team_2" => Some(TeamSpec::Dark),
        _ => None,
    }
}
/// Sends the selected champion and team to the server.
///
/// - `timer`: Send timer used to reduce reliable position update traffic.
/// - `time`: Bevy time resource used to advance the send timer.
/// - `senders`: Lightyear message senders attached to the local client link.
pub(super) fn send_local_player_state_update(
    mut timer: ResMut<PlayerStateUpdateTimer>,
    time: Res<Time>,
    selection: Res<LocalPlayerSelection>,
    mut senders: Query<&mut MessageSender<PlayerStateUpdate>, With<Client>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    for mut sender in &mut senders {
        sender.send::<PlayerStateChannel>(PlayerStateUpdate {
            champion: selection.champion,
            team: selection.team,
        });
    }
}
/// Applies server match snapshots by positioning the local player and spawning remote stand-ins.
///
/// - `commands`: ECS command buffer used to spawn and despawn remote stand-ins.
/// - `asset_server`: Asset server used to load champion scenes.
/// - `receivers`: Lightyear message receivers that contain server match snapshots.
/// - `local_spawn`: Tracks one-time local spawn placement from the server snapshot.
/// - `local_players`: Locally controlled player entities updated from the snapshot.
/// - `remote_players`: Existing remote stand-ins updated from the snapshot.
/// - `meshes`: Mesh assets used by remote health bars.
/// - `materials`: Material assets used by remote health bars.
pub(super) fn sync_remote_players_from_match_snapshot(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut receivers: Query<&mut MessageReceiver<MatchSnapshot>, With<Client>>,
    mut local_spawn: ResMut<AppliedLocalNetworkSpawn>,
    mut local_players: Query<
        (
            Entity,
            &mut Player,
            &mut Team,
            &mut Health,
            &mut Transform,
            &mut CurrentChampionVisual,
            &mut PlayerProfile,
        ),
        (With<PlayerControlled>, Without<RemotePlayerStandIn>),
    >,
    mut remote_players: Query<(
        Entity,
        &mut RemotePlayerStandIn,
        Option<&mut TrainingDummy>,
        &mut Health,
        &mut Transform,
        &mut PlayerProfile,
    )>,
    mut hud_state: ResMut<MiraHudState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    health_bar_style: Res<healthbar::OverheadHealthBarStyle>,
    player_profiles: Res<healthbar::OverheadPlayerProfiles>,
) {
    let mut latest_snapshot = None;
    for mut receiver in &mut receivers {
        for snapshot in receiver.receive() {
            latest_snapshot = Some(snapshot);
        }
    }

    let Some(snapshot) = latest_snapshot else {
        return;
    };
    debug!(
        "Received match snapshot: local_player={} players={}",
        snapshot.local_player_id,
        snapshot
            .players
            .iter()
            .map(|player| format!(
                "{}:{}:{:?}:{}/{}",
                player.player_id,
                champion_display_name(player.champion),
                player.team,
                player.health,
                player.max_health
            ))
            .collect::<Vec<_>>()
            .join(",")
    );

    apply_local_player_snapshot(
        &mut commands,
        &asset_server,
        &snapshot,
        &mut local_spawn,
        &mut local_players,
        &mut hud_state,
        &player_profiles,
    );
    sync_remote_player_stand_ins(
        &mut commands,
        &asset_server,
        &snapshot,
        &mut remote_players,
        &mut meshes,
        &mut materials,
        health_bar_style.accent_color,
        &player_profiles,
    );
}
/// Smoothly moves remote player stand-ins toward the latest server snapshot position.
///
/// - `time`: Bevy time resource used for frame-rate independent interpolation.
/// - `remote_players`: Remote stand-in transforms and movement state.
pub(super) fn interpolate_remote_player_positions(
    time: Res<Time>,
    mut remote_players: Query<(&mut RemotePlayerStandIn, &mut Transform)>,
) {
    let position_smoothing = 1.0 - (-REMOTE_POSITION_SMOOTHING * time.delta_secs()).exp();
    let rotation_smoothing = 1.0 - (-REMOTE_ROTATION_SMOOTHING * time.delta_secs()).exp();

    for (stand_in, mut transform) in &mut remote_players {
        transform.translation = transform
            .translation
            .lerp(stand_in.target_position, position_smoothing);
        transform.rotation = transform
            .rotation
            .slerp(stand_in.target_rotation, rotation_smoothing);
    }
}
/// Switches remote player stand-ins between idle and walk animations from server movement state.
///
/// - `animations`: Optional champion animation data loaded during setup.
/// - `remote_players`: Remote stand-ins used to detect movement and hierarchy roots.
/// - `animation_players`: Animation players and transitions to update.
/// - `parents`: Hierarchy parent relationships used to map animation players to champion roots.
pub(super) fn sync_remote_player_animations(
    animations: Option<Res<LocalChampionAnimations>>,
    mut remote_players: Query<(Entity, &mut RemotePlayerStandIn, &Transform)>,
    mut animation_players: Query<(Entity, &mut AnimationPlayer, &mut AnimationTransitions)>,
    parents: Query<&ChildOf>,
) {
    let Some(animations) = animations else {
        return;
    };

    for (remote_entity, stand_in, _transform) in &mut remote_players {
        let next_animation = if stand_in.moving {
            animations.walk
        } else {
            animations.idle
        };

        for (animation_entity, mut player, mut transitions) in &mut animation_players {
            if hierarchy_root(animation_entity, &parents) != remote_entity {
                continue;
            }

            if animation_is_playing(&player, next_animation) {
                continue;
            }

            transitions
                .play(&mut player, next_animation, Duration::from_millis(140))
                .repeat();
        }
    }
}
/// Checks whether an animation player is already playing a given animation node.
///
/// - `player`: Animation player to inspect.
/// - `animation`: Animation graph node to look for.
///
/// - `true` when the animation node is currently active.
fn animation_is_playing(player: &AnimationPlayer, animation: AnimationNodeIndex) -> bool {
    player
        .playing_animations()
        .any(|(active_animation, _)| *active_animation == animation)
}
/// Applies a received server snapshot to an existing remote stand-in.
///
/// - `stand_in`: Remote stand-in state to update.
/// - `snapshot_player`: Snapshot entry for the represented player.
fn apply_remote_snapshot(stand_in: &mut RemotePlayerStandIn, snapshot_player: &NetworkPlayer) {
    stand_in.target_position = Vec3::from(snapshot_player.position);
    stand_in.target_rotation = Quat::from_rotation_y(snapshot_player.yaw);
    stand_in.moving = snapshot_player.moving;
}
/// Converts a network yaw into a world rotation.
///
/// - `yaw`: Facing angle around the Y axis.
///
/// - World rotation matching the facing angle.
fn rotation_from_yaw(yaw: f32) -> Quat {
    Quat::from_rotation_y(yaw)
}
/// Moves the local player once to the spawn position assigned by the server.
///
/// - `snapshot`: Latest match snapshot received from the server.
/// - `local_spawn`: Resource tracking whether local placement was already applied.
/// - `local_players`: Locally controlled player query.
/// - `hud_state`: HUD state updated with server-provided respawn time.
fn apply_local_player_snapshot(
    commands: &mut Commands,
    asset_server: &AssetServer,
    snapshot: &MatchSnapshot,
    local_spawn: &mut AppliedLocalNetworkSpawn,
    local_players: &mut Query<
        (
            Entity,
            &mut Player,
            &mut Team,
            &mut Health,
            &mut Transform,
            &mut CurrentChampionVisual,
            &mut PlayerProfile,
        ),
        (With<PlayerControlled>, Without<RemotePlayerStandIn>),
    >,
    hud_state: &mut MiraHudState,
    player_profiles: &healthbar::OverheadPlayerProfiles,
) {
    let Some(local_snapshot) = snapshot
        .players
        .iter()
        .find(|player| player.player_id == snapshot.local_player_id)
    else {
        return;
    };

    for (entity, mut player, mut team, mut health, mut transform, mut visual, mut profile) in
        local_players
    {
        player.id = PlayerId(snapshot.local_player_id);
        *team = Team(local_snapshot.team);
        health.current = local_snapshot.health.max(0.0) as u32;
        health.max = local_snapshot.max_health.max(1.0) as u32;
        if let Some(display_name) = player_profiles.display_name(snapshot.local_player_id) {
            profile.display_name = display_name.to_string();
        }
        hud_state.set_respawn_seconds(local_snapshot.respawn_seconds);
        if visual.champion != Some(local_snapshot.champion) {
            despawn_model_root(commands, visual.model_root.take());
            let model_root = spawn_champion_model_root(
                commands,
                asset_server,
                local_snapshot.champion,
                Name::new(format!(
                    "LocalPlayer{}Model",
                    champion_display_name(local_snapshot.champion)
                )),
            );
            commands.entity(entity).add_child(model_root);
            visual.champion = Some(local_snapshot.champion);
            visual.model_root = Some(model_root);
            info!(
                "Applied local server snapshot: player={} champion={} team={:?} health={}/{} model={}",
                snapshot.local_player_id,
                champion_display_name(local_snapshot.champion),
                local_snapshot.team,
                local_snapshot.health,
                local_snapshot.max_health,
                champion_model_path(local_snapshot.champion)
            );
        }

        let authoritative_position = Vec3::from(local_snapshot.position);
        let authoritative_rotation = rotation_from_yaw(local_snapshot.yaw);
        commands.entity(entity).insert(LocalAuthoritativeTransform {
            target_position: authoritative_position,
            target_rotation: authoritative_rotation,
            moving: local_snapshot.moving,
        });

        if should_apply_local_server_position(
            *local_spawn,
            snapshot.local_player_id,
            snapshot.players.len(),
            local_snapshot.respawn_generation,
            local_snapshot.position_correction_generation,
        ) {
            transform.translation = authoritative_position;
            transform.rotation = authoritative_rotation;
            local_spawn.player_id = Some(snapshot.local_player_id);
            local_spawn.player_count = snapshot.players.len();
            local_spawn.respawn_generation = local_snapshot.respawn_generation;
            local_spawn.position_correction_generation =
                local_snapshot.position_correction_generation;
        }

        if local_snapshot.control_locked {
            transform.translation = authoritative_position;
            transform.rotation = authoritative_rotation;
        }
    }
}

/// Smoothly presents normal server-authoritative movement snapshots for the local player.
pub(super) fn reconcile_local_player_to_authoritative_snapshot(
    time: Res<Time>,
    mut local_players: Query<
        (&mut Transform, &LocalAuthoritativeTransform),
        (With<PlayerControlled>, Without<RemotePlayerStandIn>),
    >,
) {
    let blend = reconciliation_blend(time.delta_secs());
    for (mut transform, authoritative) in &mut local_players {
        if transform
            .translation
            .distance_squared(authoritative.target_position)
            > LOCAL_AUTHORITATIVE_SNAP_DISTANCE * LOCAL_AUTHORITATIVE_SNAP_DISTANCE
        {
            transform.translation = authoritative.target_position;
            transform.rotation = authoritative.target_rotation;
            continue;
        }

        transform.translation = transform
            .translation
            .lerp(authoritative.target_position, blend);
        transform.rotation = transform
            .rotation
            .slerp(authoritative.target_rotation, blend);
    }
}

/// Returns the frame-rate independent interpolation amount for local reconciliation.
fn reconciliation_blend(delta_seconds: f32) -> f32 {
    1.0 - (-LOCAL_AUTHORITATIVE_POSITION_SMOOTHING * delta_seconds.max(0.0)).exp()
}

/// Returns whether a local player transform must be reconciled with a server snapshot.
fn should_apply_local_server_position(
    applied: AppliedLocalNetworkSpawn,
    player_id: u64,
    player_count: usize,
    respawn_generation: u32,
    position_correction_generation: u32,
) -> bool {
    applied.player_id != Some(player_id)
        || applied.player_count != player_count
        || applied.respawn_generation != respawn_generation
        || applied.position_correction_generation != position_correction_generation
}
/// Updates, spawns, and removes remote player stand-ins from a server snapshot.
///
/// - `commands`: ECS command buffer used to spawn and despawn entities.
/// - `asset_server`: Asset server used to load champion scenes.
/// - `snapshot`: Latest match snapshot received from the server.
/// - `remote_players`: Existing remote stand-ins.
/// - `meshes`: Mesh assets used by remote health bars.
/// - `materials`: Material assets used by remote health bars.
fn sync_remote_player_stand_ins(
    commands: &mut Commands,
    asset_server: &AssetServer,
    snapshot: &MatchSnapshot,
    remote_players: &mut Query<(
        Entity,
        &mut RemotePlayerStandIn,
        Option<&mut TrainingDummy>,
        &mut Health,
        &mut Transform,
        &mut PlayerProfile,
    )>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    accent_color: Color,
    player_profiles: &healthbar::OverheadPlayerProfiles,
) {
    let local_team = snapshot
        .players
        .iter()
        .find(|player| player.player_id == snapshot.local_player_id)
        .map(|player| player.team)
        .unwrap_or(TeamSpec::Light);
    let remote_snapshot_players = snapshot
        .players
        .iter()
        .filter(|player| player.player_id != snapshot.local_player_id)
        .collect::<Vec<_>>();
    let mut existing_player_ids = Vec::with_capacity(remote_snapshot_players.len());

    for (entity, mut stand_in, maybe_dummy, mut health, mut transform, mut profile) in
        remote_players
    {
        let Some(snapshot_player) = remote_snapshot_players
            .iter()
            .find(|player| player.player_id == stand_in.player_id)
        else {
            commands.entity(stand_in.health_bar).despawn();
            despawn_model_root(commands, Some(stand_in.model_root));
            commands.entity(entity).despawn_children();
            commands.entity(entity).despawn();
            continue;
        };

        let is_enemy = snapshot_player.team != local_team;
        if stand_in.champion != snapshot_player.champion
            || stand_in.team != snapshot_player.team
            || stand_in.is_enemy != is_enemy
        {
            commands.entity(stand_in.health_bar).despawn();
            despawn_model_root(commands, Some(stand_in.model_root));
            commands.entity(entity).despawn_children();
            commands.entity(entity).despawn();
            continue;
        }

        let did_respawn = stand_in.respawn_generation != snapshot_player.respawn_generation;
        apply_remote_snapshot(&mut stand_in, snapshot_player);
        stand_in.respawn_generation = snapshot_player.respawn_generation;
        if did_respawn {
            transform.translation = Vec3::from(snapshot_player.position);
            transform.rotation = rotation_from_yaw(snapshot_player.yaw);
        }
        if let Some(mut dummy) = maybe_dummy {
            dummy.set_server_health(snapshot_player.health, snapshot_player.max_health);
        }
        health.current = snapshot_player.health.max(0.0) as u32;
        health.max = snapshot_player.max_health.max(1.0) as u32;
        profile.display_name = player_display_name(snapshot_player, player_profiles);
        existing_player_ids.push(stand_in.player_id);
    }

    for snapshot_player in remote_snapshot_players {
        if existing_player_ids.contains(&snapshot_player.player_id) {
            continue;
        }

        spawn_remote_player_stand_in(
            commands,
            asset_server,
            meshes,
            materials,
            snapshot_player,
            local_team,
            accent_color,
            player_profiles,
        );
    }
}
/// Spawns one remote player stand-in that can be targeted by current abilities.
///
/// - `commands`: ECS command buffer used to spawn entities.
/// - `asset_server`: Asset server used to load the champion scene.
/// - `meshes`: Mesh assets used by the health bar.
/// - `materials`: Material assets used by the health bar.
/// - `snapshot_player`: Network player data to render locally.
fn spawn_remote_player_stand_in(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    snapshot_player: &NetworkPlayer,
    local_team: TeamSpec,
    accent_color: Color,
    player_profiles: &healthbar::OverheadPlayerProfiles,
) {
    let is_enemy = snapshot_player.team != local_team;
    let mut player = commands.spawn((
        Name::new(format!(
            "RemotePlayer{}{}",
            champion_display_name(snapshot_player.champion),
            snapshot_player.player_id
        )),
        RemotePlayerStandIn {
            player_id: snapshot_player.player_id,
            champion: snapshot_player.champion,
            team: snapshot_player.team,
            is_enemy,
            health_bar: Entity::PLACEHOLDER,
            model_root: Entity::PLACEHOLDER,
            target_position: Vec3::from(snapshot_player.position),
            target_rotation: rotation_from_yaw(snapshot_player.yaw),
            moving: snapshot_player.moving,
            respawn_generation: snapshot_player.respawn_generation,
        },
        Player {
            id: PlayerId(snapshot_player.player_id),
        },
        PlayerProfile {
            display_name: player_display_name(snapshot_player, player_profiles),
        },
        Team(snapshot_player.team),
        Health {
            current: snapshot_player.health as u32,
            max: snapshot_player.max_health as u32,
        },
        Mana::new(DEFAULT_PLAYER_MANA),
        MoveSpeed(DEFAULT_PLAYER_MOVEMENT_SPEED),
        Transform::from_translation(Vec3::from(snapshot_player.position))
            .with_rotation(rotation_from_yaw(snapshot_player.yaw)),
    ));
    if is_enemy {
        player.insert(TrainingDummy::remote_player(
            snapshot_player.health,
            snapshot_player.max_health,
            REMOTE_PLAYER_HIT_RADIUS,
        ));
    }
    let player_entity = player.id();
    let model_root = spawn_champion_model_root(
        commands,
        asset_server,
        snapshot_player.champion,
        Name::new(format!(
            "RemotePlayer{}{}Model",
            champion_display_name(snapshot_player.champion),
            snapshot_player.player_id
        )),
    );
    info!(
        "Spawned remote server snapshot player={} champion={} team={:?} health={}/{} model={}",
        snapshot_player.player_id,
        champion_display_name(snapshot_player.champion),
        snapshot_player.team,
        snapshot_player.health,
        snapshot_player.max_health,
        champion_model_path(snapshot_player.champion)
    );
    commands.entity(player_entity).add_child(model_root);
    let health_bar = if is_enemy {
        healthbar::spawn_remote_enemy_player_health_bar(
            commands,
            asset_server,
            meshes,
            materials,
            player_entity,
            snapshot_player.max_health,
            accent_color,
        )
    } else {
        healthbar::spawn_remote_ally_player_health_bar(
            commands,
            asset_server,
            meshes,
            materials,
            player_entity,
            snapshot_player.max_health,
            accent_color,
        )
    };
    commands.entity(player_entity).insert(RemotePlayerStandIn {
        player_id: snapshot_player.player_id,
        champion: snapshot_player.champion,
        team: snapshot_player.team,
        is_enemy,
        health_bar,
        model_root,
        target_position: Vec3::from(snapshot_player.position),
        target_rotation: rotation_from_yaw(snapshot_player.yaw),
        moving: snapshot_player.moving,
        respawn_generation: snapshot_player.respawn_generation,
    });
}
/// Spawns a child entity that owns one champion scene.
fn spawn_champion_model_root(
    commands: &mut Commands,
    asset_server: &AssetServer,
    champion: ChampionId,
    name: Name,
) -> Entity {
    let champion_scene =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(champion_model_path(champion)));
    commands
        .spawn((name, WorldAssetRoot(champion_scene), Transform::default()))
        .id()
}
/// Removes a previously spawned champion model root and all scene children below it.
fn despawn_model_root(commands: &mut Commands, model_root: Option<Entity>) {
    let Some(model_root) = model_root else {
        return;
    };

    commands.entity(model_root).despawn_children();
    commands.entity(model_root).despawn();
}

/// Resolves the model path for a champion in the prototype roster.
fn champion_model_path(champion: ChampionId) -> &'static str {
    match champion {
        ChampionId::LIRA => LIRA_MODEL_PATH,
        ChampionId::IGNARA => IGNARA_MODEL_PATH,
        ChampionId::YUNA => YUNA_MODEL_PATH,
        ChampionId::SOPHIA => SOPHIA_MODEL_PATH,
        _ => LIRA_MODEL_PATH,
    }
}

/// Resolves a display name for a prototype champion.
fn champion_display_name(champion: ChampionId) -> &'static str {
    champion.display_name().unwrap_or("Lira")
}
fn player_display_name(
    player: &NetworkPlayer,
    player_profiles: &healthbar::OverheadPlayerProfiles,
) -> String {
    player_profiles
        .display_name(player.player_id)
        .map(str::to_string)
        .unwrap_or_else(|| "Player".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_position_correction_reconciles_the_local_player_without_a_respawn() {
        let applied = AppliedLocalNetworkSpawn {
            player_id: Some(7),
            player_count: 2,
            respawn_generation: 3,
            position_correction_generation: 4,
        };

        assert!(should_apply_local_server_position(applied, 7, 2, 3, 5,));
    }

    #[test]
    fn unchanged_server_position_does_not_override_local_prediction() {
        let applied = AppliedLocalNetworkSpawn {
            player_id: Some(7),
            player_count: 2,
            respawn_generation: 3,
            position_correction_generation: 4,
        };

        assert!(!should_apply_local_server_position(applied, 7, 2, 3, 4,));
    }
}
