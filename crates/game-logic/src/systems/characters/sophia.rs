use crate::systems::{
    CurrentChampionVisual, TrainingDummy,
    targeting::{clamp_world_point_to_map_top, ray_hit_map_top},
};
use bevy::ecs::query::QueryFilter;
use bevy::math::primitives::{Cone, Cylinder, Sphere};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_shared::game::{
    camera::TopDownCamera,
    map::MapGround,
    player::{Health, Player, PlayerControlled},
    team::Team,
};
use game_shared::network::{
    AbilitySlot, AbilityVisualEvent, AbilityVisualTuning, CastTarget, ChampionId, PlayerCommand,
    ReliableCommandChannel, WorldPosition,
};
use lightyear::prelude::*;

const SOPHIA_CHAMPION_ID: ChampionId = ChampionId(6609);

const Q_COOLDOWN_SECONDS: f32 = 6.0;
const Q_RANGE: f32 = 8.0;
const Q_TARGET_CLICK_RADIUS: f32 = 1.4;
const Q_ORB_SECONDS: f32 = 4.0;
const Q_DAMAGE_PER_SECOND: f32 = 24.0;
const Q_ORB_RADIUS: f32 = 0.28;

const W_COOLDOWN_SECONDS: f32 = 10.0;
const W_MINION_COUNT: usize = 2;
const W_MINION_SECONDS: f32 = 8.0;
const W_SEARCH_RADIUS: f32 = 4.5;
const W_CHASE_SPEED: f32 = 6.6;
const W_MINION_RADIUS: f32 = 0.34;

const E_COOLDOWN_SECONDS: f32 = 8.0;
const E_BUFF_SECONDS: f32 = 4.0;
const E_SPEED_SECONDS: f32 = 2.0;

const INDICATOR_ELEVATION: f32 = 0.09;

#[derive(Resource, Debug, Clone, Copy)]
pub(in crate::systems) struct SophiaQSettings {
    pub(in crate::systems) range: f32,
    pub(in crate::systems) target_radius: f32,
    pub(in crate::systems) orb_seconds: f32,
    pub(in crate::systems) damage_per_second: f32,
    pub(in crate::systems) orb_radius: f32,
}

impl Default for SophiaQSettings {
    fn default() -> Self {
        Self {
            range: Q_RANGE,
            target_radius: Q_TARGET_CLICK_RADIUS,
            orb_seconds: Q_ORB_SECONDS,
            damage_per_second: Q_DAMAGE_PER_SECOND,
            orb_radius: Q_ORB_RADIUS,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(in crate::systems) struct SophiaWSettings {
    pub(in crate::systems) minion_count: usize,
    pub(in crate::systems) lifetime_seconds: f32,
    pub(in crate::systems) search_radius: f32,
    pub(in crate::systems) chase_speed: f32,
    pub(in crate::systems) minion_radius: f32,
}

impl Default for SophiaWSettings {
    fn default() -> Self {
        Self {
            minion_count: W_MINION_COUNT,
            lifetime_seconds: W_MINION_SECONDS,
            search_radius: W_SEARCH_RADIUS,
            chase_speed: W_CHASE_SPEED,
            minion_radius: W_MINION_RADIUS,
        }
    }
}

impl SophiaWSettings {
    fn from_visual(visual: AbilityVisualTuning, fallback: Self) -> Self {
        Self {
            minion_count: if visual.missile_count > 0 {
                usize::from(visual.missile_count)
            } else {
                fallback.minion_count
            },
            lifetime_seconds: positive_or(
                visual.missile_lifetime_seconds,
                fallback.lifetime_seconds,
            ),
            search_radius: positive_or(visual.missile_search_radius, fallback.search_radius),
            chase_speed: positive_or(visual.missile_chase_speed, fallback.chase_speed),
            minion_radius: positive_or(visual.missile_radius, fallback.minion_radius),
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub(in crate::systems) struct SophiaESettings {
    pub(in crate::systems) buff_seconds: f32,
    pub(in crate::systems) speed_seconds: f32,
    pub(in crate::systems) damage_multiplier: f32,
}

impl Default for SophiaESettings {
    fn default() -> Self {
        Self {
            buff_seconds: E_BUFF_SECONDS,
            speed_seconds: E_SPEED_SECONDS,
            damage_multiplier: 1.2,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub(in crate::systems) struct SophiaQCastState {
    cooldown: Timer,
}

#[derive(Resource, Debug, Clone)]
pub(in crate::systems) struct SophiaWCastState {
    cooldown: Timer,
}

#[derive(Resource, Debug, Clone)]
pub(in crate::systems) struct SophiaECastState {
    cooldown: Timer,
    buff: Timer,
    speed: Timer,
}

impl Default for SophiaQCastState {
    fn default() -> Self {
        Self::ready(Q_COOLDOWN_SECONDS)
    }
}

impl Default for SophiaWCastState {
    fn default() -> Self {
        Self::ready(W_COOLDOWN_SECONDS)
    }
}

impl Default for SophiaECastState {
    fn default() -> Self {
        Self::ready(E_COOLDOWN_SECONDS)
    }
}

impl SophiaQCastState {
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

impl SophiaWCastState {
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

impl SophiaECastState {
    pub(in crate::systems) fn ready(cooldown_seconds: f32) -> Self {
        Self {
            cooldown: ready_timer(cooldown_seconds),
            buff: expired_timer(E_BUFF_SECONDS),
            speed: expired_timer(E_SPEED_SECONDS),
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

    fn consume_damage_amp(&mut self, multiplier: f32) -> f32 {
        if !self.buff.is_finished() {
            self.buff.set_elapsed(self.buff.duration());
            multiplier
        } else {
            1.0
        }
    }
}

#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct SophiaQOrb {
    timer: Timer,
    damage_timer: Timer,
    damage_per_second: f32,
    target: Option<Entity>,
    target_player_id: Option<u64>,
    radius: f32,
}

#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct SophiaMinion {
    timer: Timer,
    phase: f32,
    caster_player_id: Option<u64>,
    owner: Option<Entity>,
    target: Option<Entity>,
    settings: SophiaWSettings,
}

#[derive(Component, Debug, Clone)]
pub(in crate::systems) struct SophiaBuffArrow {
    timer: Timer,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct SophiaQRangeIndicator;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct SophiaQTargetIndicator;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct SophiaWIndicator;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::systems) struct SophiaEIndicator;

pub(in crate::systems) fn spawn_sophia_indicators(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let q_range_material = materials.add(indicator_material(
        Color::srgba(0.93, 0.72, 0.22, 0.16),
        Color::srgb(0.62, 0.35, 0.04),
    ));
    let q_target_material = materials.add(indicator_material(
        Color::srgba(1.0, 0.82, 0.25, 0.5),
        Color::srgb(0.92, 0.54, 0.06),
    ));
    let w_material = materials.add(indicator_material(
        Color::srgba(0.95, 0.4, 0.85, 0.28),
        Color::srgb(0.58, 0.08, 0.44),
    ));
    let e_material = materials.add(indicator_material(
        Color::srgba(0.35, 1.0, 0.72, 0.55),
        Color::srgb(0.08, 0.7, 0.32),
    ));

    commands.spawn((
        Name::new("SophiaQRangeIndicator"),
        SophiaQRangeIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(q_range_material),
        Transform::from_xyz(0.0, INDICATOR_ELEVATION, 0.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        Name::new("SophiaQTargetIndicator"),
        SophiaQTargetIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(q_target_material),
        Transform::from_xyz(0.0, INDICATOR_ELEVATION + 0.025, 0.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        Name::new("SophiaWIndicator"),
        SophiaWIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(w_material),
        Transform::from_xyz(0.0, INDICATOR_ELEVATION + 0.05, 0.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        Name::new("SophiaEIndicator"),
        SophiaEIndicator,
        Mesh3d(meshes.add(Mesh::from(Cone::new(0.28, 0.9)))),
        MeshMaterial3d(e_material),
        Transform::from_xyz(0.0, 1.7, 0.0)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
        Visibility::Hidden,
    ));
}

pub(in crate::systems) fn cast_q_orb_on_left_click(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<SophiaQSettings>,
    e_settings: Res<SophiaESettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<(&Transform, &Health, &CurrentChampionVisual), With<PlayerControlled>>,
    enemy_query: Query<(Entity, &TrainingDummy, &Transform)>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q_state: ResMut<SophiaQCastState>,
    mut e_state: ResMut<SophiaECastState>,
    mut commands: Commands,
) {
    q_state.cooldown.tick(time.delta());
    if !keyboard.pressed(KeyCode::KeyQ) || !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }
    if !q_state.cooldown.is_finished() {
        return;
    }

    let Ok((player_transform, health, visual)) = player_query.single() else {
        return;
    };
    if health.current == 0 || visual.champion != Some(SOPHIA_CHAMPION_ID) {
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
    let Some((target_entity, _, target_transform)) =
        find_clicked_enemy_target(cursor_hit, &enemy_query, settings.target_radius)
    else {
        return;
    };
    if horizontal_distance(origin, target_transform.translation) > settings.range {
        return;
    }

    let damage_multiplier = e_state.consume_damage_amp(e_settings.damage_multiplier);
    spawn_q_orb(
        &mut commands,
        &mut meshes,
        &mut materials,
        target_transform.translation,
        Some(target_entity),
        None,
        *settings,
        damage_multiplier,
    );
    send_ability_command(
        &mut command_senders,
        AbilitySlot::Q,
        Some(target_transform.translation),
    );
    q_state.cooldown.reset();
}

pub(in crate::systems) fn cast_w_minions(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<SophiaWSettings>,
    e_settings: Res<SophiaESettings>,
    player_query: Query<
        (Entity, &Player, &Transform, &Health, &CurrentChampionVisual),
        With<PlayerControlled>,
    >,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut w_state: ResMut<SophiaWCastState>,
    mut e_state: ResMut<SophiaECastState>,
    mut commands: Commands,
) {
    w_state.cooldown.tick(time.delta());
    if !keyboard.just_pressed(KeyCode::KeyW) || !w_state.cooldown.is_finished() {
        return;
    }

    let Ok((player_entity, player, transform, health, visual)) = player_query.single() else {
        return;
    };
    if health.current == 0 || visual.champion != Some(SOPHIA_CHAMPION_ID) {
        return;
    }

    e_state.consume_damage_amp(e_settings.damage_multiplier);
    spawn_minions(
        &mut commands,
        &mut meshes,
        &mut materials,
        transform.translation,
        Some(player_entity),
        Some(player.id.0),
        *settings,
    );
    send_ability_command(&mut command_senders, AbilitySlot::W, None);
    w_state.cooldown.reset();
}

pub(in crate::systems) fn cast_e_self_buff(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<SophiaESettings>,
    player_query: Query<(&Transform, &Health, &CurrentChampionVisual), With<PlayerControlled>>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cast_state: ResMut<SophiaECastState>,
    mut commands: Commands,
) {
    cast_state.cooldown.tick(time.delta());
    cast_state.buff.tick(time.delta());
    cast_state.speed.tick(time.delta());
    if !keyboard.just_pressed(KeyCode::KeyE) || !cast_state.cooldown.is_finished() {
        return;
    }

    let Ok((transform, health, visual)) = player_query.single() else {
        return;
    };
    if health.current == 0 || visual.champion != Some(SOPHIA_CHAMPION_ID) {
        return;
    }

    cast_state.cooldown.reset();
    cast_state.buff = Timer::from_seconds(settings.buff_seconds.max(f32::EPSILON), TimerMode::Once);
    cast_state.speed =
        Timer::from_seconds(settings.speed_seconds.max(f32::EPSILON), TimerMode::Once);
    spawn_buff_arrow(
        &mut commands,
        &mut meshes,
        &mut materials,
        transform.translation,
        settings.buff_seconds,
    );
    send_ability_command(&mut command_senders, AbilitySlot::E, None);
}

pub(in crate::systems) fn update_sophia_indicators(
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<
        (&Transform, &CurrentChampionVisual),
        (
            With<PlayerControlled>,
            Without<SophiaQRangeIndicator>,
            Without<SophiaQTargetIndicator>,
            Without<SophiaWIndicator>,
            Without<SophiaEIndicator>,
        ),
    >,
    enemy_query: Query<
        (Entity, &TrainingDummy, &Transform),
        (
            Without<SophiaQRangeIndicator>,
            Without<SophiaQTargetIndicator>,
            Without<SophiaWIndicator>,
            Without<SophiaEIndicator>,
        ),
    >,
    q_settings: Res<SophiaQSettings>,
    w_settings: Res<SophiaWSettings>,
    mut indicator_query: Query<
        (
            &mut Transform,
            &mut Visibility,
            Has<SophiaQRangeIndicator>,
            Has<SophiaQTargetIndicator>,
            Has<SophiaWIndicator>,
            Has<SophiaEIndicator>,
        ),
        Or<(
            With<SophiaQRangeIndicator>,
            With<SophiaQTargetIndicator>,
            With<SophiaWIndicator>,
            With<SophiaEIndicator>,
        )>,
    >,
) {
    for (_, mut visibility, _, _, _, _) in &mut indicator_query {
        *visibility = Visibility::Hidden;
    }

    let Ok((player_transform, visual)) = player_query.single() else {
        return;
    };
    if visual.champion != Some(SOPHIA_CHAMPION_ID) {
        return;
    }

    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };
    let origin =
        clamp_world_point_to_map_top(player_transform.translation, map_transform, *map_ground);
    let cursor_hit = cursor_hit_on_map(&windows, &camera_query, map_transform, *map_ground);

    if keyboard.pressed(KeyCode::KeyQ) {
        for (mut transform, mut visibility, is_q_range, _, _, _) in &mut indicator_query {
            if is_q_range {
                transform.translation = origin + Vec3::Y * INDICATOR_ELEVATION;
                transform.scale = Vec3::new(q_settings.range, 0.025, q_settings.range);
                *visibility = Visibility::Visible;
            }
        }

        if let Some(cursor_hit) = cursor_hit
            && let Some((_, _, target_transform)) =
                find_clicked_enemy_target(cursor_hit, &enemy_query, q_settings.target_radius)
            && horizontal_distance(origin, target_transform.translation) <= q_settings.range
        {
            for (mut transform, mut visibility, _, is_q_target, _, _) in &mut indicator_query {
                if is_q_target {
                    transform.translation =
                        target_transform.translation + Vec3::Y * (INDICATOR_ELEVATION + 0.025);
                    transform.scale =
                        Vec3::new(q_settings.target_radius, 0.035, q_settings.target_radius);
                    *visibility = Visibility::Visible;
                }
            }
        }
    }

    if keyboard.pressed(KeyCode::KeyW) {
        for (mut transform, mut visibility, _, _, is_w, _) in &mut indicator_query {
            if is_w {
                transform.translation = origin + Vec3::Y * (INDICATOR_ELEVATION + 0.05);
                transform.scale =
                    Vec3::new(w_settings.search_radius, 0.03, w_settings.search_radius);
                *visibility = Visibility::Visible;
            }
        }
    }

    if keyboard.pressed(KeyCode::KeyE) {
        for (mut transform, mut visibility, _, _, _, is_e) in &mut indicator_query {
            if is_e {
                transform.translation = origin + Vec3::Y * 1.8;
                transform.rotation = Quat::from_rotation_x(std::f32::consts::PI);
                transform.scale = Vec3::new(1.0, 1.0, 1.0);
                *visibility = Visibility::Visible;
            }
        }
    }
}

pub(in crate::systems) fn update_q_orbs(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dummy_query: Query<&mut TrainingDummy>,
    target_query: Query<&Transform, Without<SophiaQOrb>>,
    remote_player_query: Query<(&Player, &Transform), Without<SophiaQOrb>>,
    mut query: Query<(
        Entity,
        &mut SophiaQOrb,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut orb, mut transform, material_handle) in &mut query {
        orb.timer.tick(time.delta());
        orb.damage_timer.tick(time.delta());

        let target_position = orb
            .target
            .and_then(|target| {
                target_query
                    .get(target)
                    .ok()
                    .map(|transform| transform.translation)
            })
            .or_else(|| {
                orb.target_player_id.and_then(|player_id| {
                    remote_player_query
                        .iter()
                        .find(|(player, _)| player.id.0 == player_id)
                        .map(|(_, transform)| transform.translation)
                })
            });
        if let Some(position) = target_position {
            transform.translation = position + Vec3::Y * 1.75;
        }

        if orb.damage_timer.just_finished()
            && let Some(target) = orb.target
            && let Ok(mut dummy) = dummy_query.get_mut(target)
        {
            dummy.health = (dummy.health - orb.damage_per_second).max(0.0);
        }

        let progress = timer_progress(&orb.timer);
        let pulse = (orb.timer.elapsed_secs() * 9.0).sin() * 0.5 + 0.5;
        transform.scale = Vec3::splat(orb.radius * (0.9 + pulse * 0.25));
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color = Color::srgba(1.0, 0.78, 0.25, 0.95 - progress * 0.35);
            material.emissive = Color::srgb(0.9 + pulse * 0.3, 0.46, 0.08).into();
        }
        if orb.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub(in crate::systems) fn update_minions(
    time: Res<Time>,
    mut commands: Commands,
    player_query: Query<(Entity, &Player, &Team, &Health, &Transform), Without<SophiaMinion>>,
    mut query: Query<(Entity, &mut SophiaMinion, &mut Transform)>,
) {
    for (entity, mut minion, mut transform) in &mut query {
        minion.timer.tick(time.delta());
        if minion.timer.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        if minion.target.is_none() {
            let desired =
                minion_follow_position(&minion, &player_query).unwrap_or(transform.translation);
            let to_desired = desired - transform.translation;
            let distance = to_desired.length();
            if distance > 0.02 {
                transform.translation +=
                    to_desired.normalize() * (10.0 * time.delta_secs()).min(distance);
            }
            minion.target = find_minion_target(
                transform.translation,
                minion.settings.search_radius,
                minion.caster_player_id,
                &player_query,
            );
        }

        if let Some(target) = minion.target {
            let Ok((_, _, _, health, target_transform)) = player_query.get(target) else {
                minion.target = None;
                continue;
            };
            if health.current == 0 {
                minion.target = None;
                continue;
            }

            let target_back = target_transform.translation
                - (target_transform.rotation * Vec3::Z) * 0.75
                + Vec3::Y * 0.35;
            let to_target = target_back - transform.translation;
            let distance = to_target.length();
            if distance <= minion.settings.minion_radius + 0.9 {
                commands.entity(entity).despawn();
                continue;
            }
            if distance > f32::EPSILON {
                let step = minion.settings.chase_speed * time.delta_secs();
                transform.translation += to_target.normalize() * step.min(distance);
            }
        }
    }
}

pub(in crate::systems) fn update_buff_arrows(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_query: Query<&Transform, (With<PlayerControlled>, Without<SophiaBuffArrow>)>,
    mut query: Query<(
        Entity,
        &mut SophiaBuffArrow,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let player_position = player_query
        .single()
        .ok()
        .map(|transform| transform.translation);
    for (entity, mut arrow, mut transform, material_handle) in &mut query {
        arrow.timer.tick(time.delta());
        if let Some(position) = player_position {
            transform.translation =
                position + Vec3::Y * (1.65 + (arrow.timer.elapsed_secs() * 8.0).sin() * 0.08);
        }
        transform.rotation = Quat::from_rotation_x(std::f32::consts::PI);
        if let Some(material) = materials.get_mut(&material_handle.0) {
            let progress = timer_progress(&arrow.timer);
            material.base_color = Color::srgba(0.35, 1.0, 0.72, 0.82 * (1.0 - progress * 0.45));
            material.emissive = Color::srgb(0.08, 0.75, 0.32).into();
        }
        if arrow.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub(in crate::systems) fn spawn_remote_ability_visual(
    event: AbilityVisualEvent,
    q_settings: &SophiaQSettings,
    w_settings: &SophiaWSettings,
    e_settings: &SophiaESettings,
    remote_players: &Query<(Entity, &Player, &Transform), Without<PlayerControlled>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    commands: &mut Commands,
) {
    if event.champion != SOPHIA_CHAMPION_ID {
        return;
    }

    let owner = remote_players
        .iter()
        .find(|(_, player, _)| player.id.0 == event.caster_player_id);

    match event.slot {
        AbilitySlot::Q => {
            let Some(end) = event.end else {
                return;
            };
            let target_player_id = remote_players
                .iter()
                .find(|(_, _, transform)| {
                    horizontal_distance(transform.translation, Vec3::from(end)) <= 1.5
                })
                .map(|(_, player, _)| player.id.0);
            spawn_q_orb(
                commands,
                meshes,
                materials,
                Vec3::from(end),
                None,
                target_player_id,
                *q_settings,
                1.0,
            );
        }
        AbilitySlot::W => {
            let origin = owner
                .map(|(_, _, transform)| transform.translation)
                .unwrap_or_else(|| Vec3::from(event.start));
            spawn_minions(
                commands,
                meshes,
                materials,
                origin,
                owner.map(|(entity, _, _)| entity),
                Some(event.caster_player_id),
                SophiaWSettings::from_visual(event.visual, *w_settings),
            );
        }
        AbilitySlot::E => {
            let origin = owner
                .map(|(_, _, transform)| transform.translation)
                .unwrap_or_else(|| Vec3::from(event.start));
            spawn_buff_arrow(commands, meshes, materials, origin, e_settings.buff_seconds);
        }
        AbilitySlot::R => {}
    }
}

fn spawn_q_orb(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target_position: Vec3,
    target: Option<Entity>,
    target_player_id: Option<u64>,
    settings: SophiaQSettings,
    damage_multiplier: f32,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.78, 0.25, 0.96),
        emissive: Color::srgb(0.9, 0.44, 0.08).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("SophiaQOrb"),
        SophiaQOrb {
            timer: Timer::from_seconds(settings.orb_seconds.max(f32::EPSILON), TimerMode::Once),
            damage_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            damage_per_second: settings.damage_per_second * damage_multiplier,
            target,
            target_player_id,
            radius: settings.orb_radius,
        },
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(material),
        Transform::from_translation(target_position + Vec3::Y * 1.75)
            .with_scale(Vec3::splat(settings.orb_radius)),
    ));
}

fn spawn_minions(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    origin: Vec3,
    owner: Option<Entity>,
    caster_player_id: Option<u64>,
    settings: SophiaWSettings,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.96, 0.36, 0.85, 0.92),
        emissive: Color::srgb(0.68, 0.08, 0.48).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let count = settings.minion_count.max(1);
    for index in 0..count {
        let phase = index as f32 / count as f32 * std::f32::consts::TAU;
        let offset = Vec3::new(phase.cos(), 0.0, phase.sin()) * 0.95 + Vec3::Y * 0.35;
        commands.spawn((
            Name::new("SophiaMinion"),
            SophiaMinion {
                timer: Timer::from_seconds(
                    settings.lifetime_seconds.max(f32::EPSILON),
                    TimerMode::Once,
                ),
                phase,
                caster_player_id,
                owner,
                target: None,
                settings,
            },
            Mesh3d(meshes.add(Mesh::from(Cone::new(settings.minion_radius, 0.8)))),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(origin + offset),
        ));
    }
}

fn spawn_buff_arrow(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    origin: Vec3,
    lifetime_seconds: f32,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 1.0, 0.72, 0.82),
        emissive: Color::srgb(0.08, 0.75, 0.32).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("SophiaBuffArrow"),
        SophiaBuffArrow {
            timer: Timer::from_seconds(lifetime_seconds.max(f32::EPSILON), TimerMode::Once),
        },
        Mesh3d(meshes.add(Mesh::from(Cone::new(0.28, 0.9)))),
        MeshMaterial3d(material),
        Transform::from_translation(origin + Vec3::Y * 1.7)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
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

fn minion_follow_position(
    minion: &SophiaMinion,
    player_query: &Query<(Entity, &Player, &Team, &Health, &Transform), Without<SophiaMinion>>,
) -> Option<Vec3> {
    let owner_transform = minion
        .owner
        .and_then(|owner| {
            player_query
                .get(owner)
                .ok()
                .map(|(_, _, _, _, transform)| transform)
        })
        .or_else(|| {
            minion.caster_player_id.and_then(|caster_player_id| {
                player_query
                    .iter()
                    .find(|(_, player, _, _, _)| player.id.0 == caster_player_id)
                    .map(|(_, _, _, _, transform)| transform)
            })
        })?;

    let forward = owner_transform.rotation * Vec3::Z;
    let right = owner_transform.rotation * Vec3::X;
    let side_offset = if minion.phase.cos() >= 0.0 {
        0.55
    } else {
        -0.55
    };

    Some(owner_transform.translation - forward * 1.05 + right * side_offset + Vec3::Y * 0.35)
}

fn find_minion_target(
    position: Vec3,
    radius: f32,
    caster_player_id: Option<u64>,
    player_query: &Query<(Entity, &Player, &Team, &Health, &Transform), Without<SophiaMinion>>,
) -> Option<Entity> {
    let caster_player_id = caster_player_id?;
    let caster_team = player_query
        .iter()
        .find(|(_, player, _, _, _)| player.id.0 == caster_player_id)
        .map(|(_, _, team, _, _)| team.0)?;

    player_query
        .iter()
        .filter(|(_, player, team, health, transform)| {
            player.id.0 != caster_player_id
                && team.0 != caster_team
                && health.current > 0
                && horizontal_distance(position, transform.translation) <= radius
        })
        .min_by(|(_, _, _, _, left), (_, _, _, _, right)| {
            horizontal_distance(position, left.translation)
                .partial_cmp(&horizontal_distance(position, right.translation))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, _, _, _, _)| entity)
}

fn ready_timer(cooldown_seconds: f32) -> Timer {
    let mut timer = Timer::from_seconds(cooldown_seconds.max(f32::EPSILON), TimerMode::Once);
    timer.set_elapsed(timer.duration());
    timer
}

fn expired_timer(seconds: f32) -> Timer {
    let mut timer = Timer::from_seconds(seconds.max(f32::EPSILON), TimerMode::Once);
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
            champion: SOPHIA_CHAMPION_ID,
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

fn find_clicked_enemy_target<'a, F>(
    cursor_hit: Vec3,
    enemy_query: &'a Query<(Entity, &TrainingDummy, &Transform), F>,
    radius: f32,
) -> Option<(Entity, &'a TrainingDummy, &'a Transform)>
where
    F: QueryFilter,
{
    enemy_query
        .iter()
        .filter(|(_, dummy, transform)| {
            dummy.health > 0.0 && horizontal_distance(cursor_hit, transform.translation) <= radius
        })
        .min_by(|(_, _, left), (_, _, right)| {
            horizontal_distance(cursor_hit, left.translation)
                .partial_cmp(&horizontal_distance(cursor_hit, right.translation))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

fn positive_or(candidate: f32, fallback: f32) -> f32 {
    if candidate.is_finite() && candidate > 0.0 {
        candidate
    } else {
        fallback
    }
}
