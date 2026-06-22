use bevy::prelude::*;

use crate::network::ChampionId;

use super::team::{Team, TeamSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Description:
/// Identifies a player entity in gameplay systems.
///
/// Fields:
/// - `0`: Stable numeric player id.
pub struct PlayerId(pub u64);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Description:
/// Stores the player id assigned to a gameplay entity.
///
/// Fields:
/// - `id`: Stable player id.
pub struct Player {
    pub id: PlayerId,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Description:
/// Stores the champion content id assigned to a gameplay entity.
///
/// Fields:
/// - `0`: Stable champion id shared by client, server, and content files.
pub struct Champion(pub ChampionId);

#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
/// Description:
/// Stores display metadata for a player entity.
///
/// Fields:
/// - `display_name`: Name shown for the player.
pub struct PlayerProfile {
    pub display_name: String,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
/// Description:
/// Stores current and maximum health for a gameplay entity.
///
/// Fields:
/// - `current`: Current health value.
/// - `max`: Maximum health value.
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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
/// Description:
/// Stores current and maximum mana for a gameplay entity.
///
/// Fields:
/// - `current`: Current mana value.
/// - `max`: Maximum mana value.
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

#[derive(Component, Debug, Clone, Copy, PartialEq)]
/// Description:
/// Stores ground movement speed for a gameplay entity.
///
/// Fields:
/// - `0`: Movement speed in world units per second.
pub struct MoveSpeed(pub f32);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
/// Description:
/// Stores the entity facing angle around the vertical axis.
///
/// Fields:
/// - `radians`: Yaw angle in radians.
pub struct Facing {
    pub radians: f32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
/// Description:
/// Stores the active movement destination for a gameplay entity.
///
/// Fields:
/// - `position`: World-space destination position.
/// - `stop_distance`: Distance at which the destination is considered reached.
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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Description:
/// Marks the locally controlled player entity.
pub struct PlayerControlled;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
/// Description:
/// Marks an entity as selectable and attackable by gameplay systems.
///
/// Fields:
/// - `radius`: World-space targeting radius.
pub struct Targetable {
    pub radius: f32,
}

#[derive(Bundle, Debug, Clone)]
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
