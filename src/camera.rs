//! Third-person orbit camera that stays stable on the curved hull, plus
//! cursor-to-surface picking (ray vs. cylinder).

use bevy::core_pipeline::bloom::Bloom;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::components::*;
use crate::resources::*;
use crate::RADIUS;

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Bloom::NATURAL,
        DistanceFog {
            color: Color::srgb(0.02, 0.025, 0.05),
            falloff: FogFalloff::Linear {
                start: 60.0,
                end: 165.0,
            },
            ..default()
        },
        Transform::from_xyz(0.0, 20.0, -25.0).looking_at(Vec3::ZERO, Vec3::Y),
        CamTag,
        Name::new("camera"),
    ));
}

fn orbit_input(
    mut cam: ResMut<CamCtl>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
) {
    let mut delta = Vec2::ZERO;
    for m in motion.read() {
        delta += m.delta;
    }
    if buttons.pressed(MouseButton::Middle) || keys.pressed(KeyCode::ControlLeft) {
        if delta.length_squared() > 0.0 {
            cam.yaw -= delta.x * 0.006;
            cam.pitch = (cam.pitch + delta.y * 0.006).clamp(0.12, 1.35);
            cam.orbited = true;
        }
    }
    if keys.pressed(KeyCode::KeyQ) {
        cam.yaw += 0.02;
        cam.orbited = true;
    }
    if keys.pressed(KeyCode::KeyE) && keys.pressed(KeyCode::ControlLeft) {
        cam.yaw -= 0.02;
    }
    for w in wheel.read() {
        cam.dist = (cam.dist * (1.0 - w.y * 0.09)).clamp(8.0, 46.0);
    }
}

fn follow_player(
    cam: Res<CamCtl>,
    qp: Query<&SurfPos, With<PlayerTag>>,
    mut qc: Query<&mut Transform, With<CamTag>>,
) {
    let (Ok(sp), Ok(mut tf)) = (qp.single(), qc.single_mut()) else {
        return;
    };
    let target = surf_to_world(sp.u, sp.v, sp.h + 1.8);
    let base = surf_quat(sp.u, 0.0);
    let off = base
        * (Quat::from_rotation_y(cam.yaw)
            * Quat::from_rotation_x(-cam.pitch)
            * Vec3::new(0.0, 0.0, cam.dist));
    *tf = Transform::from_translation(target + off).looking_at(target, surf_normal(sp.u));
}

/// Project the cursor onto the inner cylinder surface; returns (u, v).
pub fn cursor_surf(cam: &Camera, gt: &GlobalTransform, window: &Window) -> Option<(f32, f32)> {
    let cur = window.cursor_position()?;
    let ray = cam.viewport_to_world(gt, cur).ok()?;
    let (o, d) = (ray.origin, *ray.direction);
    let a = d.x * d.x + d.y * d.y;
    if a < 1e-7 {
        return None;
    }
    let oy = o.y - RADIUS;
    let b = 2.0 * (o.x * d.x + oy * d.y);
    let c = o.x * o.x + oy * oy - RADIUS * RADIUS;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b + disc.sqrt()) / (2.0 * a);
    if t < 0.0 {
        return None;
    }
    let p = o + d * t;
    let th = p.x.atan2(RADIUS - p.y);
    let (u, v) = clamp_uv(th * RADIUS, p.z);
    Some((u, v))
}

/// Convenience system-param-free helper used by several modules.
pub fn cursor_uv(
    qc: &Query<(&Camera, &GlobalTransform), With<CamTag>>,
    qw: &Query<&Window, With<PrimaryWindow>>,
) -> Option<(f32, f32)> {
    let (cam, gt) = qc.single().ok()?;
    let window = qw.single().ok()?;
    cursor_surf(cam, gt, window)
}

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, (orbit_input, follow_player.after(crate::player::move_player)));
    }
}
