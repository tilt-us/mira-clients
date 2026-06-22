use super::{
    ExternalMovementModifier, HOLD_CURSOR_MIN_DISTANCE, HoldMoveDirection, MoveTargetMarker,
    MoveTargetMarkerFx,
    targeting::{clamp_world_point_to_map_top, ray_hit_map_top},
};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_shared::game::{
    camera::TopDownCamera,
    map::MapGround,
    player::{Health, MoveSpeed, MoveTarget, PlayerControlled},
};

const MOVE_TARGET_UPDATE_EPSILON: f32 = 0.08;
const MOVE_TARGET_REACHED_DISTANCE: f32 = 0.04;

/// Description:
/// Converts held right-click input into controlled-player movement targets.
///
/// Params:
/// - `mouse_buttons`: Mouse button input used to detect right-click movement.
/// - `windows`: Primary window used to read cursor position.
/// - `camera_query`: Top-down camera used to project the cursor into world space.
/// - `map_query`: Map ground transform and bounds used for cursor hit tests.
/// - `hold_direction`: Last valid hold movement direction for close cursor movement.
/// - `player_query`: Controlled players that receive movement targets when alive.
/// - `marker_query`: Movement marker visual updated to the selected target.
/// - `commands`: ECS command buffer used to insert `MoveTarget` components.
pub(super) fn set_move_target_from_mouse_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    mut hold_direction: ResMut<HoldMoveDirection>,
    player_query: Query<
        (
            Entity,
            &Health,
            &Transform,
            Option<&MoveTarget>,
            Option<&ExternalMovementModifier>,
        ),
        (With<PlayerControlled>, Without<MoveTargetMarker>),
    >,
    mut marker_query: Query<
        (&mut Transform, &mut Visibility, &mut MoveTargetMarkerFx),
        (With<MoveTargetMarker>, Without<PlayerControlled>),
    >,
    mut commands: Commands,
) {
    let right_hold = mouse_buttons.pressed(MouseButton::Right);
    let right_pressed = mouse_buttons.just_pressed(MouseButton::Right);
    if !right_hold {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        return;
    };

    let Ok((map_transform, map_ground)) = map_query.single() else {
        return;
    };
    let Some(target) = ray_hit_map_top(ray, map_transform, *map_ground) else {
        return;
    };

    let mut marker_target = target;
    let mut did_set_move_target = false;

    for (entity, health, player_transform, current_target, movement_modifier) in &player_query {
        if health.current == 0 || movement_modifier.is_some_and(|modifier| modifier.stunned) {
            commands.entity(entity).remove::<MoveTarget>();
            continue;
        }

        let to_cursor = Vec3::new(
            target.x - player_transform.translation.x,
            0.0,
            target.z - player_transform.translation.z,
        );
        let distance_to_cursor = to_cursor.length();

        if distance_to_cursor > f32::EPSILON {
            hold_direction.0 = to_cursor / distance_to_cursor;
        }

        let move_target = if right_hold && distance_to_cursor < HOLD_CURSOR_MIN_DISTANCE {
            let pushed = player_transform.translation + hold_direction.0 * HOLD_CURSOR_MIN_DISTANCE;
            clamp_world_point_to_map_top(pushed, map_transform, *map_ground)
        } else {
            target
        };

        marker_target = move_target;
        did_set_move_target = true;
        if should_update_move_target(current_target, move_target) {
            commands.entity(entity).insert(MoveTarget::new(move_target));
        }
    }

    if let Ok((mut marker_transform, mut marker_visibility, mut marker_fx)) =
        marker_query.single_mut()
    {
        if !did_set_move_target {
            marker_fx.active = false;
            *marker_visibility = Visibility::Hidden;
            return;
        }

        marker_transform.translation = marker_target + Vec3::Y * 0.03;
        marker_transform.scale = Vec3::splat(0.45);
        *marker_visibility = Visibility::Visible;
        if right_pressed {
            marker_fx.timer.reset();
            marker_fx.active = true;
        }
    }
}

/// Description:
/// Checks whether the active movement target should be replaced.
///
/// Params:
/// - `current_target`: Currently assigned movement target, if any.
/// - `next_position`: Newly requested movement target position.
///
/// Returns:
/// - `true` when no target exists or the target moved far enough to matter.
fn should_update_move_target(current_target: Option<&MoveTarget>, next_position: Vec3) -> bool {
    let Some(current_target) = current_target else {
        return true;
    };

    current_target.position.distance_squared(next_position)
        > MOVE_TARGET_UPDATE_EPSILON * MOVE_TARGET_UPDATE_EPSILON
}

/// Description:
/// Moves controlled players toward their current movement target and removes reached targets.
///
/// Params:
/// - `time`: Frame timing used to scale movement and turning.
/// - `commands`: ECS command buffer used to remove completed movement targets.
/// - `player_query`: Controlled alive players with movement speed, target, and transform data.
pub(super) fn move_controlled_player(
    time: Res<Time>,
    mut commands: Commands,
    mut player_query: Query<
        (
            Entity,
            &Health,
            &MoveSpeed,
            Option<&MoveTarget>,
            Option<&ExternalMovementModifier>,
            &mut Transform,
        ),
        With<PlayerControlled>,
    >,
) {
    for (entity, health, move_speed, move_target, movement_modifier, mut transform) in
        &mut player_query
    {
        if health.current == 0 || movement_modifier.is_some_and(|modifier| modifier.stunned) {
            commands.entity(entity).remove::<MoveTarget>();
            continue;
        }

        if let Some(move_target) = move_target {
            let to_target = move_target.position - transform.translation;
            let distance = to_target.length();

            if distance <= move_target.stop_distance {
                if distance <= MOVE_TARGET_REACHED_DISTANCE {
                    transform.translation = move_target.position;
                }
                commands.entity(entity).remove::<MoveTarget>();
            } else {
                let direction = to_target / distance;
                let target_yaw = direction.x.atan2(direction.z);
                let desired_rotation = Quat::from_rotation_y(target_yaw);
                let turn_blend = (10.0 * time.delta_secs()).clamp(0.0, 1.0);

                transform.rotation = transform.rotation.slerp(desired_rotation, turn_blend);

                let speed_multiplier = movement_modifier
                    .map(|modifier| modifier.speed_multiplier)
                    .unwrap_or(1.0)
                    .clamp(0.0, 2.0);
                let step = move_speed.0 * speed_multiplier * time.delta_secs();
                let movement = step.min(distance);

                transform.translation += direction * movement;
            }
        }

        if let Some(modifier) = movement_modifier
            && let Some(pull_center) = modifier.pull_center
        {
            apply_external_pull(
                &mut transform,
                pull_center,
                modifier.pull_speed,
                time.delta_secs(),
            );
        }
        transform.translation.y = 0.0;
    }
}

fn apply_external_pull(
    transform: &mut Transform,
    pull_center: Vec3,
    pull_speed: f32,
    delta_seconds: f32,
) {
    let pull_delta = Vec3::new(
        pull_center.x - transform.translation.x,
        0.0,
        pull_center.z - transform.translation.z,
    );
    let pull_distance = pull_delta.length();
    if pull_distance <= 0.05 {
        return;
    }

    let step = (pull_speed * delta_seconds).min(pull_distance);
    transform.translation += pull_delta.normalize() * step;
}

/// Description:
/// Animates and fades the movement target marker after a right-click command.
///
/// Params:
/// - `time`: Frame timing used to advance the marker animation.
/// - `materials`: Material assets used to fade the marker color.
/// - `marker_query`: Movement marker transform, visibility, animation state, and material handle.
pub(super) fn animate_move_target_marker(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut marker_query: Query<
        (
            &mut Transform,
            &mut Visibility,
            &mut MoveTargetMarkerFx,
            &MeshMaterial3d<StandardMaterial>,
        ),
        With<MoveTargetMarker>,
    >,
) {
    let Ok((mut marker_transform, mut marker_visibility, mut marker_fx, marker_material)) =
        marker_query.single_mut()
    else {
        return;
    };

    if !marker_fx.active {
        return;
    }

    marker_fx.timer.tick(time.delta());

    let duration = marker_fx.timer.duration().as_secs_f32();
    let progress = (marker_fx.timer.elapsed_secs() / duration).clamp(0.0, 1.0);

    marker_transform.scale = Vec3::splat(0.45 + progress * 0.75);

    if let Some(material) = materials.get_mut(&marker_material.0) {
        material.base_color = material.base_color.with_alpha(1.0 - progress);
        material.emissive = Color::srgba(0.4, 0.35, 0.05, 1.0 - progress).into();
    }

    if marker_fx.timer.is_finished() {
        marker_fx.active = false;
        *marker_visibility = Visibility::Hidden;
    }
}
