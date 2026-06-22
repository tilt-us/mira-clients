pub mod content;
pub mod control_api;
pub mod match_manifest;
pub mod network;
pub mod plugins;
pub mod states;

use bevy::prelude::*;
use control_api::ServerControlApiSettings;
use match_manifest::ServerMatchManifest;
use network::ServerNetworkSettings;
use plugins::ServerPlugins;

/// Description:
/// Builds and runs the dedicated server Bevy app.
///
/// Params:
/// - `network_settings`: Dedicated server networking settings parsed at startup.
/// - `control_api_settings`: REST control API settings for loading state calls.
pub fn run(
    network_settings: ServerNetworkSettings,
    control_api_settings: ServerControlApiSettings,
) {
    let match_manifest = ServerMatchManifest::load_from_environment();
    control_api::spawn(control_api_settings, match_manifest.clone());

    App::new()
        .insert_resource(network_settings)
        .insert_resource(match_manifest)
        .add_plugins(ServerPlugins)
        .run();
}
