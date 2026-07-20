use crate::systems::{TrainingDummy, targeting::ray_hit_map_top};
use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_shared::game::{camera::TopDownCamera, map::MapGround};
use game_shared::network::{
    AbilitySlot, CastTarget, ChampionId, PlayerCommand, ReliableCommandChannel, WorldPosition,
};
use lightyear::prelude::*;

pub(super) use crate::systems::horizontal_distance;

/// Returns a one-shot timer that is ready immediately.
pub(super) fn ready_timer(duration_seconds: f32) -> Timer {
    let mut timer = Timer::from_seconds(duration_seconds.max(f32::EPSILON), TimerMode::Once);
    timer.set_elapsed(timer.duration());
    timer
}

/// Returns a timer duration that is safe to use as a divisor.
pub(super) fn total_timer_seconds(timer: &Timer) -> f32 {
    timer.duration().as_secs_f32().max(f32::EPSILON)
}

/// Returns the remaining duration of a timer in seconds.
pub(super) fn remaining_timer_seconds(timer: &Timer) -> f32 {
    (total_timer_seconds(timer) - timer.elapsed().as_secs_f32()).max(0.0)
}

/// Returns the elapsed portion of a timer as a percentage.
pub(super) fn ready_timer_percent(timer: &Timer) -> f32 {
    let total_seconds = total_timer_seconds(timer);
    ((total_seconds - remaining_timer_seconds(timer)) / total_seconds * 100.0).clamp(0.0, 100.0)
}

/// Returns the elapsed portion of a timer as a value from zero to one.
pub(super) fn timer_progress(timer: &Timer) -> f32 {
    (timer.elapsed_secs() / total_timer_seconds(timer)).clamp(0.0, 1.0)
}

/// Sends one server-authoritative ability command through every active client link.
pub(super) fn send_ability_command(
    command_senders: &mut Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    champion: ChampionId,
    slot: AbilitySlot,
    target_position: Option<Vec3>,
) {
    for mut sender in command_senders.iter_mut() {
        sender.send::<ReliableCommandChannel>(PlayerCommand::CastAbility {
            champion,
            slot,
            target: CastTarget {
                position: target_position.map(WorldPosition::from),
            },
        });
    }
}

/// Projects the current cursor position onto the top surface of the map.
pub(super) fn cursor_hit_on_map(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera_query: &Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_transform: &GlobalTransform,
    map_ground: MapGround,
) -> Option<Vec3> {
    let window = windows.single().ok()?;
    let cursor_position = window.cursor_position()?;
    let (camera, camera_transform) = camera_query.single().ok()?;
    let ray = camera
        .viewport_to_world(camera_transform, cursor_position)
        .ok()?;

    ray_hit_map_top(ray, map_transform, map_ground)
}

/// Clamps a ground target to the specified horizontal cast range.
pub(super) fn clamp_cast_target(origin: Vec3, target: Vec3, range: f32) -> Vec3 {
    let horizontal_offset = Vec3::new(target.x - origin.x, 0.0, target.z - origin.z);
    if horizontal_offset.length_squared() <= range * range {
        return Vec3::new(target.x, origin.y, target.z);
    }

    origin + horizontal_offset.normalize_or_zero() * range
}

/// Returns the nearest living training target within the specified click radius.
pub(super) fn find_clicked_enemy_target<'a, F>(
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

/// Returns a finite positive value or the supplied fallback.
pub(super) fn positive_or(candidate: f32, fallback: f32) -> f32 {
    if candidate.is_finite() && candidate > 0.0 {
        candidate
    } else {
        fallback
    }
}

/// Creates an unlit transparent material for ability indicators.
pub(super) fn indicator_material(base_color: Color, emissive: Color) -> StandardMaterial {
    StandardMaterial {
        base_color,
        emissive: emissive.into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}
