use glam::{Mat4, Vec3};

/// A perspective camera that looks at a point on the map.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub lon: f32,
    pub lat: f32,
    /// Visible width of the ground at the target, in kilometres.
    pub width_km: f32,
    /// Degrees from straight down: 0 looks down, 60 is a low oblique view.
    pub tilt_deg: f32,
    /// Compass heading the camera looks toward, in degrees: 0 north up, 90 east up.
    pub heading_deg: f32,
    /// Vertical field of view in degrees.
    pub fov_deg: f32,
}

impl Camera {
    pub fn scenario() -> Camera {
        Camera { lon: -82.0, lat: 41.9, width_km: 2250.0, tilt_deg: 24.0, heading_deg: 0.0, fov_deg: 28.0 }
    }
    pub fn close() -> Camera {
        Camera { lon: -81.4, lat: 41.2, width_km: 780.0, tilt_deg: 40.0, heading_deg: 0.0, fov_deg: 28.0 }
    }
    pub fn turned() -> Camera {
        Camera { lon: -82.3, lat: 41.1, width_km: 900.0, tilt_deg: 52.0, heading_deg: 38.0, fov_deg: 28.0 }
    }
}

pub struct View {
    pub eye: Vec3,
    pub target: Vec3,
    pub view_proj: Mat4,
    pub inv_view_proj: Mat4,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    /// `target_xy` is the target in map kilometres, `ground_z` its height.
    pub fn view(&self, target_xy: (f32, f32), ground_z: f32, aspect: f32) -> View {
        let fov = self.fov_deg.to_radians();
        let hfov_tan = (fov / 2.0).tan() * aspect;
        let dist = (self.width_km / 2.0) / hfov_tan;
        let (tilt, head) = (self.tilt_deg.to_radians(), self.heading_deg.to_radians());
        // The camera looks along `fwd` on the ground; it sits behind and above the target.
        let fwd = Vec3::new(head.sin(), head.cos(), 0.0);
        let target = Vec3::new(target_xy.0, target_xy.1, ground_z);
        let eye = target - fwd * (dist * tilt.sin()) + Vec3::Z * (dist * tilt.cos());
        let (near, far) = (dist * 0.04, dist * 6.0);
        let view = Mat4::look_at_rh(eye, target, Vec3::Z);
        let proj = Mat4::perspective_rh(fov, aspect, near, far);
        let view_proj = proj * view;
        View { eye, target, view_proj, inv_view_proj: view_proj.inverse(), near, far }
    }
}
