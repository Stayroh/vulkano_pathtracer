mod controller;

use bytemuck::{Pod, Zeroable};
pub use controller::CameraController;

use dolly::prelude::*;
use glam::{Mat4, Vec3};

// Uniform buffer structure matching GLSL layout
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct CameraUniform {
    pub inv_view: [[f32; 4]; 4],
    pub inv_proj: [[f32; 4]; 4],
}

pub struct Camera {
    rig: CameraRig,
    projection: Mat4,
    aspect_ratio: f32,
    fov: f32,
    near: f32,
    far: f32,
}

impl Camera {
    pub fn new(width: u32, height: u32, fov: f32) -> Self {
        let aspect_ratio = width as f32 / height as f32;
        let near = 0.1;
        let far = 1000.0;

        let rig = CameraRig::builder()
            .with(Position::new(Vec3::new(0.0, 2.0, 5.0)))
            .with(YawPitch::new().yaw_degrees(0.0).pitch_degrees(0.0))
            .with(Smooth::new_position_rotation(1.5, 1.5))
            .build();

        let projection = Mat4::perspective_rh(fov, aspect_ratio, near, far);

        Self {
            rig,
            projection,
            aspect_ratio,
            fov,
            near,
            far,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.aspect_ratio = width as f32 / height as f32;
            self.projection = Mat4::perspective_rh(self.fov, self.aspect_ratio, self.near, self.far);
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        self.rig.update(delta_time);
    }

    pub fn view_matrix(&self) -> Mat4 {
        // Build view matrix from dolly transform
        let transform = &self.rig.final_transform;
        let rotation: glam::Quat = transform.rotation.into();
        let position: glam::Vec3 = transform.position.into();
        Mat4::from_rotation_translation(rotation, position).inverse()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        self.projection
    }

    pub fn inverse_view_matrix(&self) -> Mat4 {
        self.view_matrix().inverse()
    }

    pub fn inverse_projection_matrix(&self) -> Mat4 {
        self.projection.inverse()
    }

    pub fn position(&self) -> Vec3 {
        self.rig.final_transform.position.into()
    }

    pub fn get_ray_tracing_uniforms(&self) -> CameraUniform {
        let inv_view = self.inverse_view_matrix();
        let inv_proj = self.inverse_projection_matrix();

        CameraUniform {
            inv_view: inv_view.to_cols_array_2d(),
            inv_proj: inv_proj.to_cols_array_2d(),
        }
    }

    pub fn rig_mut(&mut self) -> &mut CameraRig {
        &mut self.rig
    }
}