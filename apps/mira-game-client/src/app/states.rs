use bevy::prelude::*;

/// Description:
/// Defines the high-level lifecycle state of the playable client app.
///
/// Fields:
/// - `Boot`: Initial state before loading begins.
/// - `Loading`: State used while assets and content are being prepared.
/// - `MainMenu`: State used by the future menu flow.
/// - `Connecting`: State used while connecting to a dedicated server.
/// - `InGame`: State used while the match view is active.
/// - `Paused`: State used when the local match view is paused.
#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
#[allow(dead_code)]
pub enum ClientState {
    #[default]
    Boot,
    Loading,
    MainMenu,
    Connecting,
    InGame,
    Paused,
}
