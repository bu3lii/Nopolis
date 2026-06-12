//! Power networks (pylons, generators, consumers, brownouts) and the
//! node-based logistics network (conveyors, sorters, visible item packets).

use bevy::prelude::*;
use std::collections::{HashMap, VecDeque};

use crate::building::{bdef, BKind, Building};
use crate::components::*;
use crate::events::EventFx;
use crate::research::TechFx;
use crate::resources::*;
use crate::{DayLight, MAP};

// ---------------- power ----------------
#[derive(Resource, Default)]
pub struct PowerLinks(pub Vec<(Vec3, Vec3)>);

fn uf_find(parent: &mut Vec<usize>, mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

#[allow(clippy::too_many_arguments)]
fn power_recompute(
    time: Res<Time>,
    mut acc: Local<f32>,
    mut grid: ResMut<Grid>,
    fx: Res<TechFx>,
    evfx: Res<EventFx>,
    day: Res<DayLight>,
    bank: Res<Bank>,
    mut info: ResMut<PowerInfo>,
    mut links: ResMut<PowerLinks>,
    mut q: Query<(Entity, &mut Building, &SurfPos)>,
) {
    *acc += time.delta_secs();
    if *acc < 2.0 && !grid.power_dirty {
        return;
    }
    *acc = 0.0;
    grid.power_dirty = false;

    struct Snap {
        e: Entity,
        kind: BKind,
        u: f32,
        v: f32,
        built: bool,
        disabled: bool,
    }
    let snap: Vec<Snap> = q
        .iter()
        .map(|(e, b, sp)| Snap {
            e,
            kind: b.kind,
            u: sp.u,
            v: sp.v,
            built: b.built >= 1.0,
            disabled: b.disabled > 0.0,
        })
        .collect();

    // pylons (and the core, which acts as a small pylon)
    let pyl: Vec<(usize, f32)> = snap
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.built && !s.disabled && matches!(s.kind, BKind::Pylon | BKind::Core)
        })
        .map(|(i, s)| {
            (
                i,
                if s.kind == BKind::Pylon {
                    9.0 * fx.pylon
                } else {
                    9.0
                },
            )
        })
        .collect();

    let mut parent: Vec<usize> = (0..pyl.len()).collect();
    for a in 0..pyl.len() {
        for b in (a + 1)..pyl.len() {
            let (ia, ra) = pyl[a];
            let (ib, rb) = pyl[b];
            let d = Vec2::new(snap[ia].u - snap[ib].u, snap[ia].v - snap[ib].v).length();
            if d <= ra + rb {
                let (fa, fb) = (uf_find(&mut parent, a), uf_find(&mut parent, b));
                parent[fa] = fb;
            }
        }
    }
    let mut net_of_root: HashMap<usize, usize> = HashMap::new();
    let mut nets: Vec<(f32, f32, f32)> = Vec::new();
    let mut pylon_net: Vec<usize> = vec![0; pyl.len()];
    for p in 0..pyl.len() {
        let r = uf_find(&mut parent, p);
        let n = *net_of_root.entry(r).or_insert_with(|| {
            nets.push((0.0, 0.0, 1.0));
            nets.len() - 1
        });
        pylon_net[p] = n;
    }

    links.0.clear();
    for a in 0..pyl.len() {
        for b in (a + 1)..pyl.len() {
            let (ia, ra) = pyl[a];
            let (ib, rb) = pyl[b];
            let d = Vec2::new(snap[ia].u - snap[ib].u, snap[ia].v - snap[ib].v).length();
            if d <= ra + rb {
                links.0.push((
                    surf_to_world(snap[ia].u, snap[ia].v, 3.0),
                    surf_to_world(snap[ib].u, snap[ib].v, 3.0),
                ));
            }
        }
    }

    // attach every powered building to the nearest in-range pylon's network
    let mut bnet: HashMap<Entity, usize> = HashMap::new();
    for s in snap.iter() {
        if !s.built {
            continue;
        }
        let mut best: Option<(f32, usize)> = None;
        for (pi, (idx, r)) in pyl.iter().enumerate() {
            let d = Vec2::new(snap[*idx].u - s.u, snap[*idx].v - s.v).length();
            if d <= *r && best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, pylon_net[pi]));
            }
        }
        if let Some((d, n)) = best {
            bnet.insert(s.e, n);
            if !matches!(s.kind, BKind::Pylon | BKind::Core) && d > 0.1 {
                if let Some((idx, _)) = pyl
                    .iter()
                    .min_by(|x, y| {
                        let dx = Vec2::new(snap[x.0].u - s.u, snap[x.0].v - s.v).length();
                        let dy = Vec2::new(snap[y.0].u - s.u, snap[y.0].v - s.v).length();
                        dx.total_cmp(&dy)
                    })
                {
                    links.0.push((
                        surf_to_world(s.u, s.v, 2.0),
                        surf_to_world(snap[*idx].u, snap[*idx].v, 3.0),
                    ));
                }
            }
        }
    }

    // production / demand per network
    let oc_power = if fx.overclock > 1.0 { 1.25 } else { 1.0 };
    for s in snap.iter() {
        if !s.built || s.disabled {
            continue;
        }
        let Some(&n) = bnet.get(&s.e) else { continue };
        let gen = match s.kind {
            BKind::Core => 6.0,
            BKind::Solar => 8.0 * (day.0 * evfx.solar).max(fx.solar_night),
            BKind::Fusion => {
                if bank.amt[R_COOLANT] > 0.5 {
                    26.0 * fx.fusion
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };
        let p = bdef(s.kind).power;
        if gen > 0.0 {
            nets[n].0 += gen;
        } else if p < 0.0 {
            nets[n].1 += -p * oc_power;
        }
    }
    for n in nets.iter_mut() {
        n.2 = if n.1 <= 0.01 {
            1.0
        } else {
            (n.0 / n.1).clamp(0.0, 1.0)
        };
    }
    info.nets = nets.clone();

    for (e, mut b, _) in q.iter_mut() {
        let p = bdef(b.kind).power;
        b.powered = if b.built < 1.0 || b.disabled > 0.0 {
            0.0
        } else if p >= 0.0 {
            1.0
        } else {
            match bnet.get(&e) {
                Some(&n) => {
                    let sat = nets[n].2;
                    if fx.brownout && sat > 0.05 {
                        sat.max(0.55)
                    } else {
                        sat
                    }
                }
                None => 0.0,
            }
        };
    }
}

fn outage_track(time: Res<Time>, clock: Res<GameClock>, info: Res<PowerInfo>, mut sim: ResMut<SimStats>) {
    if clock.phase == Phase::Night && info.nets.iter().any(|n| n.1 > 0.5 && n.2 < 0.5) {
        sim.outage += time.delta_secs();
    }
}

// ---------------- machines ----------------
fn machine_tick(
    time: Res<Time>,
    mut grid: ResMut<Grid>,
    mut bank: ResMut<Bank>,
    fx: Res<TechFx>,
    mut q: Query<&mut Building>,
) {
    let dt = time.delta_secs();
    let oc = fx.overclock;
    for mut b in q.iter_mut() {
        if b.built < 1.0 {
            continue;
        }
        let pw = b.powered;
        match b.kind {
            BKind::Extractor | BKind::Bore | BKind::Pump => {
                let cell = b.cell;
                if !Grid::inb(cell) {
                    continue;
                }
                let kind = grid.at(cell).res_kind;
                let avail = grid.at(cell).res;
                b.missing = avail <= 0.0;
                let space = 16.0 - b.buf_out[kind];
                let amt = (0.5 * pw * oc * dt).min(avail).min(space.max(0.0));
                if amt > 0.0 {
                    grid.at_mut(cell).res -= amt;
                    b.buf_out[kind] += amt;
                    if grid.at(cell).res <= 0.01 {
                        grid.at_mut(cell).res = 0.0;
                        grid.mark(cell);
                    }
                }
            }
            BKind::Smelter => {
                b.missing = b.buf_in[R_SCRAP] < 2.0;
                if !b.missing && b.buf_out[R_ALLOY] < 16.0 {
                    b.craft += dt * pw * oc / 3.0;
                    if b.craft >= 1.0 {
                        b.craft = 0.0;
                        b.buf_in[R_SCRAP] -= 2.0;
                        b.buf_out[R_ALLOY] += 1.0;
                    }
                }
            }
            BKind::Loom => {
                if b.recipe == 0 {
                    b.missing = b.buf_in[R_ALLOY] < 1.0 || b.buf_in[R_CRYSTAL] < 1.0;
                    if !b.missing && b.buf_out[R_CIRCUIT] < 16.0 {
                        b.craft += dt * pw * oc / 4.0;
                        if b.craft >= 1.0 {
                            b.craft = 0.0;
                            b.buf_in[R_ALLOY] -= 1.0;
                            b.buf_in[R_CRYSTAL] -= 1.0;
                            b.buf_out[R_CIRCUIT] += 1.0;
                        }
                    }
                } else {
                    b.missing = b.buf_in[R_ALLOY] < 1.0
                        || b.buf_in[R_CIRCUIT] < 2.0
                        || b.buf_in[R_RELIC] < 1.0;
                    if !b.missing && b.buf_out[R_LPART] < 8.0 {
                        b.craft += dt * pw * oc / 6.0;
                        if b.craft >= 1.0 {
                            b.craft = 0.0;
                            b.buf_in[R_ALLOY] -= 1.0;
                            b.buf_in[R_CIRCUIT] -= 2.0;
                            b.buf_in[R_RELIC] -= 1.0;
                            b.buf_out[R_LPART] += 1.0;
                        }
                    }
                }
            }
            BKind::Fusion => {
                if pw > 0.0 {
                    let use_rate = if fx.fusion > 1.0 { 0.09 } else { 0.18 };
                    bank.take(R_COOLANT, use_rate * dt);
                }
            }
            _ => {}
        }
    }
}

fn storage_caps(
    time: Res<Time>,
    mut acc: Local<f32>,
    fx: Res<TechFx>,
    mut bank: ResMut<Bank>,
    q: Query<&Building>,
) {
    *acc += time.delta_secs();
    if *acc < 2.0 {
        return;
    }
    *acc = 0.0;
    let silos = q
        .iter()
        .filter(|b| b.kind == BKind::Silo && b.built >= 1.0)
        .count() as f32;
    let cap = (300.0 + silos * 250.0) * fx.storage;
    bank.cap = [cap; NRES];
}

// ---------------- logistics ----------------
fn accepts(kind: BKind, recipe: u8, res: usize) -> bool {
    match kind {
        BKind::Silo => true,
        BKind::Smelter => res == R_SCRAP,
        BKind::Loom => {
            if recipe == 0 {
                res == R_ALLOY || res == R_CRYSTAL
            } else {
                res == R_ALLOY || res == R_CIRCUIT || res == R_RELIC
            }
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct MachInfo {
    kind: BKind,
    recipe: u8,
    buf_in: [f32; NRES],
    built: bool,
}

const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// BFS through the conveyor network from `start` to the closest node adjacent
/// to a machine accepting `res`. Sorters only pass their filter type.
fn route(
    grid: &Grid,
    machines: &HashMap<Entity, MachInfo>,
    filters: &HashMap<(i32, i32), usize>,
    start: (i32, i32),
    res: usize,
    exclude: Entity,
) -> Option<Vec<(i32, i32)>> {
    let mut prev: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut queue = VecDeque::new();
    queue.push_back(start);
    prev.insert(start, start);
    let mut steps = 0;
    while let Some(c) = queue.pop_front() {
        steps += 1;
        if steps > 500 {
            break;
        }
        // does an adjacent machine accept this resource?
        for d in DIRS {
            let n = (c.0 + d.0, c.1 + d.1);
            if !Grid::inb(n) {
                continue;
            }
            if let Some(e) = grid.at(n).occ {
                if e != exclude && !grid.at(n).conv {
                    if let Some(m) = machines.get(&e) {
                        if m.built && accepts(m.kind, m.recipe, res) && m.buf_in[res] < 12.0 {
                            // reconstruct path
                            let mut path = vec![c];
                            let mut cur = c;
                            while prev[&cur] != cur {
                                cur = prev[&cur];
                                path.push(cur);
                            }
                            path.reverse();
                            return Some(path);
                        }
                    }
                }
            }
        }
        for d in DIRS {
            let n = (c.0 + d.0, c.1 + d.1);
            if !Grid::inb(n) || prev.contains_key(&n) || !grid.at(n).conv {
                continue;
            }
            if let Some(&f) = filters.get(&n) {
                if f < NRES && f != res {
                    continue;
                }
            }
            prev.insert(n, c);
            queue.push_back(n);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn packet_spawn(
    time: Res<Time>,
    mut acc: Local<f32>,
    grid: Res<Grid>,
    libs: Res<Libs>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Building)>,
    qp: Query<&Packet>,
) {
    *acc += time.delta_secs();
    if *acc < 0.5 {
        return;
    }
    *acc = 0.0;
    if qp.iter().count() > 300 {
        return;
    }
    let mut machines: HashMap<Entity, MachInfo> = HashMap::new();
    let mut filters: HashMap<(i32, i32), usize> = HashMap::new();
    for (e, b) in q.iter() {
        machines.insert(
            e,
            MachInfo {
                kind: b.kind,
                recipe: b.recipe,
                buf_in: b.buf_in,
                built: b.built >= 1.0,
            },
        );
        if b.kind == BKind::Sorter {
            filters.insert(b.cell, b.filter);
        }
    }
    for (e, mut b) in q.iter_mut() {
        if b.built < 1.0 || matches!(b.kind, BKind::Conveyor | BKind::Sorter) {
            continue;
        }
        let Some(res) = (0..NRES).find(|k| b.buf_out[*k] >= 1.0) else {
            continue;
        };
        // adjacent conveyor node?
        let mut start = None;
        'f: for fc in crate::building::foot_cells(b.kind, b.cell) {
            for d in DIRS {
                let n = (fc.0 + d.0, fc.1 + d.1);
                if Grid::inb(n) && grid.at(n).conv {
                    start = Some(n);
                    break 'f;
                }
            }
        }
        let Some(start) = start else {
            b.jam = b.buf_out[res] > 10.0;
            continue;
        };
        match route(&grid, &machines, &filters, start, res, e) {
            Some(path) => {
                b.buf_out[res] -= 1.0;
                b.jam = false;
                let (u, v) = cell_center(path[0]);
                commands.spawn((
                    Packet {
                        res,
                        amount: 1.0,
                        path,
                        idx: 0,
                        t: 0.0,
                        stall: 0.0,
                    },
                    SurfPos::new(u, v, grid.elev_uv(u, v) + 0.8),
                    Mesh3d(libs.cube_s.clone()),
                    MeshMaterial3d(libs.res_mats[res].clone()),
                    Transform::default(),
                    Visibility::default(),
                    GameEntity,
                ));
            }
            None => {
                b.jam = true;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn packet_move(
    time: Res<Time>,
    grid: Res<Grid>,
    fx: Res<TechFx>,
    mut bank: ResMut<Bank>,
    mut sim: ResMut<SimStats>,
    mut commands: Commands,
    mut qp: Query<(Entity, &mut Packet, &mut SurfPos)>,
    mut qb: Query<&mut Building>,
) {
    let dt = time.delta_secs();
    let speed = 2.4 * fx.packet;
    for (e, mut p, mut sp) in qp.iter_mut() {
        if p.idx + 1 >= p.path.len() {
            // deliver to an adjacent acceptor
            let last = *p.path.last().unwrap();
            let mut done = false;
            for d in DIRS {
                let n = (last.0 + d.0, last.1 + d.1);
                if !Grid::inb(n) || grid.at(n).conv {
                    continue;
                }
                if let Some(me) = grid.at(n).occ {
                    if let Ok(mut b) = qb.get_mut(me) {
                        if b.built >= 1.0 && accepts(b.kind, b.recipe, p.res) {
                            if b.kind == BKind::Silo {
                                crate::player::deposit_carry(&mut bank, &mut sim, p.res, p.amount);
                            } else {
                                let r = p.res;
                                b.buf_in[r] = (b.buf_in[r] + p.amount).min(14.0);
                            }
                            sim.packets_delivered += 1;
                            done = true;
                            break;
                        }
                    }
                }
            }
            if done {
                commands.entity(e).despawn();
            } else {
                p.stall += dt;
                if p.stall > 12.0 {
                    commands.entity(e).despawn();
                }
            }
            continue;
        }
        let next = p.path[p.idx + 1];
        if !grid.at(next).conv {
            p.stall += dt;
            continue;
        }
        p.t += dt * speed;
        if p.t >= 1.0 {
            p.t = 0.0;
            p.idx += 1;
        }
        let a = cell_center(p.path[p.idx]);
        let b = cell_center(p.path[(p.idx + 1).min(p.path.len() - 1)]);
        sp.u = a.0 + (b.0 - a.0) * p.t;
        sp.v = a.1 + (b.1 - a.1) * p.t;
        sp.h = grid.elev_uv(sp.u, sp.v) + 0.8 + (time.elapsed_secs() * 5.0 + sp.u).sin() * 0.07;
    }
}

/// Stalled packets try to find a new route (network repaired / rebuilt).
fn packet_repath(
    time: Res<Time>,
    mut acc: Local<f32>,
    grid: Res<Grid>,
    q: Query<(Entity, &Building)>,
    mut qp: Query<&mut Packet>,
) {
    *acc += time.delta_secs();
    if *acc < 1.5 {
        return;
    }
    *acc = 0.0;
    let mut machines: HashMap<Entity, MachInfo> = HashMap::new();
    let mut filters: HashMap<(i32, i32), usize> = HashMap::new();
    for (e, b) in q.iter() {
        machines.insert(
            e,
            MachInfo {
                kind: b.kind,
                recipe: b.recipe,
                buf_in: b.buf_in,
                built: b.built >= 1.0,
            },
        );
        if b.kind == BKind::Sorter {
            filters.insert(b.cell, b.filter);
        }
    }
    for mut p in qp.iter_mut() {
        if p.stall <= 0.5 {
            continue;
        }
        let cur = p.path[p.idx.min(p.path.len() - 1)];
        if let Some(path) = route(&grid, &machines, &filters, cur, p.res, Entity::PLACEHOLDER) {
            p.path = path;
            p.idx = 0;
            p.t = 0.0;
            p.stall = 0.0;
        }
    }
}

// ---------------- overlays ----------------
fn tactical_overlay(
    ui: Res<UiState>,
    grid: Res<Grid>,
    links: Res<PowerLinks>,
    q: Query<(&Building, &SurfPos)>,
    mut giz: Gizmos,
) {
    if !ui.tactical {
        return;
    }
    for (a, b) in links.0.iter() {
        giz.line(*a, *b, Color::srgb(0.2, 0.9, 1.0));
    }
    // conveyor connectivity
    for y in 0..MAP {
        for x in 0..MAP {
            if !grid.at((x, y)).conv {
                continue;
            }
            let (u, v) = cell_center((x, y));
            for d in [(1, 0), (0, 1)] {
                let n = (x + d.0, y + d.1);
                if Grid::inb(n) && grid.at(n).conv {
                    let (nu, nv) = cell_center(n);
                    giz.line(
                        surf_to_world(u, v, grid.at((x, y)).elev + 1.2),
                        surf_to_world(nu, nv, grid.at(n).elev + 1.2),
                        Color::srgb(0.2, 1.0, 0.4),
                    );
                }
            }
        }
    }
    for (b, sp) in q.iter() {
        let w = surf_to_world(sp.u, sp.v, sp.h + 3.5);
        if b.jam {
            giz.sphere(Isometry3d::from_translation(w), 0.5, Color::srgb(1.0, 0.1, 0.1));
        } else if b.missing && b.built >= 1.0 {
            giz.sphere(Isometry3d::from_translation(w), 0.4, Color::srgb(1.0, 0.6, 0.1));
        }
        if b.disabled > 0.0 {
            giz.sphere(Isometry3d::from_translation(w), 0.7, Color::srgb(1.0, 0.0, 1.0));
        }
        if b.built >= 1.0 && bdef(b.kind).power < 0.0 && b.powered < 0.3 {
            giz.sphere(Isometry3d::from_translation(w), 0.3, Color::srgb(1.0, 1.0, 0.0));
        }
    }
}

pub struct AutomationPlugin;
impl Plugin for AutomationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PowerLinks>()
            .add_systems(
                FixedUpdate,
                (
                    power_recompute,
                    outage_track,
                    machine_tick,
                    storage_caps,
                    packet_spawn,
                    packet_repath,
                )
                    .run_if(playing),
            )
            .add_systems(Update, (packet_move.run_if(playing), tactical_overlay));
    }
}
