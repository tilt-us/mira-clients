use bevy::prelude::*;

use crate::network::ChampionId;

use super::team::{Team, TeamSpec};

/// Default maximum health assigned to a newly created player.
pub const DEFAULT_PLAYER_HEALTH: u32 = 100;

/// Default maximum mana assigned to a newly created player.
pub const DEFAULT_PLAYER_MANA: u32 = 100;

/// Default movement speed assigned to a newly created player.
pub const DEFAULT_PLAYER_MOVEMENT_SPEED: f32 = 6.0;

/// Default distance from a move target at which movement stops.
pub const DEFAULT_MOVE_TARGET_STOP_DISTANCE: f32 = 0.25;

/// Identifies a player entity in gameplay systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u64);
/// Stores the player id assigned to a gameplay entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Player {
    pub id: PlayerId,
}
/// Stores the champion content id assigned to a gameplay entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Champion(pub ChampionId);
/// Stores display metadata for a player entity.
#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerProfile {
    pub display_name: String,
}
/// Stores current and maximum health for a gameplay entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

impl Health {
    /// Creates a full health component.
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }
    /// Returns whether the entity has positive health.
    pub fn is_alive(self) -> bool {
        self.current > 0
    }
}
/// Stores current and maximum mana for a gameplay entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mana {
    pub current: u32,
    pub max: u32,
}

impl Mana {
    /// Creates a full mana component.
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }
}
/// Stores ground movement speed for a gameplay entity.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MoveSpeed(pub f32);
/// Stores the entity facing angle around the vertical axis.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Facing {
    pub radians: f32,
}
/// Stores the active movement destination for a gameplay entity.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MoveTarget {
    pub position: Vec3,
    pub stop_distance: f32,
}

impl MoveTarget {
    /// Creates a movement target with the default stop distance.
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            stop_distance: DEFAULT_MOVE_TARGET_STOP_DISTANCE,
        }
    }
}
/// Marks the locally controlled player entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerControlled;
/// Marks an entity as selectable and attackable by gameplay systems.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Targetable {
    pub radius: f32,
}
/// Bundles core gameplay components for a player entity.
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
    /// Creates a player bundle with default combat and movement stats.
    pub fn new(id: PlayerId, team: TeamSpec) -> Self {
        Self {
            player: Player { id },
            team: Team(team),
            health: Health::new(DEFAULT_PLAYER_HEALTH),
            mana: Mana::new(DEFAULT_PLAYER_MANA),
            move_speed: MoveSpeed(DEFAULT_PLAYER_MOVEMENT_SPEED),
            controlled: PlayerControlled,
        }
    }
}

/// Returns the first non-empty public name segment after removing an email domain.
pub fn public_display_name(input: &str) -> Option<String> {
    let without_email_domain = input.trim().split('@').next().unwrap_or("").trim();
    let public_name = without_email_domain
        .split(|character: char| character.is_whitespace() || matches!(character, '.' | '_' | '-'))
        .find(|part| !part.trim().is_empty())?
        .trim();

    non_empty_string(public_name)
}

/// Returns a trimmed owned string when the input contains non-whitespace characters.
pub fn non_empty_string(input: &str) -> Option<String> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
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
