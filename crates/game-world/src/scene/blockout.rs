use bevy::math::primitives::Cuboid;
use bevy::prelude::*;

/// Description:
/// Spawns a blockout wall layout for prototyping game map lanes and jungle spaces.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn wall entities.
/// - `meshes`: Mesh assets used to create wall cuboids.
/// - `wall_material`: Material handle assigned to every wall mesh.
/// - `map_size_x`: Full map width used to place boundary walls.
/// - `map_size_z`: Full map depth used to place boundary walls.
pub(super) fn spawn_game_wall_blockout(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    wall_material: Handle<StandardMaterial>,
    map_size_x: f32,
    map_size_z: f32,
) {
    let wall_height_scale = 0.42;

    let wall_specs = [
        (
            "Wall_North",
            Vec3::new(map_size_x + 4.0, 6.0, 2.5),
            Vec3::new(0.0, 3.0, map_size_z * 0.5 - 1.5),
        ),
        (
            "Wall_South",
            Vec3::new(map_size_x + 4.0, 6.0, 2.5),
            Vec3::new(0.0, 3.0, -map_size_z * 0.5 + 1.5),
        ),
        (
            "Wall_East",
            Vec3::new(2.5, 6.0, map_size_z - 4.0),
            Vec3::new(map_size_x * 0.5 - 1.5, 3.0, 0.0),
        ),
        (
            "Wall_West",
            Vec3::new(2.5, 6.0, map_size_z - 4.0),
            Vec3::new(-map_size_x * 0.5 + 1.5, 3.0, 0.0),
        ),
        (
            "BlueBase_Back",
            Vec3::new(22.0, 6.0, 3.0),
            Vec3::new(-45.0, 3.0, -47.0),
        ),
        (
            "BlueBase_Left",
            Vec3::new(3.0, 6.0, 20.0),
            Vec3::new(-47.0, 3.0, -37.0),
        ),
        (
            "BlueBase_Right",
            Vec3::new(3.0, 6.0, 12.0),
            Vec3::new(-33.0, 3.0, -43.0),
        ),
        (
            "RedBase_Back",
            Vec3::new(22.0, 6.0, 3.0),
            Vec3::new(45.0, 3.0, 47.0),
        ),
        (
            "RedBase_Right",
            Vec3::new(3.0, 6.0, 20.0),
            Vec3::new(47.0, 3.0, 37.0),
        ),
        (
            "RedBase_Left",
            Vec3::new(3.0, 6.0, 12.0),
            Vec3::new(33.0, 3.0, 43.0),
        ),
        (
            "TopLane_Blocker_A",
            Vec3::new(14.0, 5.0, 3.0),
            Vec3::new(-32.0, 2.5, 27.0),
        ),
        (
            "TopLane_Blocker_B",
            Vec3::new(11.0, 5.0, 3.0),
            Vec3::new(-14.0, 2.5, 24.0),
        ),
        (
            "TopLane_Blocker_C",
            Vec3::new(12.0, 5.0, 3.0),
            Vec3::new(24.0, 2.5, 32.0),
        ),
        (
            "MidLane_Left",
            Vec3::new(10.0, 5.0, 3.0),
            Vec3::new(-18.0, 2.5, -8.0),
        ),
        (
            "MidLane_Right",
            Vec3::new(10.0, 5.0, 3.0),
            Vec3::new(18.0, 2.5, 8.0),
        ),
        (
            "BotLane_Blocker_A",
            Vec3::new(12.0, 5.0, 3.0),
            Vec3::new(-28.0, 2.5, -32.0),
        ),
        (
            "BotLane_Blocker_B",
            Vec3::new(11.0, 5.0, 3.0),
            Vec3::new(12.0, 2.5, -22.0),
        ),
        (
            "BotLane_Blocker_C",
            Vec3::new(13.0, 5.0, 3.0),
            Vec3::new(34.0, 2.5, -26.0),
        ),
        (
            "RiverBank_NW_Outer",
            Vec3::new(3.0, 5.0, 22.0),
            Vec3::new(-20.0, 2.5, 10.0),
        ),
        (
            "RiverBank_SE_Outer",
            Vec3::new(3.0, 5.0, 22.0),
            Vec3::new(20.0, 2.5, -10.0),
        ),
        (
            "RiverBank_NW_Inner",
            Vec3::new(3.0, 5.0, 18.0),
            Vec3::new(-8.0, 2.5, 2.0),
        ),
        (
            "RiverBank_SE_Inner",
            Vec3::new(3.0, 5.0, 18.0),
            Vec3::new(8.0, 2.5, -2.0),
        ),
        (
            "JungleGate_NW",
            Vec3::new(7.0, 5.0, 3.0),
            Vec3::new(-30.0, 2.5, 8.0),
        ),
        (
            "JungleGate_NE",
            Vec3::new(7.0, 5.0, 3.0),
            Vec3::new(26.0, 2.5, 18.0),
        ),
        (
            "JungleGate_SW",
            Vec3::new(7.0, 5.0, 3.0),
            Vec3::new(-26.0, 2.5, -18.0),
        ),
        (
            "JungleGate_SE",
            Vec3::new(7.0, 5.0, 3.0),
            Vec3::new(30.0, 2.5, -8.0),
        ),
    ];

    for (name, size, position) in wall_specs {
        let lowered_height = size.y * wall_height_scale;
        let wall_mesh = meshes.add(Mesh::from(Cuboid::new(size.x, lowered_height, size.z)));
        commands.spawn((
            Name::new(name),
            Mesh3d(wall_mesh),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_xyz(position.x, lowered_height * 0.5, position.z),
        ));
    }
}
