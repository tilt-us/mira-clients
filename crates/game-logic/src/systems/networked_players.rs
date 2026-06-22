use super::{
    CurrentChampionVisual, ExternalMovementModifier, LocalChampionAnimations, TrainingDummy,
    healthbar, ui_state::MiraHudState,
};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use game_shared::game::{
    player::{
        Health, Mana, MoveSpeed, MoveTarget, Player, PlayerControlled, PlayerId, PlayerProfile,
    },
    team::{Team, TeamSpec},
};
use game_shared::network::{
    ChampionId, MatchSnapshot, NetworkPlayer, PlayerStateChannel, PlayerStateUpdate, WorldPosition,
};
use lightyear::prelude::*;
use std::time::Duration;

const LIRA_CHAMPION_ID: ChampionId = ChampionId(6606);
const IGNARA_CHAMPION_ID: ChampionId = ChampionId(6607);
const YUNA_CHAMPION_ID: ChampionId = ChampionId(6608);
const SOPHIA_CHAMPION_ID: ChampionId = ChampionId(6609);

const REMOTE_PLAYER_HIT_RADIUS: f32 = 0.9;
const LIRA_MODEL_PATH: &str = "game/champions/lira/model.glb";
const IGNARA_MODEL_PATH: &str = "game/champions/ignara/model.glb";
const YUNA_MODEL_PATH: &str = "game/champions/yuna/model.glb";
const SOPHIA_MODEL_PATH: &str = "game/champions/sophia/model.glb";
const PLAYER_STATE_UPDATE_INTERVAL_SECONDS: f32 = 1.0 / 30.0;
const REMOTE_POSITION_SMOOTHING: f32 = 24.0;
const REMOTE_ROTATION_SMOOTHING: f32 = 18.0;

#[derive(Component, Debug, Clone, Copy)]
/// Description:
/// Marks a remote player stand-in spawned from server match snapshots.
///
/// Fields:
/// - `player_id`: Network player id represented by this stand-in.
/// - `champion`: Champion id whose model is currently attached.
/// - `health_bar`: Health bar entity following the stand-in.
/// - `model_root`: Child entity that owns the spawned champion scene.
/// - `target_position`: Latest server position target used by interpolation.
/// - `target_rotation`: Latest server rotation target used by interpolation.
/// - `moving`: Latest server movement state used for animation.
/// - `respawn_generation`: Latest server respawn generation applied to this stand-in.
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

#[derive(Resource, Debug, Default, Clone, Copy)]
/// Description:
/// Tracks whether the local player was moved to its server-assigned spawn.
///
/// Fields:
/// - `player_id`: Player id whose spawn position was already applied.
/// - `player_count`: Roster size whose spawn layout was already applied.
/// - `respawn_generation`: Last local respawn generation applied to the transform.
pub(super) struct AppliedLocalNetworkSpawn {
    player_id: Option<u64>,
    player_count: usize,
    respawn_generation: u32,
}

#[derive(Resource, Debug)]
/// Description:
/// Limits how often the client sends local player state updates.
///
/// Fields:
/// - `0`: Repeating timer for local player state update messages.
pub(super) struct PlayerStateUpdateTimer(Timer);

#[derive(Resource, Debug, Clone, Copy)]
/// Description:
/// Stores the local player's requested development champion and team.
pub(super) struct LocalPlayerSelection {
    champion: ChampionId,
    team: TeamSpec,
}

impl Default for PlayerStateUpdateTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            PLAYER_STATE_UPDATE_INTERVAL_SECONDS,
            TimerMode::Repeating,
        ))
    }
}

impl Default for LocalPlayerSelection {
    fn default() -> Self {
        Self::from_args(std::env::args().skip(1))
    }
}

impl LocalPlayerSelection {
    /// Description:
    /// Parses `--champion`, `--char`, and `--team` process args into a local selection.
    fn from_args(args: impl IntoIterator<Item = String>) -> Self {
        let mut champion = LIRA_CHAMPION_ID;
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
    match value.trim().to_ascii_lowercase().as_str() {
        "6606" | "lira" => Some(LIRA_CHAMPION_ID),
        "6607" | "ignara" => Some(IGNARA_CHAMPION_ID),
        "6608" | "yuna" => Some(YUNA_CHAMPION_ID),
        "6609" | "sophia" => Some(SOPHIA_CHAMPION_ID),
        _ => None,
    }
}

fn parse_team(value: &str) -> Option<TeamSpec> {
    match value.trim().to_ascii_lowercase().as_str() {
        "0" | "neutral" | "none" => Some(TeamSpec::Neutral),
        "1" | "light" | "team1" | "team_1" => Some(TeamSpec::Light),
        "2" | "dark" | "team2" | "team_2" => Some(TeamSpec::Dark),
        _ => None,
    }
}

/// Description:
/// Sends the controlled player's current position to the server.
///
/// Params:
/// - `timer`: Send timer used to reduce reliable position update traffic.
/// - `time`: Bevy time resource used to advance the send timer.
/// - `player_query`: Locally controlled player transform, health, and movement state.
/// - `senders`: Lightyear message senders attached to the local client link.
pub(super) fn send_local_player_state_update(
    mut timer: ResMut<PlayerStateUpdateTimer>,
    time: Res<Time>,
    selection: Res<LocalPlayerSelection>,
    player_query: Query<(&Transform, &Health, Has<MoveTarget>), With<PlayerControlled>>,
    mut senders: Query<&mut MessageSender<PlayerStateUpdate>, With<Client>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let Ok((player_transform, health, moving)) = player_query.single() else {
        return;
    };

    for mut sender in &mut senders {
        sender.send::<PlayerStateChannel>(PlayerStateUpdate {
            position: WorldPosition::from(player_transform.translation),
            yaw: yaw_from_rotation(player_transform.rotation),
            moving: moving && health.current > 0,
            champion: selection.champion,
            team: selection.team,
        });
    }
}

/// Description:
/// Applies server match snapshots by positioning the local player and spawning remote stand-ins.
///
/// Params:
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
        ),
        (With<PlayerControlled>, Without<RemotePlayerStandIn>),
    >,
    mut remote_players: Query<(
        Entity,
        &mut RemotePlayerStandIn,
        Option<&mut TrainingDummy>,
        &mut Health,
        &mut Transform,
    )>,
    mut hud_state: ResMut<MiraHudState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
    );
    sync_remote_player_stand_ins(
        &mut commands,
        &asset_server,
        &snapshot,
        &mut remote_players,
        &mut meshes,
        &mut materials,
    );
}

/// Description:
/// Smoothly moves remote player stand-ins toward the latest server snapshot position.
///
/// Params:
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

/// Description:
/// Switches remote player stand-ins between idle and walk animations from server movement state.
///
/// Params:
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

/// Description:
/// Checks whether an animation player is already playing a given animation node.
///
/// Params:
/// - `player`: Animation player to inspect.
/// - `animation`: Animation graph node to look for.
///
/// Return:
/// - `true` when the animation node is currently active.
fn animation_is_playing(player: &AnimationPlayer, animation: AnimationNodeIndex) -> bool {
    player
        .playing_animations()
        .any(|(active_animation, _)| *active_animation == animation)
}

/// Description:
/// Applies a received server snapshot to an existing remote stand-in.
///
/// Params:
/// - `stand_in`: Remote stand-in state to update.
/// - `snapshot_player`: Snapshot entry for the represented player.
fn apply_remote_snapshot(stand_in: &mut RemotePlayerStandIn, snapshot_player: &NetworkPlayer) {
    stand_in.target_position = Vec3::from(snapshot_player.position);
    stand_in.target_rotation = Quat::from_rotation_y(snapshot_player.yaw);
    stand_in.moving = snapshot_player.moving;
}

/// Description:
/// Converts an entity rotation into the Y-axis yaw used by network state messages.
///
/// Params:
/// - `rotation`: World rotation to convert.
///
/// Return:
/// - Facing angle around the Y axis.
fn yaw_from_rotation(rotation: Quat) -> f32 {
    let forward = rotation * Vec3::Z;
    forward.x.atan2(forward.z)
}

/// Description:
/// Converts a network yaw into a world rotation.
///
/// Params:
/// - `yaw`: Facing angle around the Y axis.
///
/// Return:
/// - World rotation matching the facing angle.
fn rotation_from_yaw(yaw: f32) -> Quat {
    Quat::from_rotation_y(yaw)
}

/// Description:
/// Moves the local player once to the spawn position assigned by the server.
///
/// Params:
/// - `commands`: ECS command buffer used to clear movement while dead.
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
        ),
        (With<PlayerControlled>, Without<RemotePlayerStandIn>),
    >,
    hud_state: &mut MiraHudState,
) {
    let Some(local_snapshot) = snapshot
        .players
        .iter()
        .find(|player| player.player_id == snapshot.local_player_id)
    else {
        return;
    };

    for (entity, mut player, mut team, mut health, mut transform, mut visual) in local_players {
        player.id = PlayerId(snapshot.local_player_id);
        *team = Team(local_snapshot.team);
        health.current = local_snapshot.health.max(0.0) as u32;
        health.max = local_snapshot.max_health.max(1.0) as u32;
        hud_state.set_respawn_seconds(local_snapshot.respawn_seconds);
        commands.entity(entity).insert(ExternalMovementModifier {
            speed_multiplier: local_snapshot.move_speed_multiplier.clamp(0.0, 2.0),
            pull_center: local_snapshot.pull_center.map(Vec3::from),
            pull_speed: 2.4,
            stunned: local_snapshot.stunned,
        });

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

        if local_spawn.player_id != Some(snapshot.local_player_id)
            || local_spawn.player_count != snapshot.players.len()
            || local_spawn.respawn_generation != local_snapshot.respawn_generation
        {
            transform.translation = Vec3::from(local_snapshot.position);
            transform.rotation = rotation_from_yaw(local_snapshot.yaw);
            local_spawn.player_id = Some(snapshot.local_player_id);
            local_spawn.player_count = snapshot.players.len();
            local_spawn.respawn_generation = local_snapshot.respawn_generation;
        }

        if !local_snapshot.alive || local_snapshot.stunned {
            commands.entity(entity).remove::<MoveTarget>();
        }

        if local_snapshot.control_locked {
            transform.translation = Vec3::from(local_snapshot.position);
            transform.rotation = rotation_from_yaw(local_snapshot.yaw);
            commands.entity(entity).remove::<MoveTarget>();
        }
    }
}

/// Description:
/// Updates, spawns, and removes remote player stand-ins from a server snapshot.
///
/// Params:
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
    )>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
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

    for (entity, mut stand_in, maybe_dummy, mut health, mut transform) in remote_players {
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
            dummy.health = snapshot_player.health;
        }
        health.current = snapshot_player.health.max(0.0) as u32;
        health.max = snapshot_player.max_health.max(1.0) as u32;
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
        );
    }
}

/// Description:
/// Spawns one remote player stand-in that can be targeted by current abilities.
///
/// Params:
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
            display_name: format!("Player {}", snapshot_player.player_id),
        },
        Team(snapshot_player.team),
        Health {
            current: snapshot_player.health as u32,
            max: snapshot_player.max_health as u32,
        },
        Mana::new(100),
        MoveSpeed(6.0),
        Transform::from_translation(Vec3::from(snapshot_player.position))
            .with_rotation(rotation_from_yaw(snapshot_player.yaw)),
    ));
    if is_enemy {
        player.insert(TrainingDummy {
            health: snapshot_player.health,
            hit_radius: REMOTE_PLAYER_HIT_RADIUS,
        });
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
            meshes,
            materials,
            player_entity,
            snapshot_player.max_health,
        )
    } else {
        healthbar::spawn_remote_ally_player_health_bar(
            commands,
            meshes,
            materials,
            player_entity,
            snapshot_player.max_health,
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

/// Description:
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
        .spawn((name, SceneRoot(champion_scene), Transform::default()))
        .id()
}

/// Description:
/// Removes a previously spawned champion model root and all scene children below it.
fn despawn_model_root(commands: &mut Commands, model_root: Option<Entity>) {
    let Some(model_root) = model_root else {
        return;
    };

    commands.entity(model_root).despawn_children();
    commands.entity(model_root).despawn();
}

/// Description:
/// Resolves the champion model path used by the current prototype roster.
///
/// Params:
/// - `champion`: Champion id received from the server.
///
/// Return:
/// - GLB model path inside the asset root.
fn champion_model_path(champion: ChampionId) -> &'static str {
    //TODO: Remove later when mira-client is ready for lobbies
    match champion.0 {
        6606 => LIRA_MODEL_PATH,
        6607 => IGNARA_MODEL_PATH,
        6608 => YUNA_MODEL_PATH,
        6609 => SOPHIA_MODEL_PATH,
        _ => LIRA_MODEL_PATH,
    }
}

/// Description:
/// Resolves a short display name for development champion entities.
///
/// Params:
/// - `champion`: Champion id received from the server.
///
/// Return:
/// - Human-readable champion name.
fn champion_display_name(champion: ChampionId) -> &'static str {
    //TODO: Remove later when mira-client is ready for lobbies
    match champion.0 {
        6606 => "Lira",
        6607 => "Ignara",
        6608 => "Yuna",
        6609 => "Sophia",
        _ => "Lira",
    }
}

/// Description:
/// Finds the top-most hierarchy root for a scene child entity.
///
/// Params:
/// - `entity`: Entity to walk upward from.
/// - `parents`: Parent relationship query.
///
/// Return:
/// - Top-most hierarchy entity.
fn hierarchy_root(mut entity: Entity, parents: &Query<&ChildOf>) -> Entity {
    while let Ok(parent) = parents.get(entity) {
        entity = parent.0;
    }
    entity
}
