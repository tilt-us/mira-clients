use bevy::prelude::*;

/// Stores the dimensions of the playable ground plane.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MapGround {
    pub half_extents: Vec2,
    pub thickness: f32,
}

impl MapGround {
    /// Creates ground bounds from full map dimensions.
    pub fn from_size(size_x: f32, size_z: f32, thickness: f32) -> Self {
        Self {
            half_extents: Vec2::new(size_x * 0.5, size_z * 0.5),
            thickness,
        }
    }
    /// Returns the local Y coordinate of the ground's top surface.
    pub fn top_local_y(self) -> f32 {
        self.thickness * 0.5
    }
    /// Returns whether a local-space position lies inside the map's XZ bounds.
    pub fn contains_local_xz(self, position: Vec3) -> bool {
        position.x.abs() <= self.half_extents.x && position.z.abs() <= self.half_extents.y
    }
}
