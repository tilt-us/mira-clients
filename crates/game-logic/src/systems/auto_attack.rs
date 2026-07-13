use super::{
    ExternalMovementModifier, TrainingDummy,
    targeting::{clamp_world_point_to_map_top, ray_hit_map_top},
};
use bevy::math::primitives::Sphere;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_shared::game::{
    camera::TopDownCamera,
    map::MapGround,
    player::{Health, MoveTarget, Player, PlayerControlled},
};
use game_shared::network::{PlayerCommand, ReliableCommandChannel};
use lightyear::prelude::*;

const AUTO_ATTACK_RANGE: f32 = 5.0;
const AUTO_ATTACK_COOLDOWN_SECONDS: f32 = 1.0;
const AUTO_ATTACK_DAMAGE: f32 = 10.0;
const AUTO_ATTACK_PROJECTILE_RADIUS: f32 = 0.12;
const AUTO_ATTACK_PROJECTILE_HEIGHT: f32 = 0.8;
const AUTO_ATTACK_MIN_TRAVEL_SECONDS: f32 = 0.075;
const AUTO_ATTACK_MAX_TRAVEL_SECONDS: f32 = 0.45;

#[derive(Resource, Debug, Clone)]
/// Description:
/// Stores local auto-attack cooldown state.
pub(super) struct AutoAttackState {
    cooldown: Timer,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
/// Description:
/// Tracks whether the current right-click press was consumed by attack input.
pub(super) struct AutoAttackInputState {
    pub(super) consumed_right_press: bool,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
/// Description:
/// Stores the currently ordered auto-attack target.
pub(super) struct AutoAttackTarget {
    pub(super) target: Option<Entity>,
}

#[derive(Component, Debug, Clone)]
/// Description:
/// Stores one local auto-attack projectile travelling toward a clicked enemy target.
pub(super) struct AutoAttackProjectile {
    target: Entity,
    start: Vec3,
    end: Vec3,
    timer: Timer,
    damage: f32,
}

impl Default for AutoAttackState {
    fn default() -> Self {
        let mut cooldown = Timer::from_seconds(AUTO_ATTACK_COOLDOWN_SECONDS, TimerMode::Once);
        cooldown.set_elapsed(cooldown.duration());
        Self { cooldown }
    }
}

/// Description:
/// Converts right-clicks on enemy targets into local auto-attack projectiles.
pub(super) fn handle_auto_attack_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<
        (
            Entity,
            &Health,
            &Transform,
            Option<&ExternalMovementModifier>,
        ),
        With<PlayerControlled>,
    >,
    target_query: Query<(Entity, &TrainingDummy, &Transform, Option<&Player>)>,
    mut input_state: ResMut<AutoAttackInputState>,
    mut attack_target: ResMut<AutoAttackTarget>,
    mut commands: Commands,
) {
    input_state.consumed_right_press = false;

    if !mouse_buttons.just_pressed(MouseButton::Right) {
        return;
    }

    let Some(cursor_hit) = cursor_world_position(&windows, &camera_query, &map_query) else {
        return;
    };
    let Some((target_entity, target_transform, target_radius, _)) =
        clicked_enemy_target(cursor_hit, &target_query)
    else {
        return;
    };

    let Ok((player_entity, health, player_transform, movement_modifier)) = player_query.single()
    else {
        return;
    };
    if health.current == 0 || movement_modifier.is_some_and(|modifier| modifier.stunned) {
        return;
    }

    input_state.consumed_right_press = true;
    attack_target.target = Some(target_entity);
    update_attack_movement(
        &mut commands,
        player_entity,
        player_transform.translation,
        target_transform.translation,
        target_radius,
    );
}

/// Description:
/// Keeps the current auto-attack order active until another target or movement command replaces it.
pub(super) fn update_auto_attack_target(
    time: Res<Time>,
    mut attack_state: ResMut<AutoAttackState>,
    mut attack_target: ResMut<AutoAttackTarget>,
    player_query: Query<
        (
            Entity,
            &Health,
            &Transform,
            Option<&ExternalMovementModifier>,
        ),
        With<PlayerControlled>,
    >,
    target_query: Query<(Entity, &TrainingDummy, &Transform, Option<&Player>)>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    attack_state.cooldown.tick(time.delta());

    let Some(target_entity) = attack_target.target else {
        return;
    };

    let Ok((_, target, target_transform, target_player)) = target_query.get(target_entity) else {
        attack_target.target = None;
        return;
    };
    if target.health <= 0.0 {
        attack_target.target = None;
        return;
    }

    let Ok((player_entity, health, player_transform, movement_modifier)) = player_query.single()
    else {
        return;
    };
    if health.current == 0 || movement_modifier.is_some_and(|modifier| modifier.stunned) {
        commands.entity(player_entity).remove::<MoveTarget>();
        return;
    }

    let attack_distance =
        horizontal_distance(player_transform.translation, target_transform.translation);
    if attack_distance > AUTO_ATTACK_RANGE + target.hit_radius {
        update_attack_movement(
            &mut commands,
            player_entity,
            player_transform.translation,
            target_transform.translation,
            target.hit_radius,
        );
        return;
    }

    commands.entity(player_entity).remove::<MoveTarget>();
    if !attack_state.cooldown.is_finished() {
        return;
    }

    let start = player_transform.translation + Vec3::Y * AUTO_ATTACK_PROJECTILE_HEIGHT;
    let end = target_transform.translation + Vec3::Y * AUTO_ATTACK_PROJECTILE_HEIGHT;
    let travel_seconds = auto_attack_travel_seconds(attack_distance);

    commands.spawn((
        Name::new("AutoAttackProjectile"),
        AutoAttackProjectile {
            target: target_entity,
            start,
            end,
            timer: Timer::from_seconds(travel_seconds, TimerMode::Once),
            damage: AUTO_ATTACK_DAMAGE,
        },
        Mesh3d(meshes.add(Sphere::new(AUTO_ATTACK_PROJECTILE_RADIUS))),
        MeshMaterial3d(materials.add(auto_attack_projectile_material())),
        Transform::from_translation(start),
    ));
    if let Some(target_player_id) = target_player.map(|player| player.id.0) {
        send_auto_attack_command(&mut command_senders, target_player_id);
    }
    attack_state.cooldown.reset();
}

/// Description:
/// Moves active auto-attack projectiles and applies damage when they reach their target.
pub(super) fn update_auto_attack_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut target_query: Query<
        (&mut TrainingDummy, &Transform),
        (Without<AutoAttackProjectile>, With<TrainingDummy>),
    >,
    mut projectile_query: Query<
        (Entity, &mut AutoAttackProjectile, &mut Transform),
        Without<TrainingDummy>,
    >,
) {
    for (projectile_entity, mut projectile, mut transform) in &mut projectile_query {
        let Ok((mut target, target_transform)) = target_query.get_mut(projectile.target) else {
            commands.entity(projectile_entity).despawn();
            continue;
        };

        if target.health <= 0.0 {
            commands.entity(projectile_entity).despawn();
            continue;
        }

        projectile.end = target_transform.translation + Vec3::Y * AUTO_ATTACK_PROJECTILE_HEIGHT;
        projectile.timer.tick(time.delta());

        let duration = projectile.timer.duration().as_secs_f32();
        let progress = (projectile.timer.elapsed_secs() / duration).clamp(0.0, 1.0);
        transform.translation = projectile.start.lerp(projectile.end, progress);

        if projectile.timer.is_finished() {
            target.health = (target.health - projectile.damage).max(0.0);
            info!(
                "TrainingDummy hit by auto attack: -{:.1} HP (remaining {:.1})",
                projectile.damage, target.health
            );
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
    target_query: &Query<(Entity, &TrainingDummy, &Transform, Option<&Player>)>,
) -> Option<(Entity, Transform, f32, Option<u64>)> {
    target_query
        .iter()
        .filter(|(_, target, _, _)| target.health > 0.0)
        .filter(|(_, target, transform, _)| {
            horizontal_distance(cursor_hit, transform.translation) <= target.hit_radius
        })
        .min_by(|(_, _, left_transform, _), (_, _, right_transform, _)| {
            horizontal_distance(cursor_hit, left_transform.translation)
                .partial_cmp(&horizontal_distance(
                    cursor_hit,
                    right_transform.translation,
                ))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, target, transform, player)| {
            (
                entity,
                *transform,
                target.hit_radius,
                player.map(|player| player.id.0),
            )
        })
}

fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

fn update_attack_movement(
    commands: &mut Commands,
    player_entity: Entity,
    player_position: Vec3,
    target_position: Vec3,
    target_radius: f32,
) {
    let stop_distance = AUTO_ATTACK_RANGE + target_radius;
    if horizontal_distance(player_position, target_position) <= stop_distance {
        commands.entity(player_entity).remove::<MoveTarget>();
    } else {
        commands.entity(player_entity).insert(MoveTarget {
            position: target_position,
            stop_distance,
        });
    }
}

fn auto_attack_travel_seconds(distance: f32) -> f32 {
    let range_ratio = (distance / AUTO_ATTACK_RANGE).clamp(0.0, 1.0);
    (range_ratio * AUTO_ATTACK_MAX_TRAVEL_SECONDS).clamp(
        AUTO_ATTACK_MIN_TRAVEL_SECONDS,
        AUTO_ATTACK_MAX_TRAVEL_SECONDS,
    )
}

fn auto_attack_projectile_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.95),
        emissive: Color::srgba(1.0, 1.0, 1.0, 0.65).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}

fn send_auto_attack_command(
    senders: &mut Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    target_player_id: u64,
) {
    for mut sender in senders {
        sender.send::<ReliableCommandChannel>(PlayerCommand::AutoAttack { target_player_id });
    }
}
