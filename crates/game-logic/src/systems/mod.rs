use bevy::prelude::*;
use bevy_fontmesh::FontMeshPlugin;
use game_shared::network::ChampionId;

mod animation;
mod auto_attack;
mod camera;
mod characters;
mod damage_numbers;
mod healthbar;
mod lane;
mod movement;
mod networked_players;
mod setup;
mod targeting;
mod ui_state;

pub use healthbar::{OverheadHealthBarStyle, OverheadPlayerProfiles};
pub use ui_state::MiraHudState;

/// Registers server-safe gameplay systems shared by client and dedicated server.
///
/// This plugin is intentionally small during the current prototype phase. Client-only
/// rendering, input, camera, animation, and HUD systems live in `MiraClientSystemsPlugin`.
pub struct MiraGameplaySystemsPlugin;

/// Registers the client-only gameplay presentation and input systems.
///
/// Used by the playable client after Bevy asset, render, and input plugins are available.
pub struct MiraClientSystemsPlugin;

/// Client-side gameplay setup flags supplied by the embedding game client.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct MiraClientGameplaySettings {
    /// Whether the F9 shortcut may toggle a local development-only target dummy.
    pub allow_dev_dummy_toggle: bool,
    /// Whether a local development-only target dummy should be spawned during setup.
    pub spawn_dev_dummy: bool,
}

/// Compatibility plugin that registers both gameplay and client systems.
///
/// New app code should prefer `MiraGameplaySystemsPlugin` for server-safe logic and
/// `MiraClientSystemsPlugin` for client-only logic.
pub struct MiraSystemsPlugin;

/// Registers local prototype champion spawn and scene setup systems.
struct LocalSpawnSystemsPlugin;

/// Registers remote player snapshot and interpolation systems.
struct NetworkedPlayersSystemsPlugin;

/// Registers replicated lane-unit presentation systems.
struct LaneSystemsPlugin;

/// Registers local champion animation systems.
struct AnimationSystemsPlugin;

/// Registers local movement input and movement simulation systems.
struct MovementSystemsPlugin;

/// Registers local auto-attack input and projectile presentation systems.
struct AutoAttackSystemsPlugin;

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

/// Registers floating combat text presentation systems.
struct DamageNumberSystemsPlugin;

pub(super) const LOCAL_CHAMPION_ID: ChampionId = ChampionId::LIRA;
pub(super) const HOLD_CURSOR_MIN_DISTANCE: f32 = 1.35;

pub(super) fn horizontal_distance(left: Vec3, right: Vec3) -> f32 {
    Vec2::new(left.x - right.x, left.z - right.z).length()
}

pub(super) fn hierarchy_root(mut entity: Entity, parents: &Query<&ChildOf>) -> Entity {
    while let Ok(parent) = parents.get(entity) {
        entity = parent.0;
    }
    entity
}

/// Marks the transient click marker shown at the current movement target.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct MoveTargetMarker;
/// Stores combat data for enemy training targets used by ability prototypes.
///
/// - `health`: Current health value for the dummy.
/// - `hit_radius`: Collision radius used by projectile and area checks.
#[derive(Component, Debug, Clone)]
pub(super) struct TrainingDummy {
    pub(super) health: f32,
    pub(super) max_health: f32,
    pub(super) hit_radius: f32,
    pub(super) idle_seconds: f32,
    pub(super) min_health: f32,
    pub(super) local_damage_enabled: bool,
    pub(super) track_total_damage: bool,
    pub(super) total_damage: f32,
    pub(super) total_damage_idle_seconds: f32,
    pending_combat_numbers: Vec<(f32, TrainingDummyHealthChangeKind)>,
    pub(super) last_health_change_kind: TrainingDummyHealthChangeKind,
}
/// Describes the source of the latest dummy health change for floating combat text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum TrainingDummyHealthChangeKind {
    #[default]
    AutoAttack,
    Spell,
    Heal,
}

impl TrainingDummy {
    pub(super) fn new(health: f32, hit_radius: f32) -> Self {
        Self {
            health,
            max_health: health,
            hit_radius,
            idle_seconds: 0.0,
            min_health: 1.0,
            local_damage_enabled: true,
            track_total_damage: true,
            total_damage: 0.0,
            total_damage_idle_seconds: 0.0,
            pending_combat_numbers: Vec::new(),
            last_health_change_kind: TrainingDummyHealthChangeKind::AutoAttack,
        }
    }
    pub(super) fn remote_player(health: f32, max_health: f32, hit_radius: f32) -> Self {
        Self {
            health: health.clamp(0.0, max_health.max(0.0)),
            max_health: max_health.max(0.0),
            hit_radius,
            idle_seconds: 0.0,
            min_health: 0.0,
            local_damage_enabled: false,
            track_total_damage: false,
            total_damage: 0.0,
            total_damage_idle_seconds: 0.0,
            pending_combat_numbers: Vec::new(),
            last_health_change_kind: TrainingDummyHealthChangeKind::AutoAttack,
        }
    }
    pub(super) fn apply_damage(&mut self, damage: f32, kind: TrainingDummyHealthChangeKind) {
        if damage <= 0.0 || (self.health <= 0.0 && self.min_health <= 0.0) {
            return;
        }

        self.health = (self.health - damage).max(self.min_health);
        self.idle_seconds = 0.0;
        self.last_health_change_kind = kind;
        self.pending_combat_numbers.push((damage, kind));
        if self.track_total_damage {
            self.total_damage += damage;
            self.total_damage_idle_seconds = 0.0;
        }
    }
    pub(super) fn set_server_health(&mut self, health: f32, max_health: f32) {
        let max_health = max_health.max(0.0);
        let min_health = self.min_health.clamp(0.0, max_health);
        self.health = health.clamp(min_health, max_health);
        self.max_health = max_health;
        self.idle_seconds = 0.0;
        self.pending_combat_numbers.clear();
    }
    pub(super) fn can_auto_heal(&self) -> bool {
        self.local_damage_enabled && self.track_total_damage
    }
    pub(super) fn heal_to_full(&mut self) -> f32 {
        let heal = (self.max_health - self.health).max(0.0);
        if heal <= f32::EPSILON {
            return 0.0;
        }

        self.health = self.max_health;
        self.idle_seconds = 0.0;
        self.last_health_change_kind = TrainingDummyHealthChangeKind::Heal;
        self.pending_combat_numbers
            .push((heal, TrainingDummyHealthChangeKind::Heal));
        heal
    }
    pub(super) fn take_pending_combat_numbers(
        &mut self,
    ) -> Vec<(f32, TrainingDummyHealthChangeKind)> {
        std::mem::take(&mut self.pending_combat_numbers)
    }
}
/// Stores server-provided temporary movement modifiers for the local player.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(super) struct ExternalMovementModifier {
    pub(super) speed_multiplier: f32,
    pub(super) pull_center: Option<Vec3>,
    pub(super) pull_speed: f32,
    pub(super) stunned: bool,
}
/// Tracks the pulse animation state for the movement target marker.
///
/// - `timer`: Animation timer for the marker pulse.
/// - `active`: Whether the marker pulse is currently visible and animating.
#[derive(Component, Debug, Clone)]
pub(super) struct MoveTargetMarkerFx {
    pub(super) timer: Timer,
    pub(super) active: bool,
}
/// Stores the local champion animation graph and clip node indices.
///
/// - `graph`: Animation graph handle assigned to spawned animation players.
/// - `idle`: Node index for the idle animation.
/// - `walk`: Node index for the walking animation.
#[derive(Resource, Debug, Clone)]
pub(super) struct LocalChampionAnimations {
    pub(super) graph: Handle<AnimationGraph>,
    pub(super) idle: AnimationNodeIndex,
    pub(super) walk: AnimationNodeIndex,
}
/// Stores the currently selected local champion locomotion animation state.
///
/// - `moving`: Whether the controlled champion is currently moving.
/// - `stop_grace_seconds`: Time accumulated since movement stopped before switching to idle.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub(super) struct LocalChampionAnimationState {
    pub(super) moving: bool,
    pub(super) stop_grace_seconds: f32,
}
/// Stores the last meaningful movement direction while right-click movement is held.
///
/// - `0`: Normalized world-space movement direction.
#[derive(Resource, Debug, Clone, Copy)]
pub(super) struct HoldMoveDirection(pub(super) Vec3);
/// Tracks which server-assigned champion model is currently attached to an entity.
///
/// - `champion`: Champion id whose scene is already attached to this entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct CurrentChampionVisual {
    pub(super) champion: Option<ChampionId>,
    pub(super) model_root: Option<Entity>,
}

impl Default for MoveTargetMarkerFx {
    /// Returns the default configuration used by the gameplay systems plugin registry.
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.28, TimerMode::Once),
            active: false,
        }
    }
}

impl Plugin for MiraGameplaySystemsPlugin {
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
    fn build(&self, _app: &mut App) {}
}

impl Plugin for MiraClientSystemsPlugin {
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
    fn build(&self, app: &mut App) {
        app.add_plugins(FontMeshPlugin::<StandardMaterial>::default());
        app.add_plugins((
            LocalSpawnSystemsPlugin,
            NetworkedPlayersSystemsPlugin,
            LaneSystemsPlugin,
            AnimationSystemsPlugin,
            AutoAttackSystemsPlugin,
            MovementSystemsPlugin,
            LiraAbilitySystemsPlugin,
            IgnaraAbilitySystemsPlugin,
            YunaAbilitySystemsPlugin,
            SophiaAbilitySystemsPlugin,
            CameraSystemsPlugin,
            DamageNumberSystemsPlugin,
            HudSystemsPlugin,
        ));
    }
}

impl Plugin for MiraSystemsPlugin {
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
    fn build(&self, app: &mut App) {
        app.add_plugins((MiraGameplaySystemsPlugin, MiraClientSystemsPlugin));
    }
}

impl Plugin for LocalSpawnSystemsPlugin {
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
    fn build(&self, app: &mut App) {
        app.init_resource::<MiraClientGameplaySettings>()
            .init_resource::<characters::lira::LiraQSettings>()
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
            .init_resource::<healthbar::OverheadHealthBarStyle>()
            .init_resource::<healthbar::OverheadPlayerProfiles>()
            .init_resource::<setup::ClientChampionCatalog>()
            .init_resource::<networked_players::AppliedLocalNetworkSpawn>()
            .init_resource::<networked_players::PlayerStateUpdateTimer>()
            .init_resource::<networked_players::LocalPlayerSelection>()
            .add_systems(
                Startup,
                setup::spawn_local_player_and_camera.run_if(resource_exists::<AssetServer>),
            )
            .add_systems(
                Update,
                setup::toggle_dev_preview_dummy.run_if(resource_exists::<AssetServer>),
            )
            .add_systems(Update, setup::receive_champion_catalog_updates);
    }
}

impl Plugin for NetworkedPlayersSystemsPlugin {
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            networked_players::sync_remote_players_from_match_snapshot
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            networked_players::reconcile_local_player_to_authoritative_snapshot
                .after(networked_players::sync_remote_players_from_match_snapshot)
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

impl Plugin for LaneSystemsPlugin {
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                lane::sync_lane_units_from_snapshot,
                lane::interpolate_lane_unit_positions,
                lane::update_tower_attack_lines,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for AnimationSystemsPlugin {
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
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
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            movement::set_move_target_from_mouse_input
                .after(auto_attack::update_auto_attack_target)
                .after(lane::sync_lane_units_from_snapshot)
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            FixedUpdate,
            (
                movement::advance_local_navigation_routes,
                movement::move_controlled_player,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            movement::animate_move_target_marker.run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for AutoAttackSystemsPlugin {
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
    fn build(&self, app: &mut App) {
        app.init_resource::<auto_attack::AutoAttackState>()
            .init_resource::<auto_attack::AutoAttackInputState>()
            .init_resource::<auto_attack::AutoAttackTarget>()
            .add_systems(
                Update,
                auto_attack::handle_auto_attack_input.run_if(resource_exists::<AssetServer>),
            )
            .add_systems(
                Update,
                auto_attack::update_auto_attack_target
                    .after(auto_attack::handle_auto_attack_input)
                    .run_if(resource_exists::<AssetServer>),
            )
            .add_systems(
                Update,
                auto_attack::receive_remote_auto_attack_visuals
                    .after(auto_attack::update_auto_attack_target)
                    .run_if(resource_exists::<AssetServer>),
            )
            .add_systems(
                Update,
                auto_attack::receive_remote_ranged_minion_auto_attack_visuals
                    .after(auto_attack::receive_remote_auto_attack_visuals)
                    .run_if(resource_exists::<AssetServer>),
            )
            .add_systems(
                Update,
                auto_attack::update_auto_attack_projectiles
                    .after(auto_attack::receive_remote_ranged_minion_auto_attack_visuals)
                    .run_if(resource_exists::<AssetServer>),
            );
    }
}

impl Plugin for LiraAbilitySystemsPlugin {
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
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
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
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
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
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
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
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
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
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
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                healthbar::update_health_bar_positions.after(camera::update_top_down_camera),
                healthbar::update_health_bar_fills,
                healthbar::update_health_bar_texts,
                ui_state::update_mira_hud_state,
            )
                .chain()
                .run_if(resource_exists::<AssetServer>),
        );
    }
}

impl Plugin for DamageNumberSystemsPlugin {
    /// Registers Bevy resources, plugins, or systems for the gameplay systems plugin registry.
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            damage_numbers::initialize_damage_number_health_trackers
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            damage_numbers::heal_idle_training_dummies
                .after(damage_numbers::initialize_damage_number_health_trackers)
                .after(auto_attack::update_auto_attack_projectiles)
                .after(characters::lira::update_q_skillshot_projectiles)
                .after(characters::lira::update_q_skillshot_explosions)
                .after(characters::lira::update_w_arc_explosions)
                .after(characters::lira::update_e_contact_missiles)
                .after(characters::sophia::update_q_orbs)
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            damage_numbers::receive_server_combat_number_events
                .after(damage_numbers::heal_idle_training_dummies)
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            damage_numbers::spawn_damage_numbers_from_dummy_health
                .after(damage_numbers::receive_server_combat_number_events)
                .run_if(resource_exists::<AssetServer>),
        )
        .add_systems(
            Update,
            damage_numbers::update_damage_numbers
                .after(camera::update_top_down_camera)
                .run_if(resource_exists::<AssetServer>),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn development_dummy_is_disabled_by_default() {
        let settings = MiraClientGameplaySettings::default();

        assert!(!settings.allow_dev_dummy_toggle);
        assert!(!settings.spawn_dev_dummy);
    }
    #[test]
    fn local_training_dummy_keeps_one_health_and_tracks_total_damage() {
        let mut dummy = TrainingDummy::new(20.0, 0.9);

        dummy.apply_damage(50.0, TrainingDummyHealthChangeKind::Spell);

        assert_eq!(dummy.health, 1.0);
        assert_eq!(dummy.total_damage, 50.0);
        assert_eq!(
            dummy.take_pending_combat_numbers(),
            vec![(50.0, TrainingDummyHealthChangeKind::Spell)]
        );
    }
    #[test]
    fn remote_training_dummy_allows_zero_health_without_local_total_damage() {
        let mut dummy = TrainingDummy::remote_player(20.0, 20.0, 0.9);

        dummy.apply_damage(50.0, TrainingDummyHealthChangeKind::Spell);

        assert_eq!(dummy.health, 0.0);
        assert_eq!(dummy.total_damage, 0.0);
        assert!(!dummy.local_damage_enabled);
        assert!(!dummy.can_auto_heal());
    }
}
