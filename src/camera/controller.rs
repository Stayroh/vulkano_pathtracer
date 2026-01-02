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
    print_debug: bool,
    fast_move: bool,
    local_up: bool,
    local_down: bool,
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
            print_debug: false,
            fast_move: false,
            local_up: false,
            local_down: false,
        }
    }

    pub fn is_mouse_captured(&self) -> bool {
        self.mouse_captured
    }

    pub fn set_mouse_captured(&mut self, captured: bool) {
        self.mouse_captured = captured;
    }

    pub fn reset_input_state(&mut self) {
        self.move_forward = false;
        self.move_backward = false;
        self.move_left = false;
        self.move_right = false;
        self.move_up = false;
        self.move_down = false;
        self.print_debug = false;
        self.fast_move = false;
        self.local_up = false;
        self.local_down = false;
        self.mouse_captured = false;
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
            KeyCode::KeyE => {
                self.local_up = pressed;
                true
            }
            KeyCode::KeyQ => {
                self.local_down = pressed;
                true
            }
            KeyCode::Space => {
                self.move_up = pressed;
                true
            }
            KeyCode::ControlLeft => {
                self.move_down = pressed;
                true
            }
            KeyCode::KeyP => {
                self.print_debug = pressed;
                true
            }
            KeyCode::ShiftLeft => {
                self.fast_move = pressed;
                true
            }
            
            _ => false,
        }
    }

    pub fn process_scroll(&mut self, delta_y: f32, camera: &mut Camera) {
        let fov_change = delta_y * 0.1;
        let new_fov = (camera.fov - fov_change).clamp(10.0_f32.to_radians(), 120.0_f32.to_radians());
        camera.set_fov(new_fov);
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
        let mut global_movement = Vec3::ZERO;

         // Handle local up/down movement
        
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
        if self.local_up {
            movement.y += 1.0;
        }
        if self.local_down {
            movement.y -= 1.0;
        }
        if self.move_up {
            global_movement.y += 1.0;
        }
        if self.move_down {
            global_movement.y -= 1.0;
        }
        if self.print_debug {
            camera.debug_print();
        }
        
        if movement != Vec3::ZERO {
            movement = movement.normalize() ;
        }
        let rotation: glam::Quat = camera.rig_mut().final_transform.rotation.into();
        
        // Transform movement from camera space to world space
        let world_movement = (rotation * movement + global_movement) * self.speed * if self.fast_move { 4.0 } else { 1.0 } * delta_time;
        
        camera.rig_mut().driver_mut::<Position>().translate(world_movement);

    }
}