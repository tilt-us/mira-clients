use super::{
    HOLD_CURSOR_MIN_DISTANCE, HoldMoveDirection, MoveTargetMarker, MoveTargetMarkerFx,
    auto_attack::{AutoAttackInputState, AutoAttackTarget},
    targeting::{clamp_world_point_to_map_top, ray_hit_map_top},
};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use mira_game_api::game::{camera::TopDownCamera, map::MapGround, player::PlayerControlled};
use mira_game_api::network::{PlayerCommand, ReliableCommandChannel, WorldPosition};
use lightyear::prelude::*;

const MOVE_TARGET_UPDATE_EPSILON: f32 = 0.08;

/// Remembers the last movement destination sent to the dedicated server.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub(super) struct LastMoveRequest(Option<Vec3>);

/// Converts right-click input into movement requests for the dedicated server.
///
/// This system never updates a player transform. The server owns pathfinding, collision
/// resolution, speed modifiers, and the resulting match snapshot.
pub(super) fn send_move_request_from_mouse_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    attack_input: Res<AutoAttackInputState>,
    mut attack_target: ResMut<AutoAttackTarget>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    player_query: Query<&Transform, (With<PlayerControlled>, Without<MoveTargetMarker>)>,
    mut hold_direction: ResMut<HoldMoveDirection>,
    mut last_request: ResMut<LastMoveRequest>,
    mut marker_query: Query<
        (&mut Transform, &mut Visibility, &mut MoveTargetMarkerFx),
        With<MoveTargetMarker>,
    >,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
) {
    if !mouse_buttons.pressed(MouseButton::Right) {
        last_request.0 = None;
        return;
    }

    let right_pressed = mouse_buttons.just_pressed(MouseButton::Right);
    if right_pressed {
        if attack_input.consumed_right_press {
            return;
        }
        attack_target.target = None;
    } else if attack_target.target.is_some() {
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
    let Some(cursor_target) = ray_hit_map_top(ray, map_transform, *map_ground) else {
        return;
    };
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let offset = Vec3::new(
        cursor_target.x - player_transform.translation.x,
        0.0,
        cursor_target.z - player_transform.translation.z,
    );
    if offset.length_squared() > f32::EPSILON {
        hold_direction.0 = offset.normalize();
    }
    let target = if offset.length() < HOLD_CURSOR_MIN_DISTANCE {
        clamp_world_point_to_map_top(
            player_transform.translation + hold_direction.0 * HOLD_CURSOR_MIN_DISTANCE,
            map_transform,
            *map_ground,
        )
    } else {
        cursor_target
    };

    let changed = last_request.0.is_none_or(|previous| {
        previous.distance_squared(target) > MOVE_TARGET_UPDATE_EPSILON * MOVE_TARGET_UPDATE_EPSILON
    });
    if changed {
        for mut sender in &mut command_senders {
            sender
                .send::<ReliableCommandChannel>(PlayerCommand::MoveTo(WorldPosition::from(target)));
        }
        last_request.0 = Some(target);
    }

    let Ok((mut marker_transform, mut marker_visibility, mut marker_fx)) =
        marker_query.single_mut()
    else {
        return;
    };
    marker_transform.translation = target + Vec3::Y * 0.03;
    marker_transform.scale = Vec3::splat(0.45);
    *marker_visibility = Visibility::Visible;
    if right_pressed {
        marker_fx.timer.reset();
        marker_fx.active = true;
    }
}

/// Animates and fades the input feedback marker after a right-click command.
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

    if let Some(mut material) = materials.get_mut(&marker_material.0) {
        material.base_color = material.base_color.with_alpha(1.0 - progress);
        material.emissive = Color::srgba(0.4, 0.35, 0.05, 1.0 - progress).into();
    }
    if marker_fx.timer.is_finished() {
        marker_fx.active = false;
        *marker_visibility = Visibility::Hidden;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_input_queries_are_disjoint() {
        let mut app = App::new();
        app.insert_resource(ButtonInput::<MouseButton>::default())
            .insert_resource(AutoAttackInputState::default())
            .insert_resource(AutoAttackTarget::default())
            .insert_resource(HoldMoveDirection(Vec3::Z))
            .insert_resource(LastMoveRequest::default())
            .add_systems(Update, send_move_request_from_mouse_input);

        app.update();
    }
}
