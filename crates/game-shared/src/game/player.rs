use bevy::prelude::*;

use crate::network::ChampionId;

use super::team::{Team, TeamSpec};

/// Description:
/// Identifies a player entity in gameplay systems.
///
/// Fields:
/// - `0`: Stable numeric player id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u64);

/// Description:
/// Stores the player id assigned to a gameplay entity.
///
/// Fields:
/// - `id`: Stable player id.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Player {
    pub id: PlayerId,
}

/// Description:
/// Stores the champion content id assigned to a gameplay entity.
///
/// Fields:
/// - `0`: Stable champion id shared by client, server, and content files.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Champion(pub ChampionId);

/// Description:
/// Stores display metadata for a player entity.
///
/// Fields:
/// - `display_name`: Name shown for the player.
#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerProfile {
    pub display_name: String,
}

/// Description:
/// Stores current and maximum health for a gameplay entity.
///
/// Fields:
/// - `current`: Current health value.
/// - `max`: Maximum health value.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

impl Health {
    /// Description:
    /// Creates a full health component with current health equal to maximum health.
    ///
    /// Params:
    /// - `max`: Maximum health value.
    ///
    /// Return:
    /// - A new full `Health` component.
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }

    /// Description:
    /// Checks whether the entity still has positive health.
    ///
    /// Params:
    /// - `self`: Health component to inspect.
    ///
    /// Return:
    /// - `true` when current health is greater than zero.
    pub fn is_alive(self) -> bool {
        self.current > 0
    }
}

/// Description:
/// Stores current and maximum mana for a gameplay entity.
///
/// Fields:
/// - `current`: Current mana value.
/// - `max`: Maximum mana value.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mana {
    pub current: u32,
    pub max: u32,
}

impl Mana {
    /// Description:
    /// Creates a full mana component with the current mana equal to maximum mana.
    ///
    /// Params:
    /// - `max`: Maximum mana value.
    ///
    /// Return:
    /// - A new full `Mana` component.
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }
}

/// Description:
/// Stores ground movement speed for a gameplay entity.
///
/// Fields:
/// - `0`: Movement speed in world units per second.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MoveSpeed(pub f32);

/// Description:
/// Stores the entity facing angle around the vertical axis.
///
/// Fields:
/// - `radians`: Yaw angle in radians.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Facing {
    pub radians: f32,
}

/// Description:
/// Stores the active movement destination for a gameplay entity.
///
/// Fields:
/// - `position`: World-space destination position.
/// - `stop_distance`: Distance at which the destination is considered reached.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MoveTarget {
    pub position: Vec3,
    pub stop_distance: f32,
}

impl MoveTarget {
    /// Description:
    /// Creates a movement target with the default stop distance.
    ///
    /// Params:
    /// - `position`: World-space destination position.
    ///
    /// Return:
    /// - A new `MoveTarget` component.
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            stop_distance: 0.25,
        }
    }
}

/// Description:
/// Marks the locally controlled player entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerControlled;

/// Description:
/// Marks an entity as selectable and attackable by gameplay systems.
///
/// Fields:
/// - `radius`: World-space targeting radius.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Targetable {
    pub radius: f32,
}

/// Description:
/// Bundles core gameplay components for a player entity.
///
/// Fields:
/// - `player`: Player id component.
/// - `team`: Team affiliation component.
/// - `health`: Health component.
/// - `mana`: Mana component.
/// - `move_speed`: Movement speed component.
/// - `controlled`: Local control marker component.
#[derive(Bundle, Debug, Clone)]
pub struct PlayerBundle {
    pub player: Player,
    pub team: Team,
    pub health: Health,
    pub mana: Mana,
    pub move_speed: MoveSpeed,
    pub controlled: PlayerControlled,
}

impl PlayerBundle {
    /// Description:
    /// Creates a player bundle with default combat and movement stats.
    ///
    /// Params:
    /// - `id`: Player id assigned to the bundle.
    /// - `team`: Team assigned to the bundle.
    ///
    /// Return:
    /// - A configured `PlayerBundle`.
    pub fn new(id: PlayerId, team: TeamSpec) -> Self {
        Self {
            player: Player { id },
            team: Team(team),
            health: Health::new(100),
            mana: Mana::new(100),
            move_speed: MoveSpeed(6.0),
            controlled: PlayerControlled,
        }
    }
}

/// Returns the first non-empty public name segment after removing an email domain.
pub fn public_display_name(value: &str) -> Option<String> {
    let without_email_domain = value.trim().split('@').next().unwrap_or("").trim();
    let public_name = without_email_domain
        .split(|character: char| character.is_whitespace() || matches!(character, '.' | '_' | '-'))
        .find(|part| !part.trim().is_empty())?
        .trim();

    non_empty_string(public_name)
}

/// Returns a trimmed owned string when the input contains non-whitespace characters.
pub fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();

    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_public_display_names() {
        assert_eq!(
            public_display_name("Exepta Mustermann").as_deref(),
            Some("Exepta")
        );
        assert_eq!(
            public_display_name("exepta.profile").as_deref(),
            Some("exepta")
        );
        assert_eq!(
            public_display_name("exepta@example.com").as_deref(),
            Some("exepta")
        );
        assert_eq!(public_display_name("   ").as_deref(), None);
    }

    #[test]
    fn trims_non_empty_strings() {
        assert_eq!(
            non_empty_string("  avatar.png  ").as_deref(),
            Some("avatar.png")
        );
        assert_eq!(non_empty_string("\t \n").as_deref(), None);
    }
}
