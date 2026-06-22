use bevy::math::Ray3d;
use bevy::prelude::*;
use game_shared::game::map::MapGround;

/// Description:
/// Intersects a world-space ray with the top surface of the map ground.
///
/// Params:
/// - `ray`: World-space ray to test.
/// - `map_transform`: Map transform used to convert between world and local space.
/// - `map`: Map bounds and top-surface data.
///
/// Return:
/// - The world-space hit point on the map top surface, or `None` when no valid hit exists.
pub(super) fn ray_hit_map_top(
    ray: Ray3d,
    map_transform: &GlobalTransform,
    map: MapGround,
) -> Option<Vec3> {
    let map_to_world = map_transform.affine();
    let world_to_map = map_to_world.inverse();

    let local_origin = world_to_map.transform_point3(ray.origin);
    let local_direction = world_to_map.transform_vector3(ray.direction.as_vec3());
    if local_direction.y.abs() <= f32::EPSILON {
        return None;
    }

    let top_y = map.top_local_y();
    let travel = (top_y - local_origin.y) / local_direction.y;
    if travel <= 0.0 {
        return None;
    }

    let local_hit = local_origin + local_direction * travel;
    if !map.contains_local_xz(local_hit) {
        return None;
    }

    let top_hit_local = Vec3::new(local_hit.x, top_y, local_hit.z);
    Some(map_to_world.transform_point3(top_hit_local))
}

/// Description:
/// Clamps a world-space point to the map bounds and places it on the map top surface.
///
/// Params:
/// - `world_point`: Point to clamp.
/// - `map_transform`: Map transform used to convert between world and local space.
/// - `map`: Map bounds and top-surface data.
///
/// Return:
/// - The clamped world-space point on the map top surface.
pub(super) fn clamp_world_point_to_map_top(
    world_point: Vec3,
    map_transform: &GlobalTransform,
    map: MapGround,
) -> Vec3 {
    let map_to_world = map_transform.affine();
    let world_to_map = map_to_world.inverse();

    let mut local_point = world_to_map.transform_point3(world_point);
    local_point.x = local_point.x.clamp(-map.half_extents.x, map.half_extents.x);
    local_point.z = local_point.z.clamp(-map.half_extents.y, map.half_extents.y);
    local_point.y = map.top_local_y();

    map_to_world.transform_point3(local_point)
}
