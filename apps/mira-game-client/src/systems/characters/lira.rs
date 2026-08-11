use super::{
    common::{
        cursor_hit_on_map, horizontal_distance, positive_or, ready_timer_percent,
        remaining_timer_seconds, send_ability_command, total_timer_seconds,
    },
    ignara::{IgnaraESettings, IgnaraQSettings, IgnaraWSettings},
    sophia::{SophiaESettings, SophiaQSettings, SophiaWSettings},
    yuna::{YunaESettings, YunaQSettings, YunaWSettings},
};
use crate::systems::lane::{RemoteLaneUnit, is_local_spell_target};
use crate::systems::{
    CurrentChampionVisual, TrainingDummy, targeting::clamp_world_point_to_map_top,
};
use bevy::math::primitives::Sphere;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use lightyear::prelude::*;
use mira_game_api::game::{
    camera::TopDownCamera,
    map::MapGround,
    player::{Health, Player, PlayerControlled},
    team::Team,
};
use mira_game_api::network::{
    AbilitySlot, AbilityVisualEvent, AbilityVisualTuning, ChampionId, PlayerCommand,
};

const Q_CAST_COOLDOWN_SECONDS: f32 = 5.0;
const Q_PROJECTILE_TRAVEL_SECONDS: f32 = 1.0;
const Q_PROJECTILE_HEIGHT: f32 = 0.26;
const Q_EXPLOSION_LIFETIME_SECONDS: f32 = 0.28;
const Q_EXPLOSION_DAMAGE_RADIUS: f32 = 1.65;

const W_CAST_COOLDOWN_SECONDS: f32 = 6.0;
const W_PROJECTILE_TRAVEL_SECONDS: f32 = 0.78;
const W_PROJECTILE_HEIGHT: f32 = 0.85;
const W_TARGET_HEIGHT: f32 = 0.2;
const W_PROJECTILE_ARC_HEIGHT: f32 = 3.2;
const W_EXPLOSION_LIFETIME_SECONDS: f32 = 0.36;

const E_CAST_COOLDOWN_SECONDS: f32 = 8.0;
const E_MISSILE_COUNT: usize = 3;
const E_MISSILE_LIFETIME_SECONDS: f32 = 5.0;
const E_MISSILE_SEARCH_RADIUS: f32 = 6.0;
const E_MISSILE_ORBIT_RADIUS: f32 = 1.1;
const E_MISSILE_ORBIT_HEIGHT: f32 = 1.05;
const E_MISSILE_ORBIT_SPEED: f32 = 5.2;
const E_MISSILE_CHASE_SPEED: f32 = 13.0;
const E_MISSILE_RADIUS: f32 = 0.24;
const REMOTE_PLAYER_VISUAL_HIT_RADIUS: f32 = 0.9;
/// Stores visual tuning for Lira's Q skillshot preview and projectile.
///
/// - `range`: Maximum projectile travel distance in world units.
/// - `width`: Skillshot width used for preview and hit radius.
/// - `cooldown_seconds`: Local prediction cooldown duration.
/// - `travel_seconds`: Local prediction projectile travel duration.
/// - `projectile_height`: Local prediction projectile height offset.
/// - `projectile_radius`: Local prediction projectile visual and hit radius.
/// - `explosion_radius`: Local prediction impact explosion radius.
/// - `thickness`: Vertical thickness used by the preview mesh.
/// - `tip_radius`: Radius of the circular preview tip.
/// - `elevation`: Height above the map used by the preview.
/// - `hue`: Preview color hue.
/// - `saturation`: Preview color saturation.
/// - `lightness`: Preview color lightness.
/// - `alpha`: Preview color opacity.
#[derive(Resource, Debug, Clone, Copy)]
pub(in crate::systems) struct LiraQSettings {
    pub(in crate::systems) range: f32,
    pub(in crate::systems) width: f32,
    pub(in crate::systems) cooldown_seconds: f32,
    pub(in crate::systems) travel_seconds: f32,
    pub(in crate::systems) projectile_height: f32,
    pub(in crate::systems) projectile_radius: f32,
    pub(in crate::systems) explosion_radius: f32,
    pub(in crate::systems) thickness: f32,
    pub(in crate::systems) tip_radius: f32,
    pub(in crate::systems) elevation: f32,
    pub(in crate::systems) hue: f32,
    pub(in crate::systems) saturation: f32,
    pub(in crate::systems) lightness: f32,
    pub(in crate::systems) alpha: f32,
}

impl LiraQSettings {
    /// Builds the current Q preview color from the configured HSL values.
    ///
    /// - `self`: Q settings containing color channels.
    ///
    /// - The configured Q preview color.
    pub(in crate::systems) fn color(self) -> Color {
        Color::hsla(self.hue, self.saturation, self.lightness, self.alpha)
    }
}

impl Default for LiraQSettings {
    /// Returns the default configuration used by the Lira ability system.
    fn default() -> Self {
        Self {
            range: 11.5,
            width: 1.05,
            cooldown_seconds: Q_CAST_COOLDOWN_SECONDS,
            travel_seconds: Q_PROJECTILE_TRAVEL_SECONDS,
            projectile_height: Q_PROJECTILE_HEIGHT,
            projectile_radius: 0.525,
            explosion_radius: Q_EXPLOSION_DAMAGE_RADIUS,
            thickness: 0.03,
            tip_radius: 1.15,
            elevation: 0.06,
            hue: 286.0,
            saturation: 0.92,
            lightness: 0.62,
            alpha: 0.78,
        }
    }
}
/// Stores visual tuning for Lira's W arcing area spell.
///
/// - `range`: Maximum cast range in world units.
/// - `aoe_radius`: Radius of the target area and explosion.
/// - `cooldown_seconds`: Local prediction cooldown duration.
/// - `travel_seconds`: Local prediction projectile travel duration.
/// - `projectile_height`: Local prediction projectile height offset.
/// - `target_height`: Local prediction landing height offset.
/// - `thickness`: Vertical thickness used by preview meshes.
/// - `elevation`: Height above the map used by preview meshes.
/// - `alpha`: Preview color opacity.
#[derive(Resource, Debug, Clone, Copy)]
pub(in crate::systems) struct LiraWSettings {
    pub(in crate::systems) range: f32,
    pub(in crate::systems) aoe_radius: f32,
    pub(in crate::systems) cooldown_seconds: f32,
    pub(in crate::systems) travel_seconds: f32,
    pub(in crate::systems) projectile_height: f32,
    pub(in crate::systems) target_height: f32,
    pub(in crate::systems) thickness: f32,
    pub(in crate::systems) elevation: f32,
    pub(in crate::systems) alpha: f32,
}

impl LiraWSettings {
    /// Builds the current W preview color using the same purple hue family as Q.
    ///
    /// - `self`: W settings containing preview opacity.
    ///
    /// - The configured W preview color.
    pub(in crate::systems) fn color(self) -> Color {
        Color::hsla(286.0, 0.92, 0.62, self.alpha)
    }
}

impl Default for LiraWSettings {
    /// Returns the default configuration used by the Lira ability system.
    fn default() -> Self {
        Self {
            range: 8.0,
            aoe_radius: 1.35,
            cooldown_seconds: W_CAST_COOLDOWN_SECONDS,
            travel_seconds: W_PROJECTILE_TRAVEL_SECONDS,
            projectile_height: W_PROJECTILE_HEIGHT,
            target_height: W_TARGET_HEIGHT,
            thickness: 0.025,
            elevation: 0.08,
            alpha: 0.5,
        }
    }
}
/// Stores local prediction and visual tuning for Lira's E contact missiles.
///
/// - `cooldown_seconds`: Local prediction cooldown duration.
/// - `missile_count`: Number of missile visuals to spawn.
/// - `lifetime_seconds`: Local prediction missile lifetime.
/// - `search_radius`: Local prediction target search radius.
/// - `orbit_radius`: Local prediction missile orbit radius.
/// - `orbit_height`: Local prediction missile orbit height.
/// - `orbit_speed`: Local prediction missile orbit speed.
/// - `chase_speed`: Local prediction missile chase speed.
/// - `missile_radius`: Local prediction missile visual and hit radius.
#[derive(Resource, Debug, Clone, Copy)]
pub(in crate::systems) struct LiraESettings {
    pub(in crate::systems) cooldown_seconds: f32,
    pub(in crate::systems) missile_count: usize,
    pub(in crate::systems) lifetime_seconds: f32,
    pub(in crate::systems) search_radius: f32,
    pub(in crate::systems) orbit_radius: f32,
    pub(in crate::systems) orbit_height: f32,
    pub(in crate::systems) orbit_speed: f32,
    pub(in crate::systems) chase_speed: f32,
    pub(in crate::systems) missile_radius: f32,
}

impl Default for LiraESettings {
    /// Returns the default configuration used by the Lira ability system.
    fn default() -> Self {
        Self {
            cooldown_seconds: E_CAST_COOLDOWN_SECONDS,
            missile_count: E_MISSILE_COUNT,
            lifetime_seconds: E_MISSILE_LIFETIME_SECONDS,
            search_radius: E_MISSILE_SEARCH_RADIUS,
            orbit_radius: E_MISSILE_ORBIT_RADIUS,
            orbit_height: E_MISSILE_ORBIT_HEIGHT,
            orbit_speed: E_MISSILE_ORBIT_SPEED,
            chase_speed: E_MISSILE_CHASE_SPEED,
            missile_radius: E_MISSILE_RADIUS,
        }
    }
}

impl LiraESettings {
    /// Builds E visual settings from server-authoritative ability visual tuning.
    ///
    /// - `visual`: Visual tuning attached to an accepted server ability cast.
    ///
    /// - E settings suitable for local visual-only missile rendering.
    pub(in crate::systems) fn from_visual(visual: AbilityVisualTuning) -> Self {
        let fallback = Self::default();
        Self {
            cooldown_seconds: fallback.cooldown_seconds,
            missile_count: if visual.missile_count > 0 {
                usize::from(visual.missile_count)
            } else {
                fallback.missile_count
            },
            lifetime_seconds: positive_or(
                visual.missile_lifetime_seconds,
                fallback.lifetime_seconds,
            ),
            search_radius: positive_or(visual.missile_search_radius, fallback.search_radius),
            orbit_radius: positive_or(visual.missile_orbit_radius, fallback.orbit_radius),
            orbit_height: positive_or(visual.missile_orbit_height, fallback.orbit_height),
            orbit_speed: positive_or(visual.missile_orbit_speed, fallback.orbit_speed),
            chase_speed: positive_or(visual.missile_chase_speed, fallback.chase_speed),
            missile_radius: positive_or(visual.missile_radius, fallback.missile_radius),
        }
    }
}
/// Marks the rectangular body mesh used by Lira's Q skillshot preview.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct LiraQIndicatorBody;
/// Marks the circular tip mesh used by Lira's Q skillshot preview.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct LiraQIndicatorTip;
/// Stores runtime state for an active Lira Q projectile.
///
/// - `start`: Projectile start position.
/// - `end`: Projectile end position.
/// - `timer`: Travel timer for interpolation.
#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct LiraQProjectile {
    start: Vec3,
    end: Vec3,
    timer: Timer,
    explosion_radius: f32,
}
/// Stores runtime state for the Lira Q impact explosion.
///
/// - `timer`: Lifetime timer for the explosion visual.
#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct LiraQExplosion {
    timer: Timer,
}
/// Marks the range preview mesh used while aiming Lira's W spell.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct LiraWRangeIndicator;
/// Marks the cursor-following area preview mesh used while aiming Lira's W spell.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct LiraWAoeIndicator;
/// Stores runtime state for an active Lira W arcing projectile.
///
/// - `start`: Projectile start position.
/// - `end`: Projectile landing position.
/// - `timer`: Travel timer for interpolation.
/// - `arc_height`: Peak height added to the projectile arc.
/// - `explosion_radius`: Radius used to render the landing explosion.
#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct LiraWProjectile {
    start: Vec3,
    end: Vec3,
    timer: Timer,
    arc_height: f32,
    explosion_radius: f32,
}
/// Stores runtime state for the Lira W landing explosion.
///
/// - `timer`: Lifetime timer for the explosion visual.
#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct LiraWExplosion {
    timer: Timer,
}
/// Stores runtime state for one Lira E contact missile.
///
/// - `phase`: Initial orbit angle around Lira.
/// - `lifetime`: Maximum active lifetime before despawn.
/// - `mode`: Current missile behavior mode.
/// - `origin`: Fallback orbit origin used when no owner entity is available.
/// - `owner`: Optional owner entity used by remote visual-only missiles.
/// - `settings`: E settings used for missile movement and collision visuals.
#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct LiraEMissile {
    phase: f32,
    lifetime: Timer,
    mode: LiraEMissileMode,
    origin: Vec3,
    owner: Option<Entity>,
    settings: LiraESettings,
}
/// Defines the current behavior mode of a Lira E contact missile.
///
/// - `Orbiting`: Missile is orbiting Lira and searching for a target.
/// - `Chasing`: Missile is moving toward the stored target entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiraEMissileMode {
    Orbiting,
    Chasing(Entity),
}
/// Stores cooldown state for Lira's Q skillshot.
///
/// - `cooldown`: Timer that gates Q casts.
#[derive(Resource, Debug, Clone)]
pub(in crate::systems) struct LiraQCastState {
    cooldown: Timer,
}
/// Stores aim-preview state for Lira's Q skillshot.
///
/// - `suppress_until_release`: Whether the preview should stay hidden until Q is released.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub(in crate::systems) struct LiraQIndicatorState {
    suppress_until_release: bool,
}
/// Stores cooldown state for Lira's W arcing area spell.
///
/// - `cooldown`: Timer that gates W casts.
#[derive(Resource, Debug, Clone)]
pub(in crate::systems) struct LiraWCastState {
    cooldown: Timer,
}
/// Stores aim-preview state for Lira's W arcing area spell.
///
/// - `suppress_until_release`: Whether the preview should stay hidden until W is released.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub(in crate::systems) struct LiraWIndicatorState {
    suppress_until_release: bool,
}
/// Stores cooldown state for Lira's E contact missiles.
///
/// - `cooldown`: Timer that gates E casts.
#[derive(Resource, Debug, Clone)]
pub(in crate::systems) struct LiraECastState {
    cooldown: Timer,
}

impl Default for LiraQCastState {
    /// Returns the default configuration used by the Lira ability system.
    fn default() -> Self {
        Self::ready(Q_CAST_COOLDOWN_SECONDS)
    }
}

impl Default for LiraWCastState {
    /// Returns the default configuration used by the Lira ability system.
    fn default() -> Self {
        Self::ready(W_CAST_COOLDOWN_SECONDS)
    }
}

impl Default for LiraECastState {
    /// Returns the default configuration used by the Lira ability system.
    fn default() -> Self {
        Self::ready(E_CAST_COOLDOWN_SECONDS)
    }
}

impl LiraQCastState {
    /// Creates a ready Q cooldown state using the configured duration.
    ///
    /// - `cooldown_seconds`: Cooldown duration in seconds.
    ///
    /// - Ready cooldown state for local prediction.
    pub(in crate::systems) fn ready(cooldown_seconds: f32) -> Self {
        let mut cooldown = Timer::from_seconds(cooldown_seconds.max(f32::EPSILON), TimerMode::Once);
        cooldown.set_elapsed(cooldown.duration());
        Self { cooldown }
    }
    /// Returns the remaining Q cooldown duration.
    ///
    /// - `self`: Q cast state to inspect.
    ///
    /// - Remaining cooldown in seconds.
    pub(in crate::systems) fn remaining_seconds(&self) -> f32 {
        remaining_timer_seconds(&self.cooldown)
    }
    /// Returns the total Q cooldown duration.
    ///
    /// - `self`: Q cast state to inspect.
    ///
    /// - Total cooldown duration in seconds.
    pub(in crate::systems) fn total_seconds(&self) -> f32 {
        total_timer_seconds(&self.cooldown)
    }
    /// Returns how ready Q is as a percentage.
    ///
    /// - `self`: Q cast state to inspect.
    ///
    /// - Readiness percentage from `0.0` to `100.0`.
    pub(in crate::systems) fn ready_percent(&self) -> f32 {
        ready_timer_percent(&self.cooldown)
    }
}

impl LiraWCastState {
    /// Creates a ready W cooldown state using the configured duration.
    ///
    /// - `cooldown_seconds`: Cooldown duration in seconds.
    ///
    /// - Ready cooldown state for local prediction.
    pub(in crate::systems) fn ready(cooldown_seconds: f32) -> Self {
        let mut cooldown = Timer::from_seconds(cooldown_seconds.max(f32::EPSILON), TimerMode::Once);
        cooldown.set_elapsed(cooldown.duration());
        Self { cooldown }
    }
    /// Returns the remaining W cooldown duration.
    ///
    /// - `self`: W cast state to inspect.
    ///
    /// - Remaining cooldown in seconds.
    pub(in crate::systems) fn remaining_seconds(&self) -> f32 {
        remaining_timer_seconds(&self.cooldown)
    }
    /// Returns the total W cooldown duration.
    ///
    /// - `self`: W cast state to inspect.
    ///
    /// - Total cooldown duration in seconds.
    pub(in crate::systems) fn total_seconds(&self) -> f32 {
        total_timer_seconds(&self.cooldown)
    }
    /// Returns how ready W is as a percentage.
    ///
    /// - `self`: W cast state to inspect.
    ///
    /// - Readiness percentage from `0.0` to `100.0`.
    pub(in crate::systems) fn ready_percent(&self) -> f32 {
        ready_timer_percent(&self.cooldown)
    }
}

impl LiraECastState {
    /// Creates a ready E cooldown state using the configured duration.
    ///
    /// - `cooldown_seconds`: Cooldown duration in seconds.
    ///
    /// - Ready cooldown state for local prediction.
    pub(in crate::systems) fn ready(cooldown_seconds: f32) -> Self {
        let mut cooldown = Timer::from_seconds(cooldown_seconds.max(f32::EPSILON), TimerMode::Once);
        cooldown.set_elapsed(cooldown.duration());
        Self { cooldown }
    }
    /// Returns the remaining E cooldown duration.
    ///
    /// - `self`: E cast state to inspect.
    ///
    /// - Remaining cooldown in seconds.
    pub(in crate::systems) fn remaining_seconds(&self) -> f32 {
        remaining_timer_seconds(&self.cooldown)
    }
    /// Returns the total E cooldown duration.
    ///
    /// - `self`: E cast state to inspect.
    ///
    /// - Total cooldown duration in seconds.
    pub(in crate::systems) fn total_seconds(&self) -> f32 {
        total_timer_seconds(&self.cooldown)
    }
    /// Returns how ready E is as a percentage.
    ///
    /// - `self`: E cast state to inspect.
    ///
    /// - Readiness percentage from `0.0` to `100.0`.
    pub(in crate::systems) fn ready_percent(&self) -> f32 {
        ready_timer_percent(&self.cooldown)
    }
}
/// Adjusts Lira's Q preview hue from bracket key input.
///
/// - `keyboard`: Keyboard input used to read bracket key state.
/// - `settings`: Mutable Q settings containing the preview hue.
pub(in crate::systems) fn adjust_q_skillshot_indicator_color(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<LiraQSettings>,
) {
    if keyboard.pressed(KeyCode::BracketLeft) {
        settings.hue = (settings.hue - 90.0 / 60.0).rem_euclid(360.0);
    }
    if keyboard.pressed(KeyCode::BracketRight) {
        settings.hue = (settings.hue + 90.0 / 60.0).rem_euclid(360.0);
    }
}
/// Casts Lira's Q skillshot when Q is held and the left mouse button is pressed.
///
/// - `time`: Frame timing used to advance the Q cooldown.
/// - `mouse_buttons`: Mouse input used to detect the cast click.
/// - `keyboard`: Keyboard input used to require Q aim mode.
/// - `settings`: Q settings used for range, width, and elevation.
/// - `windows`: Primary window used to read cursor position.
/// - `camera_query`: Top-down camera used to project the cursor onto the map.
/// - `map_query`: Map ground transform and bounds used for targeting.
/// - `player_query`: Controlled player transform used as the cast origin.
/// - `cast_state`: Q cooldown state.
/// - `indicator_state`: Q preview state updated after casting.
/// - `command_senders`: Lightyear senders used to request the authoritative Q cast.
/// - `meshes`: Mesh assets used to spawn the projectile.
/// - `materials`: Material assets used to spawn the projectile.
/// - `commands`: ECS command buffer used to spawn the projectile.
pub(in crate::systems) fn cast_q_skillshot_on_left_click(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<LiraQSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<(&Transform, &Health, &CurrentChampionVisual), With<PlayerControlled>>,
    mut cast_state: ResMut<LiraQCastState>,
    mut indicator_state: ResMut<LiraQIndicatorState>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    cast_state.cooldown.tick(time.delta());

    if !keyboard.pressed(KeyCode::KeyQ) || !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    if !cast_state.cooldown.is_finished() {
        return;
    }

    let Ok((player_transform, health, visual)) = player_query.single() else {
        return;
    };
    if health.current == 0 || visual.champion != Some(ChampionId::LIRA) {
        return;
    }
    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };

    let origin_ground =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);

    let cursor_hit = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground);
    let direction = cursor_hit
        .map(|hit| {
            let delta = Vec3::new(hit.x - origin_ground.x, 0.0, hit.z - origin_ground.z);
            if delta.length_squared() > f32::EPSILON {
                delta.normalize()
            } else {
                Vec3::Z
            }
        })
        .unwrap_or(Vec3::Z);

    let end_ground = clamp_world_point_to_map_top(
        origin_ground + direction * settings.range,
        map_transform,
        *map_ground,
    );

    let start = origin_ground + Vec3::Y * settings.projectile_height;
    let end = end_ground + Vec3::Y * settings.projectile_height;

    spawn_q_projectile(
        &mut commands,
        &mut meshes,
        &mut materials,
        start,
        end,
        settings.projectile_radius,
        settings.travel_seconds,
        settings.explosion_radius,
    );

    send_ability_command(
        &mut command_senders,
        ChampionId::LIRA,
        AbilitySlot::Q,
        cursor_hit,
    );

    cast_state.cooldown.reset();
    indicator_state.suppress_until_release = true;
}
/// Moves active Lira Q projectile visuals.
///
/// - `time`: Frame timing used to advance projectile travel.
/// - `settings`: Q settings used to size the impact explosion.
/// - `commands`: ECS command buffer used to despawn projectiles and spawn explosions.
/// - `meshes`: Mesh assets used to spawn explosion visuals.
/// - `materials`: Material assets used to fade projectiles and spawn explosions.
/// - `projectile_query`: Active Q projectiles to update.
pub(in crate::systems) fn update_q_skillshot_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut projectile_query: Query<
        (
            Entity,
            &mut LiraQProjectile,
            &mut Transform,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<TrainingDummy>,
    >,
) {
    for (entity, mut projectile, mut transform, projectile_material) in &mut projectile_query {
        projectile.timer.tick(time.delta());

        let duration = projectile.timer.duration().as_secs_f32();
        let progress = (projectile.timer.elapsed_secs() / duration).clamp(0.0, 1.0);

        transform.translation = projectile.start.lerp(projectile.end, progress);
        transform.scale = Vec3::splat(1.0 + (progress * 12.0).sin().abs() * 0.1);

        if let Some(mut material) = materials.get_mut(&projectile_material.0) {
            material.base_color = material.base_color.with_alpha(0.45 - progress * 0.25);
        }

        if projectile.timer.is_finished() {
            let explosion_material = materials.add(white_material(0.55, 0.22));

            commands.spawn((
                Name::new("LiraQExplosion"),
                LiraQExplosion {
                    timer: Timer::from_seconds(Q_EXPLOSION_LIFETIME_SECONDS, TimerMode::Once),
                },
                Mesh3d(meshes.add(Sphere::new(projectile.explosion_radius * 0.36))),
                MeshMaterial3d(explosion_material),
                Transform::from_translation(projectile.end),
            ));

            commands.entity(entity).despawn();
        }
    }
}
/// Updates Lira Q explosion visuals.
///
/// - `time`: Frame timing used to advance explosion lifetime.
/// - `commands`: ECS command buffer used to despawn completed explosions.
/// - `materials`: Material assets used to fade explosion visuals.
/// - `explosion_query`: Active Q explosions to update.
pub(in crate::systems) fn update_q_skillshot_explosions(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut explosion_query: Query<
        (
            Entity,
            &mut LiraQExplosion,
            &mut Transform,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<TrainingDummy>,
    >,
) {
    for (entity, mut explosion, mut transform, explosion_material) in &mut explosion_query {
        explosion.timer.tick(time.delta());

        let duration = explosion.timer.duration().as_secs_f32();
        let progress = (explosion.timer.elapsed_secs() / duration).clamp(0.0, 1.0);

        transform.scale = Vec3::splat(1.0 + progress * 2.1);

        if let Some(mut material) = materials.get_mut(&explosion_material.0) {
            material.base_color = material.base_color.with_alpha(1.0 - progress);
            material.emissive *= 1.0 - progress * 0.35;
        }

        if explosion.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
/// Updates Lira's Q skillshot preview position, scale, visibility, and color.
///
/// - `keyboard`: Keyboard input used to detect Q aim mode.
/// - `settings`: Q settings used to size and color the preview.
/// - `indicator_state`: Q preview suppression state.
/// - `windows`: Primary window used to read cursor position.
/// - `camera_query`: Top-down camera used to project the cursor onto the map.
/// - `map_query`: Map ground transform and bounds used for targeting.
/// - `player_query`: Controlled player transform used as the preview origin.
/// - `materials`: Material assets used to update preview color.
/// - `body_query`: Rectangular Q preview body to update.
/// - `tip_query`: Circular Q preview tip to update.
pub(in crate::systems) fn update_q_skillshot_indicator(
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<LiraQSettings>,
    mut indicator_state: ResMut<LiraQIndicatorState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<
        (&Transform, &CurrentChampionVisual),
        (
            With<PlayerControlled>,
            Without<LiraQIndicatorBody>,
            Without<LiraQIndicatorTip>,
        ),
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut body_query: Query<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<StandardMaterial>,
        ),
        (
            With<LiraQIndicatorBody>,
            Without<PlayerControlled>,
            Without<LiraQIndicatorTip>,
        ),
    >,
    mut tip_query: Query<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<StandardMaterial>,
        ),
        (
            With<LiraQIndicatorTip>,
            Without<PlayerControlled>,
            Without<LiraQIndicatorBody>,
        ),
    >,
) {
    let q_pressed = keyboard.pressed(KeyCode::KeyQ);
    if !q_pressed {
        indicator_state.suppress_until_release = false;
    }
    let show_indicator = q_pressed && !indicator_state.suppress_until_release;

    let Ok((mut body_transform, mut body_visibility, body_material)) = body_query.single_mut()
    else {
        return;
    };
    let Ok((mut tip_transform, mut tip_visibility, tip_material)) = tip_query.single_mut() else {
        return;
    };

    if let Some(mut material) = materials.get_mut(&body_material.0) {
        material.base_color = settings.color();
        material.emissive = settings.color().with_alpha(0.42).into();
    }
    if let Some(mut material) = materials.get_mut(&tip_material.0) {
        material.base_color = settings.color();
        material.emissive = settings.color().with_alpha(0.42).into();
    }

    if !show_indicator {
        *body_visibility = Visibility::Hidden;
        *tip_visibility = Visibility::Hidden;
        return;
    }

    let Ok((player_transform, visual)) = player_query.single() else {
        *body_visibility = Visibility::Hidden;
        *tip_visibility = Visibility::Hidden;
        return;
    };
    if visual.champion != Some(ChampionId::LIRA) {
        *body_visibility = Visibility::Hidden;
        *tip_visibility = Visibility::Hidden;
        return;
    }
    let Ok((map_transform, map_ground)) = map_query.single() else {
        *body_visibility = Visibility::Hidden;
        *tip_visibility = Visibility::Hidden;
        return;
    };

    let origin =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground)
            + Vec3::Y * settings.elevation;

    let cursor_direction = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground)
        .map(|hit| {
            let delta = Vec3::new(hit.x - origin.x, 0.0, hit.z - origin.z);
            if delta.length_squared() > f32::EPSILON {
                delta.normalize()
            } else {
                Vec3::Z
            }
        })
        .unwrap_or(Vec3::Z);

    let end = clamp_world_point_to_map_top(
        origin + cursor_direction * settings.range,
        map_transform,
        *map_ground,
    ) + Vec3::Y * settings.elevation;

    let to_end = end - origin;
    let effective_range = to_end.length().max(0.001);
    let center = origin + to_end * 0.5;

    let aim_xz = Vec3::new(to_end.x, 0.0, to_end.z);
    let yaw = if aim_xz.length_squared() > f32::EPSILON {
        aim_xz.normalize().x.atan2(aim_xz.normalize().z)
    } else {
        0.0
    };

    body_transform.translation = center;
    body_transform.rotation = Quat::from_rotation_y(yaw);
    body_transform.scale = Vec3::new(settings.width, settings.thickness, effective_range);

    tip_transform.translation = end;
    tip_transform.rotation = Quat::IDENTITY;
    tip_transform.scale = Vec3::new(
        settings.tip_radius,
        settings.thickness * 1.6,
        settings.tip_radius,
    );

    *body_visibility = Visibility::Visible;
    *tip_visibility = Visibility::Visible;
}
/// Casts Lira's W arcing area spell when W is held and the left mouse button is pressed.
///
/// - `time`: Frame timing used to advance the W cooldown.
/// - `mouse_buttons`: Mouse input used to detect the cast click.
/// - `keyboard`: Keyboard input used to require W aim mode.
/// - `settings`: W settings used for range and area radius.
/// - `windows`: Primary window used to read cursor position.
/// - `camera_query`: Top-down camera used to project the cursor onto the map.
/// - `map_query`: Map ground transform and bounds used for targeting.
/// - `player_query`: Controlled player transform used as the cast origin.
/// - `cast_state`: W cooldown state.
/// - `indicator_state`: W preview state updated after casting.
/// - `command_senders`: Lightyear senders used to request the authoritative W cast.
/// - `meshes`: Mesh assets used to spawn the projectile.
/// - `materials`: Material assets used to spawn the projectile.
/// - `commands`: ECS command buffer used to spawn the projectile.
pub(in crate::systems) fn cast_w_arc_on_left_click(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<LiraWSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<(&Transform, &Health, &CurrentChampionVisual), With<PlayerControlled>>,
    mut cast_state: ResMut<LiraWCastState>,
    mut indicator_state: ResMut<LiraWIndicatorState>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    cast_state.cooldown.tick(time.delta());

    if !keyboard.pressed(KeyCode::KeyW) || !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    if !cast_state.cooldown.is_finished() {
        return;
    }

    let Ok((player_transform, health, visual)) = player_query.single() else {
        return;
    };
    if health.current == 0 || visual.champion != Some(ChampionId::LIRA) {
        return;
    }
    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };

    let origin_ground =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);
    let Some(cursor_hit) = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground)
    else {
        return;
    };
    let target_ground = clamp_cast_target(
        origin_ground,
        cursor_hit,
        settings.range,
        map_transform,
        *map_ground,
    );

    let start = origin_ground + Vec3::Y * settings.projectile_height;
    let end = target_ground + Vec3::Y * settings.target_height;
    spawn_w_projectile(
        &mut commands,
        &mut meshes,
        &mut materials,
        start,
        end,
        settings.aoe_radius,
        settings.travel_seconds,
    );

    send_ability_command(
        &mut command_senders,
        ChampionId::LIRA,
        AbilitySlot::W,
        Some(target_ground),
    );

    cast_state.cooldown.reset();
    indicator_state.suppress_until_release = true;
}
/// Moves active Lira W projectiles along an arc and spawns landing explosions.
///
/// - `time`: Frame timing used to advance projectile travel.
/// - `commands`: ECS command buffer used to despawn projectiles and spawn explosions.
/// - `meshes`: Mesh assets used to spawn explosion visuals.
/// - `materials`: Material assets used to spawn explosion visuals.
/// - `projectile_query`: Active W projectiles to update.
pub(in crate::systems) fn update_w_arc_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut projectile_query: Query<(Entity, &mut LiraWProjectile, &mut Transform)>,
) {
    for (entity, mut projectile, mut transform) in &mut projectile_query {
        projectile.timer.tick(time.delta());

        let duration = projectile.timer.duration().as_secs_f32();
        let progress = (projectile.timer.elapsed_secs() / duration).clamp(0.0, 1.0);
        let arc = (std::f32::consts::PI * progress).sin() * projectile.arc_height;

        transform.translation = projectile.start.lerp(projectile.end, progress) + Vec3::Y * arc;
        transform.scale = Vec3::splat(1.0 + (progress * 10.0).sin().abs() * 0.12);

        if projectile.timer.is_finished() {
            let explosion_material = materials.add(white_material(0.72, 0.55));

            commands.spawn((
                Name::new("LiraWExplosion"),
                LiraWExplosion {
                    timer: Timer::from_seconds(W_EXPLOSION_LIFETIME_SECONDS, TimerMode::Once),
                },
                Mesh3d(meshes.add(Sphere::new(projectile.explosion_radius * 0.36))),
                MeshMaterial3d(explosion_material),
                Transform::from_translation(projectile.end),
            ));

            commands.entity(entity).despawn();
        }
    }
}
/// Updates Lira W explosion visuals.
///
/// - `time`: Frame timing used to advance explosion lifetime.
/// - `commands`: ECS command buffer used to despawn completed explosions.
/// - `materials`: Material assets used to fade explosion visuals.
/// - `explosion_query`: Active W explosions to update.
pub(in crate::systems) fn update_w_arc_explosions(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut explosion_query: Query<
        (
            Entity,
            &mut LiraWExplosion,
            &mut Transform,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<TrainingDummy>,
    >,
) {
    for (entity, mut explosion, mut transform, explosion_material) in &mut explosion_query {
        explosion.timer.tick(time.delta());

        let duration = explosion.timer.duration().as_secs_f32();
        let progress = (explosion.timer.elapsed_secs() / duration).clamp(0.0, 1.0);

        transform.scale = Vec3::splat(1.0 + progress * 2.6);

        if let Some(mut material) = materials.get_mut(&explosion_material.0) {
            material.base_color = material.base_color.with_alpha(1.0 - progress);
            material.emissive *= 1.0 - progress * 0.45;
        }

        if explosion.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
/// Updates Lira's W range and area previews while W aim mode is active.
///
/// - `keyboard`: Keyboard input used to detect W aim mode.
/// - `settings`: W settings used to size and color the previews.
/// - `indicator_state`: W preview suppression state.
/// - `windows`: Primary window used to read cursor position.
/// - `camera_query`: Top-down camera used to project the cursor onto the map.
/// - `map_query`: Map ground transform and bounds used for targeting.
/// - `player_query`: Controlled player transform used as the preview origin.
/// - `materials`: Material assets used to update preview colors.
/// - `range_query`: W cast range preview to update.
/// - `aoe_query`: W cursor-following area preview to update.
pub(in crate::systems) fn update_w_arc_indicator(
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<LiraWSettings>,
    mut indicator_state: ResMut<LiraWIndicatorState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<
        (&Transform, &CurrentChampionVisual),
        (
            With<PlayerControlled>,
            Without<LiraWRangeIndicator>,
            Without<LiraWAoeIndicator>,
        ),
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut range_query: Query<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<StandardMaterial>,
        ),
        (
            With<LiraWRangeIndicator>,
            Without<PlayerControlled>,
            Without<LiraWAoeIndicator>,
        ),
    >,
    mut aoe_query: Query<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<StandardMaterial>,
        ),
        (
            With<LiraWAoeIndicator>,
            Without<PlayerControlled>,
            Without<LiraWRangeIndicator>,
        ),
    >,
) {
    let w_pressed = keyboard.pressed(KeyCode::KeyW);
    if !w_pressed {
        indicator_state.suppress_until_release = false;
    }
    let show_indicator = w_pressed && !indicator_state.suppress_until_release;

    let Ok((mut range_transform, mut range_visibility, range_material)) = range_query.single_mut()
    else {
        return;
    };
    let Ok((mut aoe_transform, mut aoe_visibility, aoe_material)) = aoe_query.single_mut() else {
        return;
    };

    if let Some(mut material) = materials.get_mut(&range_material.0) {
        material.base_color = settings.color().with_alpha(settings.alpha * 0.45);
        material.emissive = settings.color().with_alpha(0.2).into();
    }
    if let Some(mut material) = materials.get_mut(&aoe_material.0) {
        material.base_color = settings.color();
        material.emissive = settings.color().with_alpha(0.34).into();
    }

    if !show_indicator {
        *range_visibility = Visibility::Hidden;
        *aoe_visibility = Visibility::Hidden;
        return;
    }

    let Ok((player_transform, visual)) = player_query.single() else {
        *range_visibility = Visibility::Hidden;
        *aoe_visibility = Visibility::Hidden;
        return;
    };
    if visual.champion != Some(ChampionId::LIRA) {
        *range_visibility = Visibility::Hidden;
        *aoe_visibility = Visibility::Hidden;
        return;
    }
    let Ok((map_transform, map_ground)) = map_query.single() else {
        *range_visibility = Visibility::Hidden;
        *aoe_visibility = Visibility::Hidden;
        return;
    };

    let origin_ground =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);
    let target_ground = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground)
        .map(|hit| {
            clamp_cast_target(
                origin_ground,
                hit,
                settings.range,
                map_transform,
                *map_ground,
            )
        })
        .unwrap_or(origin_ground);

    range_transform.translation = origin_ground + Vec3::Y * settings.elevation;
    range_transform.scale = Vec3::new(settings.range, settings.thickness, settings.range);

    aoe_transform.translation = target_ground + Vec3::Y * (settings.elevation + 0.02);
    aoe_transform.scale = Vec3::new(settings.aoe_radius, settings.thickness, settings.aoe_radius);

    *range_visibility = Visibility::Visible;
    *aoe_visibility = Visibility::Visible;
}
/// Spawns Lira's E contact missiles around the controlled player.
///
/// - `time`: Frame timing used to advance the E cooldown.
/// - `keyboard`: Keyboard input used to detect E casts.
/// - `settings`: E settings used for local prediction visuals.
/// - `player_query`: Controlled player transform used as the missile orbit origin.
/// - `cast_state`: E cooldown state.
/// - `command_senders`: Lightyear senders used to request the authoritative E cast.
/// - `meshes`: Mesh assets used to spawn missiles.
/// - `materials`: Material assets used to spawn missiles.
/// - `commands`: ECS command buffer used to spawn missiles.
pub(in crate::systems) fn cast_e_contact_missiles(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<LiraESettings>,
    player_query: Query<
        (Entity, &Transform, &Health, &CurrentChampionVisual),
        With<PlayerControlled>,
    >,
    mut cast_state: ResMut<LiraECastState>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    cast_state.cooldown.tick(time.delta());

    if !keyboard.just_pressed(KeyCode::KeyE) || !cast_state.cooldown.is_finished() {
        return;
    }

    let Ok((player_entity, player_transform, health, visual)) = player_query.single() else {
        return;
    };
    if health.current == 0 || visual.champion != Some(ChampionId::LIRA) {
        return;
    }

    spawn_e_missiles(
        &mut commands,
        &mut meshes,
        &mut materials,
        player_transform.translation,
        Some(player_entity),
        *settings,
    );

    send_ability_command(&mut command_senders, ChampionId::LIRA, AbilitySlot::E, None);

    cast_state.cooldown.reset();
}
/// Updates Lira E missile visuals, orbiting around Lira until they reach a visual target.
///
/// - `time`: Frame timing used to advance missile lifetime and movement.
/// - `commands`: ECS command buffer used to despawn expired or spent missiles.
/// - `player_query`: Controlled player transform used as the orbit origin.
/// - `transform_query`: Entity transforms used by visual-only remote missiles.
/// - `dummy_query`: Enemy dummy query used for visual target search.
/// - `missile_query`: Active E missiles to update.
pub(in crate::systems) fn update_e_contact_missiles(
    time: Res<Time>,
    mut commands: Commands,
    player_query: Query<
        (Entity, &Transform, &Team),
        (With<PlayerControlled>, Without<LiraEMissile>),
    >,
    transform_query: Query<&Transform, Without<LiraEMissile>>,
    team_query: Query<&Team, Without<LiraEMissile>>,
    dummy_query: Query<
        (Entity, &TrainingDummy, &Transform, Option<&RemoteLaneUnit>),
        Without<LiraEMissile>,
    >,
    mut missile_query: Query<
        (Entity, &mut LiraEMissile, &mut Transform),
        Without<PlayerControlled>,
    >,
) {
    let local_player = player_query.single().ok();

    for (entity, mut missile, mut missile_transform) in &mut missile_query {
        missile.lifetime.tick(time.delta());
        if missile.lifetime.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        let owner_position = missile
            .owner
            .and_then(|owner| transform_query.get(owner).ok())
            .map(|transform| transform.translation)
            .or(Some(missile.origin));
        let Some(owner_position) = owner_position else {
            commands.entity(entity).despawn();
            continue;
        };

        if missile.mode == LiraEMissileMode::Orbiting {
            let owner_team = missile.owner.and_then(|owner| team_query.get(owner).ok());
            let target = find_e_visual_target(
                &dummy_query,
                local_player,
                owner_team,
                missile.owner,
                owner_position,
                missile_transform.translation,
                missile.settings.search_radius,
            );

            if let Some(target) = target {
                missile.mode = LiraEMissileMode::Chasing(target);
            }
        }

        match missile.mode {
            LiraEMissileMode::Orbiting => {
                let elapsed = missile.lifetime.elapsed_secs();
                let angle = missile.phase + elapsed * missile.settings.orbit_speed;
                let offset = Vec3::new(angle.cos(), 0.0, angle.sin())
                    * missile.settings.orbit_radius
                    + Vec3::Y * missile.settings.orbit_height;
                missile_transform.translation = owner_position + offset;
            }
            LiraEMissileMode::Chasing(target) => {
                let Ok(target_transform) = transform_query.get(target) else {
                    commands.entity(entity).despawn();
                    continue;
                };

                let target_position = target_transform.translation + Vec3::Y * 0.7;
                let to_target = target_position - missile_transform.translation;
                let distance = to_target.length();

                if distance <= missile.settings.missile_radius + REMOTE_PLAYER_VISUAL_HIT_RADIUS {
                    commands.entity(entity).despawn();
                    continue;
                }

                move_e_missile_toward_target(
                    &time,
                    &mut missile_transform,
                    target_position,
                    missile.settings.chase_speed,
                );
            }
        }
    }
}
/// Finds the nearest valid visual-only target for a Lira E missile.
///
/// - `dummies`: Enemy target query used for remote player stand-ins.
/// - `local_player`: Optional local player target.
/// - `owner`: Optional owner entity to exclude from visual targeting.
/// - `owner_position`: Current owner position used for E search range.
/// - `missile_position`: Current missile position used to pick the nearest target.
/// - `search_radius`: Missile search radius in world units.
///
/// - Entity of the nearest visual target inside E search range.
fn find_e_visual_target(
    dummies: &Query<
        (Entity, &TrainingDummy, &Transform, Option<&RemoteLaneUnit>),
        Without<LiraEMissile>,
    >,
    local_player: Option<(Entity, &Transform, &Team)>,
    owner_team: Option<&Team>,
    owner: Option<Entity>,
    owner_position: Vec3,
    missile_position: Vec3,
    search_radius: f32,
) -> Option<Entity> {
    let dummy_target = dummies
        .iter()
        .filter(|(target_entity, dummy, dummy_transform, lane_unit)| {
            Some(*target_entity) != owner
                && dummy.health > 0.0
                && is_local_spell_target(*lane_unit)
                && horizontal_distance(owner_position, dummy_transform.translation) <= search_radius
        })
        .min_by(|(_, _, left, _), (_, _, right, _)| {
            horizontal_distance(missile_position, left.translation)
                .partial_cmp(&horizontal_distance(missile_position, right.translation))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(target_entity, _, transform, _)| {
            (
                target_entity,
                horizontal_distance(missile_position, transform.translation),
            )
        });

    let local_target = local_player
        .filter(|(player_entity, player_transform, player_team)| {
            Some(*player_entity) != owner
                && owner_team.is_some_and(|owner_team| owner_team.0 != player_team.0)
                && horizontal_distance(owner_position, player_transform.translation)
                    <= search_radius
        })
        .map(|(player_entity, transform, _)| {
            (
                player_entity,
                horizontal_distance(missile_position, transform.translation),
            )
        });

    match (dummy_target, local_target) {
        (Some((dummy_entity, dummy_distance)), Some((player_entity, player_distance))) => {
            if dummy_distance <= player_distance {
                Some(dummy_entity)
            } else {
                Some(player_entity)
            }
        }
        (Some((dummy_entity, _)), None) => Some(dummy_entity),
        (None, Some((player_entity, _))) => Some(player_entity),
        (None, None) => None,
    }
}
/// Moves one Lira E missile toward its current target position.
///
/// - `time`: Frame timing used for frame-rate independent movement.
/// - `missile_transform`: Missile transform to move.
/// - `target_position`: World-space target position to chase.
/// - `chase_speed`: Missile chase speed in world units per second.
fn move_e_missile_toward_target(
    time: &Time,
    missile_transform: &mut Transform,
    target_position: Vec3,
    chase_speed: f32,
) {
    let to_target = target_position - missile_transform.translation;
    let distance = to_target.length();

    if distance > f32::EPSILON {
        let step = chase_speed * time.delta_secs();
        missile_transform.translation += to_target.normalize() * step.min(distance);
    }
}
/// Receives remote ability visual events and spawns local visual-only spell entities.
///
/// - `receivers`: Lightyear receivers containing remote ability visuals from the server.
/// - `q_settings`: Q settings used to size remote Q visuals.
/// - `w_settings`: W settings used to size remote W visuals.
/// - `target_queries`: Remote-player and full Sophia spell-target queries used by visuals.
/// - `meshes`: Mesh assets used to spawn spell visuals.
/// - `materials`: Material assets used to spawn spell visuals.
/// - `commands`: ECS command buffer used to spawn spell visuals.
pub(in crate::systems) fn receive_remote_ability_visuals(
    mut receivers: Query<&mut MessageReceiver<AbilityVisualEvent>, With<Client>>,
    q_settings: Res<LiraQSettings>,
    w_settings: Res<LiraWSettings>,
    ignara_q_settings: Res<IgnaraQSettings>,
    ignara_w_settings: Res<IgnaraWSettings>,
    ignara_e_settings: Res<IgnaraESettings>,
    yuna_q_settings: Res<YunaQSettings>,
    yuna_w_settings: Res<YunaWSettings>,
    yuna_e_settings: Res<YunaESettings>,
    sophia_q_settings: Res<SophiaQSettings>,
    sophia_w_settings: Res<SophiaWSettings>,
    sophia_e_settings: Res<SophiaESettings>,
    mut target_queries: ParamSet<(
        Query<(Entity, &Player, &Transform), Without<PlayerControlled>>,
        Query<(
            Entity,
            Option<&Player>,
            &Team,
            &Health,
            &Transform,
            Option<&RemoteLaneUnit>,
        )>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for mut receiver in &mut receivers {
        for event in receiver.receive() {
            if event.champion == ChampionId::IGNARA {
                let remote_players = target_queries.p0();
                super::ignara::spawn_remote_ability_visual(
                    event,
                    &ignara_q_settings,
                    &ignara_w_settings,
                    &ignara_e_settings,
                    &remote_players,
                    &mut meshes,
                    &mut materials,
                    &mut commands,
                );
                continue;
            }

            if event.champion == ChampionId::YUNA {
                let remote_players = target_queries.p0();
                super::yuna::spawn_remote_ability_visual(
                    event,
                    &yuna_q_settings,
                    &yuna_w_settings,
                    &yuna_e_settings,
                    &remote_players,
                    &mut meshes,
                    &mut materials,
                    &mut commands,
                );
                continue;
            }

            if event.champion == ChampionId::SOPHIA {
                let sophia_targets = target_queries.p1();
                super::sophia::spawn_remote_ability_visual(
                    event,
                    &sophia_q_settings,
                    &sophia_w_settings,
                    &sophia_e_settings,
                    &sophia_targets,
                    &mut meshes,
                    &mut materials,
                    &mut commands,
                );
                continue;
            }

            if event.champion != ChampionId::LIRA {
                continue;
            }
            match event.slot {
                AbilitySlot::Q => {
                    let Some(end) = event.end else {
                        continue;
                    };
                    spawn_q_projectile(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        Vec3::from(event.start),
                        Vec3::from(end),
                        event
                            .visual
                            .projectile_radius
                            .max((q_settings.width * 0.5).max(0.05)),
                        visual_travel_seconds(event.visual, q_settings.travel_seconds),
                        positive_or(event.visual.explosion_radius, q_settings.explosion_radius),
                    );
                }
                AbilitySlot::W => {
                    let Some(end) = event.end else {
                        continue;
                    };
                    spawn_w_projectile(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        Vec3::from(event.start),
                        Vec3::from(end),
                        event.visual.explosion_radius.max(w_settings.aoe_radius),
                        visual_travel_seconds(event.visual, w_settings.travel_seconds),
                    );
                }
                AbilitySlot::E => {
                    let remote_players = target_queries.p0();
                    let owner = remote_players
                        .iter()
                        .find(|(_, player, _)| player.id.0 == event.caster_player_id)
                        .map(|(entity, _, _)| entity);
                    spawn_e_missiles(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        Vec3::from(event.start),
                        owner,
                        LiraESettings::from_visual(event.visual),
                    );
                }
                AbilitySlot::R => {}
            }
        }
    }
}
/// Spawns a Lira Q projectile visual.
///
/// - `commands`: ECS command buffer used to spawn the projectile.
/// - `meshes`: Mesh assets used for the projectile sphere.
/// - `materials`: Material assets used for the projectile material.
/// - `start`: Projectile start position.
/// - `end`: Projectile end position.
/// - `projectile_radius`: Projectile visual radius.
/// - `travel_seconds`: Projectile visual travel duration.
/// - `explosion_radius`: Radius applied by the impact explosion visual.
fn spawn_q_projectile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    projectile_radius: f32,
    travel_seconds: f32,
    explosion_radius: f32,
) {
    let projectile_material = materials.add(white_material(0.45, 0.16));

    commands.spawn((
        Name::new("LiraQProjectile"),
        LiraQProjectile {
            start,
            end,
            timer: Timer::from_seconds(travel_seconds.max(f32::EPSILON), TimerMode::Once),
            explosion_radius,
        },
        Mesh3d(meshes.add(Sphere::new(projectile_radius))),
        MeshMaterial3d(projectile_material),
        Transform::from_translation(start),
    ));
}
/// Spawns a Lira W arcing projectile visual.
///
/// - `commands`: ECS command buffer used to spawn the projectile.
/// - `meshes`: Mesh assets used for the projectile sphere.
/// - `materials`: Material assets used for the projectile material.
/// - `start`: Projectile start position.
/// - `end`: Projectile landing position.
/// - `explosion_radius`: Radius passed to the landing explosion.
/// - `travel_seconds`: Projectile visual travel duration.
fn spawn_w_projectile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    explosion_radius: f32,
    travel_seconds: f32,
) {
    let projectile_material = materials.add(white_material(0.85, 0.45));

    commands.spawn((
        Name::new("LiraWProjectile"),
        LiraWProjectile {
            start,
            end,
            timer: Timer::from_seconds(travel_seconds.max(f32::EPSILON), TimerMode::Once),
            arc_height: W_PROJECTILE_ARC_HEIGHT,
            explosion_radius,
        },
        Mesh3d(meshes.add(Sphere::new(0.32))),
        MeshMaterial3d(projectile_material),
        Transform::from_translation(start),
    ));
}
/// Spawns Lira E missile visuals around an owner position.
///
/// - `commands`: ECS command buffer used to spawn missiles.
/// - `meshes`: Mesh assets used for missile spheres.
/// - `materials`: Material assets used for missile materials.
/// - `origin`: Initial owner position used before owner tracking updates.
/// - `owner`: Optional owner entity to orbit for remote visuals.
/// - `settings`: E settings used for missile count, size, and movement.
fn spawn_e_missiles(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    origin: Vec3,
    owner: Option<Entity>,
    settings: LiraESettings,
) {
    let missile_material = materials.add(white_material(0.92, 0.6));
    let missile_count = settings.missile_count.max(1);
    for index in 0..missile_count {
        let phase = index as f32 / missile_count as f32 * std::f32::consts::TAU;
        let offset = Vec3::new(phase.cos(), 0.0, phase.sin()) * settings.orbit_radius
            + Vec3::Y * settings.orbit_height;

        commands.spawn((
            Name::new("LiraEMissile"),
            LiraEMissile {
                phase,
                lifetime: Timer::from_seconds(
                    settings.lifetime_seconds.max(f32::EPSILON),
                    TimerMode::Once,
                ),
                mode: LiraEMissileMode::Orbiting,
                origin,
                owner,
                settings,
            },
            Mesh3d(meshes.add(Sphere::new(settings.missile_radius))),
            MeshMaterial3d(missile_material.clone()),
            Transform::from_translation(origin + offset),
        ));
    }
}
/// Returns server-provided visual travel seconds with a local fallback.
///
/// - `visual`: Visual tuning attached to an accepted server ability cast.
/// - `fallback`: Local fallback travel duration.
///
/// - Positive travel duration used by local rendering.
fn visual_travel_seconds(visual: AbilityVisualTuning, fallback: f32) -> f32 {
    positive_or(visual.travel_seconds, fallback)
}
/// Clamps a cast target to spell range and map bounds.
///
/// - `origin`: Spell origin position.
/// - `target`: Requested target position.
/// - `range`: Maximum allowed distance from origin to target.
/// - `map_transform`: Map transform used to convert between world and local space.
/// - `map_ground`: Map bounds and top-surface data.
///
/// - A world-space target point clamped to range and map bounds.
fn clamp_cast_target(
    origin: Vec3,
    target: Vec3,
    range: f32,
    map_transform: &GlobalTransform,
    map_ground: MapGround,
) -> Vec3 {
    let flat_delta = Vec3::new(target.x - origin.x, 0.0, target.z - origin.z);
    let ranged_target = if flat_delta.length() > range {
        origin + flat_delta.normalize() * range
    } else {
        target
    };

    clamp_world_point_to_map_top(ranged_target, map_transform, map_ground)
}
/// Creates a white unlit material with configurable base and emissive opacity.
///
/// - `alpha`: Base color opacity.
/// - `emissive_alpha`: Emissive color opacity.
///
/// - A configured white `StandardMaterial`.
fn white_material(alpha: f32, emissive_alpha: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, alpha),
        emissive: Color::srgba(1.0, 1.0, 1.0, emissive_alpha).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}
