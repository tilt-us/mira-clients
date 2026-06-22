use bevy::ecs::message::MessageReader;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use game_shared::game::{
    camera::{CameraFocus, CameraZoom, TopDownCamera, TopDownCameraSettings},
    player::PlayerControlled,
};

/// Description:
/// Moves the top-down camera focus target to the controlled player when centered.
///
/// Params:
/// - `player_query`: Controlled player transform used as the camera follow target.
/// - `camera_focus_query`: Camera focus components that should track the player.
pub(super) fn follow_controlled_player(
    player_query: Query<&Transform, (With<PlayerControlled>, Without<TopDownCamera>)>,
    mut camera_focus_query: Query<&mut CameraFocus, With<TopDownCamera>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    for mut focus in &mut camera_focus_query {
        if focus.centered {
            focus.target = player_transform.translation;
        }
    }
}

/// Description:
/// Applies mouse wheel input to top-down camera zoom components.
///
/// Params:
/// - `mouse_wheel_events`: Mouse wheel events collected during the frame.
/// - `camera_query`: Camera zoom components to adjust.
pub(super) fn handle_camera_zoom(
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    mut camera_query: Query<&mut CameraZoom, With<TopDownCamera>>,
) {
    let scroll_delta: f32 = mouse_wheel_events.read().map(|event| event.y).sum();
    if scroll_delta.abs() <= f32::EPSILON {
        return;
    }

    for mut zoom in &mut camera_query {
        zoom.zoom_by(-scroll_delta);
    }
}

/// Description:
/// Smoothly positions and aims the top-down camera from its focus and zoom settings.
///
/// Params:
/// - `time`: Frame timing used for interpolation.
/// - `camera_query`: Camera transforms and top-down camera state to update.
pub(super) fn update_top_down_camera(
    time: Res<Time>,
    mut camera_query: Query<
        (
            &mut Transform,
            &TopDownCameraSettings,
            &CameraZoom,
            &CameraFocus,
        ),
        With<TopDownCamera>,
    >,
) {
    for (mut transform, settings, zoom, focus) in &mut camera_query {
        let yaw_cos = settings.yaw_radians.cos();
        let yaw_sin = settings.yaw_radians.sin();
        let pitch_cos = settings.pitch_radians.cos();
        let pitch_sin = settings.pitch_radians.sin();

        let look_direction = Vec3::new(yaw_cos, 0.0, yaw_sin);
        let look_ahead = if focus.centered {
            0.0
        } else {
            settings.look_ahead_ground
        };
        let look_target = focus.target + look_direction * look_ahead;

        let desired_position = look_target
            + Vec3::new(
                yaw_cos * pitch_cos * zoom.current,
                settings.height - pitch_sin * zoom.current,
                yaw_sin * pitch_cos * zoom.current,
            );

        if focus.centered {
            transform.translation = desired_position;
        } else {
            let blend = 1.0 - (-settings.follow_lerp * time.delta_secs()).exp();
            transform.translation = transform.translation.lerp(desired_position, blend);
        }
        transform.look_at(look_target, Vec3::Y);
    }
}
