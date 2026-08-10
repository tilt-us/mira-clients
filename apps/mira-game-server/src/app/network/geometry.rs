use bevy::prelude::*;
/// Clamps a cast target to a maximum range from an origin.
pub(super) fn clamp_cast_target(origin: Vec3, target: Vec3, range: f32) -> Vec3 {
    let delta = Vec3::new(target.x - origin.x, 0.0, target.z - origin.z);
    if delta.length_squared() <= range * range {
        return Vec3::new(target.x, origin.y, target.z);
    }

    origin + delta.normalize_or_zero() * range
}
/// Computes the horizontal distance from a point to a segment.
pub(super) fn distance_to_segment_xz(point: Vec3, segment_start: Vec3, segment_end: Vec3) -> f32 {
    let point = Vec2::new(point.x, point.z);
    let segment_start = Vec2::new(segment_start.x, segment_start.z);
    let segment_end = Vec2::new(segment_end.x, segment_end.z);
    let segment = segment_end - segment_start;
    let segment_length_squared = segment.length_squared();

    if segment_length_squared <= f32::EPSILON {
        return point.distance(segment_start);
    }

    let t = ((point - segment_start).dot(segment) / segment_length_squared).clamp(0.0, 1.0);
    point.distance(segment_start + segment * t)
}
/// Computes horizontal distance between two world-space positions.
pub(super) fn horizontal_distance(left: Vec3, right: Vec3) -> f32 {
    Vec2::new(left.x, left.z).distance(Vec2::new(right.x, right.z))
}
/// Checks whether a world-space point lies inside an oriented XZ rectangle.
pub(super) fn point_in_oriented_rect_xz(point: Vec3, start: Vec3, end: Vec3, width: f32) -> bool {
    let start_2d = Vec2::new(start.x, start.z);
    let end_2d = Vec2::new(end.x, end.z);
    let point_2d = Vec2::new(point.x, point.z);
    let axis = end_2d - start_2d;
    let length = axis.length();
    if length <= f32::EPSILON {
        return false;
    }

    let forward = axis / length;
    let right = Vec2::new(forward.y, -forward.x);
    let local = point_2d - start_2d;
    let forward_distance = local.dot(forward);
    let side_distance = local.dot(right).abs();

    forward_distance >= 0.0 && forward_distance <= length && side_distance <= width * 0.5
}
