use crate::game::{lane::LaneUnitKind, team::TeamSpec};
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};
/// Marker channel for reliable client command messages.
pub struct ReliableCommandChannel;
/// Marker channel for frequent player state snapshots where only the latest packet matters.
pub struct PlayerStateChannel;

/// Identifies a champion definition shared by client and server.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChampionId(pub u32);

#[derive(Clone, Copy)]
struct PrototypeChampionMetadata {
    id: ChampionId,
    display_name: &'static str,
    asset_slug: &'static str,
}

static PROTOTYPE_CHAMPIONS: [PrototypeChampionMetadata; 4] = [
    PrototypeChampionMetadata {
        id: ChampionId::LIRA,
        display_name: "Lira",
        asset_slug: "lira",
    },
    PrototypeChampionMetadata {
        id: ChampionId::IGNARA,
        display_name: "Ignara",
        asset_slug: "ignara",
    },
    PrototypeChampionMetadata {
        id: ChampionId::YUNA,
        display_name: "Yuna",
        asset_slug: "yuna",
    },
    PrototypeChampionMetadata {
        id: ChampionId::SOPHIA,
        display_name: "Sophia",
        asset_slug: "sophia",
    },
];

impl ChampionId {
    /// Lira's stable champion identifier.
    pub const LIRA: Self = Self(6606);

    /// Ignara's stable champion identifier.
    pub const IGNARA: Self = Self(6607);

    /// Yuna's stable champion identifier.
    pub const YUNA: Self = Self(6608);

    /// Sophia's stable champion identifier.
    pub const SOPHIA: Self = Self(6609);

    /// Champions currently supported by the prototype client and server.
    pub const PROTOTYPE_ROSTER: [Self; 4] = [Self::LIRA, Self::IGNARA, Self::YUNA, Self::SOPHIA];

    /// Returns the display name for a supported prototype champion.
    pub fn display_name(self) -> Option<&'static str> {
        self.prototype_metadata()
            .map(|metadata| metadata.display_name)
    }

    /// Returns the local asset directory for a supported prototype champion.
    pub fn asset_slug(self) -> Option<&'static str> {
        self.prototype_metadata()
            .map(|metadata| metadata.asset_slug)
    }

    /// Resolves a prototype champion from its numeric id or display name.
    pub fn from_selector(selector: &str) -> Option<Self> {
        let selector = selector.trim();
        let numeric_id = selector.parse::<u32>().ok();

        PROTOTYPE_CHAMPIONS
            .iter()
            .find(|metadata| {
                metadata.display_name.eq_ignore_ascii_case(selector)
                    || numeric_id == Some(metadata.id.0)
            })
            .map(|metadata| metadata.id)
    }

    fn prototype_metadata(self) -> Option<&'static PrototypeChampionMetadata> {
        PROTOTYPE_CHAMPIONS
            .iter()
            .find(|metadata| metadata.id == self)
    }
}
/// Identifies one champion ability slot.
///
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
/// Stores a serializable world-space position for network messages.
///
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
/// Describes a world-space ability cast target.
///
/// - `position`: Optional target position for ground-targeted abilities.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct CastTarget {
    pub position: Option<WorldPosition>,
}
/// Describes a visible ability cast that other clients should render.
///
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
/// Identifies a server-authoritative combat target.
#[derive(Component, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkTargetId {
    /// A player identified by its stable network player id.
    Player(u64),
    /// A tower, Nexus, or minion identified by its stable lane-unit id.
    LaneUnit(u64),
}

/// Describes one accepted auto-attack projectile that other clients should render.
///
/// - `caster_player_id`: Player id of the attacking player.
/// - `target`: Stable identifier for the attacked player or lane unit.
/// - `start`: World-space projectile start.
/// - `end`: World-space projectile end.
/// - `travel_seconds`: Projectile travel duration used by clients.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct AutoAttackVisualEvent {
    pub caster_player_id: u64,
    pub target: NetworkTargetId,
    pub start: WorldPosition,
    pub end: WorldPosition,
    pub travel_seconds: f32,
}

/// Describes one accepted ranged-minion attack projectile that clients should render.
///
/// - `source_unit_id`: Stable lane-unit id of the attacking ranged minion.
/// - `team`: Team that owns the attacking minion and determines the projectile color.
/// - `target`: Stable identifier for the attacked player, tower, or minion.
/// - `start`: World-space projectile start.
/// - `end`: World-space projectile end.
/// - `travel_seconds`: Projectile travel duration used by clients.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct RangedMinionAutoAttackVisualEvent {
    pub source_unit_id: u64,
    pub team: TeamSpec,
    pub target: NetworkTargetId,
    pub start: WorldPosition,
    pub end: WorldPosition,
    pub travel_seconds: f32,
}
/// Describes the gameplay source of a server-authoritative combat number.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkCombatNumberKind {
    AutoAttack,
    Spell,
    Heal,
}
/// Sends one server-authoritative floating combat number to clients.
///
/// - `target_player_id`: Player id above which the number should be shown.
/// - `amount`: Positive damage or healing amount to render.
/// - `kind`: Source classification used for text sign and color.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct NetworkCombatNumberEvent {
    pub target_player_id: u64,
    pub amount: f32,
    pub kind: NetworkCombatNumberKind,
}
/// Stores server-authoritative visual tuning attached to an accepted ability cast.
///
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
/// Sends server-authoritative champion data from the match server to clients.
///
/// - `champions`: Champion definitions currently known by the match server.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ChampionCatalogUpdate {
    pub champions: Vec<NetworkChampionDefinition>,
}
/// Describes one champion definition shared by the match server.
///
/// - `id`: Stable champion id used by gameplay messages.
/// - `name`: Display name used by content and diagnostics.
/// - `stats`: Server-authoritative stats and ability tuning.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkChampionDefinition {
    pub id: ChampionId,
    pub name: String,
    pub stats: NetworkChampionStats,
}
/// Stores server-authoritative stats and ability tuning for one champion.
///
/// - `base_stats`: Base stats used by the authoritative simulation.
/// - `abilities`: Ability tuning used by the authoritative simulation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkChampionStats {
    pub base_stats: NetworkChampionBaseStats,
    pub abilities: NetworkChampionAbilities,
}
/// Stores base stats for one champion.
///
/// - `max_health`: Maximum health assigned by the match server.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct NetworkChampionBaseStats {
    pub max_health: f32,
}
/// Stores ability tuning for one champion.
///
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
/// Stores server-authoritative ability tuning for one ability slot.
///
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
/// Stores server-authoritative damage values for one ability.
///
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
/// Describes one player currently known by the authoritative server.
///
/// - `player_id`: Stable network player id assigned from the client connection.
/// - `champion`: Champion selected by the player.
/// - `team`: Team assigned by the current match/lobby.
/// - `position`: Server-assigned development spawn position.
/// - `position_correction_generation`: Monotonic counter incremented when the server rejects a position.
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
    #[serde(default)]
    pub position_correction_generation: u32,
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
/// Sends the current lightweight match roster from the server to one client.
///
/// - `local_player_id`: Player id of the receiving client.
/// - `players`: Players currently connected to the development server.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MatchSnapshot {
    pub local_player_id: u64,
    pub players: Vec<NetworkPlayer>,
}

/// Describes one server-authoritative lane unit currently active on the map.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkLaneUnit {
    /// Stable lane-unit id used by commands and client reconciliation.
    pub id: u64,
    /// Visual and combat role of the unit.
    pub kind: LaneUnitKind,
    /// Team that owns the unit.
    pub team: TeamSpec,
    /// Latest authoritative world position.
    pub position: WorldPosition,
    /// Latest authoritative facing yaw in radians.
    pub yaw: f32,
    /// Current health.
    pub health: f32,
    /// Maximum health.
    pub max_health: f32,
    /// Targeting radius used by client-side click selection.
    pub hit_radius: f32,
    /// Current attack target, including active tower projectiles.
    pub attack_target: Option<NetworkTargetId>,
}

/// Sends the latest server-authoritative single-lane state to clients.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct LaneSnapshot {
    /// Active lane units sorted by stable lane-unit id.
    pub units: Vec<NetworkLaneUnit>,
}
/// Sends the local player's current visual state to the server.
///
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
/// Sent by a client once its local display has loaded enough to enter the match.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayReady;
/// Sent by a client before it intentionally leaves the running match.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientLeave;
/// Sends the server-side loading-screen state to clients.
///
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
/// Describes one player card for the server-driven loading screen.
///
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
/// Describes an input command sent by a client to the authoritative server.
///
/// - `MoveTo`: Requests movement toward a world-space point.
/// - `AttackMove`: Requests movement toward a hostile target until it is inside basic-attack range.
/// - `CastAbility`: Requests an ability cast for the given champion and slot.
/// - `AutoAttack`: Requests a basic attack against a player or lane unit.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum PlayerCommand {
    MoveTo(WorldPosition),
    AttackMove {
        target: NetworkTargetId,
    },
    CastAbility {
        champion: ChampionId,
        slot: AbilitySlot,
        target: CastTarget,
    },
    AutoAttack {
        target: NetworkTargetId,
    },
}
/// Registers the shared Lightyear protocol used by client and server.
pub struct SharedNetworkPlugin;

impl Plugin for SharedNetworkPlugin {
    /// Registers Bevy resources, plugins, or systems for the shared network protocol system.
    fn build(&self, app: &mut App) {
        app.register_message::<PlayerCommand>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<AbilityVisualEvent>()
            .add_direction(NetworkDirection::Bidirectional);

        app.register_message::<AutoAttackVisualEvent>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<RangedMinionAutoAttackVisualEvent>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<NetworkCombatNumberEvent>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<PlayerStateUpdate>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<DisplayReady>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<ClientLeave>()
            .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<MatchSnapshot>()
            .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<LaneSnapshot>()
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
