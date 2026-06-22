use super::TrainingDummy;
use bevy::math::primitives::Cuboid;
use bevy::prelude::*;
use game_shared::game::{camera::TopDownCamera, player::Health};

const BAR_WIDTH: f32 = 1.75;
const BAR_HEIGHT: f32 = 0.24;
const BAR_FILL_HEIGHT: f32 = 0.13;
const BAR_DEPTH: f32 = 0.035;
const LIRA_BAR_OFFSET: f32 = 2.65;

#[derive(Component, Debug, Clone, Copy)]
/// Description:
/// Stores the world-space tracking data for an overhead health bar.
///
/// Fields:
/// - `target`: Entity that the health bar follows.
/// - `y_offset`: Vertical world-space offset above the target.
pub(in crate::systems) struct HealthBar {
    target: Entity,
    y_offset: f32,
}

#[derive(Component, Debug, Clone, Copy)]
/// Description:
/// Stores fill-scaling data for a health bar foreground segment.
///
/// Fields:
/// - `target`: Entity whose health controls the fill width.
/// - `max_health`: Fallback maximum health value for sources without a max-health field.
/// - `source`: Health source type used to read the correct component.
pub(in crate::systems) struct HealthBarFill {
    target: Entity,
    max_health: f32,
    source: HealthBarSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Description:
/// Identifies which component type provides health for a health bar fill.
///
/// Fields:
/// - `Player`: Reads health from the shared `Health` component.
/// - `TrainingDummy`: Reads health from an enemy training dummy or remote player stand-in.
pub(in crate::systems) enum HealthBarSource {
    Player,
    TrainingDummy,
}

/// Description:
/// Spawns an overhead health bar for a remote enemy player.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn health bar entities.
/// - `meshes`: Mesh assets used by health bar geometry.
/// - `materials`: Material assets used by health bar visuals.
/// - `target`: Remote player entity followed by the health bar.
/// - `max_health`: Maximum health value used to scale the fill.
///
/// Return:
/// - Spawned health bar entity.
pub(in crate::systems) fn spawn_remote_enemy_player_health_bar(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target: Entity,
    max_health: f32,
) -> Entity {
    spawn_health_bar(
        commands,
        meshes,
        materials,
        target,
        LIRA_BAR_OFFSET,
        max_health,
        HealthBarSource::TrainingDummy,
        Color::srgb(0.9, 0.18, 0.12),
    )
}

/// Description:
/// Spawns an overhead health bar for a remote allied player.
pub(in crate::systems) fn spawn_remote_ally_player_health_bar(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target: Entity,
    max_health: f32,
) -> Entity {
    spawn_health_bar(
        commands,
        meshes,
        materials,
        target,
        LIRA_BAR_OFFSET,
        max_health,
        HealthBarSource::Player,
        Color::srgb(0.15, 0.35, 1.0),
    )
}

/// Description:
/// Spawns an overhead health bar for the local player.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn health bar entities.
/// - `meshes`: Mesh assets used by health bar geometry.
/// - `materials`: Material assets used by health bar visuals.
/// - `target`: Player entity followed by the health bar.
///
/// Return:
/// - Spawned health bar entity.
pub(in crate::systems) fn spawn_player_health_bar(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target: Entity,
) -> Entity {
    spawn_health_bar(
        commands,
        meshes,
        materials,
        target,
        LIRA_BAR_OFFSET,
        100.0,
        HealthBarSource::Player,
        Color::srgb(0.15, 0.35, 1.0),
    )
}

/// Description:
/// Updates overhead health bar positions and rotates them to face the top-down camera.
///
/// Params:
/// - `camera_query`: Top-down camera transform used as the billboard orientation source.
/// - `target_query`: Target transforms followed by health bars.
/// - `bar_query`: Health bar tracking components and transforms to update.
pub(in crate::systems) fn update_health_bar_positions(
    camera_query: Query<&GlobalTransform, With<TopDownCamera>>,
    target_query: Query<&GlobalTransform>,
    mut bar_query: Query<(&HealthBar, &mut Transform)>,
) {
    let camera_rotation = camera_query
        .single()
        .ok()
        .map(GlobalTransform::rotation)
        .unwrap_or(Quat::IDENTITY);

    for (bar, mut transform) in &mut bar_query {
        let Ok(target_transform) = target_query.get(bar.target) else {
            continue;
        };

        transform.translation = target_transform.translation() + Vec3::Y * bar.y_offset;
        transform.rotation = camera_rotation;
    }
}

/// Description:
/// Updates health bar fill width and visibility from the tracked target health.
///
/// Params:
/// - `player_query`: Player health components.
/// - `training_dummy_query`: Enemy training dummy health components.
/// - `fill_query`: Health bar fills that should be scaled or hidden.
pub(in crate::systems) fn update_health_bar_fills(
    player_query: Query<&Health>,
    training_dummy_query: Query<&TrainingDummy>,
    mut fill_query: Query<(&HealthBarFill, &mut Transform, &mut Visibility)>,
) {
    for (fill, mut transform, mut visibility) in &mut fill_query {
        let health = match fill.source {
            HealthBarSource::Player => player_query
                .get(fill.target)
                .ok()
                .map(|health| (health.current as f32, health.max as f32)),
            HealthBarSource::TrainingDummy => training_dummy_query
                .get(fill.target)
                .ok()
                .map(|dummy| (dummy.health, fill.max_health)),
        };

        let Some((health, max_health)) = health else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let ratio = (health / max_health).clamp(0.0, 1.0);
        transform.scale.x = BAR_WIDTH * ratio;
        transform.translation.x = -BAR_WIDTH * (1.0 - ratio) * 0.5;
        *visibility = if ratio > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Description:
/// Spawns the shared child geometry for an overhead health bar.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn the health bar hierarchy.
/// - `meshes`: Mesh assets used by background, fill, and stripe geometry.
/// - `materials`: Material assets used by background, fill, and stripe visuals.
/// - `target`: Entity followed by the health bar.
/// - `y_offset`: Vertical world-space offset above the target.
/// - `max_health`: Maximum health value used by the fill component.
/// - `source`: Health source type read by the fill update system.
/// - `team_color`: Color used by the team stripe.
///
/// Return:
/// - Spawned health bar entity.
fn spawn_health_bar(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target: Entity,
    y_offset: f32,
    max_health: f32,
    source: HealthBarSource,
    team_color: Color,
) -> Entity {
    let background_mesh = meshes.add(Cuboid::new(BAR_WIDTH + 0.14, BAR_HEIGHT, BAR_DEPTH));
    let fill_mesh = meshes.add(Cuboid::new(1.0, BAR_FILL_HEIGHT, BAR_DEPTH * 1.25));
    let stripe_mesh = meshes.add(Cuboid::new(BAR_WIDTH + 0.1, 0.035, BAR_DEPTH * 1.35));

    let background_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.015, 0.018, 0.022),
        unlit: true,
        ..default()
    });
    let fill_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.82, 0.22),
        emissive: Color::srgb(0.02, 0.1, 0.02).into(),
        unlit: true,
        ..default()
    });
    let stripe_material = materials.add(StandardMaterial {
        base_color: team_color,
        emissive: team_color.with_alpha(0.32).into(),
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            Name::new("HealthBar"),
            HealthBar { target, y_offset },
            Transform::default(),
            Visibility::Visible,
        ))
        .with_children(|bar| {
            bar.spawn((
                Name::new("HealthBarBackground"),
                Mesh3d(background_mesh),
                MeshMaterial3d(background_material),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
            bar.spawn((
                Name::new("HealthBarFill"),
                HealthBarFill {
                    target,
                    max_health,
                    source,
                },
                Mesh3d(fill_mesh),
                MeshMaterial3d(fill_material),
                Transform::from_xyz(0.0, 0.025, BAR_DEPTH * 0.8)
                    .with_scale(Vec3::new(BAR_WIDTH, 1.0, 1.0)),
                Visibility::Visible,
            ));
            bar.spawn((
                Name::new("HealthBarTeamStripe"),
                Mesh3d(stripe_mesh),
                MeshMaterial3d(stripe_material),
                Transform::from_xyz(0.0, -BAR_HEIGHT * 0.5 + 0.035, BAR_DEPTH * 1.15),
            ));
        })
        .id()
}
