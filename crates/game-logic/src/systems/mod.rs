use bevy::prelude::*;
use game_shared::network::ChampionId;

mod animation;
mod camera;
mod characters;
mod healthbar;
mod movement;
mod networked_players;
mod setup;
mod targeting;
mod ui_state;

pub use ui_state::MiraHudState;

/// Registers server-safe gameplay systems shared by client and dedicated server.
///
/// Description:
/// This plugin is intentionally small during the current prototype phase. Client-only
/// rendering, input, camera, animation, and HUD systems live in `MiraClientSystemsPlugin`.
pub struct MiraGameplaySystemsPlugin;

/// Registers the client-only gameplay presentation and input systems.
///
/// Description:
/// Used by the playable client after Bevy asset, render, and input plugins are available.
pub struct MiraClientSystemsPlugin;

/// Compatibility plugin that registers both gameplay and client systems.
///
/// Description:
/// New app code should prefer `MiraGameplaySystemsPlugin` for server-safe logic and
/// `MiraClientSystemsPlugin` for client-only logic.
pub struct MiraSystemsPlugin;

/// Registers local prototype champion spawn and scene setup systems.
struct LocalSpawnSystemsPlugin;

/// Registers remote player snapshot and interpolation systems.
struct NetworkedPlayersSystemsPlugin;

/// Registers local champion animation systems.
struct AnimationSystemsPlugin;

/// Registers local movement input and movement simulation systems.
struct MovementSystemsPlugin;

/// Registers Lira ability prototype systems.
struct LiraAbilitySystemsPlugin;

/// Registers Ignara ability prototype systems.
struct IgnaraAbilitySystemsPlugin;

/// Registers Yuna ability prototype systems.
struct YunaAbilitySystemsPlugin;

/// Registers Sophia ability prototype systems.
struct SophiaAbilitySystemsPlugin;

/// Registers top-down camera control systems.
struct CameraSystemsPlugin;

/// Registers HUD state and health bar presentation systems.
struct HudSystemsPlugin;

pub(super) const LOCAL_CHAMPION_ID: u32 = 6606;
pub(super) const HOLD_CURSOR_MIN_DISTANCE: f32 = 1.35;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Description:
/// Marks the transient click marker shown at the current movement target.
pub(super) struct MoveTargetMarker;

#[derive(Component, Debug, Clone, Copy)]
/// Description:
/// Stores combat data for enemy training targets used by ability prototypes.
///
/// Fields:
/// - `health`: Current health value for the dummy.
/// - `hit_radius`: Collision radius used by projectile and area checks.
pub(super) struct TrainingDummy {
    pub(super) health: f32,
    pub(super) hit_radius: f32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
/// Description:
/// Stores server-provided temporary movement modifiers for the local player.
pub(super) struct ExternalMovementModifier {
    pub(super) speed_multiplier: f32,
    pub(super) pull_center: Option<Vec3>,
    pub(super) pull_speed: f32,
    pub(super) stunned: bool,
}

#[derive(Component, Debug, Clone)]
/// Description:
/// Tracks the pulse animation state for the movement target marker.
///
/// Fields:
/// - `timer`: Animation timer for the marker pulse.
/// - `active`: Whether the marker pulse is currently visible and animating.
pub(super) struct MoveTargetMarkerFx {
    pub(super) timer: Timer,
    pub(super) active: bool,
}

#[derive(Resource, Debug, Clone)]
/// Description:
/// Stores the local champion animation graph and clip node indices.
///
/// Fields:
/// - `graph`: Animation graph handle assigned to spawned animation players.
/// - `idle`: Node index for the idle animation.
/// - `walk`: Node index for the walking animation.
pub(super) struct LocalChampionAnimations {
    pub(super) graph: Handle<AnimationGraph>,
    pub(super) idle: AnimationNodeIndex,
    pub(super) walk: AnimationNodeIndex,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
/// Description:
/// Stores the currently selected local champion locomotion animation state.
///
/// Fields:
/// - `moving`: Whether the controlled champion is currently moving.
/// - `stop_grace_seconds`: Time accumulated since movement stopped before switching to idle.
pub(super) struct LocalChampionAnimationState {
    pub(super) moving: bool,
    pub(super) stop_grace_seconds: f32,
}

#[derive(Resource, Debug, Clone, Copy)]
/// Description:
/// Stores the last meaningful movement direction while right-click movement is held.
///
/// Fields:
/// - `0`: Normalized world-space movement direction.
pub(super) struct HoldMoveDirection(pub(super) Vec3);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Description:
/// Tracks which server-assigned champion model is currently attached to an entity.
///
/// Fields:
/// - `champion`: Champion id whose scene is already attached to this entity.
pub(super) struct CurrentChampionVisual {
    pub(super) champion: Option<ChampionId>,
    pub(super) model_root: Option<Entity>,
}

impl Default for MoveTargetMarkerFx {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.28, TimerMode::Once),
            active: false,
        }
    }
}

impl Plugin for MiraGameplaySystemsPlugin {
    fn build(&self, _app: &mut App) {}
}

impl Plugin for MiraClientSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            LocalSpawnSystemsPlugin,
            NetworkedPlayersSystemsPlugin,
            AnimationSystemsPlugin,
            MovementSystemsPlugin,
            LiraAbilitySystemsPlugin,
            IgnaraAbilitySystemsPlugin,
            YunaAbilitySystemsPlugin,
            SophiaAbilitySystemsPlugin,
            CameraSystemsPlugin,
            HudSystemsPlugin,
        ));
    }
}

impl Plugin for MiraSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((MiraGameplaySystemsPlugin, MiraClientSystemsPlugin));
    }
}

impl Plugin for LocalSpawnSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<characters::lira::LiraQSettings>()
            .init_resource::<characters::lira::LiraQCastState>()
            .init_resource::<characters::lira::LiraQIndicatorState>()
            .init_resource::<characters::lira::LiraWSettings>()
            .init_resource::<characters::lira::LiraWCastState>()
            .init_resource::<characters::lira::LiraWIndicatorState>()
            .init_resource::<characters::lira::LiraESettings>()
            .init_resource::<characters::lira::LiraECastState>()
            .init_resource::<characters::ignara::IgnaraQSettings>()
            .init_resource::<characters::ignara::IgnaraQCastState>()
            .init_resource::<characters::ignara::IgnaraWSettings>()
            .init_resource::<characters::ignara::IgnaraWCastState>()
            .init_resource::<characters::ignara::IgnaraESettings>()
            .init_resource::<characters::ignara::IgnaraECastState>()
            .init_resource::<characters::yuna::YunaQSettings>()
            .init_resource::<characters::yuna::YunaQCastState>()
            .init_resource::<characters::yuna::YunaWSettings>()
            .init_resource::<characters::yuna::YunaWCastState>()
            .init_resource::<characters::yuna::YunaESettings>()
            .init_resource::<characters::yuna::YunaECastState>()
            .init_resource::<characters::sophia::SophiaQSettings>()
            .init_resource::<characters::sophia::SophiaQCastState>()
            .init_resource::<characters::sophia::SophiaWSettings>()
            .init_resource::<characters::sophia::SophiaWCastState>()
            .init_resource::<characters::sophia::SophiaESettings>()
            .init_resource::<characters::sophia::SophiaECastState>()
            .init_resource::<ui_state::MiraHudState>()
            .init_resource::<setup::ClientChampionCatalog>()
            .init_resource::<networked_players::AppliedLocalNetworkSpawn>()
            .init_resource::<networked_players::PlayerStateUpdateTimer>()
            .init_resource::<networked_players::LocalPlayerSelection>()
            .add_systems(
                Startup,
                setup::spawn_local_player_and_camera.run_if(resource_exists::<AssetServer>),
            )
            .add_systems(Update, setup::receive_champion_catalog_updates);
    }
}

impl Plugin for NetworkedPlayersSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            networked_players::sync_remote_players_from_match_snapshot
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            networked_players::interpolate_remote_player_positions
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            FixedUpdate,
            networked_players::send_local_player_state_update
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            networked_players::sync_remote_player_animations.run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for AnimationSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animation::setup_animation_player_once_loaded
                .run_if(resource_exists::<AssetServer>)
                .run_if(resource_exists::<LocalChampionAnimations>),
        )
        .add_systems(
            Update,
            animation::sync_controlled_player_animation.run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for MovementSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            movement::set_move_target_from_mouse_input.run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            FixedUpdate,
            movement::move_controlled_player.run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            movement::animate_move_target_marker.run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for LiraAbilitySystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                characters::lira::adjust_q_skillshot_indicator_color,
                characters::lira::cast_q_skillshot_on_left_click,
                characters::lira::cast_w_arc_on_left_click,
                characters::lira::cast_e_contact_missiles,
                characters::lira::receive_remote_ability_visuals,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            (
                characters::lira::update_q_skillshot_projectiles,
                characters::lira::update_q_skillshot_explosions,
                characters::lira::update_w_arc_projectiles,
                characters::lira::update_w_arc_explosions,
                characters::lira::update_e_contact_missiles,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            (
                characters::lira::update_q_skillshot_indicator,
                characters::lira::update_w_arc_indicator,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for IgnaraAbilitySystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            characters::ignara::spawn_ignara_indicators.run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            (
                characters::ignara::update_ignara_indicators,
                characters::ignara::cast_q_burning_ground,
                characters::ignara::cast_w_fireball,
                characters::ignara::cast_e_snowball,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            (
                characters::ignara::update_q_burning_grounds,
                characters::ignara::update_w_fireballs,
                characters::ignara::update_e_snowballs,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for YunaAbilitySystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            characters::yuna::spawn_yuna_indicators.run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            (
                characters::yuna::update_yuna_indicators,
                characters::yuna::cast_q_gravity_orb,
                characters::yuna::cast_w_healing_field,
                characters::yuna::cast_e_stun_bolt,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            (
                characters::yuna::update_q_projectiles,
                characters::yuna::update_q_fields,
                characters::yuna::update_w_fields,
                characters::yuna::update_e_stun_bolts,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for SophiaAbilitySystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            characters::sophia::spawn_sophia_indicators.run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            (
                characters::sophia::update_sophia_indicators,
                characters::sophia::cast_q_orb_on_left_click,
                characters::sophia::cast_w_minions,
                characters::sophia::cast_e_self_buff,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            (
                characters::sophia::update_q_orbs,
                characters::sophia::update_minions,
                characters::sophia::update_buff_arrows,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for CameraSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                camera::handle_camera_zoom,
                camera::follow_controlled_player,
                camera::update_top_down_camera,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for HudSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                healthbar::update_health_bar_positions,
                healthbar::update_health_bar_fills,
                ui_state::update_mira_hud_state,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        );
    }
}
