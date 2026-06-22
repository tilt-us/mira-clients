use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// Description:
/// Defines the team affiliation used by gameplay entities.
///
/// Fields:
/// - `Neutral`: Entity has no playable team affiliation.
/// - `Dark`: Entity belongs to the dark team.
/// - `Light`: Entity belongs to the light team.
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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// Description:
/// Stores the team assigned to an entity.
///
/// Fields:
/// - `0`: Team affiliation value.
pub struct Team(pub TeamSpec);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Description:
/// Stores scoreboard values for a team.
///
/// Fields:
/// - `kills`: Total champion kills.
/// - `objectives`: Total objective captures.
/// - `structures`: Total structure destructions.
pub struct TeamScore {
    pub kills: u16,
    pub objectives: u16,
    pub structures: u16,
}
