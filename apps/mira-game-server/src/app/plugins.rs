use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use game_logic::MiraGameplaySystemsPlugin;
use game_shared::MiraSharedPlugin;

use super::content::ServerContentPlugin;
use super::network::ServerNetworkPlugin;
use super::states::ServerState;

/// Description:
/// Registers the dedicated server plugin stack.
pub struct ServerPlugins;

impl Plugin for ServerPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins(MinimalPlugins)
            .add_plugins(LogPlugin::default())
            .add_plugins(StatesPlugin)
            .init_state::<ServerState>()
            .add_plugins((
                MiraSharedPlugin,
                ServerContentPlugin,
                ServerNetworkPlugin,
                MiraGameplaySystemsPlugin,
            ));
    }
}
