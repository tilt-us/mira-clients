use bevy::prelude::*;

pub mod game;
pub mod network;

/// Number of player-controlled champions assigned to each non-neutral team.
///
/// Used by match setup, champion select, spawning, and future lobby validation.
pub const PLAYERS_PER_TEAM: usize = 5;

/// Registers the shared domain plugin for client and server apps.
pub struct MiraSharedPlugin;

impl Plugin for MiraSharedPlugin {
    fn build(&self, _app: &mut App) {}
}
