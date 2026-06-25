use super::settings::{ClientAppSettings, ClientLaunchSettings};
use bevy::asset::RenderAssetUsages;
use bevy::ecs::spawn::SpawnIter;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use game_logic::OverheadPlayerProfiles;
use game_shared::game::team::TeamSpec;
use game_shared::network::{
    ChampionId, DisplayReady, LoadingScreenPlayer, LoadingScreenStatus, ReliableCommandChannel,
};
use lightyear::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::network::{NetworkPingState, ping_color, ping_text};

const MINIMUM_CLIENT_LOADING_DURATION: Duration = Duration::from_secs(5);
const LOADING_SCREEN_WALLPAPERS: [&str; 4] = [
    "wallpapers/lira-loading.jpg",
    "wallpapers/ignara-loading.jpg",
    "wallpapers/yuna-loading.jpg",
    "wallpapers/sophia-loading.jpg",
];
const LOADING_TEAM_SIZE: usize = 5;
const PROGRESS_BADGE_WIDTH: u32 = 112;
const PROGRESS_BADGE_HEIGHT: u32 = 30;
const MATCH_MANIFEST_ENV: &str = "MIRA_MATCH_MANIFEST_JSON";

#[derive(Resource, Clone)]
pub struct LoadingScreenState {
    shared: Arc<Mutex<LoadingScreenSnapshot>>,
}

#[derive(Debug, Clone, PartialEq)]
struct LoadingScreenSnapshot {
    active: bool,
    complete: bool,
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

#[derive(Resource, Debug, Clone)]
struct ClientLoadingMatchManifest {
    players: HashMap<u64, ClientLoadingMatchPlayer>,
}

#[derive(Debug, Clone)]
struct ClientLoadingMatchPlayer {
    display_name: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientLoadingMatchManifestFile {
    players: Vec<ClientLoadingMatchPlayerFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientLoadingMatchPlayerFile {
    player_public_id: u64,
    #[serde(default, alias = "display_name")]
    display_name: Option<String>,
    #[serde(default, alias = "avatar_url")]
    avatar_url: Option<String>,
}

#[derive(Resource, Debug)]
struct LoadingScreenReadyGate {
    minimum_timer: Timer,
}

#[derive(Resource, Debug, Default)]
struct LoadingScreenWallpaperPreload {
    handles: Vec<Handle<Image>>,
    expected_count: usize,
}

#[derive(Resource)]
struct LoadingScreenImages {
    lira: Handle<Image>,
    ignara: Handle<Image>,
    yuna: Handle<Image>,
    sophia: Handle<Image>,
}

#[derive(Resource)]
struct LoadingProgressBadgeImage {
    handle: Handle<Image>,
}

#[derive(Resource, Default)]
struct LoadingAvatarCache {
    entries: HashMap<String, LoadingAvatarEntry>,
}

enum LoadingAvatarEntry {
    Loading(Arc<Mutex<Receiver<Result<DownloadedAvatar, String>>>>),
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
            return Self {
                players: HashMap::new(),
            };
        };

        let manifest = serde_json::from_str::<ClientLoadingMatchManifestFile>(&raw_manifest)
            .map_err(|error| {
                warn!(
                    "Failed to parse client loading match manifest from {}: {}",
                    MATCH_MANIFEST_ENV, error
                );
            });
        let Ok(manifest) = manifest else {
            return Self {
                players: HashMap::new(),
            };
        };

        let players = manifest
            .players
            .into_iter()
            .map(|player| {
                (
                    player.player_public_id,
                    ClientLoadingMatchPlayer {
                        display_name: player.display_name.as_deref().and_then(public_display_name),
                        avatar_url: player.avatar_url.as_deref().and_then(non_empty_string),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        Self { players }
    }
}

/// Mirrors the loading-screen state into the Bevy UI loading component.
pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadingScreenReadyGate>()
            .init_resource::<LoadingScreenWallpaperPreload>()
            .init_resource::<LoadingAvatarCache>()
            .init_resource::<ClientLoadingMatchManifest>()
            .add_systems(Startup, seed_overhead_profiles_from_manifest)
            .add_systems(Startup, spawn_loading_screen_ui)
            .add_systems(PreStartup, preload_loading_screen_wallpapers)
            .add_systems(Update, update_loading_screen_wallpaper_status)
            .add_systems(Update, update_loading_screen_ready_gate)
            .add_systems(Update, send_display_ready)
            .add_systems(Update, receive_loading_screen_status)
            .add_systems(Update, sync_loading_screen_ui);
    }
}

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
    state: Res<LoadingScreenState>,
    mut commands: Commands,
) {
    preload.expected_count = LOADING_SCREEN_WALLPAPERS.len();

    let images = LoadingScreenImages {
        lira: asset_server.load("wallpapers/lira-loading.jpg"),
        ignara: asset_server.load("wallpapers/ignara-loading.jpg"),
        yuna: asset_server.load("wallpapers/yuna-loading.jpg"),
        sophia: asset_server.load("wallpapers/sophia-loading.jpg"),
    };

    preload.handles = vec![
        images.lira.clone(),
        images.ignara.clone(),
        images.yuna.clone(),
        images.sophia.clone(),
    ];
    commands.insert_resource(images);

    update_snapshot(&state.shared, |snapshot| {
        snapshot.wallpaper_assets_ready = !snapshot.active;
    });
}

fn update_loading_screen_wallpaper_status(
    state: Res<LoadingScreenState>,
    preload: Res<LoadingScreenWallpaperPreload>,
    images: Res<Assets<Image>>,
) {
    if preload.expected_count == 0 {
        return;
    }

    let wallpapers_ready = wallpaper_handles_ready(&preload, &images);
    if wallpapers_ready == state.snapshot().wallpaper_assets_ready {
        return;
    }

    update_snapshot(&state.shared, |snapshot| {
        snapshot.wallpaper_assets_ready = wallpapers_ready;
        if wallpapers_ready && snapshot.status_text == "Loading match art" {
            snapshot.status_text = "Loading local arena".to_string();
        }
    });
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

pub fn loading_screen_state(settings: &ClientLaunchSettings) -> LoadingScreenState {
    let enabled = loading_screen_enabled(settings);
    let snapshot = LoadingScreenSnapshot {
        active: enabled,
        complete: !enabled,
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
    };
    let state = LoadingScreenState {
        shared: Arc::new(Mutex::new(snapshot)),
    };

    state
}

fn loading_screen_enabled(settings: &ClientLaunchSettings) -> bool {
    settings.match_id.is_some() && settings.player_public_id.is_some()
}

fn update_loading_screen_ready_gate(
    time: Res<Time>,
    mut gate: ResMut<LoadingScreenReadyGate>,
    state: Res<LoadingScreenState>,
    asset_server: Option<Res<AssetServer>>,
    preload: Res<LoadingScreenWallpaperPreload>,
    images: Option<Res<Assets<Image>>>,
    scene_roots: Query<&SceneRoot>,
) {
    let snapshot = state.snapshot();
    if !snapshot.active || snapshot.client_ready {
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
        update_snapshot(&state.shared, |snapshot| {
            snapshot.client_ready = true;
            snapshot.client_progress_percent = 100.0;
            snapshot.status_text = "Local arena ready".to_string();
        });
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

    update_snapshot(&state.shared, |snapshot| {
        snapshot.client_progress_percent = local_progress;
        snapshot.status_text = status_text;
    });
}

fn send_display_ready(
    state: Res<LoadingScreenState>,
    mut senders: Query<&mut MessageSender<DisplayReady>, With<Client>>,
) {
    let snapshot = state.snapshot();
    if !snapshot.active || !snapshot.client_ready || snapshot.ready_sent {
        return;
    }

    for mut sender in &mut senders {
        sender.send::<ReliableCommandChannel>(DisplayReady);
    }
    update_snapshot(&state.shared, |snapshot| {
        snapshot.ready_sent = true;
        snapshot.client_progress_percent = 100.0;
        snapshot.status_text = "Waiting for players".to_string();
    });
}

fn receive_loading_screen_status(
    state: Res<LoadingScreenState>,
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

    update_snapshot(&state.shared, |snapshot| {
        snapshot.ready_players = status.ready_players;
        snapshot.total_players = status.total_players.max(1);
        if status.can_close {
            snapshot.status_text = "Entering arena".to_string();
            snapshot.complete = true;
        } else if !snapshot.complete && snapshot.ready_sent {
            snapshot.status_text = "Waiting for players".to_string();
        }
        if status.players.is_empty() {
            mark_ready_players(snapshot, &status.ready_player_ids, status.ready_players);
        } else {
            let (light_players, dark_players) =
                loading_players_from_status(&status.players, &manifest);
            snapshot.light_players = light_players;
            snapshot.dark_players = dark_players;
        }
    });
}

#[derive(Component)]
struct LoadingScreenRoot;

#[derive(Component)]
struct LoadingProgressFill;

#[derive(Component)]
struct LoadingProgressBadgeText;

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
    asset_server: Res<AssetServer>,
    mut ui_images: ResMut<Assets<Image>>,
) {
    if !settings.ui_enabled {
        return;
    }

    let fallback_wallpaper = asset_server.load("wallpapers/lira-loading.jpg");
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
                TextLayout::new_with_justify(Justify::Center),
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
        TextLayout::new_with_justify(Justify::Center),
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
                        TextLayout::new_with_justify(Justify::Center),
                    ),
                ],
            ),
        ],
    )
}

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
        Query<(&mut Text, &mut TextColor), With<LoadingPingText>>,
    )>,
    mut ping_spinners: Query<
        (&mut UiTransform, &mut BorderColor),
        (With<LoadingPingSpinner>, Without<LoadingCard>),
    >,
) {
    let snapshot = state.snapshot();
    let visible = snapshot.active && !snapshot.complete;
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
        if let Some(progress_badge_image) = progress_badge_image.as_deref() {
            if let Some(image) = ui_images.get_mut(progress_badge_image.handle.id()) {
                *image = progress_badge_shape_image(accent);
            }
        }
        *last_progress_accent = Some(accent);
    }

    for (mut node, mut background) in &mut layout_nodes.p1() {
        node.width = percent(snapshot.progress_percent());
        *background = BackgroundColor(accent);
    }

    for (card, mut node, mut border) in &mut layout_nodes.p2() {
        let player = loading_card_player(&snapshot, card.team, card.index);
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
            loading_card_player(&snapshot, card_image.team, card_image.index),
        ) {
            image_node.image = loading_wallpaper(images, player.champion).clone();
        }
    }

    for (avatar, mut background) in &mut layout_nodes.p3() {
        if loading_card_player(&snapshot, avatar.team, avatar.index).is_some() {
            *background = BackgroundColor(accent);
        }
    }

    for (avatar, mut node, mut image_node) in &mut layout_nodes.p5() {
        let avatar_handle = loading_card_player(&snapshot, avatar.team, avatar.index)
            .and_then(|player| player.avatar_url.as_deref())
            .and_then(|avatar_url| {
                loading_avatar_handle(
                    avatar_url,
                    &asset_server,
                    &mut *avatar_cache,
                    &mut *ui_images,
                )
            });

        if let Some(handle) = avatar_handle {
            image_node.image = handle;
            node.display = Display::Flex;
        } else {
            node.display = Display::None;
        }
    }

    let progress = snapshot.progress_percent().round().clamp(0.0, 100.0) as u32;
    for (mut text, mut color) in &mut text_queries.p1() {
        text.0 = format!("{progress}%");
        *color = TextColor(accent_foreground_for(accent));
    }

    let ping_color = ping_color(&network_ping);
    let ping_text = ping_text(&network_ping);
    for (mut text, mut color) in &mut text_queries.p2() {
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
        if let Some(player) = loading_card_player(&snapshot, text_marker.team, text_marker.index) {
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
        match entry {
            LoadingAvatarEntry::Ready(handle) => return Some(handle.clone()),
            LoadingAvatarEntry::Failed => return None,
            LoadingAvatarEntry::Loading(receiver) => {
                let received = receiver
                    .lock()
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
                        return None;
                    }
                    Ok(Err(error)) => {
                        warn!(
                            "Failed to load loading-screen avatar from '{}': {}",
                            source, error
                        );
                        *entry = LoadingAvatarEntry::Failed;
                        return None;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        *entry = LoadingAvatarEntry::Failed;
                        return None;
                    }
                }
            }
        }
    }

    if source.starts_with("http://") || source.starts_with("https://") {
        let (sender, receiver) = channel();
        let owned_source = source.to_string();
        std::thread::spawn(move || {
            let _ = sender.send(download_avatar(&owned_source));
        });
        cache.entries.insert(
            source.to_string(),
            LoadingAvatarEntry::Loading(Arc::new(Mutex::new(receiver))),
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
    if let Some(content_type) = content_type {
        if let Ok(image) = avatar_image_from_buffer(bytes, ImageType::MimeType(content_type)) {
            return Some(image);
        }
    }

    if let Some(extension) = image_extension(source) {
        if let Ok(image) = avatar_image_from_buffer(bytes, ImageType::Extension(extension)) {
            return Some(image);
        }
    }

    ["png", "jpg", "jpeg", "webp"]
        .into_iter()
        .find_map(|extension| avatar_image_from_buffer(bytes, ImageType::Extension(extension)).ok())
}

fn avatar_image_from_buffer(
    bytes: &[u8],
    image_type: ImageType,
) -> Result<Image, bevy::image::TextureError> {
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
    snapshot: &LoadingScreenSnapshot,
    team: LoadingTeam,
    index: usize,
) -> Option<&LoadingPlayer> {
    match team {
        LoadingTeam::Light => snapshot.light_players.get(index),
        LoadingTeam::Dark => snapshot.dark_players.get(index),
    }
}

fn loading_wallpaper(images: &LoadingScreenImages, champion: ChampionId) -> &Handle<Image> {
    match champion.0 {
        6607 => &images.ignara,
        6608 => &images.yuna,
        6609 => &images.sophia,
        _ => &images.lira,
    }
}

fn mark_ready_players(
    snapshot: &mut LoadingScreenSnapshot,
    ready_player_ids: &[u64],
    ready_players: usize,
) {
    if !ready_player_ids.is_empty() {
        for player in snapshot
            .dark_players
            .iter_mut()
            .chain(snapshot.light_players.iter_mut())
        {
            player.ready = ready_player_ids.contains(&player.public_id);
        }
        return;
    }

    let mut remaining = ready_players;
    for player in snapshot
        .dark_players
        .iter_mut()
        .chain(snapshot.light_players.iter_mut())
    {
        player.ready = remaining > 0;
        remaining = remaining.saturating_sub(1);
    }
}

fn loading_players_from_status(
    status_players: &[LoadingScreenPlayer],
    manifest: &ClientLoadingMatchManifest,
) -> (Vec<LoadingPlayer>, Vec<LoadingPlayer>) {
    let mut light_players = Vec::new();
    let mut dark_players = Vec::new();

    for player in status_players {
        let manifest_player = manifest.players.get(&player.player_id);
        let loading_player = LoadingPlayer {
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
        };

        match player.team {
            TeamSpec::Light => light_players.push(loading_player),
            TeamSpec::Dark => dark_players.push(loading_player),
            TeamSpec::Neutral => light_players.push(loading_player),
        }
    }

    light_players.sort_by_key(|player| player.public_id);
    dark_players.sort_by_key(|player| player.public_id);
    (light_players, dark_players)
}

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
    match champion.0 {
        6607 => "Ignara",
        6608 => "Yuna",
        6609 => "Sophia",
        _ => "Lira",
    }
}

fn public_display_name(value: &str) -> Option<String> {
    let without_email_domain = value.trim().split('@').next().unwrap_or("").trim();
    let public_name = without_email_domain
        .split(|character: char| character.is_whitespace() || matches!(character, '.' | '_' | '-'))
        .find(|part| !part.trim().is_empty())?
        .trim();

    non_empty_string(public_name)
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();

    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
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
    let fill = rgba_bytes(Color::srgba(0.027, 0.035, 0.055, 0.94));
    let border = rgba_bytes(accent);

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

fn rgba_bytes(color: Color) -> [u8; 4] {
    let srgba = color.to_srgba();
    [
        channel_to_byte(srgba.red),
        channel_to_byte(srgba.green),
        channel_to_byte(srgba.blue),
        channel_to_byte(srgba.alpha),
    ]
}

fn channel_to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn update_snapshot(
    shared: &Arc<Mutex<LoadingScreenSnapshot>>,
    update: impl FnOnce(&mut LoadingScreenSnapshot),
) {
    let mut snapshot = shared.lock().unwrap_or_else(|error| error.into_inner());
    update(&mut snapshot);
}

impl LoadingScreenState {
    fn snapshot(&self) -> LoadingScreenSnapshot {
        self.shared
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn is_visible(&self) -> bool {
        let snapshot = self.snapshot();
        snapshot.active && !snapshot.complete
    }
}

impl LoadingScreenSnapshot {
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

    #[test]
    fn calculates_progress_per_player() {
        let snapshot = LoadingScreenSnapshot {
            active: true,
            complete: false,
            wallpaper_assets_ready: true,
            status_text: String::new(),
            client_progress_percent: 100.0,
            client_ready: true,
            ready_sent: true,
            ready_players: 1,
            total_players: 2,
            dark_players: Vec::new(),
            light_players: Vec::new(),
        };

        assert_eq!(snapshot.progress_percent(), 50.0);
    }

    #[test]
    fn uses_local_progress_before_display_ready_is_sent() {
        let snapshot = LoadingScreenSnapshot {
            active: true,
            complete: false,
            wallpaper_assets_ready: true,
            status_text: String::new(),
            client_progress_percent: 42.0,
            client_ready: false,
            ready_sent: false,
            ready_players: 2,
            total_players: 2,
            dark_players: Vec::new(),
            light_players: Vec::new(),
        };

        assert_eq!(snapshot.progress_percent(), 42.0);
    }

    #[test]
    fn trims_loading_display_names_to_public_first_part() {
        assert_eq!(
            public_display_name("Exepta Mustermann").as_deref(),
            Some("Exepta")
        );
        assert_eq!(
            public_display_name("exepta.profile").as_deref(),
            Some("exepta")
        );
        assert_eq!(
            public_display_name("exepta@example.com").as_deref(),
            Some("exepta")
        );
        assert_eq!(public_display_name("   ").as_deref(), None);
    }
}
