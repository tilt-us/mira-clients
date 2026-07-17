use bevy::app::*;

mod setup;

/// Registers world setup for a playable client scene.
///
/// Description:
/// Used by the client app after Bevy's default rendering and asset plugins are
/// available. The dedicated server does not register this plugin during the
/// current bootstrap phase because this plugin spawns renderable test geometry.
pub struct MiraWorldPlugin;

impl Plugin for MiraWorldPlugin {
    /// Registers Bevy resources, plugins, or systems for the world scene plugin.
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup::setup_flat_map);
    }
}
