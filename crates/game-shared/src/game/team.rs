use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Defines the team affiliation used by gameplay entities.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TeamSpec {
    #[default]
    Neutral,
    Dark,
    Light,
}

impl TeamSpec {
    /// Returns whether the team is playable.
    pub fn is_playable(self) -> bool {
        matches!(self, Self::Dark | Self::Light)
    }
    /// Returns the opposing playable team, or `None` for neutral.
    pub fn opponent(self) -> Option<Self> {
        match self {
            Self::Dark => Some(Self::Light),
            Self::Light => Some(Self::Dark),
            Self::Neutral => None,
        }
    }
}
/// Stores the team assigned to an entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Team(pub TeamSpec);
/// Stores scoreboard values for a team.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TeamScore {
    pub kills: u16,
    pub objectives: u16,
    pub structures: u16,
}
