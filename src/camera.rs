use macroquad::prelude::{vec2, vec3, Vec2, Vec3};

use crate::theme::{CAMERA_MAX, CAMERA_MIN, CAMERA_RADIUS, FAR_PLANE, FOV_Y, NEAR_PLANE};

#[derive(Debug, Clone, Copy, Default)]
pub struct CameraInput {
    pub forward: f32,
    pub strafe: f32,
    pub vertical: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub boost: bool,
    pub brake: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Projected {
    pub screen: Vec2,
    pub depth: f32,
    pub scale: f32,
}

#[derive(Debug, Clone)]
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: Vec3,
    pub speed: f32,
    pub frozen: bool,
    default_position: Vec3,
    default_yaw: f32,
    default_pitch: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: vec3(0.0, 4.2, -7.2),
            yaw: 0.0,
            pitch: -0.28,
            velocity: Vec3::ZERO,
            speed: 3.6,
            frozen: false,
            default_position: vec3(0.0, 4.2, -7.2),
            default_yaw: 0.0,
            default_pitch: -0.28,
        }
    }

    pub fn forward(&self) -> Vec3 {
        vec3(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        )
        .normalize_or_zero()
    }

    pub fn right(&self) -> Vec3 {
        let forward = self.forward();
        vec3(0.0, 1.0, 0.0).cross(forward).normalize_or_zero()
    }

    pub fn up(&self) -> Vec3 {
        self.forward().cross(self.right()).normalize_or_zero()
    }

    pub fn update(&mut self, dt: f32, input: CameraInput, reduced_motion: bool) {
        if self.frozen {
            self.velocity = Vec3::ZERO;
            return;
        }
        let dt = dt.clamp(0.0, 0.05);
        let turn = if reduced_motion { 1.1 } else { 1.9 };
        self.yaw += input.yaw * turn * dt;
        self.pitch += input.pitch * turn * dt;
        self.pitch = self.pitch.clamp(-1.35, 1.35);
        self.yaw = self.yaw.rem_euclid(std::f32::consts::TAU);

        let forward = self.forward();
        let right = self.right();
        let up = vec3(0.0, 1.0, 0.0);
        let mut direction = forward * input.forward + right * input.strafe + up * input.vertical;
        if direction.length_squared() > 1.0 {
            direction = direction.normalize();
        }

        let mut target_speed = self.speed;
        if input.boost {
            target_speed *= 2.6;
        }
        if input.brake {
            target_speed *= 0.15;
        }
        let target = direction * target_speed;
        let response = if reduced_motion { 4.0 } else { 7.5 };
        let blend = 1.0 - (-response * dt).exp();
        self.velocity += (target - self.velocity) * blend;

        self.position += self.velocity * dt;

        let horizontal = vec2(self.position.x, self.position.z);
        if horizontal.length() > CAMERA_RADIUS {
            let clamped = horizontal.normalize_or_zero() * CAMERA_RADIUS;
            self.position.x = clamped.x;
            self.position.z = clamped.y;
        }
        self.position.y = self.position.y.clamp(CAMERA_MIN, CAMERA_MAX);
    }

    pub fn recenter(&mut self) {
        self.position = self.default_position;
        self.yaw = self.default_yaw;
        self.pitch = self.default_pitch;
        self.velocity = Vec3::ZERO;
    }

    pub fn toggle_freeze(&mut self) {
        self.frozen = !self.frozen;
        if self.frozen {
            self.velocity = Vec3::ZERO;
        }
    }

    pub fn adjust_speed(&mut self, wheel: f32) {
        self.speed = (self.speed + wheel * 0.35).clamp(0.8, 12.0);
    }

    pub fn project(&self, point: Vec3, width: f32, height: f32) -> Option<Projected> {
        let forward = self.forward();
        let right = self.right();
        let up = self.up();
        let relative = point - self.position;
        let depth = relative.dot(forward);
        if !(NEAR_PLANE..=FAR_PLANE).contains(&depth) {
            return None;
        }
        let focal = 1.0 / (FOV_Y * 0.5).tan();
        let aspect = width / height.max(1.0);
        let x = relative.dot(right) * focal / (aspect * depth);
        let y = relative.dot(up) * focal / depth;
        let screen = vec2((x * 0.5 + 0.5) * width, (-y * 0.5 + 0.5) * height);
        Some(Projected {
            screen,
            depth,
            scale: focal / depth * height * 0.5,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_projects_to_center() {
        let camera = Camera::new();
        let point = camera.position + camera.forward() * 5.0;
        let projected = camera.project(point, 800.0, 600.0).unwrap();
        assert!((projected.screen.x - 400.0).abs() < 0.01);
        assert!((projected.screen.y - 300.0).abs() < 0.01);
    }

    #[test]
    fn points_behind_are_culled() {
        let camera = Camera::new();
        let point = camera.position - camera.forward() * 5.0;
        assert!(camera.project(point, 800.0, 600.0).is_none());
    }

    #[test]
    fn updates_position_with_thrust() {
        let mut camera = Camera::new();
        let start = camera.position;
        camera.update(
            0.05,
            CameraInput {
                forward: 1.0,
                ..CameraInput::default()
            },
            false,
        );
        assert!(camera.position.distance(start) > 0.0);
    }

    #[test]
    fn recenter_restores_defaults() {
        let mut camera = Camera::new();
        let start = camera.position;
        camera.position += vec3(1.0, 0.0, 0.0);
        camera.recenter();
        assert_eq!(camera.position, start);
    }
}
