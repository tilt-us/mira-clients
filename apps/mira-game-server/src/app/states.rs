use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
#[allow(dead_code)]
/// Description:
/// Defines the high-level lifecycle state of the dedicated server app.
///
/// Fields:
/// - `Boot`: Initial state before loading begins.
/// - `Loading`: State used while server data is loading.
/// - `Running`: State used while the server is actively simulating.
/// - `ShuttingDown`: State used while the server is shutting down.
pub enum ServerState {
    #[default]
    Boot,
    Loading,
    Running,
    ShuttingDown,
}
