use bevy::math::primitives::Cuboid;
use bevy::prelude::*;
use game_shared::game::map::MapGround;

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
    let map_size_x = 120.0;
    let map_size_z = 120.0;
    let map_thickness = 0.2;

    let map_mesh = meshes.add(Mesh::from(Cuboid::new(
        map_size_x,
        map_thickness,
        map_size_z,
    )));
    let map_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.34, 0.36, 0.39),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        Name::new("MapPlane"),
        MapGround::from_size(map_size_x, map_size_z, map_thickness),
        Mesh3d(map_mesh),
        MeshMaterial3d(map_material),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));

    commands.spawn((
        Name::new("MapSunLight"),
        DirectionalLight {
            illuminance: 18_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, -0.8, 0.0)),
    ));
}
