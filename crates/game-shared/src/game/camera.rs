use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Description:
/// Marks the active top-down gameplay camera.
pub struct TopDownCamera;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
/// Description:
/// Stores the static tuning values used to position the top-down camera.
///
/// Fields:
/// - `height`: Vertical camera offset above the look target.
/// - `pitch_radians`: Camera pitch angle in radians.
/// - `yaw_radians`: Camera yaw angle in radians.
/// - `follow_lerp`: Follow interpolation speed.
/// - `look_ahead_ground`: Ground-plane look-ahead distance when not centered.
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

#[derive(Component, Debug, Clone, Copy, PartialEq)]
/// Description:
/// Stores zoom state and zoom limits for the top-down camera.
///
/// Fields:
/// - `current`: Current camera zoom distance.
/// - `min`: Minimum allowed zoom distance.
/// - `max`: Maximum allowed zoom distance.
/// - `speed`: Zoom step multiplier applied to scroll input.
pub struct CameraZoom {
    pub current: f32,
    pub min: f32,
    pub max: f32,
    pub speed: f32,
}

impl CameraZoom {
    /// Description:
    /// Applies a signed zoom delta and clamps the result to configured limits.
    ///
    /// Params:
    /// - `self`: Mutable camera zoom state.
    /// - `delta`: Signed zoom delta to apply.
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

#[derive(Component, Debug, Clone, Copy, PartialEq)]
/// Description:
/// Stores the world-space focus point followed by the top-down camera.
///
/// Fields:
/// - `target`: World-space position the camera should look at.
/// - `centered`: Whether the camera should stay centered on the controlled player.
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

#[derive(Bundle, Debug, Clone)]
/// Description:
/// Bundles all components required to create a top-down gameplay camera.
///
/// Fields:
/// - `marker`: Marker component identifying the top-down camera.
/// - `settings`: Static camera positioning settings.
/// - `zoom`: Runtime zoom state.
/// - `focus`: Runtime camera focus state.
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
