use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_extended_ui::{
    ExtendedCam, ExtendedUiConfiguration, ExtendedUiPlugin,
    framework::ExtendedFrameworkConfiguration,
};
use bevy_transform_interpolation::prelude::TransformInterpolationPlugin;
use game_logic::{MiraClientSystemsPlugin, MiraGameplaySystemsPlugin};
use game_shared::MiraSharedPlugin;
use game_shared::network::FIXED_TIMESTEP_HZ;
use game_world::MiraWorldPlugin;

use super::settings::ClientAppSettings;
use super::states::ClientState;
use crate::app::loading_screen::LoadingScreenPlugin;
use crate::network::ClientNetworkPlugin;
use crate::ui_components::loading_screen_component::LoadingScreenComponentPlugin;
use crate::ui_components::main_component::MainComponentPlugin;

/// Description:
/// Registers the playable client plugin stack.
pub struct ClientAppPlugins;

impl Plugin for ClientAppPlugins {
    fn build(&self, app: &mut App) {
        let settings = ClientAppSettings::default();
        let asset_root = settings.asset_root.clone();
        let asset_root_text = asset_root.to_string_lossy().into_owned();
        let ui_enabled = settings.ui_enabled;

        app.insert_resource(Time::<Fixed>::from_hz(FIXED_TIMESTEP_HZ))
            .insert_resource(settings)
            .add_plugins(
                DefaultPlugins
                    .set(WindowPlugin {
                        primary_window: Some(Window {
                            title: "mira-game-client".to_string(),
                            resolution: WindowResolution::new(1920, 1080),
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
            let component_root = asset_root.join("components").to_string_lossy().into_owned();

            app.insert_resource(ExtendedUiConfiguration {
                assets_path: asset_root
                    .join("extended_ui")
                    .to_string_lossy()
                    .into_owned(),
                camera: ExtendedCam::Simple,
                ..default()
            })
            .insert_resource(ExtendedFrameworkConfiguration {
                asset_root_fs_path: asset_root_text,
                assets_component_root: "components".to_string(),
                rust_component_root: component_root,
                index_html_file: "index.html".to_string(),
            })
            .add_plugins((
                ExtendedUiPlugin,
                MainComponentPlugin,
                LoadingScreenComponentPlugin,
            ));
        }
    }
}
