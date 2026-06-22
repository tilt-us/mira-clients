use super::settings::{ClientAppSettings, ClientLaunchSettings};
use crate::ui_components::loading_screen_component::{LoadingPlayerCardStore, LoadingScreenStore};
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy_extended_ui::{
    ImageCache, framework::UiBindingStore, routing::Router,
    services::style_service::apply_calc_styles_system, styles::CssID,
};
use game_shared::game::team::TeamSpec;
use game_shared::network::{
    ChampionId, DisplayReady, LoadingScreenPlayer, LoadingScreenStatus, ReliableCommandChannel,
};
use lightyear::prelude::*;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MINIMUM_CLIENT_LOADING_DURATION: Duration = Duration::from_secs(5);
const LOADING_SCREEN_WALLPAPERS: [&str; 4] = [
    "wallpapers/lira-loading.jpg",
    "wallpapers/ignara-loading.jpg",
    "wallpapers/yuna-loading.jpg",
    "wallpapers/sophia-loading.jpg",
];
const LOADING_SCREEN_ROOT_ID: &str = "loading-screen-root";
const LOADING_PROGRESS_FILL_ID: &str = "loading-screen-progress-fill";
const LIGHT_CARD_IDS: [&str; 5] = [
    "loading-light-player-0-card",
    "loading-light-player-1-card",
    "loading-light-player-2-card",
    "loading-light-player-3-card",
    "loading-light-player-4-card",
];
const DARK_CARD_IDS: [&str; 5] = [
    "loading-dark-player-0-card",
    "loading-dark-player-1-card",
    "loading-dark-player-2-card",
    "loading-dark-player-3-card",
    "loading-dark-player-4-card",
];

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

#[derive(Resource, Debug, Default)]
struct LoadingScreenStoreCache(Option<LoadingScreenStore>);

#[derive(Resource, Debug)]
struct LoadingScreenReadyGate {
    minimum_timer: Timer,
    last_wait_log_second: Option<u32>,
}

#[derive(Resource, Debug, Default)]
struct LoadingScreenServerStatusCache(Option<(usize, usize, Vec<u64>, bool)>);

#[derive(Resource, Debug, Default)]
struct LoadingScreenRouteCache {
    loading_active: Option<bool>,
}

#[derive(Resource, Debug, Default)]
struct LoadingScreenWallpaperPreload {
    handles: Vec<Handle<Image>>,
    expected_count: usize,
}

/// Mirrors the loading-screen state into the Extended UI loading component.
pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadingScreenStoreCache>()
            .init_resource::<LoadingScreenReadyGate>()
            .init_resource::<LoadingScreenServerStatusCache>()
            .init_resource::<LoadingScreenRouteCache>()
            .init_resource::<LoadingScreenWallpaperPreload>()
            .add_systems(
                PreStartup,
                preload_loading_screen_wallpapers.run_if(resource_exists::<ImageCache>),
            )
            .add_systems(Update, update_loading_screen_wallpaper_status)
            .add_systems(Update, update_loading_screen_ready_gate)
            .add_systems(Update, send_display_ready)
            .add_systems(Update, receive_loading_screen_status)
            .add_systems(
                Update,
                sync_loading_screen_store.run_if(resource_exists::<UiBindingStore>),
            )
            .add_systems(
                PostUpdate,
                sync_loading_screen_nodes
                    .after(apply_calc_styles_system)
                    .run_if(resource_exists::<UiBindingStore>),
            )
            .add_systems(
                PostUpdate,
                sync_loading_screen_route.run_if(resource_exists::<Router>),
            );
    }
}

fn preload_loading_screen_wallpapers(
    settings: Res<ClientAppSettings>,
    mut images: ResMut<Assets<Image>>,
    mut image_cache: ResMut<ImageCache>,
    mut preload: ResMut<LoadingScreenWallpaperPreload>,
    state: Res<LoadingScreenState>,
) {
    preload.expected_count = LOADING_SCREEN_WALLPAPERS.len();

    for path in LOADING_SCREEN_WALLPAPERS {
        let fs_path = settings.asset_root.join(path);
        let Ok(bytes) = fs::read(&fs_path) else {
            warn!(
                "Failed to preload loading screen wallpaper '{}': file not found at '{}'.",
                path,
                fs_path.display()
            );
            continue;
        };

        let Ok(image) = Image::from_buffer(
            &bytes,
            ImageType::Extension("jpg"),
            CompressedImageFormats::empty(),
            true,
            ImageSampler::default(),
            RenderAssetUsages::default(),
        ) else {
            warn!(
                "Failed to decode loading screen wallpaper '{}'.",
                fs_path.display()
            );
            continue;
        };

        let handle = images.add(image);
        image_cache.map.insert(path.to_string(), handle.clone());
        preload.handles.push(handle);
    }

    let ready = preload.handles.len() == preload.expected_count;
    update_snapshot(&state.shared, |snapshot| {
        snapshot.wallpaper_assets_ready = ready;
        if ready && snapshot.status_text == "Loading match art" {
            snapshot.status_text = "Loading local arena".to_string();
        }
    });

    info!(
        "Preloaded {}/{} loading screen wallpapers into the UI image cache.",
        preload.handles.len(),
        preload.expected_count
    );
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

    info!(
        "Loading screen wallpaper preload ready: {}",
        wallpapers_ready
    );
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
        info!("Loading screen local gate ready; sending DisplayReady on the next network tick.");
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
        if gate.last_wait_log_second != Some(remaining_seconds) {
            info!(
                "Loading screen waiting locally: minimum_timer_remaining={}s scene_assets_ready={} wallpaper_assets_ready={}",
                remaining_seconds, scene_assets_ready, wallpaper_assets_ready
            );
            gate.last_wait_log_second = Some(remaining_seconds);
        }
        format!("Loading local arena ({}s)", remaining_seconds)
    } else {
        if gate.last_wait_log_second != Some(0) {
            info!(
                "Loading screen waiting locally: minimum timer done, scene_assets_ready={} wallpaper_assets_ready={}",
                scene_assets_ready, wallpaper_assets_ready
            );
            gate.last_wait_log_second = Some(0);
        }
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
    info!("Loading screen sent DisplayReady to the server.");

    update_snapshot(&state.shared, |snapshot| {
        snapshot.ready_sent = true;
        snapshot.client_progress_percent = 100.0;
        snapshot.status_text = "Waiting for players".to_string();
    });
}

fn receive_loading_screen_status(
    state: Res<LoadingScreenState>,
    mut status_cache: ResMut<LoadingScreenServerStatusCache>,
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

    let status_signature = (
        status.ready_players,
        status.total_players,
        status.ready_player_ids.clone(),
        status.can_close,
    );
    if status_cache.0.as_ref() != Some(&status_signature) {
        info!(
            "Loading screen received server status: ready={}/{} ids={:?} can_close={}",
            status.ready_players, status.total_players, status.ready_player_ids, status.can_close
        );
        status_cache.0 = Some(status_signature);
    }

    update_snapshot(&state.shared, |snapshot| {
        snapshot.ready_players = status.ready_players;
        snapshot.total_players = status.total_players.max(1);
        snapshot.status_text = if status.can_close {
            "Entering arena".to_string()
        } else if snapshot.ready_sent {
            "Waiting for players".to_string()
        } else {
            snapshot.status_text.clone()
        };
        snapshot.complete = status.can_close;
        if status.players.is_empty() {
            mark_ready_players(snapshot, &status.ready_player_ids, status.ready_players);
        } else {
            let (light_players, dark_players) = loading_players_from_status(&status.players);
            snapshot.light_players = light_players;
            snapshot.dark_players = dark_players;
        }
    });
}

fn sync_loading_screen_store(
    state: Res<LoadingScreenState>,
    mut cache: ResMut<LoadingScreenStoreCache>,
    mut ui_store: ResMut<UiBindingStore>,
) {
    let snapshot = state.snapshot();
    let next_store = loading_screen_store_from_snapshot(&snapshot);

    if cache.0.as_ref() == Some(&next_store) {
        return;
    }

    ui_store.set_store(next_store.clone());
    cache.0 = Some(next_store);
}

fn sync_loading_screen_nodes(
    launch_settings: Res<ClientLaunchSettings>,
    state: Res<LoadingScreenState>,
    mut nodes: Query<(
        &CssID,
        &mut Node,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
        Option<&mut Visibility>,
    )>,
) {
    let snapshot = state.snapshot();
    let root_display = if snapshot.active && !snapshot.complete {
        Display::Flex
    } else {
        Display::None
    };
    let progress_width = Val::Percent(snapshot.progress_percent());
    let accent_color = launch_settings.accent_color_bevy();

    for (css_id, mut node, background, _border, visibility) in &mut nodes {
        match css_id.0.as_str() {
            LOADING_SCREEN_ROOT_ID if node.display != root_display => {
                node.display = root_display;
                if let Some(mut visibility) = visibility {
                    *visibility = if root_display == Display::None {
                        Visibility::Hidden
                    } else {
                        Visibility::Visible
                    };
                }
            }
            LOADING_PROGRESS_FILL_ID => {
                if node.width != progress_width {
                    node.width = progress_width;
                }

                if let Some(mut background) = background {
                    *background = BackgroundColor(accent_color);
                }
            }
            css_id if LIGHT_CARD_IDS.contains(&css_id) => {
                let index = LIGHT_CARD_IDS
                    .iter()
                    .position(|slot_id| slot_id == &css_id)
                    .unwrap_or_default();
                if snapshot.light_players.get(index).is_some() {
                    show_loading_card(&mut node, visibility);
                } else {
                    hide_loading_card(&mut node, visibility);
                }
            }
            css_id if DARK_CARD_IDS.contains(&css_id) => {
                let index = DARK_CARD_IDS
                    .iter()
                    .position(|slot_id| slot_id == &css_id)
                    .unwrap_or_default();
                if snapshot.dark_players.get(index).is_some() {
                    show_loading_card(&mut node, visibility);
                } else {
                    hide_loading_card(&mut node, visibility);
                }
            }
            _ => {}
        }
    }
}

fn sync_loading_screen_route(
    state: Res<LoadingScreenState>,
    mut route_cache: ResMut<LoadingScreenRouteCache>,
    mut router: ResMut<Router>,
) {
    let snapshot = state.snapshot();
    let loading_active = snapshot.active && !snapshot.complete;
    if route_cache.loading_active == Some(loading_active) {
        return;
    }

    route_cache.loading_active = Some(loading_active);
    if loading_active {
        router.navigate("/");
    } else {
        router.navigate("/hud");
    }
}

fn loading_screen_store_from_snapshot(snapshot: &LoadingScreenSnapshot) -> LoadingScreenStore {
    let loading_progress_text = if snapshot.ready_sent {
        format!(
            "{}/{} ready - {}%",
            snapshot.ready_players,
            snapshot.total_players,
            snapshot.progress_percent().round().clamp(0.0, 100.0) as u32
        )
    } else {
        format!(
            "Local loading - {}%",
            snapshot.progress_percent().round().clamp(0.0, 100.0) as u32
        )
    };

    LoadingScreenStore {
        loading_title: "Match Loading".to_string(),
        loading_subtitle: "Teams are preparing for the arena".to_string(),
        loading_progress_text,
        loading_status_text: snapshot.status_text.clone(),
        dark_players: player_card_stores(&snapshot.dark_players),
        light_players: player_card_stores(&snapshot.light_players),
        dark_team_count: format!("{}/5", snapshot.dark_players.len().min(5)),
        light_team_count: format!("{}/5", snapshot.light_players.len().min(5)),
        dark_player_0_initial: player_slot_initial(&snapshot.dark_players, 0),
        dark_player_0_name: player_slot_name(&snapshot.dark_players, 0),
        dark_player_0_champion: player_slot_champion(&snapshot.dark_players, 0),
        dark_player_0_champion_class: player_slot_champion_class(&snapshot.dark_players, 0),
        dark_player_0_state: player_slot_state(&snapshot.dark_players, 0),
        dark_player_1_initial: player_slot_initial(&snapshot.dark_players, 1),
        dark_player_1_name: player_slot_name(&snapshot.dark_players, 1),
        dark_player_1_champion: player_slot_champion(&snapshot.dark_players, 1),
        dark_player_1_champion_class: player_slot_champion_class(&snapshot.dark_players, 1),
        dark_player_1_state: player_slot_state(&snapshot.dark_players, 1),
        dark_player_2_initial: player_slot_initial(&snapshot.dark_players, 2),
        dark_player_2_name: player_slot_name(&snapshot.dark_players, 2),
        dark_player_2_champion: player_slot_champion(&snapshot.dark_players, 2),
        dark_player_2_champion_class: player_slot_champion_class(&snapshot.dark_players, 2),
        dark_player_2_state: player_slot_state(&snapshot.dark_players, 2),
        dark_player_3_initial: player_slot_initial(&snapshot.dark_players, 3),
        dark_player_3_name: player_slot_name(&snapshot.dark_players, 3),
        dark_player_3_champion: player_slot_champion(&snapshot.dark_players, 3),
        dark_player_3_champion_class: player_slot_champion_class(&snapshot.dark_players, 3),
        dark_player_3_state: player_slot_state(&snapshot.dark_players, 3),
        dark_player_4_initial: player_slot_initial(&snapshot.dark_players, 4),
        dark_player_4_name: player_slot_name(&snapshot.dark_players, 4),
        dark_player_4_champion: player_slot_champion(&snapshot.dark_players, 4),
        dark_player_4_champion_class: player_slot_champion_class(&snapshot.dark_players, 4),
        dark_player_4_state: player_slot_state(&snapshot.dark_players, 4),
        light_player_0_initial: player_slot_initial(&snapshot.light_players, 0),
        light_player_0_name: player_slot_name(&snapshot.light_players, 0),
        light_player_0_champion: player_slot_champion(&snapshot.light_players, 0),
        light_player_0_champion_class: player_slot_champion_class(&snapshot.light_players, 0),
        light_player_0_state: player_slot_state(&snapshot.light_players, 0),
        light_player_1_initial: player_slot_initial(&snapshot.light_players, 1),
        light_player_1_name: player_slot_name(&snapshot.light_players, 1),
        light_player_1_champion: player_slot_champion(&snapshot.light_players, 1),
        light_player_1_champion_class: player_slot_champion_class(&snapshot.light_players, 1),
        light_player_1_state: player_slot_state(&snapshot.light_players, 1),
        light_player_2_initial: player_slot_initial(&snapshot.light_players, 2),
        light_player_2_name: player_slot_name(&snapshot.light_players, 2),
        light_player_2_champion: player_slot_champion(&snapshot.light_players, 2),
        light_player_2_champion_class: player_slot_champion_class(&snapshot.light_players, 2),
        light_player_2_state: player_slot_state(&snapshot.light_players, 2),
        light_player_3_initial: player_slot_initial(&snapshot.light_players, 3),
        light_player_3_name: player_slot_name(&snapshot.light_players, 3),
        light_player_3_champion: player_slot_champion(&snapshot.light_players, 3),
        light_player_3_champion_class: player_slot_champion_class(&snapshot.light_players, 3),
        light_player_3_state: player_slot_state(&snapshot.light_players, 3),
        light_player_4_initial: player_slot_initial(&snapshot.light_players, 4),
        light_player_4_name: player_slot_name(&snapshot.light_players, 4),
        light_player_4_champion: player_slot_champion(&snapshot.light_players, 4),
        light_player_4_champion_class: player_slot_champion_class(&snapshot.light_players, 4),
        light_player_4_state: player_slot_state(&snapshot.light_players, 4),
    }
}

fn show_loading_card(node: &mut Node, visibility: Option<Mut<Visibility>>) {
    node.display = Display::Flex;
    node.width = Val::Px(198.0);
    node.height = Val::Px(286.0);
    node.min_width = Val::Px(198.0);
    if let Some(mut visibility) = visibility {
        *visibility = Visibility::Visible;
    }
}

fn hide_loading_card(node: &mut Node, visibility: Option<Mut<Visibility>>) {
    node.display = Display::Flex;
    node.width = Val::Px(0.0);
    node.height = Val::Px(0.0);
    node.min_width = Val::Px(0.0);
    if let Some(mut visibility) = visibility {
        *visibility = Visibility::Hidden;
    }
}

fn player_card_stores(players: &[LoadingPlayer]) -> Vec<LoadingPlayerCardStore> {
    players
        .iter()
        .take(5)
        .map(|player| LoadingPlayerCardStore {
            initial: initials(&player.name),
            name: player.name.clone(),
            champion: player.champion_name.clone(),
            champion_class: champion_class(player.champion).to_string(),
            state: if player.ready {
                "Ready".to_string()
            } else {
                "Loading".to_string()
            },
        })
        .collect()
}

fn player_slot_initial(players: &[LoadingPlayer], index: usize) -> String {
    players
        .get(index)
        .map(|player| initials(&player.name))
        .unwrap_or_default()
}

fn player_slot_name(players: &[LoadingPlayer], index: usize) -> String {
    players
        .get(index)
        .map(|player| player.name.clone())
        .unwrap_or_default()
}

fn player_slot_champion(players: &[LoadingPlayer], index: usize) -> String {
    players
        .get(index)
        .map(|player| player.champion_name.clone())
        .unwrap_or_default()
}

fn player_slot_champion_class(players: &[LoadingPlayer], index: usize) -> String {
    players
        .get(index)
        .map(|player| champion_class(player.champion).to_string())
        .unwrap_or_default()
}

fn player_slot_state(players: &[LoadingPlayer], index: usize) -> String {
    players
        .get(index)
        .map(|player| {
            if player.ready {
                "Ready".to_string()
            } else {
                "Loading".to_string()
            }
        })
        .unwrap_or_default()
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
) -> (Vec<LoadingPlayer>, Vec<LoadingPlayer>) {
    let mut light_players = Vec::new();
    let mut dark_players = Vec::new();

    for player in status_players {
        let loading_player = LoadingPlayer {
            public_id: player.player_id,
            name: player
                .display_name
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| format!("Player #{}", player.player_id)),
            avatar_url: player.avatar_url.clone(),
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

fn champion_name(champion: ChampionId) -> &'static str {
    match champion.0 {
        6607 => "Ignara",
        6608 => "Yuna",
        6609 => "Sophia",
        _ => "Lira",
    }
}

fn champion_class(champion: ChampionId) -> &'static str {
    match champion.0 {
        6607 => "loading-card-ignara",
        6608 => "loading-card-yuna",
        6609 => "loading-card-sophia",
        _ => "loading-card-lira",
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
            last_wait_log_second: None,
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
}
