use super::{
    TrainingDummy, horizontal_distance,
    targeting::{clamp_world_point_to_map_top, ray_hit_map_top},
};
use bevy::math::primitives::Sphere;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use mira_game_api::game::{
    auto_attack::AUTO_ATTACK_PROJECTILE_MIN_TRAVEL_SECONDS, camera::TopDownCamera, map::MapGround,
    player::Player,
};
use mira_game_api::network::{
    AutoAttackVisualEvent, NetworkTargetId, PlayerCommand, RangedMinionAutoAttackVisualEvent,
    ReliableCommandChannel,
};
use lightyear::prelude::*;

const AUTO_ATTACK_PROJECTILE_RADIUS: f32 = 0.12;
const AUTO_ATTACK_PROJECTILE_HEIGHT: f32 = 0.8;
const AUTO_ATTACK_REQUEST_INTERVAL_SECONDS: f32 = 0.1;

/// Limits how often the client repeats an auto-attack request for an active order.
#[derive(Resource, Debug, Clone)]
pub(super) struct AutoAttackState {
    request_timer: Timer,
}
/// Tracks whether the current right-click press was consumed by attack input.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub(super) struct AutoAttackInputState {
    pub(super) consumed_right_press: bool,
}
/// Stores the currently ordered auto-attack target.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub(super) struct AutoAttackTarget {
    pub(super) target: Option<Entity>,
}
/// Stores one local auto-attack projectile travelling toward a clicked enemy target.
#[derive(Component, Debug, Clone)]
pub(super) struct AutoAttackProjectile {
    target: Option<Entity>,
    start: Vec3,
    end: Vec3,
    timer: Timer,
}

impl Default for AutoAttackState {
    /// Returns the default configuration used by the client auto-attack system.
    fn default() -> Self {
        let mut request_timer =
            Timer::from_seconds(AUTO_ATTACK_REQUEST_INTERVAL_SECONDS, TimerMode::Repeating);
        request_timer.set_elapsed(request_timer.duration());
        Self { request_timer }
    }
}
/// Converts right-clicks on enemy targets into server-authoritative attack-move orders.
pub(super) fn handle_auto_attack_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    target_query: Query<(
        Entity,
        &TrainingDummy,
        &Transform,
        Option<&Player>,
        Option<&NetworkTargetId>,
    )>,
    mut input_state: ResMut<AutoAttackInputState>,
    mut attack_target: ResMut<AutoAttackTarget>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
) {
    input_state.consumed_right_press = false;

    if !mouse_buttons.just_pressed(MouseButton::Right) {
        return;
    }

    let Some(cursor_hit) = cursor_world_position(&windows, &camera_query, &map_query) else {
        return;
    };
    let Some((target_entity, _, _, target_network_id)) =
        clicked_enemy_target(cursor_hit, &target_query)
    else {
        return;
    };

    input_state.consumed_right_press = true;
    attack_target.target = Some(target_entity);
    if let Some(target_network_id) = target_network_id {
        send_attack_move_command(&mut command_senders, target_network_id);
    }
}
/// Keeps the current auto-attack order active until another target or movement command replaces it.
pub(super) fn update_auto_attack_target(
    time: Res<Time>,
    mut attack_state: ResMut<AutoAttackState>,
    mut attack_target: ResMut<AutoAttackTarget>,
    target_query: Query<(
        Entity,
        &TrainingDummy,
        &Transform,
        Option<&Player>,
        Option<&NetworkTargetId>,
    )>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
) {
    attack_state.request_timer.tick(time.delta());

    let Some(target_entity) = attack_target.target else {
        return;
    };

    let Ok((_, target, _, target_player, target_network_id)) = target_query.get(target_entity)
    else {
        attack_target.target = None;
        return;
    };
    if target.health <= 0.0 {
        attack_target.target = None;
        return;
    }
    if !attack_state.request_timer.just_finished() {
        return;
    }

    let target_id = target_player
        .map(|player| NetworkTargetId::Player(player.id.0))
        .or(target_network_id.copied());
    if let Some(target_id) = target_id {
        send_auto_attack_command(&mut command_senders, target_id);
    }
}
/// Receives server-accepted remote auto attacks and renders their projectile for this client.
pub(super) fn receive_remote_auto_attack_visuals(
    mut commands: Commands,
    mut receivers: Query<&mut MessageReceiver<AutoAttackVisualEvent>, With<Client>>,
    target_query: Query<(Entity, Option<&Player>, Option<&NetworkTargetId>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for mut receiver in &mut receivers {
        for event in receiver.receive() {
            let Some(target_entity) = target_entity_for_network_id(event.target, &target_query)
            else {
                continue;
            };

            spawn_auto_attack_projectile(
                &mut commands,
                &mut meshes,
                &mut materials,
                Some(target_entity),
                event.start.into(),
                event.end.into(),
                event
                    .travel_seconds
                    .max(AUTO_ATTACK_PROJECTILE_MIN_TRAVEL_SECONDS),
                Color::srgba(1.0, 1.0, 1.0, 0.95),
            );
        }
    }
}

/// Receives ranged-minion attack events and renders each server-authoritative projectile.
pub(super) fn receive_remote_ranged_minion_auto_attack_visuals(
    mut commands: Commands,
    mut receivers: Query<&mut MessageReceiver<RangedMinionAutoAttackVisualEvent>, With<Client>>,
    target_query: Query<(Entity, Option<&Player>, Option<&NetworkTargetId>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for mut receiver in &mut receivers {
        for event in receiver.receive() {
            spawn_auto_attack_projectile(
                &mut commands,
                &mut meshes,
                &mut materials,
                target_entity_for_network_id(event.target, &target_query),
                event.start.into(),
                event.end.into(),
                event
                    .travel_seconds
                    .max(AUTO_ATTACK_PROJECTILE_MIN_TRAVEL_SECONDS),
                super::lane::team_color(event.team).with_alpha(0.95),
            );
        }
    }
}
/// Updates presentation-only projectiles created from server-approved events.
pub(super) fn update_auto_attack_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    target_transform_query: Query<&Transform, Without<AutoAttackProjectile>>,
    mut projectile_query: Query<(Entity, &mut AutoAttackProjectile, &mut Transform)>,
) {
    for (projectile_entity, mut projectile, mut transform) in &mut projectile_query {
        if let Some(target_entity) = projectile.target {
            match target_transform_query.get(target_entity) {
                Ok(target_transform) => {
                    projectile.end =
                        target_transform.translation + Vec3::Y * AUTO_ATTACK_PROJECTILE_HEIGHT;
                }
                Err(_) => projectile.target = None,
            }
        }
        projectile.timer.tick(time.delta());

        let duration = projectile.timer.duration().as_secs_f32();
        let progress = (projectile.timer.elapsed_secs() / duration).clamp(0.0, 1.0);
        transform.translation = projectile.start.lerp(projectile.end, progress);

        if projectile.timer.is_finished() {
            commands.entity(projectile_entity).despawn();
        }
    }
}
fn cursor_world_position(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera_query: &Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: &Query<(&GlobalTransform, &MapGround)>,
) -> Option<Vec3> {
    let window = windows.single().ok()?;
    let cursor_position = window.cursor_position()?;
    let (camera, camera_transform) = camera_query.single().ok()?;
    let ray = camera
        .viewport_to_world(camera_transform, cursor_position)
        .ok()?;
    let (map_transform, map_ground) = map_query.single().ok()?;
    ray_hit_map_top(ray, map_transform, *map_ground)
        .map(|point| clamp_world_point_to_map_top(point, map_transform, *map_ground))
}
fn clicked_enemy_target(
    cursor_hit: Vec3,
    target_query: &Query<(
        Entity,
        &TrainingDummy,
        &Transform,
        Option<&Player>,
        Option<&NetworkTargetId>,
    )>,
) -> Option<(Entity, Transform, f32, Option<NetworkTargetId>)> {
    target_query
        .iter()
        .filter(|(_, target, _, _, _)| target.health > 0.0)
        .filter(|(_, target, transform, _, _)| {
            horizontal_distance(cursor_hit, transform.translation) <= target.hit_radius
        })
        .min_by(
            |(_, _, left_transform, _, _), (_, _, right_transform, _, _)| {
                horizontal_distance(cursor_hit, left_transform.translation)
                    .partial_cmp(&horizontal_distance(
                        cursor_hit,
                        right_transform.translation,
                    ))
                    .unwrap_or(std::cmp::Ordering::Equal)
            },
        )
        .map(|(entity, target, transform, player, network_target_id)| {
            (
                entity,
                *transform,
                target.hit_radius,
                player
                    .map(|player| NetworkTargetId::Player(player.id.0))
                    .or(network_target_id.copied()),
            )
        })
}

/// Resolves the current local presentation entity for a server-authoritative target id.
fn target_entity_for_network_id(
    target: NetworkTargetId,
    target_query: &Query<(Entity, Option<&Player>, Option<&NetworkTargetId>)>,
) -> Option<Entity> {
    target_query
        .iter()
        .find_map(|(entity, player, lane_target)| {
            let matches_target = match target {
                NetworkTargetId::Player(player_id) => {
                    player.is_some_and(|player| player.id.0 == player_id)
                }
                NetworkTargetId::LaneUnit(lane_unit_id) => lane_target
                    .is_some_and(|target_id| *target_id == NetworkTargetId::LaneUnit(lane_unit_id)),
            };
            matches_target.then_some(entity)
        })
}
fn spawn_auto_attack_projectile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target: Option<Entity>,
    start: Vec3,
    end: Vec3,
    travel_seconds: f32,
    color: Color,
) {
    commands.spawn((
        Name::new("AutoAttackProjectile"),
        AutoAttackProjectile {
            target,
            start,
            end,
            timer: Timer::from_seconds(travel_seconds, TimerMode::Once),
        },
        Mesh3d(meshes.add(Sphere::new(AUTO_ATTACK_PROJECTILE_RADIUS))),
        MeshMaterial3d(materials.add(auto_attack_projectile_material(color))),
        Transform::from_translation(start),
    ));
}
fn auto_attack_projectile_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        emissive: color.with_alpha(0.65).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}
fn send_auto_attack_command(
    senders: &mut Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    target: NetworkTargetId,
) {
    for mut sender in senders {
        sender.send::<ReliableCommandChannel>(PlayerCommand::AutoAttack { target });
    }
}

/// Sends one server-authoritative attack-move order to every active client link.
fn send_attack_move_command(
    command_senders: &mut Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    target: NetworkTargetId,
) {
    for mut sender in command_senders.iter_mut() {
        sender.send::<ReliableCommandChannel>(PlayerCommand::AttackMove { target });
    }
}
