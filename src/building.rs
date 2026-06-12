//! Building definitions, placement (ghost preview, validity, rotation, cost),
//! construction progress, repair spires, shield projectors.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde::{Deserialize, Serialize};

use crate::camera::cursor_uv;
use crate::components::*;
use crate::research::TechFx;
use crate::resources::*;
use crate::ui::UiHover;
use crate::CELL;

// ---------------- kinds & defs ----------------
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum BKind {
    Core,
    Wall,
    Gate,
    Pylon,
    Solar,
    Fusion,
    Extractor,
    Bore,
    Pump,
    Silo,
    Conveyor,
    Sorter,
    Smelter,
    Loom,
    Cradle,
    Spire,
    Shield,
    Rail,
    Arc,
    Mortar,
    Radar,
    Reliquary,
    Launch,
}

pub const ALL_KINDS: [BKind; 23] = [
    BKind::Core,
    BKind::Wall,
    BKind::Gate,
    BKind::Pylon,
    BKind::Solar,
    BKind::Fusion,
    BKind::Extractor,
    BKind::Bore,
    BKind::Pump,
    BKind::Silo,
    BKind::Conveyor,
    BKind::Sorter,
    BKind::Smelter,
    BKind::Loom,
    BKind::Cradle,
    BKind::Spire,
    BKind::Shield,
    BKind::Rail,
    BKind::Arc,
    BKind::Mortar,
    BKind::Radar,
    BKind::Reliquary,
    BKind::Launch,
];

pub struct BDef {
    pub name: &'static str,
    pub cost: &'static [(usize, f32)],
    pub hp: f32,
    /// positive = generation, negative = consumption
    pub power: f32,
    pub btime: f32,
    pub foot: i32,
    pub desc: &'static str,
}

pub fn bdef(k: BKind) -> BDef {
    let d = |name, cost, hp, power, btime, foot, desc| BDef {
        name,
        cost,
        hp,
        power,
        btime,
        foot,
        desc,
    };
    match k {
        BKind::Core => d("Core Obelisk", &[], 420.0, 6.0, 1.0, 1, "Heart of the colony. Lose it = game over. Provides power + oxygen."),
        BKind::Wall => d("Hull Wall", &[(R_SCRAP, 4.0)], 170.0, 0.0, 3.0, 1, "Cheap barrier. Thorned plating wounds melee attackers."),
        BKind::Gate => d("Blast Gate", &[(R_SCRAP, 6.0), (R_ALLOY, 2.0)], 240.0, 0.0, 4.0, 1, "Tough wall segment for chokepoints."),
        BKind::Pylon => d("Power Pylon", &[(R_SCRAP, 8.0)], 60.0, 0.0, 3.0, 1, "Links buildings into a power network."),
        BKind::Solar => d("Solar Sail", &[(R_SCRAP, 12.0), (R_ALLOY, 2.0)], 70.0, 8.0, 5.0, 1, "Generates power during daylight."),
        BKind::Fusion => d("Fusion Kettle", &[(R_ALLOY, 18.0), (R_COOLANT, 6.0)], 150.0, 26.0, 10.0, 1, "Strong generator. Sips coolant from storage."),
        BKind::Extractor => d("Scrap Extractor", &[(R_SCRAP, 10.0)], 90.0, -3.0, 5.0, 1, "Mines scrap. Place on a scrap deposit."),
        BKind::Bore => d("Crystal Bore", &[(R_SCRAP, 14.0), (R_ALLOY, 4.0)], 90.0, -4.0, 6.0, 1, "Mines crystal. Place on a crystal scar."),
        BKind::Pump => d("Coolant Pump", &[(R_SCRAP, 12.0), (R_ALLOY, 2.0)], 80.0, -3.0, 5.0, 1, "Pumps coolant. Place on coolant marsh."),
        BKind::Silo => d("Storage Silo", &[(R_SCRAP, 16.0)], 120.0, 0.0, 5.0, 1, "Raises storage caps; accepts conveyor packets."),
        BKind::Conveyor => d("Conveyor Node", &[(R_SCRAP, 2.0)], 35.0, 0.0, 1.2, 1, "Moves item packets between adjacent nodes."),
        BKind::Sorter => d("Sorter Node", &[(R_SCRAP, 4.0), (R_ALLOY, 1.0)], 40.0, -1.0, 2.0, 1, "Conveyor that only passes its filter type (E to cycle)."),
        BKind::Smelter => d("Smelter", &[(R_SCRAP, 20.0)], 110.0, -5.0, 7.0, 1, "2 scrap -> 1 alloy."),
        BKind::Loom => d("Circuit Loom", &[(R_ALLOY, 12.0), (R_CRYSTAL, 6.0)], 100.0, -5.0, 8.0, 1, "Alloy+crystal -> circuits. E toggles launch-part recipe."),
        BKind::Cradle => d("Drone Cradle", &[(R_ALLOY, 10.0), (R_CIRCUIT, 2.0)], 100.0, -3.0, 8.0, 1, "Builds worker drones (1 drone core + 4 alloy each)."),
        BKind::Spire => d("Repair Spire", &[(R_ALLOY, 8.0), (R_CIRCUIT, 2.0)], 90.0, -4.0, 6.0, 1, "Auto-repairs nearby buildings using scrap."),
        BKind::Shield => d("Shield Projector", &[(R_ALLOY, 12.0), (R_CIRCUIT, 4.0)], 110.0, -6.0, 8.0, 1, "Projects shields over nearby buildings."),
        BKind::Rail => d("Rail Turret", &[(R_SCRAP, 14.0), (R_ALLOY, 6.0)], 130.0, -4.0, 6.0, 1, "Long range kinetic turret."),
        BKind::Arc => d("Arc Turret", &[(R_ALLOY, 10.0), (R_CRYSTAL, 4.0)], 110.0, -5.0, 6.0, 1, "Short range chaining shock turret."),
        BKind::Mortar => d("Flak Mortar", &[(R_ALLOY, 14.0), (R_CIRCUIT, 3.0)], 120.0, -5.0, 8.0, 1, "Area bombardment. Friendly fire!"),
        BKind::Radar => d("Radar Mast", &[(R_SCRAP, 10.0), (R_CIRCUIT, 1.0)], 70.0, -2.0, 5.0, 1, "Reveals fog of war around it."),
        BKind::Reliquary => d("Research Reliquary", &[(R_ALLOY, 10.0), (R_CRYSTAL, 8.0), (R_RELIC, 4.0)], 120.0, -4.0, 9.0, 1, "Unlocks the research tree (E or T to open)."),
        BKind::Launch => d("Launch Engine Segment", &[(R_ALLOY, 25.0), (R_CIRCUIT, 10.0), (R_LPART, 4.0)], 320.0, -8.0, 20.0, 2, "Build 6 segments to escape the necropolis."),
    }
}

#[derive(Component)]
pub struct Building {
    pub kind: BKind,
    pub cell: (i32, i32),
    pub built: f32,
    pub powered: f32,
    pub buf_in: [f32; NRES],
    pub buf_out: [f32; NRES],
    pub craft: f32,
    pub cd: f32,
    /// Sorter filter (NRES = pass anything).
    pub filter: usize,
    /// Loom recipe: 0 = circuits, 1 = launch parts.
    pub recipe: u8,
    pub disabled: f32,
    pub jam: bool,
    pub missing: bool,
}

#[derive(Resource, Default)]
pub struct BuildSel {
    pub kind: Option<BKind>,
    pub rot: u8,
}

#[derive(Component)]
pub struct GhostPart;
#[derive(Resource, Default)]
pub struct GhostEnt(pub Option<Entity>, pub Option<BKind>);

// ---------------- procedural silhouettes ----------------
type Part = (Mid, Cid, [f32; 3], [f32; 3], f32);

pub fn parts(kind: BKind) -> Vec<Part> {
    match kind {
        BKind::Core => vec![
            (Mid::Cyl, Cid::Dark, [0.0, 0.4, 0.0], [1.9, 0.8, 1.9], 0.0),
            (Mid::Cube, Cid::Grey, [0.0, 2.0, 0.0], [0.9, 3.2, 0.9], 0.0),
            (Mid::Sph, Cid::Cyan, [0.0, 3.9, 0.0], [1.0, 1.0, 1.0], 0.0),
        ],
        BKind::Wall => vec![
            (Mid::Cube, Cid::Grey, [0.0, 1.1, 0.0], [1.85, 2.2, 1.85], 0.0),
            (Mid::Cube, Cid::Cyan, [0.0, 2.0, 0.0], [1.9, 0.12, 1.9], 0.0),
        ],
        BKind::Gate => vec![
            (Mid::Cube, Cid::Dark, [-0.7, 1.3, 0.0], [0.5, 2.6, 1.85], 0.0),
            (Mid::Cube, Cid::Dark, [0.7, 1.3, 0.0], [0.5, 2.6, 1.85], 0.0),
            (Mid::Cube, Cid::Orange, [0.0, 1.3, 0.0], [1.0, 1.8, 1.4], 0.0),
        ],
        BKind::Pylon => vec![
            (Mid::Cyl, Cid::Steel, [0.0, 1.4, 0.0], [0.22, 2.8, 0.22], 0.0),
            (Mid::Sph, Cid::Cyan, [0.0, 2.9, 0.0], [0.55, 0.55, 0.55], 0.0),
        ],
        BKind::Solar => vec![
            (Mid::Cyl, Cid::Steel, [0.0, 0.9, 0.0], [0.18, 1.8, 0.18], 0.0),
            (Mid::Cube, Cid::Crystal, [0.0, 1.9, 0.0], [2.3, 0.1, 1.5], 0.25),
        ],
        BKind::Fusion => vec![
            (Mid::Cyl, Cid::Dark, [0.0, 0.9, 0.0], [1.6, 1.8, 1.6], 0.0),
            (Mid::Torus, Cid::Orange, [0.0, 1.9, 0.0], [1.4, 1.4, 1.4], 0.0),
            (Mid::Sph, Cid::Orange, [0.0, 1.9, 0.0], [0.7, 0.7, 0.7], 0.0),
        ],
        BKind::Extractor => vec![
            (Mid::Cube, Cid::Grey, [0.0, 0.5, 0.0], [1.6, 1.0, 1.6], 0.0),
            (Mid::Cone, Cid::Steel, [0.0, 1.5, 0.0], [0.9, 1.2, 0.9], 0.0),
            (Mid::CubeS, Cid::Yellow, [0.6, 1.1, 0.6], [1.0, 1.0, 1.0], 0.0),
        ],
        BKind::Bore => vec![
            (Mid::Cube, Cid::Grey, [0.0, 0.5, 0.0], [1.6, 1.0, 1.6], 0.0),
            (Mid::Sph, Cid::Crystal, [0.0, 1.6, 0.0], [1.1, 1.1, 1.1], 0.0),
        ],
        BKind::Pump => vec![
            (Mid::Cyl, Cid::Steel, [0.0, 0.7, 0.0], [1.1, 1.4, 1.1], 0.0),
            (Mid::Sph, Cid::Green, [0.0, 1.7, 0.0], [0.7, 0.7, 0.7], 0.0),
        ],
        BKind::Silo => vec![
            (Mid::Cyl, Cid::Steel, [0.0, 1.2, 0.0], [1.7, 2.4, 1.7], 0.0),
            (Mid::Sph, Cid::Grey, [0.0, 2.45, 0.0], [1.7, 0.9, 1.7], 0.0),
        ],
        BKind::Conveyor => vec![
            (Mid::Cube, Cid::Dark, [0.0, 0.14, 0.0], [1.85, 0.28, 1.85], 0.0),
            (Mid::CubeS, Cid::Cyan, [0.0, 0.35, 0.0], [0.9, 0.6, 0.9], 0.0),
        ],
        BKind::Sorter => vec![
            (Mid::Cube, Cid::Dark, [0.0, 0.14, 0.0], [1.85, 0.28, 1.85], 0.0),
            (Mid::Cone, Cid::Purple, [0.0, 0.55, 0.0], [0.7, 0.7, 0.7], 0.0),
        ],
        BKind::Smelter => vec![
            (Mid::Cube, Cid::Dark, [0.0, 0.8, 0.0], [1.7, 1.6, 1.7], 0.0),
            (Mid::Cube, Cid::Orange, [0.0, 1.75, 0.0], [1.0, 0.4, 1.0], 0.0),
        ],
        BKind::Loom => vec![
            (Mid::Cube, Cid::Grey, [0.0, 0.7, 0.0], [1.7, 1.4, 1.7], 0.0),
            (Mid::Cyl, Cid::Purple, [0.0, 1.8, 0.0], [0.6, 1.0, 0.6], 0.0),
        ],
        BKind::Cradle => vec![
            (Mid::Cube, Cid::Grey, [0.0, 0.25, 0.0], [1.9, 0.5, 1.9], 0.0),
            (Mid::Torus, Cid::Yellow, [0.0, 0.9, 0.0], [1.3, 1.3, 1.3], 0.0),
        ],
        BKind::Spire => vec![
            (Mid::Cone, Cid::Steel, [0.0, 1.3, 0.0], [0.8, 2.6, 0.8], 0.0),
            (Mid::Sph, Cid::Green, [0.0, 2.8, 0.0], [0.45, 0.45, 0.45], 0.0),
        ],
        BKind::Shield => vec![
            (Mid::Cyl, Cid::Dark, [0.0, 0.6, 0.0], [1.2, 1.2, 1.2], 0.0),
            (Mid::Sph, Cid::Crystal, [0.0, 1.8, 0.0], [1.5, 1.5, 1.5], 0.0),
        ],
        BKind::Rail => vec![
            (Mid::Cube, Cid::Dark, [0.0, 0.5, 0.0], [1.5, 1.0, 1.5], 0.0),
            (Mid::Cyl, Cid::Steel, [0.0, 1.3, 0.6], [0.3, 1.6, 0.3], 1.57),
            (Mid::CubeS, Cid::Red, [0.0, 1.3, 0.0], [1.0, 1.0, 1.0], 0.0),
        ],
        BKind::Arc => vec![
            (Mid::Cube, Cid::Dark, [0.0, 0.4, 0.0], [1.4, 0.8, 1.4], 0.0),
            (Mid::Cube, Cid::Steel, [-0.4, 1.4, 0.0], [0.2, 1.2, 0.2], 0.0),
            (Mid::Cube, Cid::Steel, [0.4, 1.4, 0.0], [0.2, 1.2, 0.2], 0.0),
            (Mid::Sph, Cid::Cyan, [0.0, 1.5, 0.0], [0.5, 0.5, 0.5], 0.0),
        ],
        BKind::Mortar => vec![
            (Mid::Cube, Cid::Dark, [0.0, 0.4, 0.0], [1.8, 0.8, 1.8], 0.0),
            (Mid::Cyl, Cid::Grey, [0.0, 1.3, -0.3], [0.7, 1.6, 0.7], 0.7),
        ],
        BKind::Radar => vec![
            (Mid::Cyl, Cid::Steel, [0.0, 1.2, 0.0], [0.2, 2.4, 0.2], 0.0),
            (Mid::Torus, Cid::Green, [0.0, 2.5, 0.0], [1.0, 1.0, 1.0], 1.57),
        ],
        BKind::Reliquary => vec![
            (Mid::Cube, Cid::Black, [0.0, 0.8, 0.0], [1.5, 1.6, 1.5], 0.0),
            (Mid::Sph, Cid::Purple, [0.0, 2.3, 0.0], [0.7, 0.7, 0.7], 0.0),
            (Mid::Torus, Cid::Purple, [0.0, 2.3, 0.0], [1.2, 1.2, 1.2], 0.0),
        ],
        BKind::Launch => vec![
            (Mid::Cube, Cid::Dark, [0.0, 0.8, 0.0], [3.6, 1.6, 3.6], 0.0),
            (Mid::Cone, Cid::Steel, [0.0, 3.4, 0.0], [2.4, 3.6, 2.4], 0.0),
            (Mid::Cube, Cid::Cyan, [0.0, 1.7, 0.0], [3.7, 0.15, 3.7], 0.0),
            (Mid::Sph, Cid::Orange, [0.0, 0.4, 0.0], [1.2, 1.2, 1.2], 0.0),
        ],
    }
}

fn spawn_parts(child_of: Entity, commands: &mut Commands, libs: &Libs, kind: BKind) {
    commands.entity(child_of).with_children(|p| {
        for (m, c, pos, sc, rx) in parts(kind) {
            p.spawn((
                Mesh3d(libs.mesh(m)),
                MeshMaterial3d(libs.mat(c)),
                Transform::from_translation(Vec3::from_array(pos))
                    .with_scale(Vec3::from_array(sc))
                    .with_rotation(Quat::from_rotation_x(rx)),
            ));
        }
    });
}

// ---------------- placement ----------------
pub fn foot_cells(kind: BKind, cell: (i32, i32)) -> Vec<(i32, i32)> {
    let f = bdef(kind).foot;
    let mut v = Vec::new();
    for dy in 0..f {
        for dx in 0..f {
            v.push((cell.0 + dx, cell.1 + dy));
        }
    }
    v
}

pub fn terrain_ok(grid: &Grid, kind: BKind, cell: (i32, i32)) -> bool {
    let c = grid.at(cell);
    match kind {
        BKind::Extractor => c.res_kind == R_SCRAP && c.res > 0.0 && c.terrain != Terrain::Marsh,
        BKind::Bore => c.terrain == Terrain::Crystal && c.res > 0.0,
        BKind::Pump => c.terrain == Terrain::Marsh,
        _ => true,
    }
}

pub fn check_place(grid: &Grid, bank: &Bank, kind: BKind, cell: (i32, i32)) -> bool {
    let cells = foot_cells(kind, cell);
    cells.iter().all(|c| {
        Grid::inb(*c) && grid.at(*c).walk && grid.at(*c).occ.is_none() && grid.at(*c).disc
    }) && terrain_ok(grid, kind, cell)
        && bank.can_afford(bdef(kind).cost)
}

pub fn spawn_building(
    commands: &mut Commands,
    libs: &Libs,
    grid: &mut Grid,
    kind: BKind,
    cell: (i32, i32),
    built: f32,
    rot: u8,
) -> Entity {
    let def = bdef(kind);
    let cells = foot_cells(kind, cell);
    let elev = cells
        .iter()
        .filter(|c| Grid::inb(**c))
        .map(|c| grid.at(*c).elev)
        .fold(0.0f32, f32::max);
    let (mut u, mut v) = cell_center(cell);
    if def.foot == 2 {
        u += CELL * 0.5;
        v += CELL * 0.5;
    }
    let e = commands
        .spawn((
            Building {
                kind,
                cell,
                built,
                powered: 0.0,
                buf_in: [0.0; NRES],
                buf_out: [0.0; NRES],
                craft: 0.0,
                cd: 0.0,
                filter: NRES,
                recipe: 0,
                disabled: 0.0,
                jam: false,
                missing: false,
            },
            SurfPos::new(u, v, elev),
            Yaw(rot as f32 * std::f32::consts::FRAC_PI_2),
            Health {
                hp: def.hp * built.max(0.25),
                max: def.hp,
            },
            Armor(if matches!(kind, BKind::Wall | BKind::Gate) {
                3.0
            } else {
                0.0
            }),
            StatusFx::default(),
            SurfVel::default(),
            Transform::from_scale(Vec3::new(1.0, 0.25 + 0.75 * built, 1.0)),
            Visibility::default(),
            GameEntity,
            Name::new(def.name),
        ))
        .id();
    spawn_parts(e, commands, libs, kind);
    for c in cells {
        if Grid::inb(c) {
            let cc = grid.at_mut(c);
            cc.occ = Some(e);
            cc.conv = matches!(kind, BKind::Conveyor | BKind::Sorter);
            grid.mark(c);
        }
    }
    grid.flow_dirty = true;
    grid.power_dirty = true;
    e
}

#[allow(clippy::too_many_arguments)]
fn ghost_and_place(
    mut commands: Commands,
    libs: Res<Libs>,
    mut grid: ResMut<Grid>,
    mut bank: ResMut<Bank>,
    mut sel: ResMut<BuildSel>,
    mut ghost: ResMut<GhostEnt>,
    mut ui: ResMut<UiState>,
    hover: Res<UiHover>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    qc: Query<(&Camera, &GlobalTransform), With<CamTag>>,
    qw: Query<&Window, With<PrimaryWindow>>,
    mut qghost: Query<&mut MeshMaterial3d<StandardMaterial>, With<GhostPart>>,
    mut qgtf: Query<(&mut SurfPos, &mut Yaw), With<GhostPart0>>,
    mut notify: EventWriter<Notify>,
    mut last: Local<Option<(i32, i32)>>,
) {
    // rebuild ghost when selection changes
    if ghost.1 != sel.kind {
        if let Some(e) = ghost.0.take() {
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.despawn();
            }
        }
        ghost.1 = sel.kind;
        if let Some(kind) = sel.kind {
            let root = commands
                .spawn((
                    SurfPos::new(0.0, 0.0, 0.0),
                    Yaw(0.0),
                    Transform::default(),
                    Visibility::default(),
                    GhostPart0,
                    Name::new("ghost"),
                ))
                .id();
            commands.entity(root).with_children(|p| {
                for (m, _c, pos, sc, rx) in parts(kind) {
                    p.spawn((
                        Mesh3d(libs.mesh(m)),
                        MeshMaterial3d(libs.ghost_ok.clone()),
                        Transform::from_translation(Vec3::from_array(pos))
                            .with_scale(Vec3::from_array(sc))
                            .with_rotation(Quat::from_rotation_x(rx)),
                        GhostPart,
                    ));
                }
            });
            ghost.0 = Some(root);
        }
    }
    let Some(kind) = sel.kind else { return };

    if keys.just_pressed(KeyCode::KeyR) {
        sel.rot = (sel.rot + 1) % 4;
    }
    if buttons.just_pressed(MouseButton::Right) {
        sel.kind = None;
        return;
    }

    let Some((u, v)) = cursor_uv(&qc, &qw) else {
        return;
    };
    let cell = cell_of(u, v);
    let valid = check_place(&grid, &bank, kind, cell);

    if let Ok((mut sp, mut yaw)) = qgtf.single_mut() {
        let (mut cu, mut cv) = cell_center(cell);
        if bdef(kind).foot == 2 {
            cu += CELL * 0.5;
            cv += CELL * 0.5;
        }
        sp.u = cu;
        sp.v = cv;
        sp.h = if Grid::inb(cell) { grid.at(cell).elev } else { 0.0 };
        yaw.0 = sel.rot as f32 * std::f32::consts::FRAC_PI_2;
    }
    let mat = if valid {
        libs.ghost_ok.clone()
    } else {
        libs.ghost_bad.clone()
    };
    for mut m in qghost.iter_mut() {
        m.0 = mat.clone();
    }

    let drag = kind == BKind::Conveyor && buttons.pressed(MouseButton::Left);
    let click = buttons.just_pressed(MouseButton::Left) || (drag && *last != Some(cell));
    if click && !hover.0 {
        if valid {
            bank.pay(bdef(kind).cost);
            spawn_building(&mut commands, &libs, &mut grid, kind, cell, 0.0, sel.rot);
            *last = Some(cell);
            ui.build_open = false;
        } else {
            notify.write(Notify("Cannot place here".into()));
        }
    }
}

/// Marker for the ghost root (separate from part marker so queries stay simple).
#[derive(Component)]
pub struct GhostPart0;

fn clear_ghost_when_inactive(
    mut commands: Commands,
    sel: Res<BuildSel>,
    mut ghost: ResMut<GhostEnt>,
) {
    if sel.kind.is_none() && ghost.0.is_some() {
        if let Some(e) = ghost.0.take() {
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.despawn();
            }
        }
        ghost.1 = None;
    }
}

// ---------------- construction & support buildings ----------------
fn construction_tick(
    time: Res<Time>,
    mut q: Query<(&mut Building, &mut Health, &mut Transform)>,
    mut notify: EventWriter<Notify>,
) {
    let dt = time.delta_secs();
    for (mut b, mut hp, mut tf) in q.iter_mut() {
        if b.built < 1.0 {
            b.built += dt * 0.30 / bdef(b.kind).btime;
            if b.built >= 1.0 {
                b.built = 1.0;
                hp.hp = hp.max;
                notify.write(Notify(format!("{} online", bdef(b.kind).name)));
            }
            tf.scale.y = 0.25 + 0.75 * b.built.min(1.0);
        }
        if b.disabled > 0.0 {
            b.disabled -= dt;
        }
    }
}

fn spire_tick(
    time: Res<Time>,
    mut acc: Local<f32>,
    mut bank: ResMut<Bank>,
    spires: Query<(&Building, &SurfPos)>,
    mut targets: Query<(&Building, &SurfPos, &mut Health)>,
) {
    *acc += time.delta_secs();
    if *acc < 0.5 {
        return;
    }
    *acc = 0.0;
    let spire_list: Vec<(f32, f32, f32)> = spires
        .iter()
        .filter(|(b, _)| b.kind == BKind::Spire && b.built >= 1.0 && b.powered > 0.2)
        .map(|(b, sp)| (sp.u, sp.v, b.powered))
        .collect();
    for (su, sv, pw) in spire_list {
        let mut best: Option<(f32, Mut<Health>)> = None;
        for (b, sp, hp) in targets.iter_mut() {
            if b.built < 1.0 || hp.hp >= hp.max {
                continue;
            }
            if Vec2::new(sp.u - su, sp.v - sv).length() > 9.0 {
                continue;
            }
            let ratio = hp.hp / hp.max;
            if best.as_ref().map_or(true, |(r, _)| ratio < *r) {
                best = Some((ratio, hp));
            }
        }
        if let Some((_, mut hp)) = best {
            if bank.take(R_SCRAP, 0.5) > 0.0 {
                hp.hp = (hp.hp + 8.0 * pw).min(hp.max);
            }
        }
    }
}

fn shield_proj_tick(
    mut commands: Commands,
    time: Res<Time>,
    mut acc: Local<f32>,
    fx: Res<TechFx>,
    projs: Query<(&Building, &SurfPos)>,
    mut targets: Query<(Entity, &Building, &SurfPos, Option<&mut ShieldC>)>,
) {
    *acc += time.delta_secs();
    if *acc < 1.0 {
        return;
    }
    *acc = 0.0;
    let sources: Vec<(f32, f32)> = projs
        .iter()
        .filter(|(b, _)| b.kind == BKind::Shield && b.built >= 1.0 && b.powered > 0.2)
        .map(|(_, sp)| (sp.u, sp.v))
        .collect();
    if sources.is_empty() {
        return;
    }
    let cap = 40.0 * fx.shield_cap;
    for (e, b, sp, shield) in targets.iter_mut() {
        if b.built < 1.0 || b.kind == BKind::Shield {
            continue;
        }
        let near = sources
            .iter()
            .any(|(u, v)| Vec2::new(sp.u - u, sp.v - v).length() <= 9.0);
        if !near {
            continue;
        }
        match shield {
            Some(mut s) => {
                s.fed = 2.5;
                s.max = cap;
            }
            None => {
                commands.entity(e).insert(ShieldC {
                    hp: 0.0,
                    max: cap,
                    fed: 2.5,
                });
            }
        }
    }
}

fn shield_decay(time: Res<Time>, mut q: Query<&mut ShieldC>) {
    let dt = time.delta_secs();
    for mut s in q.iter_mut() {
        s.fed -= dt;
        if s.fed > 0.0 {
            s.hp = (s.hp + 6.0 * dt).min(s.max);
        } else {
            s.hp = (s.hp - 10.0 * dt).max(0.0);
        }
    }
}

pub struct BuildingPlugin;
impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuildSel>()
            .init_resource::<GhostEnt>()
            .add_systems(
                Update,
                (ghost_and_place, clear_ghost_when_inactive).run_if(playing),
            )
            .add_systems(
                FixedUpdate,
                (construction_tick, spire_tick, shield_proj_tick, shield_decay).run_if(playing),
            );
    }
}
