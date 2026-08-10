use super::{
    ExternalMovementModifier, HOLD_CURSOR_MIN_DISTANCE, HoldMoveDirection, MoveTargetMarker,
    MoveTargetMarkerFx,
    auto_attack::{AutoAttackInputState, AutoAttackTarget},
    horizontal_distance,
    lane::RemoteLaneUnit,
    targeting::{clamp_world_point_to_map_top, ray_hit_map_top},
};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_shared::game::{
    camera::TopDownCamera,
    lane::LANE_PLAYER_COLLISION_RADIUS,
    lane_navigation::{
        LaneNavigationMesh, LaneNavigationObstacle, lane_navigation_obstacle_revision,
        resolve_circle_obstacle_collisions,
    },
    map::MapGround,
    player::{Health, MoveSpeed, MoveTarget, PlayerControlled},
};
use game_shared::network::{PlayerCommand, ReliableCommandChannel, WorldPosition};
use lightyear::prelude::*;
use std::collections::VecDeque;

const MOVE_TARGET_UPDATE_EPSILON: f32 = 0.08;
const MOVE_TARGET_REACHED_DISTANCE: f32 = 0.04;
const NAVIGATION_WAYPOINT_STOP_DISTANCE: f32 = 0.25;
const NAVIGATION_RECOVERY_WAYPOINT_STOP_DISTANCE: f32 = 0.001;
const NAVIGATION_SEGMENT_BLOCKED_EPSILON: f32 = 0.001;
const ATTACK_ROUTE_INNER_RANGE_BUFFER: f32 = NAVIGATION_WAYPOINT_STOP_DISTANCE + 0.08;

/// Stores a locally predicted route toward one server-authoritative movement request.
///
/// `MoveTarget` remains the immediate waypoint so the established animation and movement
/// systems continue to describe the currently active leg of this route.
#[derive(Component, Debug, Clone)]
pub(super) struct LocalNavigationRoute {
    requested_goal: Vec3,
    reachable_goal: Vec3,
    waypoints: VecDeque<Vec3>,
    recovery_waypoint: Option<Vec3>,
    obstacle_revision: u64,
    attack_target: Option<Entity>,
    attack_target_position: Option<Vec3>,
}

impl LocalNavigationRoute {
    /// Returns the next waypoint that should be assigned to the controlled player.
    fn next_waypoint(&self) -> Option<Vec3> {
        self.waypoints.front().copied()
    }

    /// Returns the arrival radius for the currently active navigation waypoint.
    fn next_waypoint_stop_distance(&self) -> f32 {
        if self.recovery_waypoint.is_some_and(|recovery_waypoint| {
            self.waypoints.front().is_some_and(|waypoint| {
                waypoint.distance_squared(recovery_waypoint)
                    <= NAVIGATION_SEGMENT_BLOCKED_EPSILON * NAVIGATION_SEGMENT_BLOCKED_EPSILON
            })
        }) {
            NAVIGATION_RECOVERY_WAYPOINT_STOP_DISTANCE
        } else {
            NAVIGATION_WAYPOINT_STOP_DISTANCE
        }
    }

    /// Drops waypoints already reached by the controlled player.
    fn discard_reached_waypoints(&mut self, position: Vec3) {
        while let Some(waypoint) = self.waypoints.front().copied() {
            if position.distance(waypoint) > self.next_waypoint_stop_distance() {
                break;
            }
            self.waypoints.pop_front();
            if self.recovery_waypoint.is_some_and(|recovery_waypoint| {
                recovery_waypoint.distance_squared(waypoint)
                    <= NAVIGATION_SEGMENT_BLOCKED_EPSILON * NAVIGATION_SEGMENT_BLOCKED_EPSILON
            }) {
                self.recovery_waypoint = None;
            }
        }
    }
}
/// Converts held right-click input into controlled-player movement targets.
///
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
    attack_input: Res<AutoAttackInputState>,
    mut attack_target: ResMut<AutoAttackTarget>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    map_query: Query<(&GlobalTransform, &MapGround)>,
    mut hold_direction: ResMut<HoldMoveDirection>,
    player_query: Query<
        (
            Entity,
            &Health,
            &Transform,
            Option<&LocalNavigationRoute>,
            Option<&ExternalMovementModifier>,
        ),
        (With<PlayerControlled>, Without<MoveTargetMarker>),
    >,
    structure_query: Query<(&RemoteLaneUnit, &Health), Without<PlayerControlled>>,
    mut marker_query: Query<
        (&mut Transform, &mut Visibility, &mut MoveTargetMarkerFx),
        (With<MoveTargetMarker>, Without<PlayerControlled>),
    >,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut commands: Commands,
) {
    let right_hold = mouse_buttons.pressed(MouseButton::Right);
    let right_pressed = mouse_buttons.just_pressed(MouseButton::Right);
    if !right_hold {
        return;
    }
    if right_pressed && attack_input.consumed_right_press {
        return;
    }
    if right_pressed {
        attack_target.target = None;
    }
    if should_preserve_auto_attack_order(right_pressed, attack_target.target.is_some()) {
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
    let structure_obstacles = live_structure_navigation_obstacles(&structure_query);
    let obstacle_revision = lane_navigation_obstacle_revision(&structure_obstacles);

    let mut marker_target = target;
    let mut did_set_move_target = false;

    for (entity, health, player_transform, current_route, movement_modifier) in &player_query {
        if health.current == 0 || movement_modifier.is_some_and(|modifier| modifier.stunned) {
            commands.entity(entity).remove::<MoveTarget>();
            commands.entity(entity).remove::<LocalNavigationRoute>();
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

        let requested_move_target = if right_hold && distance_to_cursor < HOLD_CURSOR_MIN_DISTANCE {
            let pushed = player_transform.translation + hold_direction.0 * HOLD_CURSOR_MIN_DISTANCE;
            clamp_world_point_to_map_top(pushed, map_transform, *map_ground)
        } else {
            target
        };
        let requested_goal_changed = current_route.is_none_or(|route| {
            route.attack_target.is_some()
                || route.requested_goal.distance_squared(requested_move_target)
                    > MOVE_TARGET_UPDATE_EPSILON * MOVE_TARGET_UPDATE_EPSILON
        });
        let route_is_current = current_route.is_some_and(|route| {
            !requested_goal_changed && route.obstacle_revision == obstacle_revision
        });

        if route_is_current {
            marker_target = current_route
                .map(|route| route.reachable_goal)
                .unwrap_or(requested_move_target);
            did_set_move_target = true;
            continue;
        }

        if requested_goal_changed {
            send_move_to_command(&mut command_senders, requested_move_target);
        }

        let Some(route) = plan_local_navigation_route(
            player_transform.translation,
            requested_move_target,
            obstacle_revision,
            &structure_obstacles,
        ) else {
            commands.entity(entity).remove::<MoveTarget>();
            commands.entity(entity).remove::<LocalNavigationRoute>();
            continue;
        };

        let next_waypoint = route.next_waypoint();
        let waypoint_stop_distance = route.next_waypoint_stop_distance();
        marker_target = route.reachable_goal;
        did_set_move_target = true;
        commands.entity(entity).insert(route);
        if let Some(next_waypoint) = next_waypoint {
            commands.entity(entity).insert(navigation_waypoint_target(
                next_waypoint,
                waypoint_stop_distance,
            ));
        } else {
            commands.entity(entity).remove::<MoveTarget>();
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
/// Advances local route waypoints and replans them when replicated structure obstacles change.
///
/// - `commands`: ECS command buffer used to update the immediate movement waypoint.
/// - `player_query`: Controlled players with an active local navigation route.
/// - `structure_query`: Living replicated structures used as pathfinding obstacles.
pub(super) fn advance_local_navigation_routes(
    mut commands: Commands,
    mut player_query: Query<
        (
            Entity,
            &Health,
            &Transform,
            Option<&ExternalMovementModifier>,
            &mut LocalNavigationRoute,
        ),
        With<PlayerControlled>,
    >,
    structure_query: Query<(&RemoteLaneUnit, &Health), Without<PlayerControlled>>,
) {
    let structure_obstacles = live_structure_navigation_obstacles(&structure_query);
    let obstacle_revision = lane_navigation_obstacle_revision(&structure_obstacles);

    for (entity, health, transform, movement_modifier, mut route) in &mut player_query {
        if health.current == 0 || movement_modifier.is_some_and(|modifier| modifier.stunned) {
            commands.entity(entity).remove::<MoveTarget>();
            commands.entity(entity).remove::<LocalNavigationRoute>();
            continue;
        }

        if route.obstacle_revision != obstacle_revision
            && !replan_local_navigation_route(
                &mut route,
                transform.translation,
                obstacle_revision,
                &structure_obstacles,
            )
        {
            commands.entity(entity).remove::<MoveTarget>();
            commands.entity(entity).remove::<LocalNavigationRoute>();
            continue;
        }

        route.discard_reached_waypoints(transform.translation);
        let next_waypoint = route.next_waypoint();
        if next_waypoint.is_some_and(|waypoint| {
            !route_segment_is_clear(transform.translation, waypoint, &structure_obstacles)
        }) && !replan_local_navigation_route(
            &mut route,
            transform.translation,
            obstacle_revision,
            &structure_obstacles,
        ) {
            commands.entity(entity).remove::<MoveTarget>();
            commands.entity(entity).remove::<LocalNavigationRoute>();
            continue;
        }

        route.discard_reached_waypoints(transform.translation);
        let Some(next_waypoint) = route.next_waypoint() else {
            commands.entity(entity).remove::<MoveTarget>();
            commands.entity(entity).remove::<LocalNavigationRoute>();
            continue;
        };
        let waypoint_stop_distance = route.next_waypoint_stop_distance();
        commands.entity(entity).insert(navigation_waypoint_target(
            next_waypoint,
            waypoint_stop_distance,
        ));
    }
}
/// Moves controlled players toward their current movement target and removes reached targets.
///
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
    structure_query: Query<(&RemoteLaneUnit, &Health), Without<PlayerControlled>>,
) {
    let structure_obstacles = live_structure_navigation_obstacles(&structure_query);

    for (entity, health, move_speed, move_target, movement_modifier, mut transform) in
        &mut player_query
    {
        if health.current == 0 || movement_modifier.is_some_and(|modifier| modifier.stunned) {
            commands.entity(entity).remove::<MoveTarget>();
            continue;
        }

        let start_position = transform.translation;

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
        transform.translation = resolve_circle_obstacle_collisions(
            start_position,
            transform.translation,
            LANE_PLAYER_COLLISION_RADIUS,
            &structure_obstacles,
        );
        transform.translation.y = 0.0;
    }
}

/// Builds a local route from the player's position to a server-commanded target.
fn plan_local_navigation_route(
    start: Vec3,
    requested_goal: Vec3,
    obstacle_revision: u64,
    structure_obstacles: &[LaneNavigationObstacle],
) -> Option<LocalNavigationRoute> {
    let mesh = LaneNavigationMesh::new(LANE_PLAYER_COLLISION_RADIUS, structure_obstacles);
    let mut path = mesh.find_path_with_projection(start, requested_goal)?;
    let has_recovery_waypoint = path.prepend_start_recovery_waypoint(start);
    let recovery_waypoint = has_recovery_waypoint.then_some(path.start);
    let waypoints = VecDeque::from(path.waypoints);
    let reachable_goal = waypoints.back().copied()?;

    Some(LocalNavigationRoute {
        requested_goal,
        reachable_goal,
        waypoints,
        recovery_waypoint,
        obstacle_revision,
        attack_target: None,
        attack_target_position: None,
    })
}

/// Rebuilds a route from a corrected player position while retaining its requested goal.
fn replan_local_navigation_route(
    route: &mut LocalNavigationRoute,
    start: Vec3,
    obstacle_revision: u64,
    structure_obstacles: &[LaneNavigationObstacle],
) -> bool {
    let Some(replanned_route) = plan_local_navigation_route(
        start,
        route.requested_goal,
        obstacle_revision,
        structure_obstacles,
    ) else {
        return false;
    };

    let attack_target = route.attack_target;
    let attack_target_position = route.attack_target_position;
    *route = replanned_route;
    route.attack_target = attack_target;
    route.attack_target_position = attack_target_position;
    true
}

/// Updates the locally predicted structure-safe route for one ordered basic attack.
pub(super) fn update_local_attack_navigation(
    commands: &mut Commands,
    player_entity: Entity,
    player_position: Vec3,
    target_entity: Entity,
    target_position: Vec3,
    attack_range: f32,
    current_route: Option<&LocalNavigationRoute>,
    structure_obstacles: &[LaneNavigationObstacle],
) {
    let obstacle_revision = lane_navigation_obstacle_revision(structure_obstacles);
    let matching_route = current_route.filter(|route| route.attack_target == Some(target_entity));
    let target_moved = matching_route.is_some_and(|route| {
        route
            .attack_target_position
            .is_none_or(|previous_position| {
                horizontal_distance(previous_position, target_position) > MOVE_TARGET_UPDATE_EPSILON
            })
    });
    let route_is_current = matching_route
        .is_some_and(|route| !target_moved && route.obstacle_revision == obstacle_revision);
    if route_is_current {
        return;
    }

    let requested_goal = matching_route
        .map(|route| {
            let previous_target_position = route.attack_target_position.unwrap_or(target_position);
            route.requested_goal
                + Vec3::new(
                    target_position.x - previous_target_position.x,
                    0.0,
                    target_position.z - previous_target_position.z,
                )
        })
        .unwrap_or_else(|| attack_approach_goal(player_position, target_position, attack_range));
    let Some(mut route) = plan_local_navigation_route(
        player_position,
        requested_goal,
        obstacle_revision,
        structure_obstacles,
    ) else {
        commands.entity(player_entity).remove::<MoveTarget>();
        commands
            .entity(player_entity)
            .remove::<LocalNavigationRoute>();
        return;
    };

    route.attack_target = Some(target_entity);
    route.attack_target_position = Some(target_position);
    let next_waypoint = route.next_waypoint();
    let waypoint_stop_distance = route.next_waypoint_stop_distance();
    commands.entity(player_entity).insert(route);
    if let Some(next_waypoint) = next_waypoint {
        commands
            .entity(player_entity)
            .insert(navigation_waypoint_target(
                next_waypoint,
                waypoint_stop_distance,
            ));
    } else {
        commands.entity(player_entity).remove::<MoveTarget>();
    }
}

/// Returns a destination just inside an ordered basic attack's legal range.
fn attack_approach_goal(player_position: Vec3, target_position: Vec3, attack_range: f32) -> Vec3 {
    let offset = Vec3::new(
        player_position.x - target_position.x,
        0.0,
        player_position.z - target_position.z,
    );
    let direction = if offset.length_squared() <= f32::EPSILON {
        Vec3::Z
    } else {
        offset.normalize()
    };
    target_position + direction * (attack_range - ATTACK_ROUTE_INNER_RANGE_BUFFER).max(0.0)
}

/// Builds the immediate `MoveTarget` for one local route waypoint.
fn navigation_waypoint_target(position: Vec3, stop_distance: f32) -> MoveTarget {
    MoveTarget {
        position,
        stop_distance,
    }
}

/// Keeps a held attack order from being replaced with a ground movement command.
fn should_preserve_auto_attack_order(right_pressed: bool, has_attack_target: bool) -> bool {
    !right_pressed && has_attack_target
}

/// Checks whether a route leg remains traversable after a local reconciliation.
fn route_segment_is_clear(start: Vec3, end: Vec3, obstacles: &[LaneNavigationObstacle]) -> bool {
    resolve_circle_obstacle_collisions(start, end, LANE_PLAYER_COLLISION_RADIUS, obstacles)
        .distance_squared(end)
        <= NAVIGATION_SEGMENT_BLOCKED_EPSILON * NAVIGATION_SEGMENT_BLOCKED_EPSILON
}

/// Sends one reliable server-authoritative movement request to every active client link.
fn send_move_to_command(
    command_senders: &mut Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    requested_goal: Vec3,
) {
    for mut sender in command_senders.iter_mut() {
        sender.send::<ReliableCommandChannel>(PlayerCommand::MoveTo(WorldPosition::from(
            requested_goal,
        )));
    }
}

/// Returns navigation circles for living structures replicated by the server.
pub(super) fn live_structure_navigation_obstacles(
    structure_query: &Query<(&RemoteLaneUnit, &Health), Without<PlayerControlled>>,
) -> Vec<LaneNavigationObstacle> {
    structure_query
        .iter()
        .filter_map(|(structure, health)| structure_navigation_obstacle(structure, health))
        .collect()
}

/// Converts one living replicated structure into a local navigation obstacle.
fn structure_navigation_obstacle(
    structure: &RemoteLaneUnit,
    health: &Health,
) -> Option<LaneNavigationObstacle> {
    (structure.is_structure() && health.current > 0).then(|| {
        LaneNavigationObstacle::new(structure.collision_center(), structure.collision_radius())
    })
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

/// Animates and fades the movement target marker after a right-click command.
///
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
    use game_shared::game::lane::{LaneUnitKind, lane_unit_stats};

    #[test]
    fn held_right_click_preserves_an_active_auto_attack_order() {
        assert!(should_preserve_auto_attack_order(false, true));
        assert!(!should_preserve_auto_attack_order(true, true));
        assert!(!should_preserve_auto_attack_order(false, false));
    }

    #[test]
    fn local_routes_keep_each_waypoint_leg_clear_of_a_live_tower() {
        let tower = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);
        let obstacles = vec![tower];
        let route = plan_local_navigation_route(
            Vec3::new(0.0, 0.0, -12.0),
            Vec3::new(0.0, 0.0, 12.0),
            lane_navigation_obstacle_revision(&obstacles),
            &obstacles,
        )
        .expect("a route around the tower");
        assert!(route.waypoints.len() > 1);
        assert_eq!(route.reachable_goal, Vec3::new(0.0, 0.0, 12.0));

        let mut previous = Vec3::new(0.0, 0.0, -12.0);
        for waypoint in &route.waypoints {
            assert!(route_segment_is_clear(previous, *waypoint, &obstacles));
            previous = *waypoint;
        }
    }

    #[test]
    fn local_routes_avoid_nexus_navigation_obstacles() {
        let nexus = LaneNavigationObstacle::new(
            Vec3::ZERO,
            lane_unit_stats(LaneUnitKind::Nexus).hit_radius,
        );
        let obstacles = vec![nexus];
        let start = Vec3::new(0.0, 0.0, -12.0);
        let route = plan_local_navigation_route(
            start,
            Vec3::new(0.0, 0.0, 12.0),
            lane_navigation_obstacle_revision(&obstacles),
            &obstacles,
        )
        .expect("a route around the Nexus");

        let mut previous = start;
        for waypoint in &route.waypoints {
            assert!(route_segment_is_clear(previous, *waypoint, &obstacles));
            previous = *waypoint;
        }
    }

    #[test]
    fn local_routes_keep_a_projected_start_as_a_non_skippable_recovery_leg() {
        let tower = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);
        let obstacles = vec![tower];
        let physical_tower_edge =
            Vec3::new(0.0, 0.0, -(tower.radius + LANE_PLAYER_COLLISION_RADIUS));
        let mut route = plan_local_navigation_route(
            physical_tower_edge,
            Vec3::new(0.0, 0.0, 10.0),
            lane_navigation_obstacle_revision(&obstacles),
            &obstacles,
        )
        .expect("a recovery route around the tower");
        let recovery_waypoint = route.next_waypoint().expect("a recovery waypoint");

        assert_eq!(route.recovery_waypoint, Some(recovery_waypoint));
        assert!(route_segment_is_clear(
            physical_tower_edge,
            recovery_waypoint,
            &obstacles,
        ));

        route.discard_reached_waypoints(physical_tower_edge);
        assert_eq!(route.next_waypoint(), Some(recovery_waypoint));
        assert_eq!(
            route.next_waypoint_stop_distance(),
            NAVIGATION_RECOVERY_WAYPOINT_STOP_DISTANCE
        );
    }

    #[test]
    fn local_attack_routes_stop_inside_range_after_waypoint_tolerance() {
        let tower = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);
        let obstacles = vec![tower];
        let start = Vec3::new(0.0, 0.0, -12.0);
        let target = Vec3::new(0.0, 0.0, 12.0);
        let attack_range = 5.5;
        let goal = attack_approach_goal(start, target, attack_range);
        let route = plan_local_navigation_route(
            start,
            goal,
            lane_navigation_obstacle_revision(&obstacles),
            &obstacles,
        )
        .expect("a tower-safe attack route");
        assert!(route.waypoints.len() > 1);
        let mut previous = start;
        for waypoint in &route.waypoints {
            assert!(route_segment_is_clear(previous, *waypoint, &obstacles));
            previous = *waypoint;
        }
        assert!(
            horizontal_distance(route.reachable_goal, target) + NAVIGATION_WAYPOINT_STOP_DISTANCE
                <= attack_range
        );
    }

    #[test]
    fn replanning_keeps_local_attack_order_metadata() {
        let start = Vec3::new(0.0, 0.0, -12.0);
        let goal = Vec3::new(0.0, 0.0, 12.0);
        let mut route =
            plan_local_navigation_route(start, goal, lane_navigation_obstacle_revision(&[]), &[])
                .expect("a direct route");
        let target = Entity::PLACEHOLDER;
        let target_position = Vec3::new(0.0, 0.0, 14.0);
        route.attack_target = Some(target);
        route.attack_target_position = Some(target_position);
        let tower = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);
        let obstacles = vec![tower];

        assert!(replan_local_navigation_route(
            &mut route,
            start,
            lane_navigation_obstacle_revision(&obstacles),
            &obstacles,
        ));
        assert_eq!(route.attack_target, Some(target));
        assert_eq!(route.attack_target_position, Some(target_position));
    }

    #[test]
    fn replanning_uses_the_latest_tower_obstacle_revision() {
        let start = Vec3::new(0.0, 0.0, -12.0);
        let goal = Vec3::new(0.0, 0.0, 12.0);
        let mut route =
            plan_local_navigation_route(start, goal, lane_navigation_obstacle_revision(&[]), &[])
                .expect("a direct route");
        let tower = LaneNavigationObstacle::new(Vec3::ZERO, 1.25);
        let obstacles = vec![tower];
        let revision = lane_navigation_obstacle_revision(&obstacles);

        assert!(replan_local_navigation_route(
            &mut route, start, revision, &obstacles
        ));
        assert_eq!(route.obstacle_revision, revision);
        assert!(route.waypoints.len() > 1);
    }
}
