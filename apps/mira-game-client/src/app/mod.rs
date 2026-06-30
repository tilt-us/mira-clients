pub mod leave_menu;
pub mod loading_screen;
pub mod main_hud;
pub mod plugins;
pub mod settings;
pub mod states;

use bevy::prelude::*;
use loading_screen::loading_screen_state;
use plugins::ClientAppPlugins;
use settings::{ClientLaunchGate, ClientLaunchSettings};

use crate::network::ClientNetworkSettings;

/// Description:
/// Builds and runs the playable client Bevy app.
///
/// Params:
/// - `launch_settings`: Matchmaking launch parameters parsed at startup.
/// - `network_settings`: Client networking settings parsed at startup.
/// - `launch_gate`: Startup validation result used to decide whether gameplay may load.
pub fn run(
    launch_settings: ClientLaunchSettings,
    network_settings: ClientNetworkSettings,
    launch_gate: ClientLaunchGate,
) {
    App::new()
        .insert_resource(loading_screen_state(&launch_settings))
        .insert_resource(launch_gate)
        .insert_resource(launch_settings)
        .insert_resource(network_settings)
        .add_plugins(ClientAppPlugins)
        .run();
}
