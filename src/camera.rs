use cgmath::{EuclideanSpace, Quaternion, Rad, Rotation, Rotation3, Vector3, Zero};
use winit::{dpi::PhysicalSize, event::{ElementState, KeyEvent, WindowEvent}, keyboard::{KeyCode, PhysicalKey}};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix().into();
    }
}

pub struct Camera {
    eye: cgmath::Point3<f32>,
    rotation: cgmath::Quaternion<f32>,
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);
 
impl Camera {
    pub fn new(aspect: f32, fovy: f32, znear: f32, zfar: f32) -> Camera {
        Camera {
            eye: (0.0, 1.0, 2.0).into(),
            rotation: cgmath::Quaternion::from_angle_y(cgmath::Rad(0.0)),
            aspect, fovy, znear, zfar
        }
    }

    pub fn update_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    fn build_view_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        let view = cgmath::Matrix4::from(self.rotation) * cgmath::Matrix4::from_translation(-self.eye.to_vec());
        let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);

        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }
}

pub struct CameraController {
    speed: f32,

    yaw: f32,
    pitch: f32,

    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    is_up_pressed: bool,
    is_down_pressed: bool
}

impl CameraController {
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            
            yaw: 0.0,
            pitch: 0.0,

            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            is_up_pressed: false,
            is_down_pressed: false
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent, size: PhysicalSize<u32>) -> bool {
        match event {
            WindowEvent::KeyboardInput { event: KeyEvent {
                state,
                physical_key: PhysicalKey::Code(keycode),
                ..
            }, .. } => {
                let is_pressed = *state == ElementState::Pressed;
                match keycode {
                    KeyCode::KeyW | KeyCode::ArrowUp => {
                        self.is_forward_pressed = is_pressed;
                        true
                    }
                    KeyCode::KeyA | KeyCode::ArrowLeft => {
                        self.is_left_pressed = is_pressed;
                        true
                    }
                    KeyCode::KeyS | KeyCode::ArrowDown => {
                        self.is_backward_pressed = is_pressed;
                        true
                    }
                    KeyCode::KeyD | KeyCode::ArrowRight => {
                        self.is_right_pressed = is_pressed;
                        true
                    }
                    KeyCode::Space => {
                        self.is_up_pressed = is_pressed;
                        true
                    }
                    KeyCode::ShiftLeft => {
                        self.is_down_pressed = is_pressed;
                        true
                    }
                    _ => false,
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                let delta = cgmath::Vector2::new(
                    position.x as f32 - size.width as f32 / 2.0,
                    position.y as f32 - size.height as f32 / 2.0,
                );
                // Update camera rotation based on cursor movement
                let sensitivity = 0.001;

                self.yaw += delta.x * sensitivity;
                self.pitch += delta.y * sensitivity;

                // Clamp pitch to avoid flipping
                let pitch_limit = std::f32::consts::FRAC_PI_2 * (5.0 / 6.0);
                self.pitch = self.pitch.clamp(-pitch_limit, pitch_limit);

                true
            },
            _ => false,
        }
    }

    pub fn update_camera(&self, camera: &mut Camera, delta_time: f32) {
        use cgmath::InnerSpace;

        let up = Vector3::unit_y();
        let forward = camera.rotation.conjugate() * Vector3::unit_z();
        let forward = Vector3::new(forward.x, 0.0, forward.z).normalize();
        let right = forward.cross(up).normalize();
        
        let mut movement = Vector3::zero();

        if self.is_forward_pressed {
            movement -= forward;
        }
        if self.is_backward_pressed {
            movement += forward;
        }
        if self.is_left_pressed {
            movement += right;
        }
        if self.is_right_pressed {
            movement -= right;
        }
        if self.is_up_pressed {
            movement += up;
        }
        if self.is_down_pressed {
            movement -= up;
        }

        if movement.magnitude() > 0.0 {
            movement = movement.normalize() * self.speed * delta_time;
            camera.eye += movement;
        }

        let yaw_rot = Quaternion::from_angle_y(Rad(self.yaw));
        let pitch_rot = Quaternion::from_angle_x(Rad(self.pitch));

        // Apply pitch after yaw
        camera.rotation = pitch_rot * yaw_rot;
    }
}