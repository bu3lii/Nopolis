//! Procedural world generation, curved terrain meshes, fog of war, minimap.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use crate::building::{BKind, Building};
use crate::components::*;
use crate::enemies::Enemy;
use crate::research::TechFx;
use crate::resources::*;
use crate::{CELL, MAP, TILE, TILES};

// ---------------- generation ----------------
pub fn generate(grid: &mut Grid, seed: u64) {
    *grid = Grid::new(seed);
    let mut rng = Rng::new(seed ^ 0xA17C_E55A);
    let ctr = (MAP / 2, MAP / 2);

    for y in 0..MAP {
        for x in 0..MAP {
            let (fx, fy) = (x as f32, y as f32);
            let n1 = fbm(fx * 0.07, fy * 0.07, seed);
            let n2 = fbm(fx * 0.05 + 9.3, fy * 0.05 - 4.1, seed ^ 0xBEEF);
            let e = fbm(fx * 0.11, fy * 0.11, seed ^ 0x1234);
            let c = grid.at_mut((x, y));
            c.elev = (e * 1.8).max(0.0).min(2.0);
            c.terrain = if n2 > 0.42 {
                Terrain::Marsh
            } else if n1 > 0.30 {
                Terrain::Regolith
            } else if n1 < -0.40 {
                Terrain::Glass
            } else {
                Terrain::Hull
            };
            if c.terrain == Terrain::Marsh {
                c.res = 300.0;
                c.res_kind = R_COOLANT;
                c.elev = 0.0;
            }
            // map border is impassable
            if x == 0 || y == 0 || x == MAP - 1 || y == MAP - 1 {
                c.walk = false;
                c.elev = 2.5;
            }
        }
    }

    let blob = |grid: &mut Grid, rng: &mut Rng, cx: i32, cy: i32, r: i32, f: &mut dyn FnMut(&mut Cell, &mut Rng)| {
        for dy in -r..=r {
            for dx in -r..=r {
                let c = (cx + dx, cy + dy);
                if Grid::inb(c) && dx * dx + dy * dy <= r * r {
                    f(grid.at_mut(c), rng);
                }
            }
        }
    };

    // scrap clusters
    for _ in 0..26 {
        let cx = 3 + rng.below(MAP - 6);
        let cy = 3 + rng.below(MAP - 6);
        if (cx - ctr.0).abs() < 6 && (cy - ctr.1).abs() < 6 {
            continue;
        }
        let r = 1 + rng.below(2);
        blob(grid, &mut rng, cx, cy, r, &mut |c, rng| {
            if c.terrain != Terrain::Marsh {
                c.terrain = Terrain::Regolith;
                c.res_kind = R_SCRAP;
                c.res = rng.range(60.0, 150.0);
            }
        });
    }
    // crystal scars
    for _ in 0..15 {
        let cx = 3 + rng.below(MAP - 6);
        let cy = 3 + rng.below(MAP - 6);
        if (cx - ctr.0).abs() < 10 && (cy - ctr.1).abs() < 10 {
            continue;
        }
        let r = 1 + rng.below(2);
        blob(grid, &mut rng, cx, cy, r, &mut |c, rng| {
            c.terrain = Terrain::Crystal;
            c.res_kind = R_CRYSTAL;
            c.res = rng.range(50.0, 120.0);
        });
    }
    // ancient ruins
    for _ in 0..16 {
        let cx = 4 + rng.below(MAP - 8);
        let cy = 4 + rng.below(MAP - 8);
        if (cx - ctr.0).abs() < 9 && (cy - ctr.1).abs() < 9 {
            continue;
        }
        let r = rng.below(2);
        blob(grid, &mut rng, cx, cy, r, &mut |c, rng| {
            c.terrain = Terrain::Ruin;
            c.res_kind = R_RELIC;
            c.res = rng.range(25.0, 60.0);
            c.elev += 0.4;
        });
    }
    // chokepoint ridges: impassable raised glass walls with gaps
    for _ in 0..12 {
        let mut x = 4 + rng.below(MAP - 8);
        let mut y = 4 + rng.below(MAP - 8);
        let horiz = rng.below(2) == 0;
        let len = 8 + rng.below(11);
        for i in 0..len {
            if i % 5 == 4 {
                // gap = chokepoint
            } else if Grid::inb((x, y)) && ((x - ctr.0).abs() > 8 || (y - ctr.1).abs() > 8) {
                let c = grid.at_mut((x, y));
                c.walk = false;
                c.elev = 2.4;
                c.terrain = Terrain::Glass;
                c.res = 0.0;
            }
            if horiz {
                x += 1;
            } else {
                y += 1;
            }
        }
    }
    // home plateau
    blob(grid, &mut rng, ctr.0, ctr.1, 7, &mut |c, _| {
        c.terrain = Terrain::Hull;
        c.elev = 0.2;
        c.walk = true;
        c.res = 0.0;
        c.corrupt = 0.0;
    });
    // enemy nests on an outer ring + corrupted growth around them
    for i in 0..9 {
        let ang = i as f32 / 9.0 * std::f32::consts::TAU + rng.range(0.0, 0.5);
        let rad = rng.range(30.0, 43.0);
        let cx = (ctr.0 as f32 + ang.cos() * rad) as i32;
        let cy = (ctr.1 as f32 + ang.sin() * rad) as i32;
        if !Grid::inb((cx, cy)) {
            continue;
        }
        blob(grid, &mut rng, cx, cy, 2, &mut |c, rng| {
            c.terrain = Terrain::Growth;
            c.corrupt = rng.range(0.5, 0.9);
            c.walk = true;
            c.elev = c.elev.min(0.8);
        });
        grid.at_mut((cx, cy)).walk = true;
        grid.nests_gen.push((cx, cy));
    }
    grid.mark_all();
    grid.flow_dirty = true;
    grid.power_dirty = true;
}

pub fn reveal(grid: &mut Grid, c: (i32, i32), r: i32) {
    for dy in -r..=r {
        for dx in -r..=r {
            let p = (c.0 + dx, c.1 + dy);
            if Grid::inb(p) && dx * dx + dy * dy <= r * r && !grid.at(p).disc {
                grid.at_mut(p).disc = true;
                grid.mark(p);
            }
        }
    }
}

// ---------------- terrain colors ----------------
fn terrain_rgb(c: &Cell) -> [f32; 3] {
    let mut col = match c.terrain {
        Terrain::Hull => [0.13, 0.15, 0.19],
        Terrain::Regolith => [0.26, 0.22, 0.16],
        Terrain::Crystal => [0.18, 0.34, 0.72],
        Terrain::Marsh => [0.05, 0.24, 0.26],
        Terrain::Ruin => [0.10, 0.07, 0.13],
        Terrain::Glass => [0.07, 0.21, 0.13],
        Terrain::Growth => [0.24, 0.06, 0.28],
    };
    if c.res > 0.0 && c.terrain != Terrain::Marsh {
        let rc = res_color(c.res_kind).to_linear();
        for (o, r) in col.iter_mut().zip([rc.red, rc.green, rc.blue]) {
            *o = *o * 0.7 + r * 0.3;
        }
    }
    let k = (c.corrupt * 0.8).min(0.85);
    col[0] = col[0] * (1.0 - k) + 0.38 * k;
    col[1] = col[1] * (1.0 - k) + 0.04 * k;
    col[2] = col[2] * (1.0 - k) + 0.42 * k;
    let shade = 0.85 + c.elev * 0.12;
    let vis = if c.disc { 1.0 } else { 0.045 };
    [col[0] * shade * vis, col[1] * shade * vis, col[2] * shade * vis]
}

// ---------------- tile meshes ----------------
fn build_tile_mesh(grid: &Grid, tx: i32, ty: i32) -> Mesh {
    let n = (TILE + 1) as usize;
    let mut pos = Vec::with_capacity(n * n);
    let mut nor = Vec::with_capacity(n * n);
    let mut col = Vec::with_capacity(n * n);
    let mut idx: Vec<u32> = Vec::with_capacity((TILE * TILE * 6) as usize);
    let half = half_map();

    for j in 0..=TILE {
        for i in 0..=TILE {
            let gx = (tx * TILE + i).min(MAP - 1);
            let gy = (ty * TILE + j).min(MAP - 1);
            let cell = grid.at((gx, gy));
            let u = (tx * TILE + i) as f32 * CELL - half;
            let v = (ty * TILE + j) as f32 * CELL - half;
            let p = surf_to_world(u, v, cell.elev);
            let nrm = surf_normal(u);
            pos.push([p.x, p.y, p.z]);
            nor.push([nrm.x, nrm.y, nrm.z]);
            let c = terrain_rgb(cell);
            col.push([c[0], c[1], c[2], 1.0]);
        }
    }
    for j in 0..TILE as u32 {
        for i in 0..TILE as u32 {
            let a = j * (TILE as u32 + 1) + i;
            let b = a + 1;
            let c = a + TILE as u32 + 1;
            let d = c + 1;
            idx.extend([a, c, b, b, c, d]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

fn spawn_tiles(
    mut commands: Commands,
    grid: Res<Grid>,
    libs: Res<Libs>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mut handles = Vec::new();
    for ty in 0..TILES {
        for tx in 0..TILES {
            let h = meshes.add(build_tile_mesh(&grid, tx, ty));
            commands.spawn((
                Mesh3d(h.clone()),
                MeshMaterial3d(libs.terrain_mat.clone()),
                Transform::IDENTITY,
                Name::new("tile"),
            ));
            handles.push(h);
        }
    }
    commands.insert_resource(TileHandles(handles));
}

fn rebuild_dirty_tiles(
    mut grid: ResMut<Grid>,
    handles: Option<Res<TileHandles>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(handles) = handles else { return };
    // limit per-frame remeshing for smoothness
    let mut budget = 9;
    for t in 0..(TILES * TILES) as usize {
        if grid.dirty[t] && budget > 0 {
            grid.dirty[t] = false;
            budget -= 1;
            let (tx, ty) = (t as i32 % TILES, t as i32 / TILES);
            meshes.insert(&handles.0[t], build_tile_mesh(&grid, tx, ty));
        }
    }
}

// ---------------- fog of war ----------------
fn fog_discovery(
    time: Res<Time>,
    mut acc: Local<f32>,
    mut grid: ResMut<Grid>,
    fx: Res<TechFx>,
    qp: Query<&SurfPos, With<PlayerTag>>,
    qb: Query<(&Building, &SurfPos)>,
) {
    *acc += time.delta_secs();
    if *acc < 0.4 {
        return;
    }
    *acc = 0.0;
    if let Ok(sp) = qp.single() {
        reveal(&mut grid, sp.cell(), 11);
    }
    for (b, sp) in qb.iter() {
        if b.kind == BKind::Radar && b.built >= 1.0 && b.powered > 0.2 {
            reveal(&mut grid, sp.cell(), (14.0 * fx.radar) as i32);
        }
    }
}

/// Hide enemies/nests standing in undiscovered cells.
fn fog_visibility(
    time: Res<Time>,
    mut acc: Local<f32>,
    grid: Res<Grid>,
    mut q: Query<(&SurfPos, &mut Visibility), Or<(With<Enemy>, With<Nest>)>>,
) {
    *acc += time.delta_secs();
    if *acc < 0.5 {
        return;
    }
    *acc = 0.0;
    for (sp, mut vis) in q.iter_mut() {
        let c = sp.cell();
        let show = Grid::inb(c) && grid.at(c).disc;
        *vis = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

// ---------------- minimap ----------------
#[allow(clippy::too_many_arguments)]
fn minimap_update(
    time: Res<Time>,
    mut acc: Local<f32>,
    grid: Res<Grid>,
    img_res: Option<Res<MinimapImg>>,
    mut images: ResMut<Assets<Image>>,
    fx: Res<TechFx>,
    rally: Res<RallyPoint>,
    qe: Query<&SurfPos, With<Enemy>>,
    qn: Query<&SurfPos, With<Nest>>,
    qp: Query<&SurfPos, With<PlayerTag>>,
) {
    *acc += time.delta_secs();
    if *acc < 0.5 {
        return;
    }
    *acc = 0.0;
    let Some(img_res) = img_res else { return };
    let Some(img) = images.get_mut(&img_res.0) else {
        return;
    };
    let Some(data) = img.data.as_mut() else { return };

    let put = |c: (i32, i32), rgb: [u8; 3], data: &mut Vec<u8>| {
        if Grid::inb(c) {
            let i = ((MAP - 1 - c.1) * MAP + c.0) as usize * 4;
            data[i] = rgb[0];
            data[i + 1] = rgb[1];
            data[i + 2] = rgb[2];
            data[i + 3] = 255;
        }
    };
    for y in 0..MAP {
        for x in 0..MAP {
            let cell = grid.at((x, y));
            let mut c = terrain_rgb(cell);
            if !cell.disc {
                c = [0.02, 0.02, 0.03];
            }
            if cell.disc && cell.occ.is_some() {
                c = if cell.conv {
                    [0.1, 0.5, 0.5]
                } else {
                    [0.2, 0.9, 1.0]
                };
            }
            if fx.ruin_scan && cell.terrain == Terrain::Ruin && cell.res > 0.0 {
                c = [0.8, 0.4, 1.0];
            }
            put(
                (x, y),
                [
                    (c[0].sqrt() * 255.0) as u8,
                    (c[1].sqrt() * 255.0) as u8,
                    (c[2].sqrt() * 255.0) as u8,
                ],
                data,
            );
        }
    }
    for sp in qe.iter() {
        let c = sp.cell();
        if Grid::inb(c) && grid.at(c).disc {
            put(c, [255, 40, 40], data);
        }
    }
    for sp in qn.iter() {
        let c = sp.cell();
        if fx.nest_loc || (Grid::inb(c) && grid.at(c).disc) {
            put(c, [255, 0, 255], data);
            put((c.0 + 1, c.1), [255, 0, 255], data);
        }
    }
    if let Some((u, v)) = rally.0 {
        put(cell_of(u, v), [255, 160, 0], data);
    }
    if let Ok(sp) = qp.single() {
        let c = sp.cell();
        for d in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            put((c.0 + d.0, c.1 + d.1), [255, 255, 255], data);
        }
    }
}

pub struct ProcgenPlugin;
impl Plugin for ProcgenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_tiles).add_systems(
            Update,
            (
                rebuild_dirty_tiles,
                fog_discovery.run_if(playing),
                fog_visibility,
                minimap_update,
            ),
        );
    }
}
