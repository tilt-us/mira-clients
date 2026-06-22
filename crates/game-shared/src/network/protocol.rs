use crate::game::team::TeamSpec;
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

/// Description:
/// Marker channel for reliable client command messages.
pub struct ReliableCommandChannel;

/// Description:
/// Marker channel for frequent player state snapshots where only the latest packet matters.
pub struct PlayerStateChannel;

/// Description:
/// Identifies a champion definition shared by client and server.
///
/// Fields:
/// - `0`: Numeric champion id matching the champion data asset.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChampionId(pub u32);

/// Description:
/// Identifies one champion ability slot.
///
/// Fields:
/// - `Q`: First basic ability slot.
/// - `W`: Second basic ability slot.
/// - `E`: Third basic ability slot.
/// - `R`: Ultimate ability slot.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbilitySlot {
    Q,
    W,
    E,
    R,
}

/// Description:
/// Stores a serializable world-space position for network messages.
///
/// Fields:
/// - `x`: World-space X coordinate.
/// - `y`: World-space Y coordinate.
/// - `z`: World-space Z coordinate.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct WorldPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Vec3> for WorldPosition {
    fn from(value: Vec3) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
        }
    }
}

impl From<WorldPosition> for Vec3 {
    fn from(value: WorldPosition) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

/// Description:
/// Describes a world-space ability cast target.
///
/// Fields:
/// - `position`: Optional target position for ground-targeted abilities.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct CastTarget {
    pub position: Option<WorldPosition>,
}

/// Description:
/// Describes a visible ability cast that other clients should render.
///
/// Fields:
/// - `caster_player_id`: Player id of the casting player.
/// - `champion`: Champion that cast the ability.
/// - `slot`: Ability slot that was cast.
/// - `start`: World-space cast origin.
/// - `end`: Optional world-space target or projectile end position.
/// - `visual`: Server-authoritative visual timing and scale values for the cast.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct AbilityVisualEvent {
    pub caster_player_id: u64,
    pub champion: ChampionId,
    pub slot: AbilitySlot,
    pub start: WorldPosition,
    pub end: Option<WorldPosition>,
    pub visual: AbilityVisualTuning,
}

/// Description:
/// Stores server-authoritative visual tuning attached to an accepted ability cast.
///
/// Fields:
/// - `travel_seconds`: Travel duration used by projectile visuals.
/// - `projectile_radius`: Radius used to render projectile visuals.
/// - `explosion_radius`: Radius used to render area impact visuals.
/// - `missile_count`: Number of missile visuals to spawn.
/// - `missile_lifetime_seconds`: Lifetime used by missile visuals.
/// - `missile_search_radius`: Search radius used by missile visuals.
/// - `missile_orbit_radius`: Orbit radius used by missile visuals.
/// - `missile_orbit_height`: Orbit height used by missile visuals.
/// - `missile_orbit_speed`: Orbit speed used by missile visuals.
/// - `missile_chase_speed`: Chase speed used by missile visuals.
/// - `missile_radius`: Radius used to render missile visuals.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub struct AbilityVisualTuning {
    pub travel_seconds: f32,
    pub projectile_radius: f32,
    pub explosion_radius: f32,
    pub missile_count: u16,
    pub missile_lifetime_seconds: f32,
    pub missile_search_radius: f32,
    pub missile_orbit_radius: f32,
    pub missile_orbit_height: f32,
    pub missile_orbit_speed: f32,
    pub missile_chase_speed: f32,
    pub missile_radius: f32,
}

/// Description:
/// Sends server-authoritative champion data from the match server to clients.
///
/// Fields:
/// - `champions`: Champion definitions currently known by the match server.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ChampionCatalogUpdate {
    pub champions: Vec<NetworkChampionDefinition>,
}

/// Description:
/// Describes one champion definition shared by the match server.
///
/// Fields:
/// - `id`: Stable champion id used by gameplay messages.
/// - `name`: Display name used by content and diagnostics.
/// - `stats`: Server-authoritative stats and ability tuning.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkChampionDefinition {
    pub id: ChampionId,
    pub name: String,
    pub stats: NetworkChampionStats,
}

/// Description:
/// Stores server-authoritative stats and ability tuning for one champion.
///
/// Fields:
/// - `base_stats`: Base stats used by the authoritative simulation.
/// - `abilities`: Ability tuning used by the authoritative simulation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkChampionStats {
    pub base_stats: NetworkChampionBaseStats,
    pub abilities: NetworkChampionAbilities,
}

/// Description:
/// Stores base stats for one champion.
///
/// Fields:
/// - `max_health`: Maximum health assigned by the match server.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct NetworkChampionBaseStats {
    pub max_health: f32,
}

/// Description:
/// Stores ability tuning for one champion.
///
/// Fields:
/// - `q`: Tuning for the first basic ability.
/// - `w`: Tuning for the second basic ability.
/// - `e`: Tuning for the third basic ability.
/// - `r`: Optional tuning for the ultimate ability.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkChampionAbilities {
    pub q: NetworkAbilityDefinition,
    pub w: NetworkAbilityDefinition,
    pub e: NetworkAbilityDefinition,
    #[serde(default)]
    pub r: Option<NetworkAbilityDefinition>,
}

/// Description:
/// Stores server-authoritative ability tuning for one ability slot.
///
/// Fields:
/// - `damage`: Damage values applied by this ability.
/// - `cooldown_seconds`: Cooldown duration applied by the match server.
/// - `range`: Maximum cast or search range in world units.
/// - `travel_seconds`: Travel duration for projectile-style ability simulations.
/// - `projectile_height`: Height offset used for projectile spawn positions.
/// - `projectile_radius`: Collision radius used by projectile hit tests.
/// - `target_height`: Height offset used for target or landing positions.
/// - `explosion_radius`: Radius used by area damage checks.
/// - `missile_count`: Number of contact missiles spawned by missile-style abilities.
/// - `missile_lifetime_seconds`: Lifetime of contact missiles.
/// - `missile_search_radius`: Search radius used by contact missiles.
/// - `missile_orbit_radius`: Orbit radius used by contact missiles.
/// - `missile_orbit_height`: Orbit height used by contact missiles.
/// - `missile_orbit_speed`: Orbit speed used by contact missiles.
/// - `missile_chase_speed`: Chase speed used by contact missiles.
/// - `missile_radius`: Collision radius used by contact missiles.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
#[serde(default)]
pub struct NetworkAbilityDefinition {
    pub damage: NetworkAbilityDamage,
    pub cooldown_seconds: f32,
    pub range: f32,
    pub travel_seconds: f32,
    pub projectile_height: f32,
    pub projectile_radius: f32,
    pub target_height: f32,
    pub explosion_radius: f32,
    pub missile_count: usize,
    pub missile_lifetime_seconds: f32,
    pub missile_search_radius: f32,
    pub missile_orbit_radius: f32,
    pub missile_orbit_height: f32,
    pub missile_orbit_speed: f32,
    pub missile_chase_speed: f32,
    pub missile_radius: f32,
    pub width: f32,
    pub lifetime_seconds: f32,
    pub target_radius: f32,
    pub damage_per_second: f32,
    pub pull_speed: f32,
    pub move_speed_multiplier: f32,
    pub heal: f32,
    pub stun_seconds: f32,
    pub slow_seconds: f32,
    pub speed_seconds: f32,
    pub damage_multiplier: f32,
    pub small_distance: f32,
    pub medium_distance: f32,
    pub small_damage: f32,
    pub medium_damage: f32,
    pub large_damage: f32,
}

/// Description:
/// Stores server-authoritative damage values for one ability.
///
/// Fields:
/// - `direct_hit`: Damage applied by direct projectile or contact hits.
/// - `area`: Damage applied by area explosions or impact zones.
/// - `missile`: Damage applied by individual homing/contact missiles.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
#[serde(default)]
pub struct NetworkAbilityDamage {
    pub direct_hit: f32,
    pub area: f32,
    pub missile: f32,
}

/// Description:
/// Describes one player currently known by the authoritative server.
///
/// Fields:
/// - `player_id`: Stable network player id assigned from the client connection.
/// - `champion`: Champion selected by the player.
/// - `team`: Team assigned by the current match/lobby.
/// - `position`: Server-assigned development spawn position.
/// - `yaw`: Current facing angle around the Y axis.
/// - `moving`: Whether the player is currently moving.
/// - `health`: Current health value used by client-side stand-ins.
/// - `max_health`: Maximum health value used by client-side stand-ins.
/// - `alive`: Whether the player can currently move and cast.
/// - `stunned`: Whether the player is currently unable to move or cast.
/// - `control_locked`: Whether the server is currently overriding local movement.
/// - `move_speed_multiplier`: Current server-authoritative movement speed multiplier.
/// - `pull_center`: Optional world-space pull center currently affecting this player.
/// - `respawn_generation`: Monotonic counter incremented each time the player respawns.
/// - `respawn_seconds`: Remaining server-authoritative respawn time in seconds.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkPlayer {
    pub player_id: u64,
    pub champion: ChampionId,
    pub team: TeamSpec,
    pub position: WorldPosition,
    pub yaw: f32,
    pub moving: bool,
    pub health: f32,
    pub max_health: f32,
    pub alive: bool,
    pub stunned: bool,
    pub control_locked: bool,
    pub move_speed_multiplier: f32,
    pub pull_center: Option<WorldPosition>,
    pub respawn_generation: u32,
    pub respawn_seconds: f32,
}

/// Description:
/// Sends the current lightweight match roster from the server to one client.
///
/// Fields:
/// - `local_player_id`: Player id of the receiving client.
/// - `players`: Players currently connected to the development server.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MatchSnapshot {
    pub local_player_id: u64,
    pub players: Vec<NetworkPlayer>,
}

/// Description:
/// Sends the local player's current visual state to the server.
///
/// Fields:
/// - `position`: Current local player world-space position.
/// - `yaw`: Current local player facing angle around the Y axis.
/// - `moving`: Whether the local player is currently moving.
/// - `champion`: Champion selected by this client.
/// - `team`: Team selected by this client.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct PlayerStateUpdate {
    pub position: WorldPosition,
    pub yaw: f32,
    pub moving: bool,
    pub champion: ChampionId,
    pub team: TeamSpec,
}

/// Description:
/// Sent by a client once its local display has loaded enough to enter the match.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayReady;

/// Description:
/// Sent by a client before it intentionally leaves the running match.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientLeave;

/// Description:
/// Sends the server-side loading-screen state to clients.
///
/// Fields:
/// - `ready_players`: Number of clients that have sent `DisplayReady`.
/// - `total_players`: Number of players expected for the match.
/// - `ready_player_ids`: Netcode player ids that are ready.
/// - `players`: Players that should be rendered on the loading screen.
/// - `can_close`: Whether every expected player is ready.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LoadingScreenStatus {
    pub ready_players: usize,
    pub total_players: usize,
    pub ready_player_ids: Vec<u64>,
    pub players: Vec<LoadingScreenPlayer>,
    pub can_close: bool,
}

/// Description:
/// Describes one player card for the server-driven loading screen.
///
/// Fields:
/// - `player_id`: Public player id used by networking and diagnostics.
/// - `display_name`: Optional launcher-provided display name.
/// - `avatar_url`: Optional launcher-provided avatar URL.
/// - `champion`: Champion shown on the card.
/// - `team`: Team row where the card should be placed.
/// - `ready`: Whether the player has sent `DisplayReady`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LoadingScreenPlayer {
    pub player_id: u64,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub champion: ChampionId,
    pub team: TeamSpec,
    pub ready: bool,
}

/// Description:
/// Describes an input command sent by a client to the authoritative server.
///
/// Fields:
/// - `MoveTo`: Requests movement toward a world-space point.
/// - `CastAbility`: Requests an ability cast for the given champion and slot.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum PlayerCommand {
    MoveTo(WorldPosition),
    CastAbility {
        champion: ChampionId,
        slot: AbilitySlot,
        target: CastTarget,
    },
}

/// Description:
/// Registers the shared Lightyear protocol used by client and server.
pub struct SharedNetworkPlugin;

impl Plugin for SharedNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.register_message::<PlayerCommand>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<AbilityVisualEvent>()
            .add_direction(NetworkDirection::Bidirectional);

        app.register_message::<PlayerStateUpdate>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<DisplayReady>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<ClientLeave>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<MatchSnapshot>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<LoadingScreenStatus>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<ChampionCatalogUpdate>()
            .add_direction(NetworkDirection::ServerToClient);

        app.add_channel::<ReliableCommandChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);

        app.add_channel::<PlayerStateChannel>(ChannelSettings {
            mode: ChannelMode::SequencedUnreliable,
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);
    }
}
