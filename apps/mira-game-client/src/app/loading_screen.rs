//! Provides the client loading screen, including readiness tracking, player data, and UI synchronization.

use super::settings::{ClientAppSettings, ClientLaunchSettings};
use bevy::asset::RenderAssetUsages;
use bevy::ecs::spawn::SpawnIter;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use game_logic::OverheadPlayerProfiles;
use game_shared::game::player::{non_empty_string, public_display_name};
use game_shared::game::team::TeamSpec;
use game_shared::network::{
    ChampionId, DisplayReady, LauncherMatchManifest, LoadingScreenPlayer, LoadingScreenStatus,
    ReliableCommandChannel,
};
use lightyear::prelude::*;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use crate::network::{NetworkPingState, ping_color, ping_text};

const MINIMUM_CLIENT_LOADING_DURATION: Duration = Duration::from_secs(5);
const INITIAL_CONNECTION_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAXIMUM_INITIAL_CONNECTION_RETRIES: u8 = 2;
const LOADING_TEAM_SIZE: usize = 5;
const PROGRESS_BADGE_WIDTH: u32 = 112;
const PROGRESS_BADGE_HEIGHT: u32 = 30;
const MATCH_MANIFEST_ENV: &str = "MIRA_MATCH_MANIFEST_JSON";

/// Tracks the current loading-screen progress, readiness, and player data.
#[derive(Resource, Debug)]
pub struct LoadingScreenState {
    active: bool,
    complete: bool,
    connection_error: Option<String>,
    wallpaper_assets_ready: bool,
    status_text: String,
    client_progress_percent: f32,
    client_ready: bool,
    ready_sent: bool,
    ready_players: usize,
    total_players: usize,
    dark_players: Vec<LoadingPlayer>,
    light_players: Vec<LoadingPlayer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadingPlayer {
    public_id: u64,
    name: String,
    avatar_url: Option<String>,
    champion: ChampionId,
    champion_name: String,
    ready: bool,
}

/// Stores player profile metadata parsed from the client match manifest.
#[derive(Resource, Debug, Clone)]
struct ClientLoadingMatchManifest {
    players: HashMap<u64, ClientLoadingMatchPlayer>,
}

#[derive(Debug, Clone)]
struct ClientLoadingMatchPlayer {
    team: TeamSpec,
    champion: ChampionId,
    display_name: Option<String>,
    avatar_url: Option<String>,
}

/// Tracks the minimum local loading duration before the client reports readiness.
#[derive(Resource, Debug)]
struct LoadingScreenReadyGate {
    minimum_timer: Timer,
}

/// Tracks whether the loading-screen client has completed its first game-server connection.
#[derive(Resource, Debug, Default)]
struct LoadingScreenConnectionState {
    has_connected: bool,
    initial_retry_count: u8,
    retry_timer: Option<Timer>,
}

impl LoadingScreenConnectionState {
    fn mark_connected(&mut self) {
        self.has_connected = true;
        self.initial_retry_count = 0;
        self.retry_timer = None;
    }

    fn schedule_initial_retry(&mut self) -> bool {
        if self.has_connected
            || self.retry_timer.is_some()
            || self.initial_retry_count >= MAXIMUM_INITIAL_CONNECTION_RETRIES
        {
            return false;
        }

        self.initial_retry_count += 1;
        self.retry_timer = Some(Timer::new(INITIAL_CONNECTION_RETRY_DELAY, TimerMode::Once));
        true
    }

    fn retry_is_ready(&mut self, elapsed: Duration) -> bool {
        let Some(retry_timer) = self.retry_timer.as_mut() else {
            return false;
        };

        retry_timer.tick(elapsed);
        retry_timer.is_finished()
    }

    fn clear_pending_retry(&mut self) {
        self.retry_timer = None;
    }
}

/// Tracks wallpaper assets while they are being preloaded.
#[derive(Resource, Debug, Default)]
struct LoadingScreenWallpaperPreload {
    handles: Vec<Handle<Image>>,
    expected_count: usize,
}

#[derive(Resource)]
struct LoadingScreenImages {
    wallpapers: HashMap<ChampionId, Handle<Image>>,
}

impl LoadingScreenImages {
    fn load(asset_server: &AssetServer) -> Self {
        let wallpapers = ChampionId::PROTOTYPE_ROSTER
            .into_iter()
            .map(|champion| {
                let asset_slug = champion
                    .asset_slug()
                    .expect("prototype champions must have asset slugs");
                let path = format!("wallpapers/{asset_slug}-loading.jpg");
                (champion, asset_server.load(path))
            })
            .collect();

        Self { wallpapers }
    }

    fn wallpaper(&self, champion: ChampionId) -> &Handle<Image> {
        self.wallpapers
            .get(&champion)
            .or_else(|| self.wallpapers.get(&ChampionId::LIRA))
            .expect("prototype roster must include Lira")
    }

    fn wallpaper_handles(&self) -> impl Iterator<Item = &Handle<Image>> {
        self.wallpapers.values()
    }
}

#[derive(Resource)]
struct LoadingProgressBadgeImage {
    handle: Handle<Image>,
}

/// Caches the loading state and image handles for player avatars.
#[derive(Resource, Default)]
struct LoadingAvatarCache {
    entries: HashMap<String, LoadingAvatarEntry>,
}

/// Represents a pending, ready, or failed avatar cache entry.
enum LoadingAvatarEntry {
    Loading(Mutex<Receiver<Result<DownloadedAvatar, String>>>),
    Ready(Handle<Image>),
    Failed,
}

struct DownloadedAvatar {
    bytes: Vec<u8>,
    content_type: Option<String>,
}

impl FromWorld for ClientLoadingMatchManifest {
    fn from_world(_world: &mut World) -> Self {
        let Ok(raw_manifest) = std::env::var(MATCH_MANIFEST_ENV) else {
            return Self::empty();
        };

        Self::from_json(&raw_manifest).unwrap_or_else(|error| {
            warn!(
                "Failed to parse client loading match manifest from {}: {}",
                MATCH_MANIFEST_ENV, error
            );
            Self::empty()
        })
    }
}

impl ClientLoadingMatchManifest {
    fn empty() -> Self {
        Self {
            players: HashMap::new(),
        }
    }

    fn from_json(raw_manifest: &str) -> Result<Self, serde_json::Error> {
        let manifest = serde_json::from_str::<LauncherMatchManifest>(raw_manifest)?;
        let players = manifest
            .players
            .into_iter()
            .map(|player| {
                (
                    player.player_public_id,
                    ClientLoadingMatchPlayer {
                        team: player.team,
                        champion: player.champion_id,
                        display_name: player.display_name.as_deref().and_then(public_display_name),
                        avatar_url: player.avatar_url.as_deref().and_then(non_empty_string),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        Ok(Self { players })
    }
}

/// Registers the resources and systems that manage the client loading screen.
pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadingScreenReadyGate>()
            .init_resource::<LoadingScreenConnectionState>()
            .init_resource::<LoadingScreenWallpaperPreload>()
            .init_resource::<LoadingAvatarCache>()
            .init_resource::<ClientLoadingMatchManifest>()
            .add_systems(
                Startup,
                (
                    seed_loading_screen_players_from_manifest,
                    seed_overhead_profiles_from_manifest,
                    spawn_loading_screen_ui,
                )
                    .chain(),
            )
            .add_systems(PreStartup, preload_loading_screen_wallpapers)
            .add_systems(
                Update,
                (
                    record_loading_screen_connection,
                    record_loading_screen_disconnect,
                    retry_initial_game_server_connection,
                    update_loading_screen_wallpaper_status,
                    update_loading_screen_ready_gate,
                    send_display_ready,
                    receive_loading_screen_status,
                    sync_loading_screen_ui,
                )
                    .chain(),
            );
    }
}

/// Seeds loading-screen player cards from the launcher-provided match manifest.
fn seed_loading_screen_players_from_manifest(
    mut state: ResMut<LoadingScreenState>,
    manifest: Res<ClientLoadingMatchManifest>,
) {
    if !state.active || manifest.players.is_empty() {
        return;
    }

    let (light_players, dark_players) = loading_players_from_manifest(&manifest);
    let total_players = light_players.len() + dark_players.len();
    if total_players == 0 {
        return;
    }

    state.ready_players = 0;
    state.total_players = total_players;
    state.light_players = light_players;
    state.dark_players = dark_players;
}

/// Seeds in-world player profile names from match-manifest metadata.
fn seed_overhead_profiles_from_manifest(
    manifest: Res<ClientLoadingMatchManifest>,
    mut overhead_profiles: ResMut<OverheadPlayerProfiles>,
) {
    for (player_id, player) in &manifest.players {
        if let Some(display_name) = player.display_name.as_deref() {
            overhead_profiles.set_display_name(*player_id, display_name.to_string());
        }
    }
}

fn preload_loading_screen_wallpapers(
    asset_server: Res<AssetServer>,
    mut preload: ResMut<LoadingScreenWallpaperPreload>,
    mut state: ResMut<LoadingScreenState>,
    mut commands: Commands,
) {
    let images = LoadingScreenImages::load(&asset_server);
    preload.handles = images.wallpaper_handles().cloned().collect();
    preload.expected_count = preload.handles.len();
    commands.insert_resource(images);

    state.wallpaper_assets_ready = !state.active;
}

fn update_loading_screen_wallpaper_status(
    mut state: ResMut<LoadingScreenState>,
    preload: Res<LoadingScreenWallpaperPreload>,
    images: Res<Assets<Image>>,
) {
    if preload.expected_count == 0 {
        return;
    }

    let wallpapers_ready = wallpaper_handles_ready(&preload, &images);
    if wallpapers_ready == state.wallpaper_assets_ready {
        return;
    }

    state.wallpaper_assets_ready = wallpapers_ready;
    if wallpapers_ready && state.status_text == "Loading match art" {
        state.status_text = "Loading local arena".to_string();
    }
}

fn wallpaper_handles_ready(
    preload: &LoadingScreenWallpaperPreload,
    images: &Assets<Image>,
) -> bool {
    preload.handles.len() == preload.expected_count
        && preload
            .handles
            .iter()
            .all(|handle| images.get(handle.id()).is_some())
}

/// Creates loading-screen state from the supplied launch settings.
pub fn loading_screen_state(settings: &ClientLaunchSettings) -> LoadingScreenState {
    let enabled = loading_screen_enabled(settings);
    LoadingScreenState {
        active: enabled,
        complete: !enabled,
        connection_error: None,
        wallpaper_assets_ready: !enabled,
        status_text: if enabled {
            "Loading local arena".to_string()
        } else {
            "Ready".to_string()
        },
        client_progress_percent: 0.0,
        client_ready: !enabled,
        ready_sent: !enabled,
        ready_players: 0,
        total_players: 0,
        dark_players: Vec::new(),
        light_players: Vec::new(),
    }
}

fn loading_screen_enabled(settings: &ClientLaunchSettings) -> bool {
    settings.match_id.is_some() && settings.player_public_id.is_some()
}

/// Advances local loading progress and reports readiness after assets and the minimum delay complete.
fn update_loading_screen_ready_gate(
    time: Res<Time>,
    mut gate: ResMut<LoadingScreenReadyGate>,
    mut state: ResMut<LoadingScreenState>,
    asset_server: Option<Res<AssetServer>>,
    preload: Res<LoadingScreenWallpaperPreload>,
    images: Option<Res<Assets<Image>>>,
    scene_roots: Query<&WorldAssetRoot>,
) {
    if !state.active || state.client_ready || state.connection_error.is_some() {
        return;
    }

    gate.minimum_timer.tick(time.delta());
    let minimum_done = gate.minimum_timer.is_finished();
    let scene_assets_ready = asset_server
        .as_deref()
        .map(|asset_server| {
            let mut scene_count = 0usize;
            let all_scenes_loaded = scene_roots.iter().all(|scene_root| {
                scene_count += 1;
                asset_server.is_loaded_with_dependencies(scene_root.0.id())
            });
            scene_count == 0 || all_scenes_loaded
        })
        .unwrap_or(true);
    let wallpaper_assets_ready = if preload.expected_count == 0 {
        true
    } else {
        images
            .as_deref()
            .map(|images| wallpaper_handles_ready(&preload, images))
            .unwrap_or(false)
    };
    let render_ready = scene_assets_ready && wallpaper_assets_ready;

    if minimum_done && render_ready {
        state.client_ready = true;
        state.client_progress_percent = 100.0;
        state.status_text = "Local arena ready".to_string();
        return;
    }

    let timer_duration = gate.minimum_timer.duration().as_secs_f32();
    let timer_progress = if timer_duration <= 0.0 {
        100.0
    } else {
        (gate.minimum_timer.elapsed_secs() / timer_duration * 100.0).clamp(0.0, 100.0)
    };
    let local_progress = if minimum_done {
        90.0
    } else {
        timer_progress.min(90.0)
    };
    let status_text = if !minimum_done {
        let remaining_seconds = gate.minimum_timer.remaining_secs().ceil().max(0.0) as u32;
        format!("Loading local arena ({}s)", remaining_seconds)
    } else {
        if !wallpaper_assets_ready {
            "Loading match art".to_string()
        } else {
            "Loading champion assets".to_string()
        }
    };

    state.client_progress_percent = local_progress;
    state.status_text = status_text;
}

fn send_display_ready(
    mut state: ResMut<LoadingScreenState>,
    connection: Res<LoadingScreenConnectionState>,
    mut senders: Query<&mut MessageSender<DisplayReady>, With<Client>>,
) {
    if !state.active || !state.client_ready || state.ready_sent || state.connection_error.is_some()
    {
        return;
    }

    if !connection.has_connected {
        state.status_text = "Connecting to game server".to_string();
        return;
    }

    for mut sender in &mut senders {
        sender.send::<ReliableCommandChannel>(DisplayReady);
    }
    state.ready_sent = true;
    state.client_progress_percent = 100.0;
    state.status_text = "Waiting for players".to_string();
}

/// Applies the latest server loading status to local state and player profiles.
fn receive_loading_screen_status(
    mut state: ResMut<LoadingScreenState>,
    manifest: Res<ClientLoadingMatchManifest>,
    mut overhead_profiles: ResMut<OverheadPlayerProfiles>,
    mut receivers: Query<&mut MessageReceiver<LoadingScreenStatus>, With<Client>>,
) {
    let mut latest_status = None;
    for mut receiver in &mut receivers {
        for status in receiver.receive() {
            latest_status = Some(status);
        }
    }

    let Some(status) = latest_status else {
        return;
    };

    for player in &status.players {
        if let Some(display_name) = player.display_name.as_deref() {
            overhead_profiles.set_display_name(player.player_id, display_name.to_string());
        }
    }

    if state.connection_error.is_some() {
        return;
    }

    state.ready_players = status.ready_players;
    state.total_players = status.total_players.max(1);
    if status.can_close {
        state.status_text = "Entering arena".to_string();
        state.complete = true;
    } else if !state.complete && state.ready_sent {
        state.status_text = "Waiting for players".to_string();
    }
    if status.players.is_empty() {
        mark_ready_players(&mut state, &status.ready_player_ids, status.ready_players);
    } else {
        let (light_players, dark_players) = loading_players_from_status(&status.players, &manifest);
        state.light_players = light_players;
        state.dark_players = dark_players;
    }
}

/// Records the first successful game-server connection during loading.
fn record_loading_screen_connection(
    connected_clients: Query<Entity, (With<Client>, Added<Connected>)>,
    mut connection: ResMut<LoadingScreenConnectionState>,
) {
    if connected_clients.is_empty() {
        return;
    }

    connection.mark_connected();
}

/// Records a failed game-server connection while keeping the loading screen visible.
fn record_loading_screen_disconnect(
    disconnected_clients: Query<&Disconnected, (With<Client>, Added<Disconnected>)>,
    mut state: ResMut<LoadingScreenState>,
    mut connection: ResMut<LoadingScreenConnectionState>,
) {
    if !state.active || state.complete || state.connection_error.is_some() {
        return;
    }

    for disconnected in &disconnected_clients {
        let Some(reason) = disconnected.reason.as_deref() else {
            continue;
        };
        if reason == "Client trigger" {
            continue;
        }

        if connection.schedule_initial_retry() {
            info!(
                retry = connection.initial_retry_count,
                "Game-server connection ended before the initial handshake; retrying."
            );
            state.status_text = "Connecting to game server".to_string();
            return;
        }

        warn!("Game server connection failed: {reason}");
        let status_text = loading_connection_error_text(reason);
        state.connection_error = Some(status_text.to_string());
        state.client_ready = true;
        state.client_progress_percent = 100.0;
        state.status_text = status_text.to_string();
        return;
    }
}

/// Retries a transient disconnect that occurs before the first successful connection.
fn retry_initial_game_server_connection(
    time: Res<Time>,
    state: Res<LoadingScreenState>,
    mut connection: ResMut<LoadingScreenConnectionState>,
    disconnected_clients: Query<Entity, (With<Client>, With<Disconnected>)>,
    mut commands: Commands,
) {
    if !state.active || state.complete || state.connection_error.is_some() {
        return;
    }
    if !connection.retry_is_ready(time.delta()) {
        return;
    }

    let Some(client_entity) = disconnected_clients.iter().next() else {
        connection.clear_pending_retry();
        return;
    };

    connection.clear_pending_retry();
    commands.trigger(Connect {
        entity: client_entity,
    });
}

fn loading_connection_error_text(reason: &str) -> &'static str {
    if reason.contains("ConnectionRequestTimedOut") {
        "Connection to game server timed out"
    } else {
        "Connection to game server lost"
    }
}

#[derive(Component)]
struct LoadingScreenRoot;

#[derive(Component)]
struct LoadingProgressFill;

#[derive(Component)]
struct LoadingProgressBadgeText;

#[derive(Component)]
struct LoadingStatusText;

#[derive(Component)]
struct LoadingPingText;

#[derive(Component)]
struct LoadingPingSpinner;

#[derive(Component)]
struct LoadingPingSpinnerDot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum LoadingTeam {
    Light,
    Dark,
}

#[derive(Component, Clone, Copy)]
struct LoadingCard {
    team: LoadingTeam,
    index: usize,
}

#[derive(Component, Clone, Copy)]
struct LoadingCardImage {
    team: LoadingTeam,
    index: usize,
}

#[derive(Component, Clone, Copy)]
struct LoadingCardAvatar {
    team: LoadingTeam,
    index: usize,
}

#[derive(Component, Clone, Copy)]
struct LoadingCardAvatarImage {
    team: LoadingTeam,
    index: usize,
}

#[derive(Component, Clone, Copy)]
struct LoadingCardAccentText {
    team: LoadingTeam,
    index: usize,
    kind: LoadingCardTextKind,
}

#[derive(Clone, Copy)]
enum LoadingCardTextKind {
    Initial,
    Name,
    ChampionTitle,
    State,
}

fn spawn_loading_screen_ui(
    mut commands: Commands,
    settings: Res<ClientAppSettings>,
    launch_settings: Res<ClientLaunchSettings>,
    loading_images: Res<LoadingScreenImages>,
    mut ui_images: ResMut<Assets<Image>>,
) {
    if !settings.ui_enabled {
        return;
    }

    let fallback_wallpaper = loading_images.wallpaper(ChampionId::LIRA).clone();
    let progress_badge_image = ui_images.add(progress_badge_shape_image(
        launch_settings.accent_color_bevy(),
    ));
    commands.insert_resource(LoadingProgressBadgeImage {
        handle: progress_badge_image.clone(),
    });

    commands.spawn((
        LoadingScreenRoot,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            width: percent(100),
            height: percent(100),
            min_width: percent(100),
            min_height: percent(100),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(px(54), px(38)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.047, 0.063, 0.094, 1.0)),
        ZIndex(9000),
        Pickable::IGNORE,
        children![
            loading_ping_panel(),
            loading_team_lane(LoadingTeam::Light, fallback_wallpaper.clone()),
            loading_progress_panel(progress_badge_image),
            loading_team_lane(LoadingTeam::Dark, fallback_wallpaper),
        ],
    ));
}

fn loading_ping_panel() -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            top: px(22),
            right: px(28),
            display: Display::Flex,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: UiRect::axes(px(10), px(6)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.027, 0.035, 0.055, 0.76)),
        children![
            loading_ping_spinner(),
            (
                LoadingPingText,
                Text::new("0ms"),
                TextFont::from_font_size(13.0),
                TextColor(Color::srgb_u8(0x2B, 0xB8, 0x61)),
            ),
        ],
    )
}

fn loading_ping_spinner() -> impl Bundle {
    (
        LoadingPingSpinner,
        Node {
            position_type: PositionType::Relative,
            width: px(16),
            height: px(16),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(percent(50)),
            ..default()
        },
        BorderColor::all(Color::srgb_u8(0x2B, 0xB8, 0x61)),
        UiTransform::from_rotation(Rot2::radians(0.0)),
        children![(
            LoadingPingSpinnerDot,
            Node {
                position_type: PositionType::Absolute,
                top: px(-3),
                left: percent(50),
                width: px(6),
                height: px(6),
                border_radius: BorderRadius::all(percent(50)),
                ..default()
            },
            UiTransform::from_translation(Val2::px(-3.0, 0.0)),
            BackgroundColor(Color::srgb_u8(0x2B, 0xB8, 0x61)),
        )],
    )
}

fn loading_team_lane(team: LoadingTeam, fallback_wallpaper: Handle<Image>) -> impl Bundle {
    (
        Node {
            width: percent(100),
            min_height: px(344),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: match team {
                LoadingTeam::Light => JustifyContent::FlexStart,
                LoadingTeam::Dark => JustifyContent::FlexEnd,
            },
            ..default()
        },
        children![(
            Node {
                width: percent(100),
                min_height: px(430),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: px(28),
                ..default()
            },
            Children::spawn(SpawnIter((0..LOADING_TEAM_SIZE).map(move |index| {
                loading_card(team, index, fallback_wallpaper.clone())
            }))),
        )],
    )
}

fn loading_card(team: LoadingTeam, index: usize, fallback_wallpaper: Handle<Image>) -> impl Bundle {
    (
        LoadingCard { team, index },
        Node {
            position_type: PositionType::Relative,
            width: px(275),
            height: px(430),
            min_width: px(275),
            display: Display::None,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::FlexEnd,
            overflow: Overflow::clip(),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.063, 0.078, 0.114, 1.0)),
        BorderColor::all(match team {
            LoadingTeam::Light => Color::srgba(0.94, 0.82, 0.54, 0.33),
            LoadingTeam::Dark => Color::srgba(0.49, 0.51, 1.0, 0.26),
        }),
        children![
            (
                LoadingCardImage { team, index },
                ImageNode {
                    image: fallback_wallpaper,
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(50),
                    top: px(0),
                    width: px(765),
                    height: px(430),
                    ..default()
                },
                UiTransform::from_translation(Val2::px(-382.5, 0.0)),
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    right: px(0),
                    top: px(0),
                    bottom: px(0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.40)),
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    right: px(0),
                    top: px(0),
                    height: px(38),
                    display: Display::Flex,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
                children![loading_card_text(
                    team,
                    index,
                    LoadingCardTextKind::ChampionTitle,
                    "Lira"
                )],
            ),
            (
                Node {
                    position_type: PositionType::Relative,
                    min_height: px(152),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: px(4),
                    padding: UiRect::new(px(12), px(12), px(22), px(12)),
                    border: UiRect::top(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.082, 0.098, 0.133, 0.93)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.14)),
                children![
                    loading_card_avatar(team, index),
                    loading_card_text(team, index, LoadingCardTextKind::Name, "Player"),
                    loading_card_text(team, index, LoadingCardTextKind::State, "Loading"),
                ],
            ),
        ],
    )
}

fn loading_card_avatar(team: LoadingTeam, index: usize) -> impl Bundle {
    (
        LoadingCardAvatar { team, index },
        Node {
            position_type: PositionType::Absolute,
            top: px(-34),
            width: px(68),
            height: px(68),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(8)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.125, 0.141, 0.173, 1.0)),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.20)),
        children![
            (
                LoadingCardAvatarImage { team, index },
                ImageNode {
                    image: Handle::default(),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
                Node {
                    width: percent(100),
                    height: percent(100),
                    display: Display::None,
                    ..default()
                },
            ),
            (
                LoadingCardAccentText {
                    team,
                    index,
                    kind: LoadingCardTextKind::Initial,
                },
                Text::new("?"),
                TextFont::from_font_size(22.0),
                TextColor(Color::srgba(0.93, 0.95, 0.97, 1.0)),
                TextLayout::justify(Justify::Center),
            ),
        ],
    )
}

fn loading_card_text(
    team: LoadingTeam,
    index: usize,
    kind: LoadingCardTextKind,
    value: &'static str,
) -> impl Bundle {
    let (size, color) = match kind {
        LoadingCardTextKind::Name => (15.0, Color::srgba(0.93, 0.95, 0.97, 1.0)),
        LoadingCardTextKind::ChampionTitle => (15.0, Color::WHITE),
        LoadingCardTextKind::State => (10.0, Color::srgba(0.95, 0.77, 0.36, 1.0)),
        LoadingCardTextKind::Initial => (18.0, Color::srgba(0.93, 0.95, 0.97, 1.0)),
    };

    (
        LoadingCardAccentText { team, index, kind },
        Text::new(value),
        TextFont::from_font_size(size),
        TextColor(color),
        TextLayout::justify(Justify::Center),
    )
}

fn loading_progress_panel(progress_badge_image: Handle<Image>) -> impl Bundle {
    (
        Node {
            width: percent(100),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: px(4),
            ..default()
        },
        children![
            (
                Node {
                    width: px(720),
                    max_width: percent(80),
                    height: px(12),
                    display: Display::Flex,
                    overflow: Overflow::clip(),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.027, 0.035, 0.055, 1.0)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.14)),
                children![(
                    LoadingProgressFill,
                    Node {
                        width: percent(0),
                        height: percent(100),
                        min_width: px(0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.95, 0.77, 0.36, 1.0)),
                )],
            ),
            (
                Node {
                    position_type: PositionType::Relative,
                    width: px(PROGRESS_BADGE_WIDTH as f32),
                    height: px(PROGRESS_BADGE_HEIGHT as f32),
                    display: Display::Flex,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![
                    (
                        ImageNode {
                            image: progress_badge_image,
                            image_mode: NodeImageMode::Stretch,
                            ..default()
                        },
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            right: px(0),
                            top: px(0),
                            bottom: px(0),
                            ..default()
                        },
                    ),
                    (
                        LoadingProgressBadgeText,
                        Text::new("0%"),
                        TextFont::from_font_size(13.0),
                        TextColor(Color::srgba(0.93, 0.95, 0.97, 1.0)),
                        TextLayout::justify(Justify::Center),
                    ),
                ],
            ),
            (
                LoadingStatusText,
                Text::new("Loading local arena"),
                TextFont::from_font_size(15.0),
                TextColor(Color::srgba(0.78, 0.82, 0.88, 1.0)),
                TextLayout::justify(Justify::Center),
            ),
        ],
    )
}

/// Synchronizes loading-screen UI nodes with state, assets, and network ping.
fn sync_loading_screen_ui(
    time: Res<Time>,
    state: Res<LoadingScreenState>,
    launch_settings: Res<ClientLaunchSettings>,
    network_ping: Res<NetworkPingState>,
    asset_server: Res<AssetServer>,
    images: Option<Res<LoadingScreenImages>>,
    progress_badge_image: Option<Res<LoadingProgressBadgeImage>>,
    mut ui_images: ResMut<Assets<Image>>,
    mut avatar_cache: ResMut<LoadingAvatarCache>,
    mut last_progress_accent: Local<Option<Color>>,
    mut layout_nodes: ParamSet<(
        Query<&mut Node, With<LoadingScreenRoot>>,
        Query<(&mut Node, &mut BackgroundColor), With<LoadingProgressFill>>,
        Query<(&LoadingCard, &mut Node, &mut BorderColor), With<LoadingCard>>,
        Query<(&LoadingCardAvatar, &mut BackgroundColor)>,
        Query<(&LoadingCardImage, &mut ImageNode)>,
        Query<(&LoadingCardAvatarImage, &mut Node, &mut ImageNode)>,
        Query<&mut BackgroundColor, With<LoadingPingSpinnerDot>>,
    )>,
    mut text_queries: ParamSet<(
        Query<(&LoadingCardAccentText, &mut Text, &mut TextColor)>,
        Query<(&mut Text, &mut TextColor), With<LoadingProgressBadgeText>>,
        Query<(&mut Text, &mut TextColor), With<LoadingStatusText>>,
        Query<(&mut Text, &mut TextColor), With<LoadingPingText>>,
    )>,
    mut ping_spinners: Query<
        (&mut UiTransform, &mut BorderColor),
        (With<LoadingPingSpinner>, Without<LoadingCard>),
    >,
) {
    let visible = state.is_visible();
    for mut root in &mut layout_nodes.p0() {
        root.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }

    let accent = launch_settings.accent_color_bevy();
    let local_player_public_id = launch_settings
        .player_public_id
        .as_deref()
        .and_then(|public_id| public_id.parse::<u64>().ok());
    if last_progress_accent.as_ref() != Some(&accent) {
        if let Some(progress_badge_image) = progress_badge_image.as_deref()
            && let Some(mut image) = ui_images.get_mut(progress_badge_image.handle.id())
        {
            *image = progress_badge_shape_image(accent);
        }
        *last_progress_accent = Some(accent);
    }

    for (mut node, mut background) in &mut layout_nodes.p1() {
        node.width = percent(state.progress_percent());
        *background = BackgroundColor(accent);
    }

    for (card, mut node, mut border) in &mut layout_nodes.p2() {
        let player = loading_card_player(&state, card.team, card.index);
        let is_local_player =
            player.is_some_and(|player| Some(player.public_id) == local_player_public_id);
        node.display = if player.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        node.border = UiRect::all(px(if is_local_player { 5 } else { 1 }));
        border.set_all(accent);
    }

    for (card_image, mut image_node) in &mut layout_nodes.p4() {
        if let (Some(images), Some(player)) = (
            images.as_deref(),
            loading_card_player(&state, card_image.team, card_image.index),
        ) {
            image_node.image = images.wallpaper(player.champion).clone();
        }
    }

    for (avatar, mut background) in &mut layout_nodes.p3() {
        if loading_card_player(&state, avatar.team, avatar.index).is_some() {
            *background = BackgroundColor(accent);
        }
    }

    for (avatar, mut node, mut image_node) in &mut layout_nodes.p5() {
        let avatar_handle = loading_card_player(&state, avatar.team, avatar.index)
            .and_then(|player| player.avatar_url.as_deref())
            .and_then(|avatar_url| {
                loading_avatar_handle(avatar_url, &asset_server, &mut avatar_cache, &mut ui_images)
            });

        if let Some(handle) = avatar_handle {
            image_node.image = handle;
            node.display = Display::Flex;
        } else {
            node.display = Display::None;
        }
    }

    let progress = state.progress_percent().round().clamp(0.0, 100.0) as u32;
    for (mut text, mut color) in &mut text_queries.p1() {
        text.0 = format!("{progress}%");
        *color = TextColor(accent_foreground_for(accent));
    }

    let status_color = if state.connection_error.is_some() {
        Color::srgb_u8(0xED, 0x5C, 0x5C)
    } else {
        Color::srgba(0.78, 0.82, 0.88, 1.0)
    };
    for (mut text, mut color) in &mut text_queries.p2() {
        text.0 = state.status_text.clone();
        *color = TextColor(status_color);
    }

    let ping_color = ping_color(&network_ping);
    let ping_text = ping_text(&network_ping);
    for (mut text, mut color) in &mut text_queries.p3() {
        text.0 = ping_text.clone();
        *color = TextColor(ping_color);
    }
    for (mut transform, mut border) in &mut ping_spinners {
        transform.rotation = Rot2::radians(time.elapsed_secs() * 1.4);
        border.set_all(ping_color);
    }
    for mut background in &mut layout_nodes.p6() {
        *background = BackgroundColor(ping_color);
    }

    for (text_marker, mut text, mut color) in &mut text_queries.p0() {
        if let Some(player) = loading_card_player(&state, text_marker.team, text_marker.index) {
            text.0 = match text_marker.kind {
                LoadingCardTextKind::Initial => {
                    if player
                        .avatar_url
                        .as_deref()
                        .is_some_and(|source| avatar_is_ready(source, &avatar_cache))
                    {
                        String::new()
                    } else {
                        initials(&player.name)
                    }
                }
                LoadingCardTextKind::Name => player.name.clone(),
                LoadingCardTextKind::ChampionTitle => player.champion_name.to_ascii_uppercase(),
                LoadingCardTextKind::State => {
                    if player.ready {
                        "READY".to_string()
                    } else {
                        "LOADING".to_string()
                    }
                }
            };
            if matches!(text_marker.kind, LoadingCardTextKind::State) {
                *color = TextColor(accent);
            } else if matches!(text_marker.kind, LoadingCardTextKind::Initial) {
                *color = TextColor(accent_foreground_for(accent));
            }
        }
    }
}

/// Returns a cached avatar image handle or begins loading the avatar.
fn loading_avatar_handle(
    source: &str,
    asset_server: &AssetServer,
    cache: &mut LoadingAvatarCache,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }

    if let Some(entry) = cache.entries.get_mut(source) {
        return match entry {
            LoadingAvatarEntry::Ready(handle) => Some(handle.clone()),
            LoadingAvatarEntry::Failed => None,
            LoadingAvatarEntry::Loading(receiver) => {
                let received = receiver
                    .get_mut()
                    .unwrap_or_else(|error| error.into_inner())
                    .try_recv();

                match received {
                    Ok(Ok(download)) => {
                        if let Some(image) = avatar_image_from_download(
                            source,
                            &download.bytes,
                            download.content_type.as_deref(),
                        ) {
                            let handle = images.add(image);
                            *entry = LoadingAvatarEntry::Ready(handle.clone());
                            return Some(handle);
                        }

                        warn!("Failed to decode loading-screen avatar from '{}'.", source);
                        *entry = LoadingAvatarEntry::Failed;
                        None
                    }
                    Ok(Err(error)) => {
                        warn!(
                            "Failed to load loading-screen avatar from '{}': {}",
                            source, error
                        );
                        *entry = LoadingAvatarEntry::Failed;
                        None
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        *entry = LoadingAvatarEntry::Failed;
                        None
                    }
                }
            }
        };
    }

    if source.starts_with("http://") || source.starts_with("https://") {
        let (sender, receiver) = channel();
        let owned_source = source.to_string();
        std::thread::spawn(move || {
            let _ = sender.send(download_avatar(&owned_source));
        });
        cache.entries.insert(
            source.to_string(),
            LoadingAvatarEntry::Loading(Mutex::new(receiver)),
        );
        return None;
    }

    let handle = asset_server.load(source.to_string());
    cache.entries.insert(
        source.to_string(),
        LoadingAvatarEntry::Ready(handle.clone()),
    );
    Some(handle)
}

fn avatar_is_ready(source: &str, cache: &LoadingAvatarCache) -> bool {
    matches!(
        cache.entries.get(source.trim()),
        Some(LoadingAvatarEntry::Ready(_))
    )
}

fn download_avatar(source: &str) -> Result<DownloadedAvatar, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(source)
        .send()
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let bytes = response
        .bytes()
        .map_err(|error| error.to_string())?
        .to_vec();

    Ok(DownloadedAvatar {
        bytes,
        content_type,
    })
}

fn avatar_image_from_download(
    source: &str,
    bytes: &[u8],
    content_type: Option<&str>,
) -> Option<Image> {
    if let Some(content_type) = content_type
        && let Ok(image) = avatar_image_from_buffer(bytes, ImageType::MimeType(content_type))
    {
        return Some(image);
    }

    if let Some(extension) = image_extension(source)
        && let Ok(image) = avatar_image_from_buffer(bytes, ImageType::Extension(extension))
    {
        return Some(image);
    }

    ["png", "jpg", "jpeg", "webp"]
        .into_iter()
        .find_map(|extension| avatar_image_from_buffer(bytes, ImageType::Extension(extension)).ok())
}

fn avatar_image_from_buffer(bytes: &[u8], image_type: ImageType) -> Result<Image, TextureError> {
    Image::from_buffer(
        bytes,
        image_type,
        CompressedImageFormats::empty(),
        true,
        ImageSampler::linear(),
        RenderAssetUsages::default(),
    )
}

fn image_extension(source: &str) -> Option<&str> {
    let without_query = source.split(['?', '#']).next().unwrap_or(source);
    without_query
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .filter(|extension| !extension.is_empty() && extension.len() <= 5)
}

fn loading_card_player(
    state: &LoadingScreenState,
    team: LoadingTeam,
    index: usize,
) -> Option<&LoadingPlayer> {
    match team {
        LoadingTeam::Light => state.light_players.get(index),
        LoadingTeam::Dark => state.dark_players.get(index),
    }
}

fn mark_ready_players(
    state: &mut LoadingScreenState,
    ready_player_ids: &[u64],
    ready_players: usize,
) {
    if !ready_player_ids.is_empty() {
        for player in state
            .dark_players
            .iter_mut()
            .chain(state.light_players.iter_mut())
        {
            player.ready = ready_player_ids.contains(&player.public_id);
        }
        return;
    }

    let mut remaining = ready_players;
    for player in state
        .dark_players
        .iter_mut()
        .chain(state.light_players.iter_mut())
    {
        player.ready = remaining > 0;
        remaining = remaining.saturating_sub(1);
    }
}

/// Builds team-specific loading-screen players from launcher match-manifest data.
fn loading_players_from_manifest(
    manifest: &ClientLoadingMatchManifest,
) -> (Vec<LoadingPlayer>, Vec<LoadingPlayer>) {
    split_loading_players_by_team(manifest.players.iter().map(|(&player_id, player)| {
        (
            player.team,
            LoadingPlayer {
                public_id: player_id,
                name: player
                    .display_name
                    .clone()
                    .unwrap_or_else(|| "Player".to_string()),
                avatar_url: player.avatar_url.clone(),
                champion: player.champion,
                champion_name: champion_name(player.champion).to_string(),
                ready: false,
            },
        )
    }))
}

/// Builds team-specific loading-screen players from the server status.
fn loading_players_from_status(
    status_players: &[LoadingScreenPlayer],
    manifest: &ClientLoadingMatchManifest,
) -> (Vec<LoadingPlayer>, Vec<LoadingPlayer>) {
    split_loading_players_by_team(status_players.iter().map(|player| {
        let manifest_player = manifest.players.get(&player.player_id);
        (
            player.team,
            LoadingPlayer {
                public_id: player.player_id,
                name: loading_player_display_name(player, manifest),
                avatar_url: player
                    .avatar_url
                    .as_deref()
                    .and_then(non_empty_string)
                    .or_else(|| {
                        manifest_player
                            .and_then(|player| player.avatar_url.as_deref())
                            .and_then(non_empty_string)
                    }),
                champion: player.champion,
                champion_name: champion_name(player.champion).to_string(),
                ready: player.ready,
            },
        )
    }))
}

fn split_loading_players_by_team(
    players: impl IntoIterator<Item = (TeamSpec, LoadingPlayer)>,
) -> (Vec<LoadingPlayer>, Vec<LoadingPlayer>) {
    let mut light_players = Vec::new();
    let mut dark_players = Vec::new();

    for (team, player) in players {
        match team {
            TeamSpec::Light => light_players.push(player),
            TeamSpec::Dark => dark_players.push(player),
            TeamSpec::Neutral => light_players.push(player),
        }
    }

    light_players.sort_by_key(|player| player.public_id);
    dark_players.sort_by_key(|player| player.public_id);
    (light_players, dark_players)
}

/// Resolves the display name from server and manifest player data.
fn loading_player_display_name(
    player: &LoadingScreenPlayer,
    manifest: &ClientLoadingMatchManifest,
) -> String {
    player
        .display_name
        .as_deref()
        .and_then(public_display_name)
        .or_else(|| {
            manifest
                .players
                .get(&player.player_id)
                .and_then(|player| player.display_name.as_deref())
                .and_then(public_display_name)
        })
        .unwrap_or_else(|| "Player".to_string())
}

fn champion_name(champion: ChampionId) -> &'static str {
    champion.display_name().unwrap_or("Lira")
}

fn initials(name: &str) -> String {
    let value = name
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>();
    if value.is_empty() {
        "?".to_string()
    } else {
        value.to_ascii_uppercase()
    }
}

fn progress_badge_shape_image(accent: Color) -> Image {
    let width = PROGRESS_BADGE_WIDTH;
    let height = PROGRESS_BADGE_HEIGHT;
    let mut data = vec![0; (width * height * 4) as usize];
    let fill = Color::srgba(0.027, 0.035, 0.055, 0.94)
        .to_srgba()
        .to_u8_array();
    let border = accent.to_srgba().to_u8_array();

    for y in 0..height {
        let t = y as f32 / height.saturating_sub(1) as f32;
        let outer_inset = 4.0 + 18.0 * t;
        let inner_inset = outer_inset + 2.0;
        let outer_left = outer_inset.round() as i32;
        let outer_right = width as i32 - outer_left - 1;
        let inner_left = inner_inset.round() as i32;
        let inner_right = width as i32 - inner_left - 1;

        for x in 0..width {
            let x_i32 = x as i32;
            if x_i32 < outer_left || x_i32 > outer_right {
                continue;
            }

            let pixel = if y < 2
                || y >= height.saturating_sub(2)
                || x_i32 < inner_left
                || x_i32 > inner_right
            {
                border
            } else {
                fill
            };
            let offset = ((y * width + x) * 4) as usize;
            data[offset..offset + 4].copy_from_slice(&pixel);
        }
    }

    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn accent_foreground_for(color: Color) -> Color {
    let srgba = color.to_srgba();
    let luminance = 0.2126 * srgba.red + 0.7152 * srgba.green + 0.0722 * srgba.blue;
    if luminance > 0.58 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

impl LoadingScreenState {
    /// Returns whether the loading screen is currently visible.
    pub fn is_visible(&self) -> bool {
        self.active && !self.complete
    }

    /// Returns the progress percentage shown by the loading screen.
    fn progress_percent(&self) -> f32 {
        if !self.ready_sent {
            return self.client_progress_percent.clamp(0.0, 100.0);
        }
        if self.complete {
            return 100.0;
        }
        if self.total_players == 0 {
            return 0.0;
        }
        ((self.ready_players as f32 / self.total_players as f32) * 100.0).clamp(0.0, 100.0)
    }
}

impl Default for LoadingScreenReadyGate {
    fn default() -> Self {
        Self {
            minimum_timer: Timer::new(MINIMUM_CLIENT_LOADING_DURATION, TimerMode::Once),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loading_screen_state_for_test() -> LoadingScreenState {
        LoadingScreenState {
            active: true,
            complete: false,
            connection_error: None,
            wallpaper_assets_ready: true,
            status_text: String::new(),
            client_progress_percent: 100.0,
            client_ready: true,
            ready_sent: true,
            ready_players: 1,
            total_players: 2,
            dark_players: Vec::new(),
            light_players: Vec::new(),
        }
    }

    #[test]
    fn calculates_progress_per_player() {
        let snapshot = loading_screen_state_for_test();

        assert_eq!(snapshot.progress_percent(), 50.0);
    }

    #[test]
    fn uses_local_progress_before_display_ready_is_sent() {
        let mut snapshot = loading_screen_state_for_test();
        snapshot.client_progress_percent = 42.0;
        snapshot.client_ready = false;
        snapshot.ready_sent = false;
        snapshot.ready_players = 2;

        assert_eq!(snapshot.progress_percent(), 42.0);
    }

    #[test]
    fn initial_disconnect_schedules_a_retry_without_showing_an_error() {
        let mut app = App::new();
        app.insert_resource(loading_screen_state_for_test())
            .init_resource::<LoadingScreenConnectionState>()
            .add_systems(Update, record_loading_screen_disconnect);
        app.world_mut().spawn((
            Client::default(),
            Disconnected {
                reason: Some("Client disconnected: ConnectionRequestTimedOut".to_string()),
            },
        ));

        app.update();

        let state = app.world().resource::<LoadingScreenState>();
        let connection = app.world().resource::<LoadingScreenConnectionState>();
        assert_eq!(state.connection_error, None);
        assert_eq!(state.status_text, "Connecting to game server");
        assert_eq!(connection.initial_retry_count, 1);
        assert!(connection.retry_timer.is_some());
    }

    #[test]
    fn exhausted_initial_retries_show_the_connection_error() {
        let mut connection = LoadingScreenConnectionState::default();
        connection.initial_retry_count = MAXIMUM_INITIAL_CONNECTION_RETRIES;

        let mut app = App::new();
        app.insert_resource(loading_screen_state_for_test())
            .insert_resource(connection)
            .add_systems(Update, record_loading_screen_disconnect);
        app.world_mut().spawn((
            Client::default(),
            Disconnected {
                reason: Some("Client disconnected: ConnectionRequestTimedOut".to_string()),
            },
        ));

        app.update();

        assert_eq!(
            app.world()
                .resource::<LoadingScreenState>()
                .connection_error
                .as_deref(),
            Some("Connection to game server timed out")
        );
    }

    #[test]
    fn established_connection_disconnect_blocks_display_ready_in_the_same_update() {
        let mut state = loading_screen_state_for_test();
        state.ready_sent = false;

        let mut app = App::new();
        app.insert_resource(state)
            .insert_resource(LoadingScreenConnectionState {
                has_connected: true,
                ..default()
            })
            .add_systems(
                Update,
                (record_loading_screen_disconnect, send_display_ready).chain(),
            );
        app.world_mut().spawn((
            Client::default(),
            Disconnected {
                reason: Some("Client disconnected: ConnectionRequestTimedOut".to_string()),
            },
        ));

        app.update();

        let state = app.world().resource::<LoadingScreenState>();
        assert_eq!(
            state.connection_error.as_deref(),
            Some("Connection to game server timed out")
        );
        assert!(!state.ready_sent);
        assert!(state.is_visible());
    }

    #[test]
    fn display_ready_waits_for_the_first_server_connection() {
        let mut state = loading_screen_state_for_test();
        state.ready_sent = false;

        let mut app = App::new();
        app.insert_resource(state)
            .init_resource::<LoadingScreenConnectionState>()
            .add_systems(Update, send_display_ready);

        app.update();

        let state = app.world().resource::<LoadingScreenState>();
        assert!(!state.ready_sent);
        assert_eq!(state.status_text, "Connecting to game server");
    }

    #[test]
    fn builds_loading_players_from_launcher_manifest() {
        let manifest = ClientLoadingMatchManifest::from_json(
            r#"{
                "matchId": "match-1",
                "players": [
                    {
                        "playerPublicId": 8,
                        "team": "Dark",
                        "championId": 6607,
                        "displayName": "Dark Player",
                        "avatarUrl": ""
                    },
                    {
                        "playerPublicId": 7,
                        "team": "Light",
                        "championId": 6609,
                        "displayName": "Second Light"
                    },
                    {
                        "playerPublicId": 4,
                        "team": "Light",
                        "championId": 6606,
                        "display_name": "First Light",
                        "avatar_url": "avatars/first.png"
                    }
                ]
            }"#,
        )
        .expect("launcher manifest should parse");

        let (light_players, dark_players) = loading_players_from_manifest(&manifest);

        assert_eq!(
            light_players
                .iter()
                .map(|player| player.public_id)
                .collect::<Vec<_>>(),
            vec![4, 7]
        );
        assert_eq!(light_players[0].name, "First");
        assert_eq!(light_players[0].champion, ChampionId::LIRA);
        assert_eq!(
            light_players[0].avatar_url.as_deref(),
            Some("avatars/first.png")
        );
        assert!(!light_players[0].ready);
        assert_eq!(dark_players[0].name, "Dark");
        assert_eq!(dark_players[0].champion_name, "Ignara");
        assert_eq!(dark_players[0].avatar_url, None);
    }

    #[test]
    fn describes_connection_request_timeouts() {
        assert_eq!(
            loading_connection_error_text("Client disconnected: ConnectionRequestTimedOut"),
            "Connection to game server timed out"
        );
    }
}
