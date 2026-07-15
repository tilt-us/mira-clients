use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Description:
/// Defines the team affiliation used by gameplay entities.
///
/// Fields:
/// - `Neutral`: Entity has no playable team affiliation.
/// - `Dark`: Entity belongs to the dark team.
/// - `Light`: Entity belongs to the light team.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TeamSpec {
    #[default]
    Neutral,
    Dark,
    Light,
}

impl TeamSpec {
    /// Description:
    /// Checks whether the team is one of the playable teams.
    ///
    /// Params:
    /// - `self`: Team spec to inspect.
    ///
    /// Return:
    /// - `true` for `Dark` or `Light`.
    pub fn is_playable(self) -> bool {
        matches!(self, Self::Dark | Self::Light)
    }

    /// Description:
    /// Returns the opposing playable team.
    ///
    /// Params:
    /// - `self`: Team spec to inspect.
    ///
    /// Return:
    /// - The opposing team for playable teams, or `None` for neutral.
    pub fn opponent(self) -> Option<Self> {
        match self {
            Self::Dark => Some(Self::Light),
            Self::Light => Some(Self::Dark),
            Self::Neutral => None,
        }
    }
}

/// Description:
/// Stores the team assigned to an entity.
///
/// Fields:
/// - `0`: Team affiliation value.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Team(pub TeamSpec);

/// Description:
/// Stores scoreboard values for a team.
///
/// Fields:
/// - `kills`: Total champion kills.
/// - `objectives`: Total objective captures.
/// - `structures`: Total structure destructions.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TeamScore {
    pub kills: u16,
    pub objectives: u16,
    pub structures: u16,
}
