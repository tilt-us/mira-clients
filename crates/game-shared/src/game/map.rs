use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
/// Description:
/// Stores the dimensions of the playable ground plane.
///
/// Fields:
/// - `half_extents`: Half-width and half-depth of the map in local XZ space.
/// - `thickness`: Vertical thickness of the ground mesh.
pub struct MapGround {
    pub half_extents: Vec2,
    pub thickness: f32,
}

impl MapGround {
    /// Description:
    /// Creates map ground bounds from full map dimensions.
    ///
    /// Params:
    /// - `size_x`: Full map width on the local X axis.
    /// - `size_z`: Full map depth on the local Z axis.
    /// - `thickness`: Vertical ground thickness.
    ///
    /// Return:
    /// - A configured `MapGround` component.
    pub fn from_size(size_x: f32, size_z: f32, thickness: f32) -> Self {
        Self {
            half_extents: Vec2::new(size_x * 0.5, size_z * 0.5),
            thickness,
        }
    }

    /// Description:
    /// Returns the local Y coordinate of the ground top surface.
    ///
    /// Params:
    /// - `self`: Map ground bounds.
    ///
    /// Return:
    /// - Local Y coordinate for the top face of the map ground.
    pub fn top_local_y(self) -> f32 {
        self.thickness * 0.5
    }

    /// Description:
    /// Checks whether a local-space position lies inside the map XZ bounds.
    ///
    /// Params:
    /// - `self`: Map ground bounds.
    /// - `position`: Local-space position to test.
    ///
    /// Return:
    /// - `true` when the position is inside the local XZ bounds.
    pub fn contains_local_xz(self, position: Vec3) -> bool {
        position.x.abs() <= self.half_extents.x && position.z.abs() <= self.half_extents.y
    }
}
