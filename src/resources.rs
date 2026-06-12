//! Global resources: terrain grid, bank, clock, RNG, shared mesh/material
//! libraries, spatial hash and misc game-wide state.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::components::*;
use crate::{AppState, MAP, TILE, TILES};

// ---------------- terrain ----------------
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Terrain {
    Hull,
    Regolith,
    Crystal,
    Marsh,
    Ruin,
    Glass,
    Growth,
}
impl Terrain {
    pub fn name(self) -> &'static str {
        match self {
            Terrain::Hull => "Hull Plate",
            Terrain::Regolith => "Regolith Dust",
            Terrain::Crystal => "Crystal Scar",
            Terrain::Marsh => "Coolant Marsh",
            Terrain::Ruin => "Black-Metal Ruin",
            Terrain::Glass => "Reactor Glass",
            Terrain::Growth => "Corrupted Growth",
        }
    }
    pub fn to_u8(self) -> u8 {
        match self {
            Terrain::Hull => 0,
            Terrain::Regolith => 1,
            Terrain::Crystal => 2,
            Terrain::Marsh => 3,
            Terrain::Ruin => 4,
            Terrain::Glass => 5,
            Terrain::Growth => 6,
        }
    }
    pub fn from_u8(b: u8) -> Self {
        match b {
            1 => Terrain::Regolith,
            2 => Terrain::Crystal,
            3 => Terrain::Marsh,
            4 => Terrain::Ruin,
            5 => Terrain::Glass,
            6 => Terrain::Growth,
            _ => Terrain::Hull,
        }
    }
}

#[derive(Clone)]
pub struct Cell {
    pub terrain: Terrain,
    pub elev: f32,
    pub res: f32,
    pub res_kind: usize,
    pub corrupt: f32,
    pub disc: bool,
    pub walk: bool,
    pub occ: Option<Entity>,
    /// Cell carries a conveyor/sorter (units may still walk over it).
    pub conv: bool,
}
impl Default for Cell {
    fn default() -> Self {
        Self {
            terrain: Terrain::Hull,
            elev: 0.0,
            res: 0.0,
            res_kind: R_SCRAP,
            corrupt: 0.0,
            disc: false,
            walk: true,
            occ: None,
            conv: false,
        }
    }
}

#[derive(Resource)]
pub struct Grid {
    pub seed: u64,
    pub cells: Vec<Cell>,
    pub dirty: Vec<bool>,
    pub flow: Vec<u32>,
    pub flow_dirty: bool,
    pub power_dirty: bool,
    pub nests_gen: Vec<(i32, i32)>,
}
impl Grid {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            cells: vec![Cell::default(); (MAP * MAP) as usize],
            dirty: vec![true; (TILES * TILES) as usize],
            flow: vec![u32::MAX; (MAP * MAP) as usize],
            flow_dirty: true,
            power_dirty: true,
            nests_gen: Vec::new(),
        }
    }
    pub fn inb(c: (i32, i32)) -> bool {
        c.0 >= 0 && c.1 >= 0 && c.0 < MAP && c.1 < MAP
    }
    pub fn idx(c: (i32, i32)) -> usize {
        (c.1 * MAP + c.0) as usize
    }
    pub fn at(&self, c: (i32, i32)) -> &Cell {
        &self.cells[Self::idx(c)]
    }
    pub fn at_mut(&mut self, c: (i32, i32)) -> &mut Cell {
        &mut self.cells[Self::idx(c)]
    }
    pub fn mark(&mut self, c: (i32, i32)) {
        if Self::inb(c) {
            let t = ((c.1 / TILE) * TILES + c.0 / TILE) as usize;
            self.dirty[t] = true;
        }
    }
    pub fn mark_all(&mut self) {
        self.dirty.iter_mut().for_each(|d| *d = true);
    }
    pub fn walkable(&self, c: (i32, i32)) -> bool {
        if !Self::inb(c) {
            return false;
        }
        let cell = self.at(c);
        cell.walk && (cell.occ.is_none() || cell.conv)
    }
    pub fn elev_uv(&self, u: f32, v: f32) -> f32 {
        let c = cell_of(u, v);
        if Self::inb(c) {
            self.at(c).elev
        } else {
            0.0
        }
    }
}

// ---------------- deterministic RNG / noise ----------------
#[derive(Clone, Serialize, Deserialize)]
pub struct Rng(pub u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn f32(&mut self) -> f32 {
        (self.next() >> 40) as f32 / 16_777_216.0
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + self.f32() * (b - a)
    }
    pub fn below(&mut self, n: i32) -> i32 {
        if n <= 0 {
            0
        } else {
            (self.next() % n as u64) as i32
        }
    }
}

#[derive(Resource)]
pub struct RngRes(pub Rng);

pub fn hash2(x: i32, y: i32, seed: u64) -> f32 {
    let mut h = seed
        ^ (x as i64 as u64).wrapping_mul(0x8DA6_B343_9D31_07A1)
        ^ (y as i64 as u64).wrapping_mul(0xD816_3841_F0E5_2BB7);
    h = h.wrapping_mul(0x2545_F491_4F6C_DD1D);
    h ^= h >> 29;
    h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 32;
    ((h >> 40) as f32 / 16_777_216.0) * 2.0 - 1.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

pub fn vnoise(x: f32, y: f32, seed: u64) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let fx = smooth(x - xi as f32);
    let fy = smooth(y - yi as f32);
    let a = hash2(xi, yi, seed);
    let b = hash2(xi + 1, yi, seed);
    let c = hash2(xi, yi + 1, seed);
    let d = hash2(xi + 1, yi + 1, seed);
    a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy
}

pub fn fbm(x: f32, y: f32, seed: u64) -> f32 {
    0.55 * vnoise(x, y, seed) + 0.3 * vnoise(x * 2.1, y * 2.1, seed ^ 77) + 0.15 * vnoise(x * 4.3, y * 4.3, seed ^ 991)
}

// ---------------- economy ----------------
#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct Bank {
    pub amt: [f32; NRES],
    pub cap: [f32; NRES],
    pub lifetime: [f32; NRES],
}
impl Default for Bank {
    fn default() -> Self {
        Self {
            amt: [80.0, 20.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            cap: [300.0; NRES],
            lifetime: [0.0; NRES],
        }
    }
}
impl Bank {
    /// Returns the amount actually accepted.
    pub fn add(&mut self, kind: usize, amount: f32) -> f32 {
        let space = (self.cap[kind] - self.amt[kind]).max(0.0);
        let got = amount.min(space);
        self.amt[kind] += got;
        self.lifetime[kind] += got;
        got
    }
    pub fn take(&mut self, kind: usize, amount: f32) -> f32 {
        let got = amount.min(self.amt[kind]);
        self.amt[kind] -= got;
        got
    }
    pub fn can_afford(&self, cost: &[(usize, f32)]) -> bool {
        cost.iter().all(|(k, a)| self.amt[*k] >= *a)
    }
    pub fn pay(&mut self, cost: &[(usize, f32)]) -> bool {
        if !self.can_afford(cost) {
            return false;
        }
        for (k, a) in cost {
            self.amt[*k] -= a;
        }
        true
    }
}

// ---------------- clock / phases ----------------
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Phase {
    Dawn,
    Day,
    Dusk,
    Night,
}
impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Dawn => "DAWN",
            Phase::Day => "DAY",
            Phase::Dusk => "DUSK",
            Phase::Night => "NIGHT ASSAULT",
        }
    }
    pub fn to_u8(self) -> u8 {
        match self {
            Phase::Dawn => 0,
            Phase::Day => 1,
            Phase::Dusk => 2,
            Phase::Night => 3,
        }
    }
    pub fn from_u8(b: u8) -> Self {
        match b {
            1 => Phase::Day,
            2 => Phase::Dusk,
            3 => Phase::Night,
            _ => Phase::Dawn,
        }
    }
}

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct GameClock {
    pub day: u32,
    pub phase: Phase,
    pub t: f32,
}
impl Default for GameClock {
    fn default() -> Self {
        Self {
            day: 1,
            phase: Phase::Dawn,
            t: 0.0,
        }
    }
}
impl GameClock {
    pub fn phase_len(&self) -> f32 {
        match self.phase {
            Phase::Dawn => 22.0,
            Phase::Day => 115.0,
            Phase::Dusk => 18.0,
            Phase::Night => (75.0 + self.day as f32 * 6.0).min(140.0),
        }
    }
    /// 0..1 across the whole cycle, used for the sun angle.
    pub fn cycle01(&self) -> f32 {
        let p = (self.t / self.phase_len()).clamp(0.0, 1.0);
        match self.phase {
            Phase::Dawn => p * 0.12,
            Phase::Day => 0.12 + p * 0.58,
            Phase::Dusk => 0.70 + p * 0.10,
            Phase::Night => 0.80 + p * 0.20,
        }
    }
}

// ---------------- player ----------------
#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct PStats {
    pub hp: f32,
    pub max_hp: f32,
    pub armor: f32,
    pub stam: f32,
    pub max_stam: f32,
    pub heat: f32,
    pub oxy: f32,
    pub max_oxy: f32,
    pub carry_kind: usize,
    pub carry_amt: f32,
    pub carry_cap: f32,
    pub bandwidth: u32,
    pub dash_cd: f32,
    pub jet_cd: f32,
}
impl Default for PStats {
    fn default() -> Self {
        Self {
            hp: 100.0,
            max_hp: 100.0,
            armor: 2.0,
            stam: 100.0,
            max_stam: 100.0,
            heat: 0.0,
            oxy: 100.0,
            max_oxy: 100.0,
            carry_kind: R_SCRAP,
            carry_amt: 0.0,
            carry_cap: 24.0,
            bandwidth: 6,
            dash_cd: 0.0,
            jet_cd: 0.0,
        }
    }
}

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct WeaponState {
    pub cur: usize,
    pub ammo: i32,
    pub mag: i32,
    pub reload: f32,
    pub cd: f32,
}
impl Default for WeaponState {
    fn default() -> Self {
        Self {
            cur: 0,
            ammo: 24,
            mag: 24,
            reload: 0.0,
            cd: 0.0,
        }
    }
}
pub const WEAPON_NAMES: [&str; 3] = ["Kinetic Carbine", "Arc Lance", "Grav Hammer"];

// ---------------- camera ----------------
#[derive(Resource)]
pub struct CamCtl {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub orbited: bool,
}
impl Default for CamCtl {
    fn default() -> Self {
        Self {
            yaw: 0.6,
            pitch: 0.7,
            dist: 18.0,
            orbited: false,
        }
    }
}

// ---------------- UI state ----------------
#[derive(Resource, Default)]
pub struct UiState {
    pub build_open: bool,
    pub tactical: bool,
    pub map_big: bool,
    pub research_open: bool,
    pub trade_open: bool,
    pub sel: Option<Entity>,
    pub notes: Vec<(String, f32)>,
}

#[derive(Resource, Default)]
pub struct EndMsg(pub String);

// ---------------- misc sim state ----------------
#[derive(Resource, Default)]
pub struct SimStats {
    pub packets_delivered: u32,
    pub outage: f32,
    pub conveyor_dmg: f32,
    pub pylon_dmg: f32,
    pub drone_deaths: f32,
    pub relic_acc: f32,
    pub active_jobs: u32,
}

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct LaunchState {
    pub built: u32,
    pub countdown: Option<f32>,
}
impl Default for LaunchState {
    fn default() -> Self {
        Self {
            built: 0,
            countdown: None,
        }
    }
}
pub const LAUNCH_SEGMENTS: u32 = 6;

#[derive(Resource)]
pub struct Tutorial {
    pub step: usize,
    pub done: bool,
}
impl Default for Tutorial {
    fn default() -> Self {
        Self {
            step: 0,
            done: false,
        }
    }
}

#[derive(Resource, Default)]
pub struct RallyPoint(pub Option<(f32, f32)>);

/// Per-frame spatial hash of enemies for radius queries.
#[derive(Resource, Default)]
pub struct EGrid {
    pub map: HashMap<(i32, i32), Vec<(Entity, f32, f32)>>,
}
pub const BUCKET: f32 = 8.0;
impl EGrid {
    pub fn clear(&mut self) {
        self.map.clear();
    }
    pub fn insert(&mut self, e: Entity, u: f32, v: f32) {
        let k = ((u / BUCKET).floor() as i32, (v / BUCKET).floor() as i32);
        self.map.entry(k).or_default().push((e, u, v));
    }
    pub fn near(&self, u: f32, v: f32, r: f32) -> Vec<(Entity, f32, f32)> {
        let mut out = Vec::new();
        let b0 = (((u - r) / BUCKET).floor() as i32, ((v - r) / BUCKET).floor() as i32);
        let b1 = (((u + r) / BUCKET).floor() as i32, ((v + r) / BUCKET).floor() as i32);
        for bx in b0.0..=b1.0 {
            for by in b0.1..=b1.1 {
                if let Some(list) = self.map.get(&(bx, by)) {
                    for (e, eu, ev) in list {
                        if Vec2::new(eu - u, ev - v).length_squared() <= r * r {
                            out.push((*e, *eu, *ev));
                        }
                    }
                }
            }
        }
        out
    }
}

#[derive(Resource, Default)]
pub struct PowerInfo {
    /// (production, demand, satisfaction) per network.
    pub nets: Vec<(f32, f32, f32)>,
}

// ---------------- shared mesh / material libraries ----------------
#[derive(Clone, Copy)]
pub enum Mid {
    Cube,
    CubeS,
    Sph,
    Cyl,
    Cone,
    Torus,
    Cap,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Cid {
    Dark,
    Steel,
    Grey,
    Cyan,
    Orange,
    Red,
    Green,
    Purple,
    Yellow,
    White,
    Crystal,
    Bone,
    Black,
}

#[derive(Resource)]
pub struct Libs {
    pub cube: Handle<Mesh>,
    pub cube_s: Handle<Mesh>,
    pub sph: Handle<Mesh>,
    pub cyl: Handle<Mesh>,
    pub cone: Handle<Mesh>,
    pub torus: Handle<Mesh>,
    pub cap: Handle<Mesh>,
    pub mats: Vec<Handle<StandardMaterial>>,
    pub terrain_mat: Handle<StandardMaterial>,
    pub ghost_ok: Handle<StandardMaterial>,
    pub ghost_bad: Handle<StandardMaterial>,
    pub res_mats: Vec<Handle<StandardMaterial>>,
}
impl Libs {
    pub fn mesh(&self, m: Mid) -> Handle<Mesh> {
        match m {
            Mid::Cube => self.cube.clone(),
            Mid::CubeS => self.cube_s.clone(),
            Mid::Sph => self.sph.clone(),
            Mid::Cyl => self.cyl.clone(),
            Mid::Cone => self.cone.clone(),
            Mid::Torus => self.torus.clone(),
            Mid::Cap => self.cap.clone(),
        }
    }
    pub fn mat(&self, c: Cid) -> Handle<StandardMaterial> {
        self.mats[c as usize].clone()
    }
}

#[derive(Resource)]
pub struct TileHandles(pub Vec<Handle<Mesh>>);

#[derive(Resource)]
pub struct MinimapImg(pub Handle<Image>);

fn setup_libs(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let mut mat = |c: Color, e: f32, rough: f32| -> Handle<StandardMaterial> {
        let lin = c.to_linear();
        materials.add(StandardMaterial {
            base_color: c,
            emissive: LinearRgba::rgb(lin.red * e, lin.green * e, lin.blue * e),
            perceptual_roughness: rough,
            metallic: 0.4,
            ..default()
        })
    };
    // Order must match Cid.
    let mats = vec![
        mat(Color::srgb(0.10, 0.11, 0.14), 0.0, 0.9),  // Dark
        mat(Color::srgb(0.35, 0.38, 0.45), 0.0, 0.6),  // Steel
        mat(Color::srgb(0.22, 0.24, 0.28), 0.0, 0.8),  // Grey
        mat(Color::srgb(0.2, 0.9, 1.0), 3.0, 0.4),     // Cyan
        mat(Color::srgb(1.0, 0.55, 0.15), 3.0, 0.4),   // Orange
        mat(Color::srgb(1.0, 0.2, 0.2), 3.5, 0.4),     // Red
        mat(Color::srgb(0.3, 1.0, 0.4), 3.0, 0.4),     // Green
        mat(Color::srgb(0.75, 0.3, 1.0), 3.0, 0.4),    // Purple
        mat(Color::srgb(1.0, 0.9, 0.25), 2.5, 0.4),    // Yellow
        mat(Color::srgb(0.95, 0.95, 1.0), 1.5, 0.3),   // White
        mat(Color::srgb(0.45, 0.65, 1.0), 1.8, 0.2),   // Crystal
        mat(Color::srgb(0.8, 0.78, 0.7), 0.0, 0.7),    // Bone
        mat(Color::srgb(0.04, 0.04, 0.06), 0.0, 0.95), // Black
    ];

    let terrain_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        metallic: 0.05,
        ..default()
    });
    let ghost_ok = materials.add(StandardMaterial {
        base_color: Color::srgba(0.2, 1.0, 0.4, 0.35),
        emissive: LinearRgba::rgb(0.05, 0.4, 0.1),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let ghost_bad = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.2, 0.2, 0.35),
        emissive: LinearRgba::rgb(0.4, 0.05, 0.05),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let res_mats = (0..NRES)
        .map(|i| {
            let c = res_color(i).to_linear();
            materials.add(StandardMaterial {
                base_color: res_color(i),
                emissive: LinearRgba::rgb(c.red * 2.5, c.green * 2.5, c.blue * 2.5),
                ..default()
            })
        })
        .collect();

    let img = Image::new_fill(
        Extent3d {
            width: MAP as u32,
            height: MAP as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[8, 8, 12, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    commands.insert_resource(MinimapImg(images.add(img)));

    commands.insert_resource(Libs {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cube_s: meshes.add(Cuboid::new(0.32, 0.32, 0.32)),
        sph: meshes.add(Sphere::new(0.5)),
        cyl: meshes.add(Cylinder::new(0.5, 1.0)),
        cone: meshes.add(Cone {
            radius: 0.5,
            height: 1.0,
        }),
        torus: meshes.add(Torus::new(0.18, 0.55)),
        cap: meshes.add(Capsule3d::new(0.4, 0.7)),
        mats,
        terrain_mat,
        ghost_ok,
        ghost_bad,
        res_mats,
    });
}

fn kickoff(mut ev: EventWriter<NewGame>) {
    ev.write(NewGame);
}

pub fn playing(state: Res<State<AppState>>) -> bool {
    *state.get() == AppState::Playing
}

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Grid::new(1))
            .insert_resource(RngRes(Rng::new(1)))
            .init_resource::<Bank>()
            .init_resource::<GameClock>()
            .init_resource::<PStats>()
            .init_resource::<WeaponState>()
            .init_resource::<CamCtl>()
            .init_resource::<UiState>()
            .init_resource::<SimStats>()
            .init_resource::<LaunchState>()
            .init_resource::<Tutorial>()
            .init_resource::<RallyPoint>()
            .init_resource::<EGrid>()
            .init_resource::<PowerInfo>()
            .init_resource::<EndMsg>()
            .add_event::<DmgEvent>()
            .add_event::<ExplodeEvent>()
            .add_event::<Notify>()
            .add_event::<DoSave>()
            .add_event::<DoLoad>()
            .add_event::<NewGame>()
            .add_systems(Startup, (setup_libs, kickoff).chain())
            .add_systems(
                PostUpdate,
                sync_surf_transforms.before(bevy::transform::TransformSystem::TransformPropagate),
            );
    }
}
