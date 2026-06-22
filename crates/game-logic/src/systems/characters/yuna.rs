use crate::systems::{
    CurrentChampionVisual, TrainingDummy,
    targeting::{clamp_world_point_to_map_top, ray_hit_map_top},
};
use bevy::ecs::query::QueryFilter;
use bevy::math::primitives::{Cylinder, Sphere};
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

const YUNA_CHAMPION_ID: ChampionId = ChampionId(6608);

const Q_COOLDOWN_SECONDS: f32 = 8.0;
const Q_RANGE: f32 = 8.0;
const Q_ORB_RADIUS: f32 = 1.25;
const Q_AOE_RADIUS: f32 = 4.5;
const Q_FIELD_SECONDS: f32 = 3.0;
const Q_TRAVEL_SECONDS: f32 = 0.65;
const Q_ELEVATION: f32 = 0.08;

const W_COOLDOWN_SECONDS: f32 = 9.0;
const W_RADIUS: f32 = 4.5;
const W_FIELD_SECONDS: f32 = 3.0;

const E_COOLDOWN_SECONDS: f32 = 8.0;
const E_RANGE: f32 = 9.0;
const E_TARGET_CLICK_RADIUS: f32 = 1.4;
const E_TRAVEL_SECONDS: f32 = 0.45;
const E_PROJECTILE_RADIUS: f32 = 0.32;

#[derive(Resource, Debug, Clone, Copy)]
pub(in crate::systems) struct YunaQSettings {
    pub(in crate::systems) range: f32,
    pub(in crate::systems) orb_radius: f32,
    pub(in crate::systems) aoe_radius: f32,
    pub(in crate::systems) field_seconds: f32,
    pub(in crate::systems) travel_seconds: f32,
}

impl Default for YunaQSettings {
    fn default() -> Self {
        Self {
            range: Q_RANGE,
            orb_radius: Q_ORB_RADIUS,
            aoe_radius: Q_AOE_RADIUS,
            field_seconds: Q_FIELD_SECONDS,
            travel_seconds: Q_TRAVEL_SECONDS,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(in crate::systems) struct YunaWSettings {
    pub(in crate::systems) radius: f32,
    pub(in crate::systems) field_seconds: f32,
}

impl Default for YunaWSettings {
    fn default() -> Self {
        Self {
            radius: W_RADIUS,
            field_seconds: W_FIELD_SECONDS,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(in crate::systems) struct YunaESettings {
    pub(in crate::systems) range: f32,
    pub(in crate::systems) travel_seconds: f32,
    pub(in crate::systems) projectile_radius: f32,
}

impl Default for YunaESettings {
    fn default() -> Self {
        Self {
            range: E_RANGE,
            travel_seconds: E_TRAVEL_SECONDS,
            projectile_radius: E_PROJECTILE_RADIUS,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(in crate::systems) struct YunaQCastState {
    cooldown: Timer,
}

#[derive(Resource, Debug, Clone)]
pub(in crate::systems) struct YunaWCastState {
    cooldown: Timer,
}

#[derive(Resource, Debug, Clone)]
pub(in crate::systems) struct YunaECastState {
    cooldown: Timer,
}

impl Default for YunaQCastState {
    fn default() -> Self {
        Self::ready(Q_COOLDOWN_SECONDS)
    }
}

impl Default for YunaWCastState {
    fn default() -> Self {
        Self::ready(W_COOLDOWN_SECONDS)
    }
}

impl Default for YunaECastState {
    fn default() -> Self {
        Self::ready(E_COOLDOWN_SECONDS)
    }
}

impl YunaQCastState {
    pub(in crate::systems) fn ready(cooldown_seconds: f32) -> Self {
        Self {
            cooldown: ready_timer(cooldown_seconds),
        }
    }

    pub(in crate::systems) fn remaining_seconds(&self) -> f32 {
        remaining_timer_seconds(&self.cooldown)
    }

    pub(in crate::systems) fn total_seconds(&self) -> f32 {
        total_timer_seconds(&self.cooldown)
    }

    pub(in crate::systems) fn ready_percent(&self) -> f32 {
        ready_timer_percent(&self.cooldown)
    }
}

impl YunaWCastState {
    pub(in crate::systems) fn ready(cooldown_seconds: f32) -> Self {
        Self {
            cooldown: ready_timer(cooldown_seconds),
        }
    }

    pub(in crate::systems) fn remaining_seconds(&self) -> f32 {
        remaining_timer_seconds(&self.cooldown)
    }

    pub(in crate::systems) fn total_seconds(&self) -> f32 {
        total_timer_seconds(&self.cooldown)
    }

    pub(in crate::systems) fn ready_percent(&self) -> f32 {
        ready_timer_percent(&self.cooldown)
    }
}

impl YunaECastState {
    pub(in crate::systems) fn ready(cooldown_seconds: f32) -> Self {
        Self {
            cooldown: ready_timer(cooldown_seconds),
        }
    }

    pub(in crate::systems) fn remaining_seconds(&self) -> f32 {
        remaining_timer_seconds(&self.cooldown)
    }

    pub(in crate::systems) fn total_seconds(&self) -> f32 {
        total_timer_seconds(&self.cooldown)
    }

    pub(in crate::systems) fn ready_percent(&self) -> f32 {
        ready_timer_percent(&self.cooldown)
    }
}

#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct YunaQProjectile {
    start: Vec3,
    end: Vec3,
    timer: Timer,
    settings: YunaQSettings,
}

#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct YunaQOrb {
    timer: Timer,
    radius: f32,
}

#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct YunaQField {
    timer: Timer,
    radius: f32,
}

#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct YunaWField {
    timer: Timer,
    radius: f32,
    caster_player_id: Option<u64>,
}

#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct YunaEStunBolt {
    start: Vec3,
    end: Vec3,
    timer: Timer,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct YunaQRangeIndicator;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct YunaQTargetIndicator;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct YunaWIndicator;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct YunaERangeIndicator;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct YunaETargetIndicator;

pub(in crate::systems) fn spawn_yuna_indicators(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let q_range_material = materials.add(indicator_material(
        Color::srgba(0.25, 0.8, 1.0, 0.18),
        Color::srgb(0.05, 0.35, 0.65),
    ));
    let q_target_material = materials.add(indicator_material(
        Color::srgba(0.25, 0.95, 1.0, 0.42),
        Color::srgb(0.08, 0.65, 0.9),
    ));
    let w_material = materials.add(indicator_material(
        Color::srgba(0.15, 1.0, 0.55, 0.3),
        Color::srgb(0.02, 0.55, 0.18),
    ));
    let e_range_material = materials.add(indicator_material(
        Color::srgba(0.35, 0.65, 1.0, 0.16),
        Color::srgb(0.08, 0.18, 0.65),
    ));
    let e_target_material = materials.add(indicator_material(
        Color::srgba(0.45, 0.82, 1.0, 0.62),
        Color::srgb(0.12, 0.42, 1.0),
    ));

    commands.spawn((
        Name::new("YunaQRangeIndicator"),
        YunaQRangeIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(q_range_material),
        Transform::from_xyz(0.0, Q_ELEVATION, 0.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        Name::new("YunaQTargetIndicator"),
        YunaQTargetIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(q_target_material),
        Transform::from_xyz(0.0, Q_ELEVATION + 0.025, 0.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        Name::new("YunaWIndicator"),
        YunaWIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(w_material),
        Transform::from_xyz(0.0, Q_ELEVATION + 0.05, 0.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        Name::new("YunaERangeIndicator"),
        YunaERangeIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(e_range_material),
        Transform::from_xyz(0.0, Q_ELEVATION + 0.075, 0.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        Name::new("YunaETargetIndicator"),
        YunaETargetIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(e_target_material),
        Transform::from_xyz(0.0, Q_ELEVATION + 0.1, 0.0),
        Visibility::Hidden,
    ));
}

pub(in crate::systems) fn cast_q_gravity_orb(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<YunaQSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<(&Transform, &Health, &CurrentChampionVisual), With<PlayerControlled>>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cast_state: ResMut<YunaQCastState>,
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
    if health.current == 0 || visual.champion != Some(YUNA_CHAMPION_ID) {
        return;
    }

    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };
    let Some(cursor_hit) = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground)
    else {
        return;
    };
    let origin =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);
    let target = clamp_world_point_to_map_top(
        clamp_cast_target(origin, cursor_hit, settings.range),
        map_transform,
        *map_ground,
    );
    let start = origin + Vec3::Y * 0.8;
    let end = target + Vec3::Y * 0.55;

    spawn_q_projectile(
        &mut commands,
        &mut meshes,
        &mut materials,
        start,
        end,
        *settings,
    );
    send_ability_command(&mut command_senders, AbilitySlot::Q, Some(target));
    cast_state.cooldown.reset();
}

pub(in crate::systems) fn cast_w_healing_field(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<YunaWSettings>,
    player_query: Query<
        (&Player, &Transform, &Health, &CurrentChampionVisual),
        With<PlayerControlled>,
    >,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cast_state: ResMut<YunaWCastState>,
    mut commands: Commands,
) {
    cast_state.cooldown.tick(time.delta());
    if !keyboard.just_pressed(KeyCode::KeyW) || !cast_state.cooldown.is_finished() {
        return;
    }

    let Ok((player, player_transform, health, visual)) = player_query.single() else {
        return;
    };
    if health.current == 0 || visual.champion != Some(YUNA_CHAMPION_ID) {
        return;
    }

    let center = Vec3::new(
        player_transform.translation.x,
        0.0,
        player_transform.translation.z,
    );
    spawn_w_field(
        &mut commands,
        &mut meshes,
        &mut materials,
        center,
        *settings,
        Some(player.id.0),
    );
    send_ability_command(&mut command_senders, AbilitySlot::W, Some(center));
    cast_state.cooldown.reset();
}

pub(in crate::systems) fn cast_e_stun_bolt(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<YunaESettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<(&Transform, &Health, &CurrentChampionVisual), With<PlayerControlled>>,
    enemy_query: Query<(&TrainingDummy, &Transform)>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cast_state: ResMut<YunaECastState>,
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
    if health.current == 0 || visual.champion != Some(YUNA_CHAMPION_ID) {
        return;
    }

    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };
    let Some(cursor_hit) = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground)
    else {
        return;
    };
    let origin =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);
    let Some((_, target_transform)) =
        find_clicked_enemy_target(cursor_hit, &enemy_query, E_TARGET_CLICK_RADIUS)
    else {
        return;
    };
    if horizontal_distance(origin, target_transform.translation) > settings.range {
        return;
    }

    let target = clamp_cast_target(origin, target_transform.translation, settings.range);
    spawn_e_stun_bolt(
        &mut commands,
        &mut meshes,
        &mut materials,
        origin + Vec3::Y * 0.85,
        target + Vec3::Y * 0.85,
        *settings,
    );
    send_ability_command(&mut command_senders, AbilitySlot::E, Some(target));
    cast_state.cooldown.reset();
}

pub(in crate::systems) fn update_yuna_indicators(
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<
        (&Transform, &CurrentChampionVisual),
        (
            With<PlayerControlled>,
            Without<YunaQRangeIndicator>,
            Without<YunaQTargetIndicator>,
            Without<YunaWIndicator>,
            Without<YunaERangeIndicator>,
            Without<YunaETargetIndicator>,
        ),
    >,
    enemy_query: Query<
        (&TrainingDummy, &Transform),
        (
            Without<YunaQRangeIndicator>,
            Without<YunaQTargetIndicator>,
            Without<YunaWIndicator>,
            Without<YunaERangeIndicator>,
            Without<YunaETargetIndicator>,
        ),
    >,
    q_settings: Res<YunaQSettings>,
    w_settings: Res<YunaWSettings>,
    e_settings: Res<YunaESettings>,
    mut indicator_query: Query<
        (
            &mut Transform,
            &mut Visibility,
            Has<YunaQRangeIndicator>,
            Has<YunaQTargetIndicator>,
            Has<YunaWIndicator>,
            Has<YunaERangeIndicator>,
            Has<YunaETargetIndicator>,
        ),
        Or<(
            With<YunaQRangeIndicator>,
            With<YunaQTargetIndicator>,
            With<YunaWIndicator>,
            With<YunaERangeIndicator>,
            With<YunaETargetIndicator>,
        )>,
    >,
) {
    for (_, mut visibility, _, _, _, _, _) in &mut indicator_query {
        *visibility = Visibility::Hidden;
    }

    let Ok((player_transform, visual)) = player_query.single() else {
        return;
    };
    if visual.champion != Some(YUNA_CHAMPION_ID) {
        return;
    }

    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };
    let origin =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);
    let cursor_hit = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground);

    if keyboard.pressed(KeyCode::KeyQ) {
        let target = cursor_hit
            .map(|hit| clamp_cast_target(origin, hit, q_settings.range))
            .unwrap_or(origin);
        for (mut transform, mut visibility, is_q_range, is_q_target, _, _, _) in
            &mut indicator_query
        {
            if is_q_range {
                transform.translation = origin + Vec3::Y * Q_ELEVATION;
                transform.scale = Vec3::new(q_settings.range, 0.025, q_settings.range);
                *visibility = Visibility::Visible;
            }
            if is_q_target {
                transform.translation = target + Vec3::Y * (Q_ELEVATION + 0.025);
                transform.scale = Vec3::new(q_settings.aoe_radius, 0.035, q_settings.aoe_radius);
                *visibility = Visibility::Visible;
            }
        }
    }

    if keyboard.pressed(KeyCode::KeyW) {
        for (mut transform, mut visibility, _, _, is_w, _, _) in &mut indicator_query {
            if !is_w {
                continue;
            }
            transform.translation = origin + Vec3::Y * (Q_ELEVATION + 0.05);
            transform.scale = Vec3::new(w_settings.radius, 0.03, w_settings.radius);
            *visibility = Visibility::Visible;
        }
    }

    if keyboard.pressed(KeyCode::KeyE) {
        for (mut transform, mut visibility, _, _, _, is_e_range, _) in &mut indicator_query {
            if !is_e_range {
                continue;
            }
            transform.translation = origin + Vec3::Y * (Q_ELEVATION + 0.075);
            transform.scale = Vec3::new(e_settings.range, 0.025, e_settings.range);
            *visibility = Visibility::Visible;
        }

        if let Some(cursor_hit) = cursor_hit
            && let Some((_, target_transform)) =
                find_clicked_enemy_target(cursor_hit, &enemy_query, E_TARGET_CLICK_RADIUS)
            && horizontal_distance(origin, target_transform.translation) <= e_settings.range
        {
            for (mut transform, mut visibility, _, _, _, _, is_e_target) in &mut indicator_query {
                if !is_e_target {
                    continue;
                }
                transform.translation =
                    target_transform.translation + Vec3::Y * (Q_ELEVATION + 0.1);
                transform.scale = Vec3::new(E_TARGET_CLICK_RADIUS, 0.035, E_TARGET_CLICK_RADIUS);
                *visibility = Visibility::Visible;
            }
        }
    }
}

pub(in crate::systems) fn update_q_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut YunaQProjectile, &mut Transform)>,
) {
    for (entity, mut projectile, mut transform) in &mut query {
        projectile.timer.tick(time.delta());
        let duration = projectile.timer.duration().as_secs_f32().max(f32::EPSILON);
        let progress = (projectile.timer.elapsed_secs() / duration).clamp(0.0, 1.0);
        let arc = (std::f32::consts::PI * progress).sin() * 0.8;
        transform.translation = projectile.start.lerp(projectile.end, progress) + Vec3::Y * arc;
        transform.scale =
            Vec3::splat(1.0 + (projectile.timer.elapsed_secs() * 12.0).sin().abs() * 0.18);

        if projectile.timer.is_finished() {
            let center = projectile.end - Vec3::Y * 0.55;
            spawn_q_field_and_orb(
                &mut commands,
                &mut meshes,
                &mut materials,
                center,
                projectile.settings,
            );
            commands.entity(entity).despawn();
        }
    }
}

pub(in crate::systems) fn update_q_fields(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut orb_query: Query<(
        Entity,
        &mut YunaQOrb,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut field_query: Query<
        (
            Entity,
            &mut YunaQField,
            &mut Transform,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<YunaQOrb>,
    >,
) {
    for (entity, mut orb, mut transform, material_handle) in &mut orb_query {
        orb.timer.tick(time.delta());
        let progress = timer_progress(&orb.timer);
        let pulse = (orb.timer.elapsed_secs() * 10.0).sin() * 0.5 + 0.5;
        transform.scale = Vec3::splat(orb.radius * (0.92 + pulse * 0.18));
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color = Color::srgba(0.15, 0.9, 1.0, 0.92 - progress * 0.2);
            material.emissive = Color::srgb(0.1 + pulse * 0.15, 0.75, 1.25).into();
        }
        if orb.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }

    for (entity, mut field, mut transform, material_handle) in &mut field_query {
        field.timer.tick(time.delta());
        let progress = timer_progress(&field.timer);
        let pulse = (field.timer.elapsed_secs() * 7.5).sin() * 0.5 + 0.5;
        transform.rotation *= Quat::from_rotation_y(time.delta_secs() * 1.8);
        transform.scale = Vec3::new(
            field.radius * (0.96 + pulse * 0.05),
            0.025,
            field.radius * (0.96 + pulse * 0.05),
        );
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color = Color::srgba(
                0.1,
                0.72,
                1.0,
                (0.42 + pulse * 0.18) * (1.0 - progress * 0.3),
            );
            material.emissive = Color::srgb(0.02, 0.32 + pulse * 0.22, 0.95).into();
        }
        if field.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub(in crate::systems) fn update_w_fields(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_query: Query<(&Player, &Transform), Without<YunaWField>>,
    local_player_query: Query<(&Player, &Transform), (With<PlayerControlled>, Without<YunaWField>)>,
    mut query: Query<(
        Entity,
        &mut YunaWField,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut field, mut transform, material_handle) in &mut query {
        field.timer.tick(time.delta());
        if let Some(center) =
            yuna_w_follow_position(field.caster_player_id, &player_query).or_else(|| {
                local_player_query
                    .single()
                    .ok()
                    .map(|(_, transform)| transform.translation)
            })
        {
            transform.translation =
                Vec3::new(center.x, 0.0, center.z) + Vec3::Y * (Q_ELEVATION + 0.04);
        }

        let progress = timer_progress(&field.timer);
        let pulse = (field.timer.elapsed_secs() * 8.0).sin() * 0.5 + 0.5;
        transform.scale = Vec3::new(
            field.radius * (0.93 + pulse * 0.09),
            0.03,
            field.radius * (0.93 + pulse * 0.09),
        );
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color = Color::srgba(
                0.12,
                1.0,
                0.55,
                (0.46 + pulse * 0.18) * (1.0 - progress * 0.35),
            );
            material.emissive = Color::srgb(0.03, 0.75 + pulse * 0.2, 0.22).into();
        }
        if field.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub(in crate::systems) fn update_e_stun_bolts(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(
        Entity,
        &mut YunaEStunBolt,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut bolt, mut transform, material_handle) in &mut query {
        bolt.timer.tick(time.delta());
        let duration = bolt.timer.duration().as_secs_f32().max(f32::EPSILON);
        let progress = (bolt.timer.elapsed_secs() / duration).clamp(0.0, 1.0);
        let arc = (std::f32::consts::PI * progress).sin() * 0.35;
        transform.translation = bolt.start.lerp(bolt.end, progress) + Vec3::Y * arc;
        transform.scale = Vec3::splat(1.0 + (bolt.timer.elapsed_secs() * 22.0).sin().abs() * 0.22);
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color = Color::srgba(0.45, 0.82, 1.0, 0.92);
            material.emissive = Color::srgb(0.18, 0.5, 1.3).into();
        }
        if bolt.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub(in crate::systems) fn spawn_remote_ability_visual(
    event: AbilityVisualEvent,
    q_settings: &YunaQSettings,
    w_settings: &YunaWSettings,
    e_settings: &YunaESettings,
    remote_players: &Query<(Entity, &Player, &Transform), Without<PlayerControlled>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    commands: &mut Commands,
) {
    if event.champion != YUNA_CHAMPION_ID {
        return;
    }

    match event.slot {
        AbilitySlot::Q => {
            let Some(end) = event.end else {
                return;
            };
            spawn_q_projectile(
                commands,
                meshes,
                materials,
                Vec3::from(event.start),
                Vec3::from(end),
                *q_settings,
            );
        }
        AbilitySlot::W => {
            spawn_w_field(
                commands,
                meshes,
                materials,
                Vec3::from(event.start),
                *w_settings,
                Some(event.caster_player_id),
            );
        }
        AbilitySlot::E => {
            let Some(end) = event.end else {
                return;
            };
            let start = remote_players
                .iter()
                .find(|(_, player, _)| player.id.0 == event.caster_player_id)
                .map(|(_, _, transform)| transform.translation + Vec3::Y * 0.85)
                .unwrap_or_else(|| Vec3::from(event.start));
            spawn_e_stun_bolt(
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

fn spawn_q_projectile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    settings: YunaQSettings,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.18, 0.9, 1.0, 0.94),
        emissive: Color::srgb(0.12, 0.65, 1.1).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("YunaQProjectile"),
        YunaQProjectile {
            start,
            end,
            timer: Timer::from_seconds(settings.travel_seconds.max(f32::EPSILON), TimerMode::Once),
            settings,
        },
        Mesh3d(meshes.add(Sphere::new(0.42))),
        MeshMaterial3d(material),
        Transform::from_translation(start),
    ));
}

fn spawn_q_field_and_orb(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
    settings: YunaQSettings,
) {
    let field_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.08, 0.62, 1.0, 0.54),
        emissive: Color::srgb(0.02, 0.35, 0.95).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let orb_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.13, 0.9, 1.0, 0.95),
        emissive: Color::srgb(0.08, 0.75, 1.25).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("YunaQGravityField"),
        YunaQField {
            timer: Timer::from_seconds(settings.field_seconds.max(f32::EPSILON), TimerMode::Once),
            radius: settings.aoe_radius,
        },
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(field_material),
        Transform::from_translation(center + Vec3::Y * Q_ELEVATION).with_scale(Vec3::new(
            settings.aoe_radius,
            0.025,
            settings.aoe_radius,
        )),
    ));
    commands.spawn((
        Name::new("YunaQGravityOrb"),
        YunaQOrb {
            timer: Timer::from_seconds(settings.field_seconds.max(f32::EPSILON), TimerMode::Once),
            radius: settings.orb_radius,
        },
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(orb_material),
        Transform::from_translation(center + Vec3::Y * 0.75)
            .with_scale(Vec3::splat(settings.orb_radius)),
    ));
}

fn spawn_w_field(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
    settings: YunaWSettings,
    caster_player_id: Option<u64>,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.12, 1.0, 0.55, 0.58),
        emissive: Color::srgb(0.03, 0.7, 0.22).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("YunaWHealingField"),
        YunaWField {
            timer: Timer::from_seconds(settings.field_seconds.max(f32::EPSILON), TimerMode::Once),
            radius: settings.radius,
            caster_player_id,
        },
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(material),
        Transform::from_translation(center + Vec3::Y * (Q_ELEVATION + 0.04)).with_scale(Vec3::new(
            settings.radius,
            0.03,
            settings.radius,
        )),
    ));
}

fn spawn_e_stun_bolt(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    settings: YunaESettings,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.45, 0.82, 1.0, 0.94),
        emissive: Color::srgb(0.18, 0.48, 1.25).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("YunaEStunBolt"),
        YunaEStunBolt {
            start,
            end,
            timer: Timer::from_seconds(settings.travel_seconds.max(f32::EPSILON), TimerMode::Once),
        },
        Mesh3d(meshes.add(Sphere::new(settings.projectile_radius))),
        MeshMaterial3d(material),
        Transform::from_translation(start),
    ));
}

fn indicator_material(base_color: Color, emissive: Color) -> StandardMaterial {
    StandardMaterial {
        base_color,
        emissive: emissive.into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}

fn yuna_w_follow_position(
    caster_player_id: Option<u64>,
    player_query: &Query<(&Player, &Transform), Without<YunaWField>>,
) -> Option<Vec3> {
    let caster_player_id = caster_player_id?;
    player_query
        .iter()
        .find(|(player, _)| player.id.0 == caster_player_id)
        .map(|(_, transform)| transform.translation)
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

fn timer_progress(timer: &Timer) -> f32 {
    (timer.elapsed_secs() / timer.duration().as_secs_f32().max(f32::EPSILON)).clamp(0.0, 1.0)
}

fn send_ability_command(
    senders: &mut Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    slot: AbilitySlot,
    target_position: Option<Vec3>,
) {
    for mut sender in senders {
        sender.send::<ReliableCommandChannel>(PlayerCommand::CastAbility {
            champion: YUNA_CHAMPION_ID,
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

fn clamp_cast_target(origin: Vec3, target: Vec3, range: f32) -> Vec3 {
    let delta = Vec3::new(target.x - origin.x, 0.0, target.z - origin.z);
    if delta.length_squared() <= range * range {
        return Vec3::new(target.x, origin.y, target.z);
    }

    origin + delta.normalize_or_zero() * range
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

fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}
