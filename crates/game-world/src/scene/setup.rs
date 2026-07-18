use bevy::math::primitives::{Cuboid, Cylinder};
use bevy::prelude::*;
use game_shared::game::{
    lane::{LANE_HALF_WIDTH, LANE_SPAWN_Z, lane_spawn_position},
    map::MapGround,
    team::TeamSpec,
};

const MAP_THICKNESS: f32 = 0.2;
const LANE_LENGTH: f32 = LANE_SPAWN_Z * 2.0;
const WALL_THICKNESS: f32 = 0.35;
const WALL_HEIGHT: f32 = 0.8;
const SPAWN_MARKER_RADIUS: f32 = 2.4;
const SPAWN_MARKER_HEIGHT: f32 = 0.04;

/// Description:
/// Spawns the flat playable map plane and directional scene light.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn map and light entities.
/// - `meshes`: Mesh assets used to create the map plane.
/// - `materials`: Material assets used to create the map material.
pub(super) fn setup_flat_map(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let lane_mesh = meshes.add(Mesh::from(Cuboid::new(
        LANE_HALF_WIDTH * 2.0,
        MAP_THICKNESS,
        LANE_LENGTH,
    )));
    let lane_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.29, 0.28),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        Name::new("SingleLaneMap"),
        MapGround::from_size(LANE_HALF_WIDTH * 2.0, LANE_LENGTH, MAP_THICKNESS),
        Mesh3d(lane_mesh),
        MeshMaterial3d(lane_material),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));

    let wall_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.12, 0.13),
        perceptual_roughness: 1.0,
        ..default()
    });
    let side_wall_mesh = meshes.add(Mesh::from(Cuboid::new(
        WALL_THICKNESS,
        WALL_HEIGHT,
        LANE_LENGTH + WALL_THICKNESS * 2.0,
    )));
    let end_wall_mesh = meshes.add(Mesh::from(Cuboid::new(
        LANE_HALF_WIDTH * 2.0 + WALL_THICKNESS * 2.0,
        WALL_HEIGHT,
        WALL_THICKNESS,
    )));

    for (name, x) in [
        (
            "SingleLaneWallLeft",
            -(LANE_HALF_WIDTH + WALL_THICKNESS * 0.5),
        ),
        (
            "SingleLaneWallRight",
            LANE_HALF_WIDTH + WALL_THICKNESS * 0.5,
        ),
    ] {
        commands.spawn((
            Name::new(name),
            Mesh3d(side_wall_mesh.clone()),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_xyz(x, WALL_HEIGHT * 0.5, 0.0),
        ));
    }

    for (name, z) in [
        (
            "SingleLaneWallLightEnd",
            -(LANE_SPAWN_Z + WALL_THICKNESS * 0.5),
        ),
        ("SingleLaneWallDarkEnd", LANE_SPAWN_Z + WALL_THICKNESS * 0.5),
    ] {
        commands.spawn((
            Name::new(name),
            Mesh3d(end_wall_mesh.clone()),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_xyz(0.0, WALL_HEIGHT * 0.5, z),
        ));
    }

    let spawn_marker_mesh = meshes.add(Mesh::from(Cylinder::new(
        SPAWN_MARKER_RADIUS,
        SPAWN_MARKER_HEIGHT,
    )));
    let light_spawn_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.38, 0.95),
        perceptual_roughness: 0.65,
        ..default()
    });
    let dark_spawn_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.12, 0.16),
        perceptual_roughness: 0.65,
        ..default()
    });

    for (name, team, material) in [
        ("LightTeamSpawn", TeamSpec::Light, light_spawn_material),
        ("DarkTeamSpawn", TeamSpec::Dark, dark_spawn_material),
    ] {
        let spawn_position = lane_spawn_position(team);
        commands.spawn((
            Name::new(name),
            Mesh3d(spawn_marker_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(
                spawn_position + Vec3::Y * (MAP_THICKNESS * 0.5 + SPAWN_MARKER_HEIGHT * 0.5),
            ),
        ));
    }

    commands.spawn((
        Name::new("MapSunLight"),
        DirectionalLight {
            illuminance: 18_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, -0.8, 0.0)),
    ));
}
