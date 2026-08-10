use bevy::prelude::*;
use polyanya::Triangulation;
use polyanya_glam::Vec2 as NavigationVec2;

use super::lane::{LANE_HALF_WIDTH, LANE_SPAWN_Z};

/// Extra clearance kept between a moving unit and the lane boundary or a static obstacle.
pub const LANE_NAVIGATION_CLEARANCE: f32 = 0.05;

const NAVIGATION_CIRCLE_SEGMENTS: usize = 16;
const NAVIGATION_SEARCH_DELTA: f32 = 0.25;
const NAVIGATION_SEARCH_STEPS: u32 = 32;
const COLLISION_SKIN_DISTANCE: f32 = 0.001;

/// Describes a circular static obstacle used while building a lane navigation mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneNavigationObstacle {
    /// Ground-space center of the obstacle.
    pub center: Vec3,
    /// Physical horizontal radius of the obstacle.
    pub radius: f32,
}

impl LaneNavigationObstacle {
    /// Creates a circular lane navigation obstacle.
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }
}

/// Resolves a movement segment against circular lane-navigation obstacles.
///
/// The mover radius is added to every obstacle. The returned position remains outside the first
/// obstacle reached by the segment, preventing a large position update from crossing a tower.
pub fn resolve_circle_obstacle_collisions(
    start: Vec3,
    desired: Vec3,
    mover_radius: f32,
    obstacles: &[LaneNavigationObstacle],
) -> Vec3 {
    let mut resolved = desired;

    for obstacle in obstacles {
        let candidate = resolve_horizontal_circle_collision(
            start,
            desired,
            obstacle.center,
            obstacle.radius.max(0.0) + mover_radius.max(0.0),
        );
        if candidate != desired
            && (resolved == desired
                || horizontal_distance_squared(start, candidate)
                    < horizontal_distance_squared(start, resolved))
        {
            resolved = candidate;
        }
    }

    resolved
}

fn resolve_horizontal_circle_collision(
    start: Vec3,
    desired: Vec3,
    center: Vec3,
    radius: f32,
) -> Vec3 {
    let start_xz = Vec2::new(start.x, start.z);
    let desired_xz = Vec2::new(desired.x, desired.z);
    let center_xz = Vec2::new(center.x, center.z);
    let mut segment_start = start_xz;
    let movement = desired_xz - start_xz;
    let radius_squared = radius * radius;
    let start_offset = segment_start - center_xz;

    if start_offset.length_squared() < radius_squared {
        let outward = if start_offset.length_squared() > f32::EPSILON {
            start_offset.normalize()
        } else if movement.length_squared() > f32::EPSILON {
            -movement.normalize()
        } else {
            Vec2::X
        };
        segment_start = center_xz + outward * radius;
    }

    let segment = desired_xz - segment_start;
    let segment_length_squared = segment.length_squared();
    if segment_length_squared <= f32::EPSILON {
        return with_horizontal_position(desired, segment_start);
    }

    let offset = segment_start - center_xz;
    let half_linear_coefficient = offset.dot(segment);
    let quadratic_constant = offset.length_squared() - radius_squared;
    let discriminant = half_linear_coefficient * half_linear_coefficient
        - segment_length_squared * quadratic_constant;
    if discriminant < 0.0 {
        return desired;
    }

    let root = discriminant.sqrt();
    let entry_time = (-half_linear_coefficient - root) / segment_length_squared;
    let exit_time = (-half_linear_coefficient + root) / segment_length_squared;
    if exit_time <= 0.0 || entry_time >= 1.0 {
        return desired;
    }

    if entry_time <= 0.0 && offset.dot(segment) >= 0.0 {
        return desired;
    }

    let skin = (COLLISION_SKIN_DISTANCE / segment.length()).min(entry_time.max(0.0));
    let stop_time = (entry_time.max(0.0) - skin).max(0.0);
    with_horizontal_position(desired, segment_start + segment * stop_time)
}

fn with_horizontal_position(position: Vec3, horizontal: Vec2) -> Vec3 {
    Vec3::new(horizontal.x, position.y, horizontal.y)
}

fn horizontal_distance_squared(left: Vec3, right: Vec3) -> f32 {
    Vec2::new(left.x - right.x, left.z - right.z).length_squared()
}

/// Stores a baked polygon navigation mesh for one lane mover radius.
#[derive(Debug, Clone)]
pub struct LaneNavigationMesh {
    mesh: polyanya::Mesh,
    agent_radius: f32,
}

impl LaneNavigationMesh {
    /// Builds a lane navigation mesh with holes for every valid static obstacle.
    pub fn new(agent_radius: f32, obstacles: &[LaneNavigationObstacle]) -> Self {
        let agent_radius = agent_radius.max(0.0);
        let mut triangulation = Triangulation::from_outer_edges(&lane_outer_edges());
        triangulation.set_agent_radius(agent_radius + LANE_NAVIGATION_CLEARANCE);
        triangulation.agent_radius_on_outer_edge(true);
        triangulation.set_agent_radius_segments(8);

        for obstacle in normalized_obstacles(obstacles) {
            triangulation.add_obstacle(circle_obstacle_edges(obstacle));
        }

        let mut mesh = triangulation.as_navmesh();
        while mesh.merge_polygons() {}
        mesh.bake();
        mesh.set_search_delta(NAVIGATION_SEARCH_DELTA)
            .set_search_steps(NAVIGATION_SEARCH_STEPS);

        Self { mesh, agent_radius }
    }

    /// Returns the radius this mesh was baked for.
    pub fn agent_radius(&self) -> f32 {
        self.agent_radius
    }

    /// Returns an any-angle route from `start` to the closest reachable version of `goal`.
    ///
    /// The returned waypoints include a recovery step when `start` must be projected onto the
    /// mesh, then end at the reachable final destination.
    pub fn find_path(&self, start: Vec3, goal: Vec3) -> Option<Vec<Vec3>> {
        let mut path = self.find_path_with_projection(start, goal)?;
        path.prepend_start_recovery_waypoint(start);
        Some(path.waypoints)
    }

    /// Returns a route together with the nearest valid start position used by the mesh.
    ///
    /// Callers can use `start` to recover safely when an obstacle was introduced around an
    /// existing mover between snapshots.
    pub fn find_path_with_projection(&self, start: Vec3, goal: Vec3) -> Option<LaneNavigationPath> {
        if !is_finite_ground_position(start) || !is_finite_ground_position(goal) {
            return None;
        }

        let start = self.mesh.get_closest_point(to_navigation_vec2(start))?;
        let goal = self.mesh.get_closest_point(to_navigation_vec2(goal))?;
        let path = self.mesh.path(start, goal)?;

        let mut waypoints = path
            .path
            .into_iter()
            .map(|waypoint| Vec3::new(waypoint.x, 0.0, waypoint.y))
            .collect::<Vec<_>>();
        waypoints.dedup_by(|left, right| left.distance_squared(*right) <= 0.000_001);
        Some(LaneNavigationPath {
            start: Vec3::new(start.position().x, 0.0, start.position().y),
            waypoints,
        })
    }
}

/// Stores a route returned by a lane navigation mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct LaneNavigationPath {
    /// Closest traversable equivalent of the requested start position.
    pub start: Vec3,
    /// Ordered route waypoints ending at the closest traversable goal position.
    pub waypoints: Vec<Vec3>,
}

impl LaneNavigationPath {
    /// Ensures the projected start position is the first waypoint when the requested start was
    /// outside the mesh.
    ///
    /// Returns whether the route begins with a recovery waypoint.
    pub fn prepend_start_recovery_waypoint(&mut self, requested_start: Vec3) -> bool {
        const RECOVERY_POINT_EPSILON_SQUARED: f32 = 0.000_001;

        if self.start.distance_squared(requested_start) <= RECOVERY_POINT_EPSILON_SQUARED {
            return false;
        }

        let starts_with_projected_start = self.waypoints.first().is_some_and(|waypoint| {
            waypoint.distance_squared(self.start) <= RECOVERY_POINT_EPSILON_SQUARED
        });
        if !starts_with_projected_start {
            self.waypoints.insert(0, self.start);
        }

        true
    }
}

/// Returns the lane bounds available to the center of a mover with the given radius.
pub fn lane_navigation_bounds(agent_radius: f32) -> Vec2 {
    let clearance = agent_radius.max(0.0) + LANE_NAVIGATION_CLEARANCE;
    Vec2::new(
        (LANE_HALF_WIDTH - clearance).max(0.0),
        (LANE_SPAWN_Z - clearance).max(0.0),
    )
}

/// Computes a stable revision for a set of lane navigation obstacles.
pub fn lane_navigation_obstacle_revision(obstacles: &[LaneNavigationObstacle]) -> u64 {
    let mut revision = 0xcbf2_9ce4_8422_2325_u64;
    for obstacle in normalized_obstacles(obstacles) {
        for value in [
            obstacle.center.x.to_bits(),
            obstacle.center.z.to_bits(),
            obstacle.radius.to_bits(),
        ] {
            revision ^= u64::from(value);
            revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    revision
}

/// Returns static obstacles in a deterministic order while dropping invalid inputs.
pub fn normalized_obstacles(obstacles: &[LaneNavigationObstacle]) -> Vec<LaneNavigationObstacle> {
    let mut candidates = obstacles
        .iter()
        .copied()
        .filter(|obstacle| {
            is_finite_ground_position(obstacle.center)
                && obstacle.radius.is_finite()
                && obstacle.radius > 0.0
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(obstacle_sort_key);

    let mut normalized = Vec::with_capacity(candidates.len());
    for mut candidate in candidates {
        let mut index = 0;
        while index < normalized.len() {
            if obstacles_overlap(candidate, normalized[index]) {
                candidate = enclosing_obstacle(candidate, normalized.swap_remove(index));
                index = 0;
            } else {
                index += 1;
            }
        }
        if obstacle_fits_lane(candidate) {
            normalized.push(candidate);
        }
    }
    normalized.sort_by_key(obstacle_sort_key);
    normalized
}

fn obstacle_sort_key(obstacle: &LaneNavigationObstacle) -> (u32, u32, u32) {
    (
        canonical_zero_bits(obstacle.center.x),
        canonical_zero_bits(obstacle.center.z),
        canonical_zero_bits(obstacle.radius),
    )
}

fn canonical_zero_bits(value: f32) -> u32 {
    if value == 0.0 { 0 } else { value.to_bits() }
}

fn obstacles_overlap(left: LaneNavigationObstacle, right: LaneNavigationObstacle) -> bool {
    let center_distance =
        Vec2::new(left.center.x, left.center.z).distance(Vec2::new(right.center.x, right.center.z));
    center_distance < left.radius + right.radius
}

fn enclosing_obstacle(
    left: LaneNavigationObstacle,
    right: LaneNavigationObstacle,
) -> LaneNavigationObstacle {
    let left_center = Vec2::new(left.center.x, left.center.z);
    let right_center = Vec2::new(right.center.x, right.center.z);
    let delta = right_center - left_center;
    let distance = delta.length();
    if left.radius >= distance + right.radius {
        return left;
    }
    if right.radius >= distance + left.radius {
        return right;
    }

    let radius = (distance + left.radius + right.radius) * 0.5;
    let center = if distance <= f32::EPSILON {
        left_center
    } else {
        left_center + delta * ((radius - left.radius) / distance)
    };
    LaneNavigationObstacle::new(Vec3::new(center.x, 0.0, center.y), radius)
}

fn obstacle_fits_lane(obstacle: LaneNavigationObstacle) -> bool {
    let segment_angle = std::f32::consts::PI / NAVIGATION_CIRCLE_SEGMENTS as f32;
    let polygon_radius = obstacle.radius / segment_angle.cos();
    obstacle.center.x.abs() + polygon_radius < LANE_HALF_WIDTH
        && obstacle.center.z.abs() + polygon_radius < LANE_SPAWN_Z
}

fn lane_outer_edges() -> [NavigationVec2; 4] {
    [
        NavigationVec2::new(-LANE_HALF_WIDTH, -LANE_SPAWN_Z),
        NavigationVec2::new(LANE_HALF_WIDTH, -LANE_SPAWN_Z),
        NavigationVec2::new(LANE_HALF_WIDTH, LANE_SPAWN_Z),
        NavigationVec2::new(-LANE_HALF_WIDTH, LANE_SPAWN_Z),
    ]
}

fn circle_obstacle_edges(obstacle: LaneNavigationObstacle) -> Vec<NavigationVec2> {
    let segment_angle = std::f32::consts::PI / NAVIGATION_CIRCLE_SEGMENTS as f32;
    let radius = obstacle.radius / segment_angle.cos();
    (0..NAVIGATION_CIRCLE_SEGMENTS)
        .map(|index| {
            let angle = index as f32 * std::f32::consts::TAU / NAVIGATION_CIRCLE_SEGMENTS as f32;
            NavigationVec2::new(
                obstacle.center.x + radius * angle.cos(),
                obstacle.center.z + radius * angle.sin(),
            )
        })
        .collect()
}

fn to_navigation_vec2(position: Vec3) -> NavigationVec2 {
    NavigationVec2::new(position.x, position.z)
}

fn is_finite_ground_position(position: Vec3) -> bool {
    position.x.is_finite() && position.z.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obstacle_clearance(path: &[Vec3], start: Vec3, obstacle: LaneNavigationObstacle) -> f32 {
        path.iter()
            .fold((start, f32::INFINITY), |(previous, clearance), waypoint| {
                let segment = Vec2::new(waypoint.x - previous.x, waypoint.z - previous.z);
                let from_center = Vec2::new(
                    obstacle.center.x - previous.x,
                    obstacle.center.z - previous.z,
                );
                let t = if segment.length_squared() <= f32::EPSILON {
                    0.0
                } else {
                    (from_center.dot(segment) / segment.length_squared()).clamp(0.0, 1.0)
                };
                let nearest = Vec2::new(previous.x, previous.z) + segment * t;
                let next_clearance = clearance
                    .min(nearest.distance(Vec2::new(obstacle.center.x, obstacle.center.z)));
                (*waypoint, next_clearance)
            })
            .1
    }

    #[test]
    fn direct_paths_keep_the_requested_goal() {
        let mesh = LaneNavigationMesh::new(0.5, &[]);
        let start = Vec3::new(-1.0, 0.0, -10.0);
        let goal = Vec3::new(1.0, 0.0, 10.0);

        let path = mesh.find_path(start, goal).expect("a direct lane path");

        assert_eq!(path, vec![goal]);
    }

    #[test]
    fn paths_route_around_live_towers_with_agent_clearance() {
        let obstacle = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);
        let agent_radius = 0.9;
        let mesh = LaneNavigationMesh::new(agent_radius, &[obstacle]);
        let start = Vec3::new(0.0, 0.0, -12.0);
        let goal = Vec3::new(0.0, 0.0, 12.0);

        let path = mesh
            .find_path(start, goal)
            .expect("a lane route around a tower");

        assert!(path.len() > 1);
        assert!(path.iter().any(|waypoint| waypoint.x.abs() > 1.0));
        assert!(
            obstacle_clearance(&path, start, obstacle) >= obstacle.radius + agent_radius - 0.001
        );
    }

    #[test]
    fn every_lane_mover_radius_keeps_clear_of_a_tower() {
        let obstacle = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);
        let start = Vec3::new(0.0, 0.0, -12.0);
        let goal = Vec3::new(0.0, 0.0, 12.0);

        for agent_radius in [0.45, 0.55, 0.9] {
            let mesh = LaneNavigationMesh::new(agent_radius, &[obstacle]);
            let path = mesh
                .find_path(start, goal)
                .expect("a route around the tower");

            assert!(
                obstacle_clearance(&path, start, obstacle)
                    >= obstacle.radius + agent_radius - 0.001
            );
        }
    }

    #[test]
    fn goals_inside_an_obstacle_are_projected_to_the_reachable_edge() {
        let obstacle = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);
        let mesh = LaneNavigationMesh::new(0.9, &[obstacle]);
        let path = mesh
            .find_path(Vec3::new(0.0, 0.0, -10.0), Vec3::ZERO)
            .expect("a projected tower-edge goal");
        let destination = *path.last().expect("a destination waypoint");

        assert!(destination.distance(Vec3::ZERO) >= 1.25 + 0.9 - 0.001);
    }

    #[test]
    fn projected_starts_recover_outside_a_new_obstacle() {
        let obstacle = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);
        let mesh = LaneNavigationMesh::new(0.9, &[obstacle]);
        let path = mesh
            .find_path_with_projection(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0))
            .expect("a route with a projected start");

        assert!(path.start.distance(Vec3::ZERO) >= 1.25 + 0.9 - 0.001);
    }

    #[test]
    fn paths_include_a_recovery_waypoint_from_the_physical_tower_edge() {
        let obstacle = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);
        let mesh = LaneNavigationMesh::new(0.9, &[obstacle]);
        let physical_edge = Vec3::new(0.0, 0.0, -(obstacle.radius + 0.9));
        let projected = mesh
            .find_path_with_projection(physical_edge, Vec3::new(0.0, 0.0, 10.0))
            .expect("a recovery path");
        let path = mesh
            .find_path(physical_edge, Vec3::new(0.0, 0.0, 10.0))
            .expect("a route containing its recovery step");

        assert!(projected.start.distance(physical_edge) > 0.0);
        assert_eq!(path.first().copied(), Some(projected.start));
    }

    #[test]
    fn projected_start_already_in_a_route_remains_a_recovery_waypoint() {
        let requested_start = Vec3::new(0.0, 0.0, -2.15);
        let projected_start = Vec3::new(0.24, 0.0, -2.23);
        let destination = Vec3::new(1.0, 0.0, 4.0);
        let mut path = LaneNavigationPath {
            start: projected_start,
            waypoints: vec![projected_start, destination],
        };

        assert!(path.prepend_start_recovery_waypoint(requested_start));
        assert_eq!(path.waypoints, vec![projected_start, destination]);
    }

    #[test]
    fn tower_edge_routes_remain_reachable_for_every_minion_radius() {
        let obstacle = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);

        for agent_radius in [0.45, 0.55, 0.9] {
            let mesh = LaneNavigationMesh::new(agent_radius, &[obstacle]);
            let physical_clearance = obstacle.radius + agent_radius + 0.15;
            for side in [-1.0, 1.0] {
                let start = Vec3::new(0.0, 0.0, -side * 6.0);
                let goal = Vec3::new(side * physical_clearance, 0.0, 0.0);

                let path = mesh
                    .find_path(start, goal)
                    .expect("a combat-ring route around a tower");

                assert!(
                    path.last().is_some_and(|destination| {
                        destination.distance(goal) <= NAVIGATION_SEARCH_DELTA
                    }),
                    "route did not retain the reachable combat-ring goal for {agent_radius}"
                );
            }
        }
    }

    #[test]
    fn obstacle_revisions_ignore_input_order() {
        let left = LaneNavigationObstacle::new(Vec3::new(-1.0, 0.0, 2.0), 1.0);
        let right = LaneNavigationObstacle::new(Vec3::new(1.0, 0.0, -2.0), 1.0);

        assert_eq!(
            lane_navigation_obstacle_revision(&[left, right]),
            lane_navigation_obstacle_revision(&[right, left])
        );
    }

    #[test]
    fn overlapping_obstacles_are_merged_before_baking() {
        let left = LaneNavigationObstacle::new(Vec3::new(-0.6, 0.0, 0.0), 1.0);
        let right = LaneNavigationObstacle::new(Vec3::new(0.6, 0.0, 0.0), 1.0);

        let obstacles = normalized_obstacles(&[left, right]);

        assert_eq!(obstacles.len(), 1);
        assert!(obstacles[0].radius >= 1.6);
    }
}
