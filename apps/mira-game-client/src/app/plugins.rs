use crate::{MiraClientGameplaySettings, MiraClientSystemsPlugin, OverheadHealthBarStyle};
use bevy::app::AppExit;
use bevy::asset::AssetMetaCheck;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::feathers::{FeathersPlugins, dark_theme::create_dark_theme, theme::UiTheme};
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::{
    MonitorSelection, PresentMode, VideoModeSelection, WindowMode, WindowResolution,
};
use bevy_transform_interpolation::prelude::TransformInterpolationPlugin;
use mira_game_api::MiraGameApiPlugin;
use mira_game_api::network::FIXED_TIMESTEP_HZ;

use super::settings::{
    ClientAppSettings, ClientLaunchGate, ClientLaunchSettings, ClientScreenMode,
};
use super::states::ClientState;
use super::world::MiraWorldPlugin;
use crate::app::leave_menu::LeaveMenuPlugin;
use crate::app::loading_screen::LoadingScreenPlugin;
use crate::app::main_hud::MainHudPlugin;
use crate::network::ClientNetworkPlugin;

const BLOCKED_LAUNCH_WINDOW_WIDTH: u32 = 960;
const BLOCKED_LAUNCH_WINDOW_HEIGHT: u32 = 540;
const PLAYABLE_WINDOW_WIDTH: u32 = 1_920;
const PLAYABLE_WINDOW_HEIGHT: u32 = 1_080;
const UI_CAMERA_RENDER_ORDER: isize = 100;
const BLOCKED_LAUNCH_SCREEN_Z_INDEX: i32 = 20_000;

/// Registers the playable client plugin stack.
pub struct ClientAppPlugins;

impl Plugin for ClientAppPlugins {
    fn build(&self, app: &mut App) {
        let app_settings = ClientAppSettings::default();
        let asset_root_path = app_settings.asset_root.to_string_lossy().into_owned();
        let ui_enabled = app_settings.ui_enabled;
        if let Some(error) = app_settings.asset_root_error.clone() {
            error!("{error}");
            app.insert_resource(ClientLaunchGate::Blocked { message: error });
        }
        let (screen_mode, health_bar_style, gameplay_settings) = app
            .world()
            .get_resource::<ClientLaunchSettings>()
            .map(|launch_settings| {
                (
                    launch_settings.screen_mode,
                    OverheadHealthBarStyle {
                        accent_color: launch_settings.accent_color_bevy(),
                    },
                    MiraClientGameplaySettings {
                        allow_dev_dummy_toggle: launch_settings.dev_preview,
                        ..default()
                    },
                )
            })
            .unwrap_or_else(|| {
                (
                    ClientScreenMode::default(),
                    OverheadHealthBarStyle::default(),
                    MiraClientGameplaySettings::default(),
                )
            });
        let launch_blocked = app
            .world()
            .get_resource::<ClientLaunchGate>()
            .and_then(ClientLaunchGate::blocked_message)
            .is_some();
        let window_resolution = if launch_blocked {
            WindowResolution::new(BLOCKED_LAUNCH_WINDOW_WIDTH, BLOCKED_LAUNCH_WINDOW_HEIGHT)
        } else {
            WindowResolution::new(PLAYABLE_WINDOW_WIDTH, PLAYABLE_WINDOW_HEIGHT)
        };
        let window_mode = if launch_blocked {
            WindowMode::Windowed
        } else {
            bevy_window_mode(screen_mode)
        };

        app.insert_resource(Time::<Fixed>::from_hz(FIXED_TIMESTEP_HZ))
            .insert_resource(health_bar_style)
            .insert_resource(gameplay_settings)
            .insert_resource(app_settings)
            .add_plugins(
                DefaultPlugins
                    .set(WindowPlugin {
                        primary_window: Some(Window {
                            title: "mira-game-client".to_string(),
                            resolution: window_resolution,
                            mode: window_mode,
                            present_mode: PresentMode::AutoNoVsync,
                            ..default()
                        }),
                        ..default()
                    })
                    .set(AssetPlugin {
                        file_path: asset_root_path,
                        meta_check: AssetMetaCheck::Never,
                        ..default()
                    }),
            )
            .add_plugins(FeathersPlugins)
            .insert_resource(UiTheme(create_dark_theme()))
            .init_state::<ClientState>();

        if launch_blocked {
            app.add_systems(Startup, setup_ui_camera)
                .add_plugins(BlockedLaunchScreenPlugin);
            return;
        }

        app.add_plugins(TransformInterpolationPlugin::default())
            .add_plugins((
                MiraGameApiPlugin,
                ClientNetworkPlugin,
                MiraWorldPlugin,
                MiraClientSystemsPlugin,
                LoadingScreenPlugin,
            ));

        if ui_enabled {
            app.add_systems(Startup, setup_ui_camera).add_plugins((
                FrameTimeDiagnosticsPlugin::default(),
                MainHudPlugin,
                LeaveMenuPlugin,
            ));
        }
    }
}

fn bevy_window_mode(screen_mode: ClientScreenMode) -> WindowMode {
    match screen_mode {
        ClientScreenMode::Full => {
            WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current)
        }
        ClientScreenMode::Window => WindowMode::Windowed,
        ClientScreenMode::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
    }
}

fn setup_ui_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: UI_CAMERA_RENDER_ORDER,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        IsDefaultUiCamera,
    ));
}

struct BlockedLaunchScreenPlugin;

impl Plugin for BlockedLaunchScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_blocked_launch_screen)
            .add_systems(Update, handle_blocked_launch_close_button);
    }
}

#[derive(Component)]
struct BlockedLaunchScreenRoot;

#[derive(Component)]
struct BlockedLaunchCloseButton;

fn spawn_blocked_launch_screen(mut commands: Commands, launch_gate: Res<ClientLaunchGate>) {
    let Some(message) = launch_gate.blocked_message() else {
        return;
    };

    commands.spawn((
        BlockedLaunchScreenRoot,
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
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: px(20),
            padding: UiRect::all(px(28)),
            ..default()
        },
        BackgroundColor(Color::srgb_u8(0x0B, 0x10, 0x18)),
        ZIndex(BLOCKED_LAUNCH_SCREEN_Z_INDEX),
        children![
            (
                Text::new("MIRA"),
                TextFont::from_font_size(28.0),
                TextColor(Color::srgb_u8(0xF2, 0xC4, 0x5B)),
                TextLayout::justify(Justify::Center),
            ),
            (
                Node {
                    width: px(760),
                    max_width: percent(86),
                    display: Display::Flex,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                children![(
                    Text::new(message.to_string()),
                    TextFont::from_font_size(22.0),
                    TextColor(Color::srgb_u8(0xED, 0xF2, 0xF7)),
                    TextLayout::justify(Justify::Center),
                )],
            ),
            (
                Button,
                BlockedLaunchCloseButton,
                Node {
                    width: px(156),
                    height: px(44),
                    display: Display::Flex,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::srgb_u8(0xF2, 0xC4, 0x5B)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.22)),
                children![(
                    Text::new("Close"),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb_u8(0x0B, 0x10, 0x18)),
                    TextLayout::justify(Justify::Center),
                )],
            ),
        ],
    ));
}

fn handle_blocked_launch_close_button(
    mut interactions: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<Button>,
            With<BlockedLaunchCloseButton>,
        ),
    >,
    mut app_exit: MessageWriter<AppExit>,
) {
    for interaction in &mut interactions {
        if *interaction == Interaction::Pressed {
            app_exit.write(AppExit::Success);
        }
    }
}
