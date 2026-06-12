//! Shared components, events and the curved-surface coordinate system.
//!
//! All gameplay logic runs in flat "surface coordinates" (u, v, h):
//!   u = arc-length along the ring circumference, v = along the cylinder axis,
//!   h = height above the hull (toward the ring axis).
//! Rendering maps these onto the inner surface of a cylinder of RADIUS, so the
//! horizon curves upward while simulation stays a simple 2D grid.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{CELL, MAP, RADIUS};

// ---------------- resource kinds ----------------
pub const NRES: usize = 8;
pub const R_SCRAP: usize = 0;
pub const R_ALLOY: usize = 1;
pub const R_CRYSTAL: usize = 2;
pub const R_COOLANT: usize = 3;
pub const R_CIRCUIT: usize = 4;
pub const R_RELIC: usize = 5;
pub const R_CORE: usize = 6;
pub const R_LPART: usize = 7;
pub const RES_NAMES: [&str; NRES] = [
    "Scrap", "Alloy", "Crystal", "Coolant", "Circuit", "Relic", "DrnCore", "L.Part",
];

pub fn res_color(i: usize) -> Color {
    match i {
        R_SCRAP => Color::srgb(0.75, 0.75, 0.7),
        R_ALLOY => Color::srgb(0.55, 0.75, 0.95),
        R_CRYSTAL => Color::srgb(0.4, 0.6, 1.0),
        R_COOLANT => Color::srgb(0.3, 0.95, 0.85),
        R_CIRCUIT => Color::srgb(0.4, 1.0, 0.4),
        R_RELIC => Color::srgb(0.85, 0.5, 1.0),
        R_CORE => Color::srgb(1.0, 0.85, 0.3),
        _ => Color::srgb(1.0, 0.55, 0.2),
    }
}

// ---------------- curved surface math ----------------
pub fn surf_to_world(u: f32, v: f32, h: f32) -> Vec3 {
    let th = u / RADIUS;
    let r = RADIUS - h;
    Vec3::new(th.sin() * r, RADIUS - th.cos() * r, v)
}

pub fn surf_normal(u: f32) -> Vec3 {
    let th = u / RADIUS;
    Vec3::new(-th.sin(), th.cos(), 0.0)
}

/// Rotation that aligns local +Y with the surface normal at u, then yaws about it.
pub fn surf_quat(u: f32, yaw: f32) -> Quat {
    Quat::from_rotation_z(u / RADIUS) * Quat::from_rotation_y(yaw)
}

pub fn surf_tf(u: f32, v: f32, h: f32, yaw: f32) -> Transform {
    Transform::from_translation(surf_to_world(u, v, h)).with_rotation(surf_quat(u, yaw))
}

pub fn half_map() -> f32 {
    MAP as f32 * CELL * 0.5
}

pub fn cell_of(u: f32, v: f32) -> (i32, i32) {
    (
        ((u + half_map()) / CELL).floor() as i32,
        ((v + half_map()) / CELL).floor() as i32,
    )
}

pub fn cell_center(c: (i32, i32)) -> (f32, f32) {
    (
        (c.0 as f32 + 0.5) * CELL - half_map(),
        (c.1 as f32 + 0.5) * CELL - half_map(),
    )
}

pub fn clamp_uv(u: f32, v: f32) -> (f32, f32) {
    let m = half_map() - 0.6;
    (u.clamp(-m, m), v.clamp(-m, m))
}

// ---------------- core components ----------------
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct SurfPos {
    pub u: f32,
    pub v: f32,
    pub h: f32,
}
impl SurfPos {
    pub fn new(u: f32, v: f32, h: f32) -> Self {
        Self { u, v, h }
    }
    pub fn cell(&self) -> (i32, i32) {
        cell_of(self.u, self.v)
    }
    pub fn world(&self) -> Vec3 {
        surf_to_world(self.u, self.v, self.h)
    }
    pub fn dist(&self, o: &SurfPos) -> f32 {
        Vec2::new(self.u - o.u, self.v - o.v).length()
    }
    pub fn dist_uv(&self, u: f32, v: f32) -> f32 {
        Vec2::new(self.u - u, self.v - v).length()
    }
}

/// Velocity in surface coordinates: x = du, y = dh, z = dv.
#[derive(Component, Default)]
pub struct SurfVel(pub Vec3);

#[derive(Component)]
pub struct Yaw(pub f32);

#[derive(Component)]
pub struct Health {
    pub hp: f32,
    pub max: f32,
}
impl Health {
    pub fn new(v: f32) -> Self {
        Self { hp: v, max: v }
    }
}

/// Shield points granted by null priests / shield projectors. `fed` is a grace
/// timer; when the source dies the shield decays.
#[derive(Component)]
pub struct ShieldC {
    pub hp: f32,
    pub max: f32,
    pub fed: f32,
}

#[derive(Component, Default)]
pub struct StatusFx {
    pub burn: f32,
    pub slow: f32,
    pub shock: f32,
    pub corrupt: f32,
}

#[derive(Component)]
pub struct Armor(pub f32);

/// Everything that should be despawned on new-game / load.
#[derive(Component)]
pub struct GameEntity;

#[derive(Component)]
pub struct PlayerTag;
#[derive(Component)]
pub struct SunTag;
#[derive(Component)]
pub struct CamTag;
#[derive(Component)]
pub struct RallyMarker;

#[derive(Component)]
pub struct Nest {
    pub level: u32,
    pub cell: (i32, i32),
}

// ---------------- combat-related shared components ----------------
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum DmgSrc {
    PKin,
    PArc,
    PHam,
    Turret,
    Mortar,
    Wall,
    Drone,
    Enemy,
    Hazard,
}
impl DmgSrc {
    pub fn idx(self) -> usize {
        match self {
            DmgSrc::PKin => 0,
            DmgSrc::PArc => 1,
            DmgSrc::PHam => 2,
            DmgSrc::Turret => 3,
            DmgSrc::Mortar => 4,
            DmgSrc::Wall => 5,
            DmgSrc::Drone => 6,
            DmgSrc::Enemy => 7,
            DmgSrc::Hazard => 7,
        }
    }
}

#[derive(Component)]
pub struct Projectile {
    pub dir: Vec2,
    pub speed: f32,
    pub dmg: f32,
    pub life: f32,
    pub friendly: bool,
    pub aoe: f32,
    pub knock: f32,
    pub src: DmgSrc,
    pub burn: bool,
}

#[derive(Component)]
pub struct MortarShell {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub t: f32,
    pub dur: f32,
    pub dmg: f32,
    pub radius: f32,
    pub src: DmgSrc,
}

#[derive(Component)]
pub struct BeamVis {
    pub life: f32,
}

#[derive(Component)]
pub struct Particle {
    pub vel: Vec3,
    pub life: f32,
    pub grav: f32,
}

#[derive(Component)]
pub struct FlashLight {
    pub t: f32,
}

#[derive(Component)]
pub struct FloatDmg {
    pub world: Vec3,
    pub t: f32,
}

// ---------------- logistics ----------------
#[derive(Component)]
pub struct Packet {
    pub res: usize,
    pub amount: f32,
    pub path: Vec<(i32, i32)>,
    pub idx: usize,
    pub t: f32,
    pub stall: f32,
}

// ---------------- events ----------------
#[derive(Event)]
pub struct DmgEvent {
    pub target: Entity,
    pub dmg: f32,
    pub src: DmgSrc,
    pub burn: bool,
    pub shock: bool,
    pub slow: bool,
    pub knock: Vec2,
}
impl DmgEvent {
    pub fn new(target: Entity, dmg: f32, src: DmgSrc) -> Self {
        Self {
            target,
            dmg,
            src,
            burn: false,
            shock: false,
            slow: false,
            knock: Vec2::ZERO,
        }
    }
}

#[derive(Event)]
pub struct ExplodeEvent {
    pub u: f32,
    pub v: f32,
    pub radius: f32,
    pub dmg: f32,
    pub src: DmgSrc,
    /// Friendly explosions also damage own buildings and the player.
    pub friendly: bool,
}

#[derive(Event)]
pub struct Notify(pub String);

#[derive(Event)]
pub struct DoSave;
#[derive(Event)]
pub struct DoLoad;
#[derive(Event)]
pub struct NewGame;

// ---------------- transform sync ----------------
pub fn sync_surf_transforms(
    mut q: Query<(&SurfPos, Option<&Yaw>, &mut Transform), Or<(Changed<SurfPos>, Changed<Yaw>)>>,
) {
    for (sp, yaw, mut tf) in q.iter_mut() {
        let s = tf.scale;
        *tf = surf_tf(sp.u, sp.v, sp.h, yaw.map_or(0.0, |y| y.0));
        tf.scale = s;
    }
}
