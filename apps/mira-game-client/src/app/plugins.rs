use bevy::asset::AssetMetaCheck;
use bevy::feathers::{FeathersPlugins, dark_theme::create_dark_theme, theme::UiTheme};
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode, WindowResolution};
use bevy_transform_interpolation::prelude::TransformInterpolationPlugin;
use game_logic::{MiraClientSystemsPlugin, MiraGameplaySystemsPlugin, OverheadHealthBarStyle};
use game_shared::MiraSharedPlugin;
use game_shared::network::FIXED_TIMESTEP_HZ;
use game_world::MiraWorldPlugin;

use super::settings::{
    ClientAppSettings, ClientLaunchGate, ClientLaunchSettings, ClientScreenMode,
};
use super::states::ClientState;
use crate::app::leave_menu::LeaveMenuPlugin;
use crate::app::loading_screen::LoadingScreenPlugin;
use crate::app::main_hud::MainHudPlugin;
use crate::network::ClientNetworkPlugin;

/// Description:
/// Registers the playable client plugin stack.
pub struct ClientAppPlugins;

impl Plugin for ClientAppPlugins {
    fn build(&self, app: &mut App) {
        let settings = ClientAppSettings::default();
        let asset_root = settings.asset_root.clone();
        let asset_root_text = asset_root.to_string_lossy().into_owned();
        let ui_enabled = settings.ui_enabled;
        let screen_mode = app
            .world()
            .get_resource::<ClientLaunchSettings>()
            .map(|settings| settings.screen_mode)
            .unwrap_or_default();
        let health_bar_style = app
            .world()
            .get_resource::<ClientLaunchSettings>()
            .map(|settings| OverheadHealthBarStyle {
                accent_color: settings.accent_color_bevy(),
            })
            .unwrap_or_default();
        let launch_blocked = app
            .world()
            .get_resource::<ClientLaunchGate>()
            .and_then(ClientLaunchGate::blocked_message)
            .is_some();

        app.insert_resource(Time::<Fixed>::from_hz(FIXED_TIMESTEP_HZ))
            .insert_resource(health_bar_style)
            .insert_resource(settings)
            .add_plugins(
                DefaultPlugins
                    .set(WindowPlugin {
                        primary_window: Some(Window {
                            title: "mira-game-client".to_string(),
                            resolution: WindowResolution::new(1920, 1080),
                            mode: bevy_window_mode(screen_mode),
                            ..default()
                        }),
                        ..default()
                    })
                    .set(AssetPlugin {
                        file_path: asset_root_text.clone(),
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
                MiraSharedPlugin,
                ClientNetworkPlugin,
                MiraWorldPlugin,
                MiraGameplaySystemsPlugin,
                MiraClientSystemsPlugin,
                LoadingScreenPlugin,
            ));

        if ui_enabled {
            app.add_systems(Startup, setup_ui_camera)
                .add_plugins((MainHudPlugin, LeaveMenuPlugin));
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
            order: 100,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        IsDefaultUiCamera,
    ));
}

struct BlockedLaunchScreenPlugin;

impl Plugin for BlockedLaunchScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_blocked_launch_screen);
    }
}

#[derive(Component)]
struct BlockedLaunchScreenRoot;

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
        ZIndex(20_000),
        Pickable::IGNORE,
        children![
            (
                Text::new("MIRA"),
                TextFont::from_font_size(28.0),
                TextColor(Color::srgb_u8(0xF2, 0xC4, 0x5B)),
                TextLayout::new_with_justify(Justify::Center),
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
                    TextLayout::new_with_justify(Justify::Center),
                )],
            ),
        ],
    ));
}
