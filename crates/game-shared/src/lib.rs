use bevy::prelude::*;

pub mod game;
pub mod network;

/// Number of player-controlled champions assigned to each non-neutral team.
///
/// Used by match setup, champion select, spawning, and future lobby validation.
pub const PLAYERS_PER_TEAM: usize = 5;

/// Registers shared type setup for both client and server apps.
///
/// Description:
/// Used as the first domain plugin before systems that read shared components,
/// protocol messages, or gameplay data.
pub struct MiraSharedPlugin;

impl Plugin for MiraSharedPlugin {
    fn build(&self, _app: &mut App) {}
}
