use super::Camera;
use dolly::prelude::*;
use glam::Vec3;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct CameraController {
    move_forward: bool,
    move_backward: bool,
    move_left: bool,
    move_right: bool,
    move_up: bool,
    move_down: bool,
    speed: f32,
    sensitivity: f32,
    mouse_captured: bool,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            move_forward: false,
            move_backward: false,
            move_left: false,
            move_right: false,
            move_up: false,
            move_down: false,
            speed,
            sensitivity,
            mouse_captured: false,
        }
    }

    pub fn is_mouse_captured(&self) -> bool {
        self.mouse_captured
    }

    pub fn set_mouse_captured(&mut self, captured: bool) {
        self.mouse_captured = captured;
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity;
    }

    pub fn process_keyboard(&mut self, key_code: KeyCode, state: ElementState) -> bool {
        let pressed = state == ElementState::Pressed;
        match key_code {
            KeyCode::KeyW => {
                self.move_forward = pressed;
                true
            }
            KeyCode::KeyS => {
                self.move_backward = pressed;
                true
            }
            KeyCode::KeyA => {
                self.move_left = pressed;
                true
            }
            KeyCode::KeyD => {
                self.move_right = pressed;
                true
            }
            KeyCode::Space => {
                self.move_up = pressed;
                true
            }
            KeyCode::ShiftLeft | KeyCode::ControlLeft => {
                self.move_down = pressed;
                true
            }
            _ => false,
        }
    }

    pub fn process_mouse(&mut self, delta_x: f64, delta_y: f64, camera: &mut Camera) {
        if !self.mouse_captured {
            return;
        }

        let yaw_delta = (delta_x as f32) * self.sensitivity;
        let pitch_delta = (delta_y as f32) * self.sensitivity;

        camera
            .rig_mut()
            .driver_mut::<YawPitch>()
            .rotate_yaw_pitch(-yaw_delta, -pitch_delta);
    }

    pub fn update_camera(&self, camera: &mut Camera, delta_time: f32) {
        let mut movement = Vec3::ZERO;

        if self.move_forward {
            movement.z -= 1.0;
        }
        if self.move_backward {
            movement.z += 1.0;
        }
        if self.move_left {
            movement.x -= 1.0;
        }
        if self.move_right {
            movement.x += 1.0;
        }
        if self.move_up {
            movement.y += 1.0;
        }
        if self.move_down {
            movement.y -= 1.0;
        }

        if movement != Vec3::ZERO {
            movement = movement.normalize() * self.speed * delta_time;
            camera.rig_mut().driver_mut::<Position>().translate(movement);
        }
    }
}