use crate::systems::{
    CurrentChampionVisual, TrainingDummy,
    targeting::{clamp_world_point_to_map_top, ray_hit_map_top},
};
use bevy::ecs::query::QueryFilter;
use bevy::math::primitives::{Cuboid, Cylinder, Sphere};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_shared::game::{
    camera::TopDownCamera,
    map::MapGround,
    player::{Health, Player, PlayerControlled},
};
use game_shared::network::{
    AbilitySlot, AbilityVisualEvent, CastTarget, ChampionId, PlayerCommand, ReliableCommandChannel,
    WorldPosition,
};
use lightyear::prelude::*;

const IGNARA_CHAMPION_ID: ChampionId = ChampionId(6607);

const Q_COOLDOWN_SECONDS: f32 = 7.0;
const Q_WIDTH: f32 = 3.0;

const Q_RANGE: f32 = 8.0;
const Q_LIFETIME_SECONDS: f32 = 3.0;
const Q_ELEVATION: f32 = 0.075;

const W_COOLDOWN_SECONDS: f32 = 6.0;
const W_RANGE: f32 = 9.0;
const W_TARGET_CLICK_RADIUS: f32 = 1.4;
const W_TRAVEL_SECONDS: f32 = 0.55;
const W_FIREBALL_RADIUS: f32 = 0.34;

const E_COOLDOWN_SECONDS: f32 = 9.0;
const E_WIDTH: f32 = 1.5;
const E_RANGE: f32 = 10.0;
const E_TRAVEL_SECONDS: f32 = 0.95;
const E_SMALL_DISTANCE: f32 = 2.5;
const E_MEDIUM_DISTANCE: f32 = 6.75;
const E_SMALL_DAMAGE_MIN: f32 = 45.0;
const E_SMALL_DAMAGE_MAX: f32 = 65.0;
const E_MEDIUM_DAMAGE_MIN: f32 = 100.0;
const E_MEDIUM_DAMAGE_MAX: f32 = 135.0;
const E_LARGE_DAMAGE_MIN: f32 = 140.0;
const E_LARGE_DAMAGE_MAX: f32 = 220.0;

#[derive(Resource, Debug, Clone, Copy)]
/// Description:
/// Stores local visual and cast tuning for Ignara's Q burning ground.
pub(in crate::systems) struct IgnaraQSettings {
    pub(in crate::systems) width: f32,
    pub(in crate::systems) range: f32,
    pub(in crate::systems) lifetime_seconds: f32,
}

impl Default for IgnaraQSettings {
    fn default() -> Self {
        Self {
            width: Q_WIDTH,
            range: Q_RANGE,
            lifetime_seconds: Q_LIFETIME_SECONDS,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
/// Description:
/// Stores local visual and cast tuning for Ignara's W fireball.
pub(in crate::systems) struct IgnaraWSettings {
    pub(in crate::systems) range: f32,
    pub(in crate::systems) travel_seconds: f32,
    pub(in crate::systems) radius: f32,
}

impl Default for IgnaraWSettings {
    fn default() -> Self {
        Self {
            range: W_RANGE,
            travel_seconds: W_TRAVEL_SECONDS,
            radius: W_FIREBALL_RADIUS,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
/// Description:
/// Stores local visual and cast tuning for Ignara's E growing snowball.
pub(in crate::systems) struct IgnaraESettings {
    pub(in crate::systems) width: f32,
    pub(in crate::systems) range: f32,
    pub(in crate::systems) travel_seconds: f32,
}

impl Default for IgnaraESettings {
    fn default() -> Self {
        Self {
            width: E_WIDTH,
            range: E_RANGE,
            travel_seconds: E_TRAVEL_SECONDS,
        }
    }
}

#[derive(Resource, Debug, Clone)]
/// Description:
/// Stores cooldown state for Ignara's Q ability.
pub(in crate::systems) struct IgnaraQCastState {
    cooldown: Timer,
}

#[derive(Resource, Debug, Clone)]
/// Description:
/// Stores cooldown state for Ignara's W ability.
pub(in crate::systems) struct IgnaraWCastState {
    cooldown: Timer,
}

#[derive(Resource, Debug, Clone)]
/// Description:
/// Stores cooldown state for Ignara's E ability.
pub(in crate::systems) struct IgnaraECastState {
    cooldown: Timer,
}

impl Default for IgnaraQCastState {
    fn default() -> Self {
        Self::ready(Q_COOLDOWN_SECONDS)
    }
}

impl Default for IgnaraWCastState {
    fn default() -> Self {
        Self::ready(W_COOLDOWN_SECONDS)
    }
}

impl Default for IgnaraECastState {
    fn default() -> Self {
        Self::ready(E_COOLDOWN_SECONDS)
    }
}

impl IgnaraQCastState {
    pub(in crate::systems) fn ready(cooldown_seconds: f32) -> Self {
        Self {
            cooldown: ready_timer(cooldown_seconds),
        }
    }

    /// Description:
    /// Returns the remaining Q cooldown duration.
    pub(in crate::systems) fn remaining_seconds(&self) -> f32 {
        remaining_timer_seconds(&self.cooldown)
    }

    /// Description:
    /// Returns the total Q cooldown duration.
    pub(in crate::systems) fn total_seconds(&self) -> f32 {
        total_timer_seconds(&self.cooldown)
    }

    /// Description:
    /// Returns how ready Q is as a percentage.
    pub(in crate::systems) fn ready_percent(&self) -> f32 {
        ready_timer_percent(&self.cooldown)
    }
}

impl IgnaraWCastState {
    pub(in crate::systems) fn ready(cooldown_seconds: f32) -> Self {
        Self {
            cooldown: ready_timer(cooldown_seconds),
        }
    }

    /// Description:
    /// Returns the remaining W cooldown duration.
    pub(in crate::systems) fn remaining_seconds(&self) -> f32 {
        remaining_timer_seconds(&self.cooldown)
    }

    /// Description:
    /// Returns the total W cooldown duration.
    pub(in crate::systems) fn total_seconds(&self) -> f32 {
        total_timer_seconds(&self.cooldown)
    }

    /// Description:
    /// Returns how ready W is as a percentage.
    pub(in crate::systems) fn ready_percent(&self) -> f32 {
        ready_timer_percent(&self.cooldown)
    }
}

impl IgnaraECastState {
    pub(in crate::systems) fn ready(cooldown_seconds: f32) -> Self {
        Self {
            cooldown: ready_timer(cooldown_seconds),
        }
    }

    /// Description:
    /// Returns the remaining E cooldown duration.
    pub(in crate::systems) fn remaining_seconds(&self) -> f32 {
        remaining_timer_seconds(&self.cooldown)
    }

    /// Description:
    /// Returns the total E cooldown duration.
    pub(in crate::systems) fn total_seconds(&self) -> f32 {
        total_timer_seconds(&self.cooldown)
    }

    /// Description:
    /// Returns how ready E is as a percentage.
    pub(in crate::systems) fn ready_percent(&self) -> f32 {
        ready_timer_percent(&self.cooldown)
    }
}

#[derive(Component, Debug, Clone)]
/// Description:
/// Stores runtime visual state for Ignara's pulsing Q burning ground.
pub(in crate::systems) struct IgnaraQBurningGround {
    timer: Timer,
    width: f32,
    length: f32,
}

#[derive(Component, Debug, Clone)]
/// Description:
/// Stores runtime visual state for Ignara's W fireball projectile.
pub(in crate::systems) struct IgnaraWFireball {
    start: Vec3,
    end: Vec3,
    timer: Timer,
}

#[derive(Component, Debug, Clone)]
/// Description:
/// Stores runtime visual state for Ignara's E growing snowball projectile.
pub(in crate::systems) struct IgnaraESnowball {
    start: Vec3,
    end: Vec3,
    timer: Timer,
    width: f32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Description:
/// Marks Ignara's rectangular Q cast preview.
pub(in crate::systems) struct IgnaraQIndicator;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Description:
/// Marks Ignara's W cast range preview.
pub(in crate::systems) struct IgnaraWRangeIndicator;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Description:
/// Marks Ignara's W target hover preview.
pub(in crate::systems) struct IgnaraWTargetIndicator;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Description:
/// Marks Ignara's rectangular E cast preview.
pub(in crate::systems) struct IgnaraEIndicator;

/// Description:
/// Spawns Ignara-only prediction indicators.
pub(in crate::systems) fn spawn_ignara_indicators(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let q_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.06, 0.02, 0.46),
        emissive: Color::srgb(0.75, 0.03, 0.01).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let w_range_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.3, 0.04, 0.18),
        emissive: Color::srgb(0.35, 0.08, 0.01).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let w_target_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.18, 0.02, 0.62),
        emissive: Color::srgb(1.0, 0.12, 0.01).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let e_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.42, 0.08, 0.42),
        emissive: Color::srgb(0.8, 0.18, 0.02).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands.spawn((
        Name::new("IgnaraQIndicator"),
        IgnaraQIndicator,
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
        MeshMaterial3d(q_material),
        Transform::from_xyz(0.0, Q_ELEVATION, 0.0),
        Visibility::Hidden,
    ));

    commands.spawn((
        Name::new("IgnaraWRangeIndicator"),
        IgnaraWRangeIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(w_range_material),
        Transform::from_xyz(0.0, Q_ELEVATION, 0.0),
        Visibility::Hidden,
    ));

    commands.spawn((
        Name::new("IgnaraWTargetIndicator"),
        IgnaraWTargetIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(w_target_material),
        Transform::from_xyz(0.0, Q_ELEVATION + 0.025, 0.0),
        Visibility::Hidden,
    ));

    commands.spawn((
        Name::new("IgnaraEIndicator"),
        IgnaraEIndicator,
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
        MeshMaterial3d(e_material),
        Transform::from_xyz(0.0, Q_ELEVATION + 0.015, 0.0),
        Visibility::Hidden,
    ));
}

/// Description:
/// Casts Ignara Q as a rectangular burning ground in the aimed direction.
pub(in crate::systems) fn cast_q_burning_ground(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<IgnaraQSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<(&Transform, &Health, &CurrentChampionVisual), With<PlayerControlled>>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cast_state: ResMut<IgnaraQCastState>,
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
    if health.current == 0 || visual.champion != Some(IGNARA_CHAMPION_ID) {
        return;
    }

    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };
    let origin_ground =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);
    let cursor_hit = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground);
    let direction = aim_direction(origin_ground, cursor_hit);
    let end_ground = clamp_world_point_to_map_top(
        origin_ground + direction * settings.range,
        map_transform,
        *map_ground,
    );

    spawn_q_burning_ground(
        &mut commands,
        &mut meshes,
        &mut materials,
        origin_ground,
        end_ground,
        *settings,
    );
    send_ability_command(&mut command_senders, AbilitySlot::Q, Some(end_ground));
    cast_state.cooldown.reset();
}

/// Description:
/// Casts Ignara W as a point-click fireball toward the clicked position.
pub(in crate::systems) fn cast_w_fireball(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<IgnaraWSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<(&Transform, &Health, &CurrentChampionVisual), With<PlayerControlled>>,
    enemy_query: Query<(&TrainingDummy, &Transform)>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cast_state: ResMut<IgnaraWCastState>,
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
    if health.current == 0 || visual.champion != Some(IGNARA_CHAMPION_ID) {
        return;
    }
    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };
    let Some(cursor_hit) = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground)
    else {
        return;
    };

    let origin_ground =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);
    let Some((_, enemy_transform)) =
        find_clicked_enemy_target(cursor_hit, &enemy_query, W_TARGET_CLICK_RADIUS)
    else {
        return;
    };
    if horizontal_distance(origin_ground, enemy_transform.translation) > settings.range {
        return;
    }
    let target_ground =
        clamp_cast_target(origin_ground, enemy_transform.translation, settings.range);
    let start = origin_ground + Vec3::Y * 0.75;
    let end = target_ground + Vec3::Y * 0.75;

    spawn_w_fireball(
        &mut commands,
        &mut meshes,
        &mut materials,
        start,
        end,
        *settings,
    );
    send_ability_command(&mut command_senders, AbilitySlot::W, Some(target_ground));
    cast_state.cooldown.reset();
}

/// Description:
/// Casts Ignara E as a rolling projectile that grows while travelling.
pub(in crate::systems) fn cast_e_snowball(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<IgnaraESettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<(&Transform, &Health, &CurrentChampionVisual), With<PlayerControlled>>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cast_state: ResMut<IgnaraECastState>,
    mut commands: Commands,
) {
    cast_state.cooldown.tick(time.delta());

    if !keyboard.pressed(KeyCode::KeyE) || !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if !cast_state.cooldown.is_finished() {
        return;
    }

    let Ok((player_transform, health, visual)) = player_query.single() else {
        return;
    };
    if health.current == 0 || visual.champion != Some(IGNARA_CHAMPION_ID) {
        return;
    }
    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };

    let origin_ground =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);
    let cursor_hit = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground);
    let direction = aim_direction(origin_ground, cursor_hit);
    let end_ground = clamp_world_point_to_map_top(
        origin_ground + direction * settings.range,
        map_transform,
        *map_ground,
    );

    spawn_e_snowball(
        &mut commands,
        &mut meshes,
        &mut materials,
        origin_ground + Vec3::Y * 0.45,
        end_ground + Vec3::Y * 0.45,
        *settings,
    );
    send_ability_command(&mut command_senders, AbilitySlot::E, Some(end_ground));
    cast_state.cooldown.reset();
}

/// Description:
/// Updates Ignara ability prediction indicators while Q/W/E aim keys are held.
pub(in crate::systems) fn update_ignara_indicators(
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<
        (&Transform, &CurrentChampionVisual),
        (
            With<PlayerControlled>,
            Without<IgnaraQIndicator>,
            Without<IgnaraWRangeIndicator>,
            Without<IgnaraWTargetIndicator>,
            Without<IgnaraEIndicator>,
        ),
    >,
    enemy_query: Query<
        (&TrainingDummy, &Transform),
        (
            Without<IgnaraQIndicator>,
            Without<IgnaraWRangeIndicator>,
            Without<IgnaraWTargetIndicator>,
            Without<IgnaraEIndicator>,
        ),
    >,
    q_settings: Res<IgnaraQSettings>,
    w_settings: Res<IgnaraWSettings>,
    e_settings: Res<IgnaraESettings>,
    mut indicator_query: Query<
        (
            &mut Transform,
            &mut Visibility,
            Has<IgnaraQIndicator>,
            Has<IgnaraWRangeIndicator>,
            Has<IgnaraWTargetIndicator>,
            Has<IgnaraEIndicator>,
        ),
        Or<(
            With<IgnaraQIndicator>,
            With<IgnaraWRangeIndicator>,
            With<IgnaraWTargetIndicator>,
            With<IgnaraEIndicator>,
        )>,
    >,
) {
    for (_, mut visibility, _, _, _, _) in &mut indicator_query {
        *visibility = Visibility::Hidden;
    }

    let Ok((player_transform, visual)) = player_query.single() else {
        return;
    };
    if visual.champion != Some(IGNARA_CHAMPION_ID) {
        return;
    }

    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };

    let origin =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);
    let cursor_hit = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground);
    let direction = aim_direction(origin, cursor_hit);

    if keyboard.pressed(KeyCode::KeyQ) {
        let end = clamp_world_point_to_map_top(
            origin + direction * q_settings.range,
            map_transform,
            *map_ground,
        );
        for (mut transform, mut visibility, is_q, _, _, _) in &mut indicator_query {
            if !is_q {
                continue;
            }
            set_rect_indicator(
                &mut transform,
                origin,
                end,
                q_settings.width,
                0.035,
                Q_ELEVATION,
            );
            *visibility = Visibility::Visible;
        }
    }

    if keyboard.pressed(KeyCode::KeyW) {
        for (mut transform, mut visibility, _, is_w_range, _, _) in &mut indicator_query {
            if !is_w_range {
                continue;
            }
            transform.translation = origin + Vec3::Y * Q_ELEVATION;
            transform.scale = Vec3::new(w_settings.range, 0.025, w_settings.range);
            *visibility = Visibility::Visible;
        }

        if let Some(cursor_hit) = cursor_hit
            && let Some((_, target_transform)) =
                find_clicked_enemy_target(cursor_hit, &enemy_query, W_TARGET_CLICK_RADIUS)
            && horizontal_distance(origin, target_transform.translation) <= w_settings.range
        {
            for (mut transform, mut visibility, _, _, is_w_target, _) in &mut indicator_query {
                if !is_w_target {
                    continue;
                }
                transform.translation =
                    target_transform.translation + Vec3::Y * (Q_ELEVATION + 0.025);
                transform.scale = Vec3::new(W_TARGET_CLICK_RADIUS, 0.035, W_TARGET_CLICK_RADIUS);
                *visibility = Visibility::Visible;
            }
        }
    }

    if keyboard.pressed(KeyCode::KeyE) {
        let end = clamp_world_point_to_map_top(
            origin + direction * e_settings.range,
            map_transform,
            *map_ground,
        );
        for (mut transform, mut visibility, _, _, _, is_e) in &mut indicator_query {
            if !is_e {
                continue;
            }
            set_rect_indicator(
                &mut transform,
                origin,
                end,
                e_settings.width,
                0.035,
                Q_ELEVATION + 0.015,
            );
            *visibility = Visibility::Visible;
        }
    }
}

/// Description:
/// Updates Ignara Q pulsing burning ground visuals.
pub(in crate::systems) fn update_q_burning_grounds(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(
        Entity,
        &mut IgnaraQBurningGround,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut ground, mut transform, material_handle) in &mut query {
        ground.timer.tick(time.delta());
        let duration = ground.timer.duration().as_secs_f32().max(f32::EPSILON);
        let progress = (ground.timer.elapsed_secs() / duration).clamp(0.0, 1.0);
        let pulse = (ground.timer.elapsed_secs() * 8.0).sin() * 0.5 + 0.5;
        transform.scale = Vec3::new(
            ground.width * (1.0 + pulse * 0.05),
            0.035,
            ground.length * (1.0 + pulse * 0.015),
        );

        if let Some(material) = materials.get_mut(&material_handle.0) {
            let alpha = (0.58 + pulse * 0.22) * (1.0 - progress * 0.35);
            material.base_color = Color::srgba(1.0, 0.08, 0.025, alpha);
            material.emissive = Color::srgb(0.75 + pulse * 0.35, 0.04, 0.01).into();
        }

        if ground.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Description:
/// Updates Ignara W fireball visuals.
pub(in crate::systems) fn update_w_fireballs(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(
        Entity,
        &mut IgnaraWFireball,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut fireball, mut transform, material_handle) in &mut query {
        fireball.timer.tick(time.delta());
        let duration = fireball.timer.duration().as_secs_f32().max(f32::EPSILON);
        let progress = (fireball.timer.elapsed_secs() / duration).clamp(0.0, 1.0);
        let arc = (std::f32::consts::PI * progress).sin() * 0.65;
        transform.translation = fireball.start.lerp(fireball.end, progress) + Vec3::Y * arc;
        transform.scale = Vec3::splat(1.0 + (progress * 18.0).sin().abs() * 0.16);

        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color = Color::srgba(1.0, 0.28, 0.02, 0.88);
            material.emissive = Color::srgb(1.2, 0.22, 0.02).into();
        }

        if fireball.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Description:
/// Updates Ignara E rolling snowball visuals.
pub(in crate::systems) fn update_e_snowballs(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(
        Entity,
        &mut IgnaraESnowball,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut snowball, mut transform, material_handle) in &mut query {
        snowball.timer.tick(time.delta());
        let duration = snowball.timer.duration().as_secs_f32().max(f32::EPSILON);
        let progress = (snowball.timer.elapsed_secs() / duration).clamp(0.0, 1.0);
        let travelled = snowball.start.distance(snowball.end) * progress;
        let radius = e_visual_radius(travelled, snowball.width);

        transform.translation = snowball.start.lerp(snowball.end, progress);
        transform.scale = Vec3::splat(radius.max(0.1));
        transform.rotate_local_x(time.delta_secs() * 7.5);

        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color = Color::srgba(1.0, 0.52, 0.12, 0.86);
            material.emissive = Color::srgb(0.95, 0.24, 0.02).into();
        }

        if snowball.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Description:
/// Spawns one remote Ignara ability visual from a server event.
pub(in crate::systems) fn spawn_remote_ability_visual(
    event: AbilityVisualEvent,
    q_settings: &IgnaraQSettings,
    w_settings: &IgnaraWSettings,
    e_settings: &IgnaraESettings,
    remote_players: &Query<(Entity, &Player, &Transform), Without<PlayerControlled>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    commands: &mut Commands,
) {
    if event.champion != IGNARA_CHAMPION_ID {
        return;
    }

    match event.slot {
        AbilitySlot::Q => {
            let Some(end) = event.end else {
                return;
            };
            spawn_q_burning_ground(
                commands,
                meshes,
                materials,
                Vec3::from(event.start),
                Vec3::from(end),
                *q_settings,
            );
        }
        AbilitySlot::W => {
            let Some(end) = event.end else {
                return;
            };
            spawn_w_fireball(
                commands,
                meshes,
                materials,
                Vec3::from(event.start),
                Vec3::from(end),
                *w_settings,
            );
        }
        AbilitySlot::E => {
            let Some(end) = event.end else {
                return;
            };
            let start = remote_players
                .iter()
                .find(|(_, player, _)| player.id.0 == event.caster_player_id)
                .map(|(_, _, transform)| transform.translation + Vec3::Y * 0.45)
                .unwrap_or_else(|| Vec3::from(event.start));
            spawn_e_snowball(
                commands,
                meshes,
                materials,
                start,
                Vec3::from(end),
                *e_settings,
            );
        }
        AbilitySlot::R => {}
    }
}

fn spawn_q_burning_ground(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    settings: IgnaraQSettings,
) {
    let to_end = end - start;
    let length = Vec2::new(to_end.x, to_end.z).length().max(0.1);
    let center = start + to_end * 0.5 + Vec3::Y * Q_ELEVATION;
    let yaw = to_end.x.atan2(to_end.z);
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.08, 0.025, 0.72),
        emissive: Color::srgb(0.95, 0.04, 0.01).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands.spawn((
        Name::new("IgnaraQBurningGround"),
        IgnaraQBurningGround {
            timer: Timer::from_seconds(
                settings.lifetime_seconds.max(f32::EPSILON),
                TimerMode::Once,
            ),
            width: settings.width,
            length,
        },
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
        MeshMaterial3d(material),
        Transform::from_translation(center)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(Vec3::new(settings.width, 0.035, length)),
    ));
}

fn spawn_w_fireball(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    settings: IgnaraWSettings,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.28, 0.02, 0.9),
        emissive: Color::srgb(1.2, 0.22, 0.02).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands.spawn((
        Name::new("IgnaraWFireball"),
        IgnaraWFireball {
            start,
            end,
            timer: Timer::from_seconds(settings.travel_seconds.max(f32::EPSILON), TimerMode::Once),
        },
        Mesh3d(meshes.add(Sphere::new(settings.radius))),
        MeshMaterial3d(material),
        Transform::from_translation(start),
    ));
}

fn spawn_e_snowball(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    settings: IgnaraESettings,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.52, 0.12, 0.88),
        emissive: Color::srgb(0.95, 0.24, 0.02).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands.spawn((
        Name::new("IgnaraESnowball"),
        IgnaraESnowball {
            start,
            end,
            timer: Timer::from_seconds(settings.travel_seconds.max(f32::EPSILON), TimerMode::Once),
            width: settings.width,
        },
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(material),
        Transform::from_translation(start).with_scale(Vec3::splat(0.35)),
    ));
}

fn ready_timer(cooldown_seconds: f32) -> Timer {
    let mut timer = Timer::from_seconds(cooldown_seconds.max(f32::EPSILON), TimerMode::Once);
    timer.set_elapsed(timer.duration());
    timer
}

fn total_timer_seconds(timer: &Timer) -> f32 {
    timer.duration().as_secs_f32().max(f32::EPSILON)
}

fn remaining_timer_seconds(timer: &Timer) -> f32 {
    (total_timer_seconds(timer) - timer.elapsed().as_secs_f32()).max(0.0)
}

fn ready_timer_percent(timer: &Timer) -> f32 {
    let total = total_timer_seconds(timer);
    ((total - remaining_timer_seconds(timer)) / total * 100.0).clamp(0.0, 100.0)
}

fn send_ability_command(
    senders: &mut Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    slot: AbilitySlot,
    target_position: Option<Vec3>,
) {
    for mut sender in senders {
        sender.send::<ReliableCommandChannel>(PlayerCommand::CastAbility {
            champion: IGNARA_CHAMPION_ID,
            slot,
            target: CastTarget {
                position: target_position.map(WorldPosition::from),
            },
        });
    }
}

fn cursor_hit_on_map(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera_query: &Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_transform: &GlobalTransform,
    map_ground: MapGround,
) -> Option<Vec3> {
    windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position())
        .and_then(|cursor| {
            camera_query
                .single()
                .ok()
                .and_then(|(camera, camera_transform)| {
                    camera.viewport_to_world(camera_transform, cursor).ok()
                })
        })
        .and_then(|ray| ray_hit_map_top(ray, map_transform, map_ground))
}

fn aim_direction(origin: Vec3, target: Option<Vec3>) -> Vec3 {
    target
        .map(|target| Vec3::new(target.x - origin.x, 0.0, target.z - origin.z))
        .filter(|delta| delta.length_squared() > f32::EPSILON)
        .map(|delta| delta.normalize())
        .unwrap_or(Vec3::Z)
}

fn clamp_cast_target(origin: Vec3, target: Vec3, range: f32) -> Vec3 {
    let delta = Vec3::new(target.x - origin.x, 0.0, target.z - origin.z);
    if delta.length_squared() <= range * range {
        return Vec3::new(target.x, origin.y, target.z);
    }

    origin + delta.normalize_or_zero() * range
}

fn set_rect_indicator(
    transform: &mut Transform,
    start: Vec3,
    end: Vec3,
    width: f32,
    thickness: f32,
    elevation: f32,
) {
    let to_end = end - start;
    let length = Vec2::new(to_end.x, to_end.z).length().max(0.1);
    let center = start + to_end * 0.5 + Vec3::Y * elevation;
    let yaw = to_end.x.atan2(to_end.z);

    transform.translation = center;
    transform.rotation = Quat::from_rotation_y(yaw);
    transform.scale = Vec3::new(width, thickness, length);
}

fn find_clicked_enemy_target<'a, F>(
    cursor_hit: Vec3,
    enemy_query: &'a Query<(&TrainingDummy, &Transform), F>,
    radius: f32,
) -> Option<(&'a TrainingDummy, &'a Transform)>
where
    F: QueryFilter,
{
    enemy_query
        .iter()
        .filter(|(dummy, transform)| {
            dummy.health > 0.0 && horizontal_distance(cursor_hit, transform.translation) <= radius
        })
        .min_by(|(_, left), (_, right)| {
            horizontal_distance(cursor_hit, left.translation)
                .partial_cmp(&horizontal_distance(cursor_hit, right.translation))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn e_visual_radius(travelled: f32, width: f32) -> f32 {
    let base_radius = width * 0.28;
    let progress = (travelled / E_RANGE).clamp(0.0, 1.0);
    base_radius * (1.0 + progress * 1.85)
}

fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

#[allow(dead_code)]
fn e_damage_for_distance(distance: f32) -> (f32, f32) {
    if distance < E_SMALL_DISTANCE {
        (E_SMALL_DAMAGE_MIN, E_SMALL_DAMAGE_MAX)
    } else if distance < E_MEDIUM_DISTANCE {
        (E_MEDIUM_DAMAGE_MIN, E_MEDIUM_DAMAGE_MAX)
    } else {
        (E_LARGE_DAMAGE_MIN, E_LARGE_DAMAGE_MAX)
    }
}
