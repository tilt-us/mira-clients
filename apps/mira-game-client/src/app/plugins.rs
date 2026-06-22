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

use super::settings::{ClientAppSettings, ClientLaunchSettings, ClientScreenMode};
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
            .init_state::<ClientState>()
            .add_plugins(TransformInterpolationPlugin::default())
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
