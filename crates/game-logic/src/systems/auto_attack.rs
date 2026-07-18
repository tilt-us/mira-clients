use super::{
    CurrentChampionVisual, ExternalMovementModifier, TrainingDummy, TrainingDummyHealthChangeKind,
    targeting::{clamp_world_point_to_map_top, ray_hit_map_top},
};
use bevy::math::primitives::Sphere;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_shared::game::{
    auto_attack::{AUTO_ATTACK_COMBO_RESET_SECONDS, auto_attack_combo},
    camera::TopDownCamera,
    map::MapGround,
    player::{Health, MoveTarget, Player, PlayerControlled},
};
use game_shared::network::{
    AutoAttackVisualEvent, ChampionId, NetworkTargetId, PlayerCommand, ReliableCommandChannel,
};
use lightyear::prelude::*;

const AUTO_ATTACK_RANGE: f32 = 5.0;
const AUTO_ATTACK_PROJECTILE_RADIUS: f32 = 0.12;
const AUTO_ATTACK_PROJECTILE_HEIGHT: f32 = 0.8;
const AUTO_ATTACK_MIN_TRAVEL_SECONDS: f32 = 0.075;
const AUTO_ATTACK_MAX_TRAVEL_SECONDS: f32 = 0.45;

/// Description:
/// Stores local auto-attack cooldown state.
#[derive(Resource, Debug, Clone)]
pub(super) struct AutoAttackState {
    cooldown: Timer,
    combo_stage: usize,
    combo_target: Option<Entity>,
    combo_reset_seconds: f32,
}

/// Description:
/// Tracks whether the current right-click press was consumed by attack input.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub(super) struct AutoAttackInputState {
    pub(super) consumed_right_press: bool,
}

/// Description:
/// Stores the currently ordered auto-attack target.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub(super) struct AutoAttackTarget {
    pub(super) target: Option<Entity>,
}

/// Description:
/// Stores one local auto-attack projectile travelling toward a clicked enemy target.
#[derive(Component, Debug, Clone)]
pub(super) struct AutoAttackProjectile {
    target: Entity,
    start: Vec3,
    end: Vec3,
    timer: Timer,
    damage: f32,
    apply_local_damage: bool,
}

impl Default for AutoAttackState {
    /// Returns the default configuration used by the client auto-attack system.
    fn default() -> Self {
        let mut cooldown = Timer::from_seconds(
            auto_attack_combo(ChampionId(super::LOCAL_CHAMPION_ID)).cooldown_seconds(),
            TimerMode::Once,
        );
        cooldown.set_elapsed(cooldown.duration());
        Self {
            cooldown,
            combo_stage: 0,
            combo_target: None,
            combo_reset_seconds: 0.0,
        }
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
            &CurrentChampionVisual,
            Option<&ExternalMovementModifier>,
        ),
        With<PlayerControlled>,
    >,
    target_query: Query<(
        Entity,
        &TrainingDummy,
        &Transform,
        Option<&Player>,
        Option<&NetworkTargetId>,
    )>,
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

    let Ok((player_entity, health, player_transform, _, movement_modifier)) = player_query.single()
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
            &CurrentChampionVisual,
            Option<&ExternalMovementModifier>,
        ),
        With<PlayerControlled>,
    >,
    target_query: Query<(
        Entity,
        &TrainingDummy,
        &Transform,
        Option<&Player>,
        Option<&NetworkTargetId>,
    )>,
    mut command_senders: Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    attack_state.cooldown.tick(time.delta());
    tick_combo_reset(&mut attack_state, time.delta_secs());

    let Some(target_entity) = attack_target.target else {
        return;
    };

    let Ok((_, target, target_transform, target_player, target_network_id)) =
        target_query.get(target_entity)
    else {
        attack_target.target = None;
        return;
    };
    if target.health <= 0.0 {
        attack_target.target = None;
        return;
    }

    let Ok((player_entity, health, player_transform, visual, movement_modifier)) =
        player_query.single()
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

    let champion = visual
        .champion
        .unwrap_or(ChampionId(super::LOCAL_CHAMPION_ID));
    let combo = auto_attack_combo(champion);
    let target_id = target_player
        .map(|player| NetworkTargetId::Player(player.id.0))
        .or(target_network_id.copied());
    let apply_local_damage = target_id.is_none();
    let combo_stage = next_combo_stage(
        &mut attack_state,
        target_entity,
        combo.combo_length,
        combo.cooldown_seconds(),
    );
    let damage = if apply_local_damage {
        combo.damage_for_stage(combo_stage)
    } else {
        0.0
    };

    let start = player_transform.translation + Vec3::Y * AUTO_ATTACK_PROJECTILE_HEIGHT;
    let end = target_transform.translation + Vec3::Y * AUTO_ATTACK_PROJECTILE_HEIGHT;
    let travel_seconds = auto_attack_travel_seconds(attack_distance);

    spawn_auto_attack_projectile(
        &mut commands,
        &mut meshes,
        &mut materials,
        target_entity,
        start,
        end,
        travel_seconds,
        damage,
        apply_local_damage,
    );
    if let Some(target_id) = target_id {
        send_auto_attack_command(&mut command_senders, target_id);
    }
    attack_state
        .cooldown
        .set_duration(std::time::Duration::from_secs_f32(combo.cooldown_seconds()));
    attack_state.cooldown.reset();
}

/// Description:
/// Receives server-accepted remote auto attacks and renders their projectile for this client.
pub(super) fn receive_remote_auto_attack_visuals(
    mut commands: Commands,
    mut receivers: Query<&mut MessageReceiver<AutoAttackVisualEvent>, With<Client>>,
    target_query: Query<(Entity, Option<&Player>, Option<&NetworkTargetId>)>,
    local_player_query: Query<&Player, With<PlayerControlled>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let local_player_id = local_player_query.single().ok().map(|player| player.id.0);

    for mut receiver in &mut receivers {
        for event in receiver.receive() {
            if local_player_id == Some(event.caster_player_id) {
                continue;
            }

            let Some((target_entity, _, _)) = target_query.iter().find(|(_, player, target_id)| {
                matches!(
                    event.target,
                    NetworkTargetId::Player(player_id)
                        if player.is_some_and(|player| player.id.0 == player_id)
                ) || matches!(
                    event.target,
                    NetworkTargetId::LaneUnit(lane_unit_id)
                        if target_id.is_some_and(|target_id| *target_id == NetworkTargetId::LaneUnit(lane_unit_id))
                )
            })
            else {
                continue;
            };

            spawn_auto_attack_projectile(
                &mut commands,
                &mut meshes,
                &mut materials,
                target_entity,
                event.start.into(),
                event.end.into(),
                event.travel_seconds.max(AUTO_ATTACK_MIN_TRAVEL_SECONDS),
                0.0,
                false,
            );
        }
    }
}

/// Description:
/// Moves active auto-attack projectiles and applies damage when they reach their target.
pub(super) fn update_auto_attack_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    target_transform_query: Query<&Transform, Without<AutoAttackProjectile>>,
    mut dummy_query: Query<&mut TrainingDummy, Without<AutoAttackProjectile>>,
    mut projectile_query: Query<
        (Entity, &mut AutoAttackProjectile, &mut Transform),
        Without<TrainingDummy>,
    >,
) {
    for (projectile_entity, mut projectile, mut transform) in &mut projectile_query {
        let Ok(target_transform) = target_transform_query.get(projectile.target) else {
            commands.entity(projectile_entity).despawn();
            continue;
        };

        if projectile.apply_local_damage
            && !dummy_query
                .get(projectile.target)
                .is_ok_and(|target| target.health > 0.0)
        {
            commands.entity(projectile_entity).despawn();
            continue;
        }

        projectile.end = target_transform.translation + Vec3::Y * AUTO_ATTACK_PROJECTILE_HEIGHT;
        projectile.timer.tick(time.delta());

        let duration = projectile.timer.duration().as_secs_f32();
        let progress = (projectile.timer.elapsed_secs() / duration).clamp(0.0, 1.0);
        transform.translation = projectile.start.lerp(projectile.end, progress);

        if projectile.timer.is_finished() {
            if !projectile.apply_local_damage {
                commands.entity(projectile_entity).despawn();
                continue;
            }

            if let Ok(mut target) = dummy_query.get_mut(projectile.target) {
                target.apply_damage(projectile.damage, TrainingDummyHealthChangeKind::AutoAttack);
                info!(
                    "TrainingDummy hit by auto attack: -{:.1} HP (remaining {:.1})",
                    projectile.damage, target.health
                );
            }
            commands.entity(projectile_entity).despawn();
        }
    }
}

/// Runs the cursor world position step for the client auto-attack system.
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

/// Runs the clicked enemy target step for the client auto-attack system.
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

/// Runs the horizontal distance step for the client auto-attack system.
fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// Runs the update attack movement step for the client auto-attack system.
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

/// Runs the next combo stage step for the client auto-attack system.
fn next_combo_stage(
    attack_state: &mut AutoAttackState,
    target: Entity,
    combo_length: usize,
    cooldown_seconds: f32,
) -> usize {
    let combo_length = combo_length.max(1);
    let stage = if attack_state.combo_target == Some(target) {
        attack_state.combo_stage.min(combo_length - 1)
    } else {
        0
    };

    attack_state.combo_stage = (stage + 1) % combo_length;
    attack_state.combo_target = Some(target);
    attack_state.combo_reset_seconds = AUTO_ATTACK_COMBO_RESET_SECONDS + cooldown_seconds;
    stage
}

/// Runs the tick combo reset step for the client auto-attack system.
fn tick_combo_reset(attack_state: &mut AutoAttackState, delta_seconds: f32) {
    if attack_state.combo_reset_seconds <= 0.0 {
        return;
    }

    attack_state.combo_reset_seconds = (attack_state.combo_reset_seconds - delta_seconds).max(0.0);
    if attack_state.combo_reset_seconds <= 0.0 {
        attack_state.combo_stage = 0;
        attack_state.combo_target = None;
    }
}

/// Runs the auto attack travel seconds step for the client auto-attack system.
fn auto_attack_travel_seconds(distance: f32) -> f32 {
    let range_ratio = (distance / AUTO_ATTACK_RANGE).clamp(0.0, 1.0);
    (range_ratio * AUTO_ATTACK_MAX_TRAVEL_SECONDS).clamp(
        AUTO_ATTACK_MIN_TRAVEL_SECONDS,
        AUTO_ATTACK_MAX_TRAVEL_SECONDS,
    )
}

/// Runs the spawn auto attack projectile step for the client auto-attack system.
fn spawn_auto_attack_projectile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target: Entity,
    start: Vec3,
    end: Vec3,
    travel_seconds: f32,
    damage: f32,
    apply_local_damage: bool,
) {
    commands.spawn((
        Name::new("AutoAttackProjectile"),
        AutoAttackProjectile {
            target,
            start,
            end,
            timer: Timer::from_seconds(travel_seconds, TimerMode::Once),
            damage,
            apply_local_damage,
        },
        Mesh3d(meshes.add(Sphere::new(AUTO_ATTACK_PROJECTILE_RADIUS))),
        MeshMaterial3d(materials.add(auto_attack_projectile_material())),
        Transform::from_translation(start),
    ));
}

/// Runs the auto attack projectile material step for the client auto-attack system.
fn auto_attack_projectile_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.95),
        emissive: Color::srgba(1.0, 1.0, 1.0, 0.65).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}

/// Runs the send auto attack command step for the client auto-attack system.
fn send_auto_attack_command(
    senders: &mut Query<&mut MessageSender<PlayerCommand>, With<Client>>,
    target: NetworkTargetId,
) {
    for mut sender in senders {
        sender.send::<ReliableCommandChannel>(PlayerCommand::AutoAttack { target });
    }
}
