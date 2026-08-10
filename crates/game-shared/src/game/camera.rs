use bevy::prelude::*;

/// Marks the active top-down gameplay camera.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TopDownCamera;
/// Stores static tuning values for the top-down camera.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct TopDownCameraSettings {
    pub height: f32,
    pub pitch_radians: f32,
    pub yaw_radians: f32,
    pub follow_lerp: f32,
    pub look_ahead_ground: f32,
}

impl Default for TopDownCameraSettings {
    fn default() -> Self {
        Self {
            height: 2.4,
            pitch_radians: (-50.0_f32).to_radians(),
            yaw_radians: 45.0_f32.to_radians(),
            follow_lerp: 16.0,
            look_ahead_ground: 0.0,
        }
    }
}
/// Stores zoom state and limits for the top-down camera.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CameraZoom {
    pub current: f32,
    pub min: f32,
    pub max: f32,
    pub speed: f32,
}

impl CameraZoom {
    /// Applies a signed zoom delta within the configured limits.
    pub fn zoom_by(&mut self, delta: f32) {
        self.current = (self.current + delta * self.speed).clamp(self.min, self.max);
    }
}

impl Default for CameraZoom {
    fn default() -> Self {
        Self {
            current: 12.0,
            min: 3.0,
            max: 20.0,
            speed: 0.85,
        }
    }
}
/// Stores the world-space focus point followed by the top-down camera.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CameraFocus {
    pub target: Vec3,
    pub centered: bool,
}

impl Default for CameraFocus {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            centered: true,
        }
    }
}
/// Bundles the components required for a top-down gameplay camera.
#[derive(Bundle, Debug, Clone)]
pub struct TopDownCameraBundle {
    pub marker: TopDownCamera,
    pub settings: TopDownCameraSettings,
    pub zoom: CameraZoom,
    pub focus: CameraFocus,
}

impl Default for TopDownCameraBundle {
    fn default() -> Self {
        Self {
            marker: TopDownCamera,
            settings: TopDownCameraSettings::default(),
            zoom: CameraZoom::default(),
            focus: CameraFocus::default(),
        }
    }
}
