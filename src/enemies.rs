//! Ancient machines: nests, night assaults, role-based AI, flow-field pathing,
//! corruption spread and the between-night adaptation analysis.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::building::{BKind, Building};
use crate::combat::spawn_proj;
use crate::components::*;
use crate::events::EventFx;
use crate::resources::*;
use crate::workers::Drone;
use crate::{AppState, MAP};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum EKind {
    Mite,
    Knight,
    Spitter,
    Priest,
    Leech,
    Bomber,
    Titan,
    Parasite,
}
pub const EKINDS: [EKind; 8] = [
    EKind::Mite,
    EKind::Knight,
    EKind::Spitter,
    EKind::Priest,
    EKind::Leech,
    EKind::Bomber,
    EKind::Titan,
    EKind::Parasite,
];

pub struct EDef {
    pub name: &'static str,
    pub hp: f32,
    pub armor: f32,
    pub speed: f32,
    pub dmg: f32,
    pub range: f32,
    pub cd: f32,
    pub cost: f32,
}

pub fn edef(k: EKind) -> EDef {
    let d = |name, hp, armor, speed, dmg, range, cd, cost| EDef {
        name,
        hp,
        armor,
        speed,
        dmg,
        range,
        cd,
        cost,
    };
    match k {
        EKind::Mite => d("Needle Mite", 18.0, 0.0, 7.0, 4.0, 1.5, 0.8, 1.0),
        EKind::Knight => d("Ossuary Knight", 95.0, 6.0, 3.2, 13.0, 1.9, 1.4, 3.0),
        EKind::Spitter => d("Choir Spitter", 35.0, 0.0, 4.2, 8.0, 13.0, 2.2, 2.0),
        EKind::Priest => d("Null Priest", 65.0, 2.0, 3.4, 0.0, 0.0, 0.0, 4.0),
        EKind::Leech => d("Phase Leech", 45.0, 0.0, 4.6, 9.0, 1.6, 1.0, 3.0),
        EKind::Bomber => d("Lattice Bomber", 30.0, 0.0, 6.0, 38.0, 1.6, 1.0, 3.0),
        EKind::Titan => d("Grave Titan", 650.0, 10.0, 2.1, 36.0, 3.2, 2.0, 14.0),
        EKind::Parasite => d("Signal Parasite", 25.0, 0.0, 6.5, 0.0, 1.4, 1.0, 2.0),
    }
}

#[derive(Component)]
pub struct Enemy {
    pub kind: EKind,
    pub target: Option<Entity>,
    pub mv: Vec2,
    pub atk: f32,
    pub decide: f32,
    pub retreat: bool,
    pub home: (i32, i32),
}

#[derive(Event)]
pub struct DisablePylon(pub Entity);

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct Adapt {
    pub kills: [f32; 8],
    pub w: [f32; 8],
    pub report: String,
}
impl Default for Adapt {
    fn default() -> Self {
        Self {
            kills: [0.0; 8],
            w: [1.0; 8],
            report: String::from("No assault data yet."),
        }
    }
}

#[derive(Resource, Default)]
pub struct WavePlan {
    pub queue: Vec<EKind>,
    pub final_mode: bool,
}

// ---------------- spawning ----------------
pub fn spawn_nest(commands: &mut Commands, libs: &Libs, cell: (i32, i32), level: u32) -> Entity {
    let (u, v) = cell_center(cell);
    let e = commands
        .spawn((
            Nest { level, cell },
            SurfPos::new(u, v, 0.3),
            Health::new(260.0),
            Armor(2.0),
            StatusFx::default(),
            SurfVel::default(),
            Transform::default(),
            Visibility::default(),
            GameEntity,
            Name::new("nest"),
        ))
        .id();
    commands.entity(e).with_children(|p| {
        p.spawn((
            Mesh3d(libs.torus.clone()),
            MeshMaterial3d(libs.mat(Cid::Black)),
            Transform::from_xyz(0.0, 0.4, 0.0).with_scale(Vec3::new(2.4, 1.6, 2.4)),
        ));
        p.spawn((
            Mesh3d(libs.sph.clone()),
            MeshMaterial3d(libs.mat(Cid::Red)),
            Transform::from_xyz(0.0, 0.7, 0.0).with_scale(Vec3::splat(1.1)),
        ));
    });
    e
}

type EPart = (Mid, Cid, [f32; 3], [f32; 3]);
fn eparts(k: EKind) -> Vec<EPart> {
    match k {
        EKind::Mite => vec![
            (Mid::Sph, Cid::Bone, [0.0, 0.4, 0.0], [0.8, 0.5, 1.0]),
            (Mid::Sph, Cid::Red, [0.0, 0.65, -0.3], [0.3, 0.3, 0.3]),
        ],
        EKind::Knight => vec![
            (Mid::Cube, Cid::Bone, [0.0, 1.0, 0.0], [0.9, 1.8, 0.7]),
            (Mid::Cube, Cid::Steel, [0.0, 1.0, -0.5], [1.2, 1.4, 0.2]),
            (Mid::Sph, Cid::Red, [0.0, 1.9, 0.0], [0.4, 0.4, 0.4]),
        ],
        EKind::Spitter => vec![
            (Mid::Cone, Cid::Bone, [0.0, 0.8, 0.0], [0.9, 1.6, 0.9]),
            (Mid::Sph, Cid::Green, [0.0, 1.3, 0.0], [0.5, 0.5, 0.5]),
        ],
        EKind::Priest => vec![
            (Mid::Cap, Cid::Black, [0.0, 1.1, 0.0], [0.9, 1.4, 0.9]),
            (Mid::Torus, Cid::Purple, [0.0, 2.3, 0.0], [1.1, 1.0, 1.1]),
        ],
        EKind::Leech => vec![
            (Mid::Sph, Cid::Crystal, [0.0, 0.5, 0.0], [1.3, 0.5, 1.3]),
            (Mid::Sph, Cid::Purple, [0.0, 0.8, 0.0], [0.45, 0.45, 0.45]),
        ],
        EKind::Bomber => vec![
            (Mid::Sph, Cid::Orange, [0.0, 0.7, 0.0], [1.1, 1.1, 1.1]),
            (Mid::CubeS, Cid::Dark, [0.0, 1.3, 0.0], [1.4, 1.4, 1.4]),
        ],
        EKind::Titan => vec![
            (Mid::Cube, Cid::Dark, [0.0, 1.8, 0.0], [2.4, 3.0, 1.7]),
            (Mid::Cube, Cid::Bone, [-1.5, 1.6, 0.0], [0.6, 2.4, 0.6]),
            (Mid::Cube, Cid::Bone, [1.5, 1.6, 0.0], [0.6, 2.4, 0.6]),
            (Mid::Sph, Cid::Red, [0.0, 2.6, -0.6], [0.9, 0.9, 0.9]),
        ],
        EKind::Parasite => vec![
            (Mid::CubeS, Cid::Dark, [0.0, 0.4, 0.0], [1.2, 0.8, 1.6]),
            (Mid::Sph, Cid::Yellow, [0.0, 0.65, 0.0], [0.45, 0.45, 0.45]),
        ],
    }
}

pub fn spawn_enemy(
    commands: &mut Commands,
    libs: &Libs,
    kind: EKind,
    u: f32,
    v: f32,
    home: (i32, i32),
    day: u32,
) -> Entity {
    let def = edef(kind);
    let scale = 1.0 + 0.12 * (day.saturating_sub(1)) as f32;
    let e = commands
        .spawn((
            Enemy {
                kind,
                target: None,
                mv: Vec2::ZERO,
                atk: 0.0,
                decide: 0.0,
                retreat: false,
                home,
            },
            SurfPos::new(u, v, 0.2),
            Yaw(0.0),
            SurfVel::default(),
            Health::new(def.hp * scale),
            Armor(def.armor),
            StatusFx::default(),
            Transform::default(),
            Visibility::default(),
            GameEntity,
            Name::new(def.name),
        ))
        .id();
    commands.entity(e).with_children(|p| {
        for (m, c, pos, sc) in eparts(kind) {
            p.spawn((
                Mesh3d(libs.mesh(m)),
                MeshMaterial3d(libs.mat(c)),
                Transform::from_translation(Vec3::from_array(pos)).with_scale(Vec3::from_array(sc)),
            ));
        }
    });
    e
}

// ---------------- flow field ----------------
fn flow_recompute(
    time: Res<Time>,
    mut acc: Local<f32>,
    mut grid: ResMut<Grid>,
    qb: Query<(&Building, &SurfPos)>,
) {
    *acc += time.delta_secs();
    if !grid.flow_dirty && *acc < 3.0 {
        return;
    }
    *acc = 0.0;
    grid.flow_dirty = false;
    let Some(core) = qb.iter().find(|(b, _)| b.kind == BKind::Core) else {
        return;
    };
    let start = core.1.cell();
    let n = (MAP * MAP) as usize;
    let mut dist = vec![u32::MAX; n];
    let mut heap: BinaryHeap<Reverse<(u32, i32, i32)>> = BinaryHeap::new();
    dist[Grid::idx(start)] = 0;
    heap.push(Reverse((0, start.0, start.1)));
    while let Some(Reverse((d, x, y))) = heap.pop() {
        if dist[Grid::idx((x, y))] < d {
            continue;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let c = (x + dx, y + dy);
            if !Grid::inb(c) {
                continue;
            }
            let cell = grid.at(c);
            if !cell.walk {
                continue;
            }
            // buildings are passable for pathing but expensive: enemies prefer
            // to flow around short walls and smash through long ones
            let cost: u32 = if cell.occ.is_some() && !cell.conv { 14 } else { 1 };
            let nd = d + cost;
            if nd < dist[Grid::idx(c)] {
                dist[Grid::idx(c)] = nd;
                heap.push(Reverse((nd, c.0, c.1)));
            }
        }
    }
    grid.flow = dist;
}

pub fn flow_dir(grid: &Grid, c: (i32, i32)) -> Option<Vec2> {
    if !Grid::inb(c) {
        return None;
    }
    let mut best: Option<((i32, i32), u32)> = None;
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)] {
        let nc = (c.0 + dx, c.1 + dy);
        if !Grid::inb(nc) {
            continue;
        }
        let d = grid.flow[Grid::idx(nc)];
        if d == u32::MAX {
            continue;
        }
        if best.map_or(true, |(_, bd)| d < bd) {
            best = Some((nc, d));
        }
    }
    let here = grid.flow[Grid::idx(c)];
    best.and_then(|(nc, d)| {
        if here != u32::MAX && d >= here && here == 0 {
            return None;
        }
        let (cu, cv) = cell_center(nc);
        let (hu, hv) = cell_center(c);
        Some(Vec2::new(cu - hu, cv - hv).normalize_or_zero())
    })
}

// ---------------- wave planning ----------------
fn base_weights(day: u32, adapt: &Adapt) -> [f32; 8] {
    let d = day as f32;
    let mut w = [
        4.0,
        0.8 + 0.25 * d,
        0.8 + 0.2 * d,
        if day >= 3 { 0.8 } else { 0.0 },
        if day >= 4 { 0.6 } else { 0.0 },
        if day >= 3 { 0.7 } else { 0.0 },
        if day >= 7 { 0.15 } else { 0.0 },
        if day >= 2 { 0.6 } else { 0.0 },
    ];
    for i in 0..8 {
        w[i] *= adapt.w[i];
    }
    w
}

fn plan_wave(day: u32, adapt: &Adapt, rng: &mut Rng, mult: f32) -> Vec<EKind> {
    let w = base_weights(day, adapt);
    let total: f32 = w.iter().sum();
    let mut budget = (8.0 + day as f32 * 5.0) * mult;
    let mut out = Vec::new();
    while budget > 0.0 && total > 0.0 && out.len() < 280 {
        let mut roll = rng.f32() * total;
        let mut pick = EKind::Mite;
        for (i, k) in EKINDS.iter().enumerate() {
            roll -= w[i];
            if roll <= 0.0 {
                pick = *k;
                break;
            }
        }
        budget -= edef(pick).cost;
        out.push(pick);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn phase_watch(
    clock: Res<GameClock>,
    launch: Res<LaunchState>,
    adapt: Res<Adapt>,
    mut rng: ResMut<RngRes>,
    mut plan: ResMut<WavePlan>,
    mut last_phase: Local<Option<Phase>>,
    mut final_started: Local<bool>,
    mut notify: EventWriter<Notify>,
) {
    if *last_phase != Some(clock.phase) {
        if clock.phase == Phase::Night {
            plan.queue = plan_wave(clock.day, &adapt, &mut rng.0, 1.0);
            notify.write(Notify(format!(
                "Night assault: {} signatures detected",
                plan.queue.len()
            )));
        }
        *last_phase = Some(clock.phase);
    }
    if launch.countdown.is_some() && !*final_started {
        *final_started = true;
        plan.final_mode = true;
        let mut wave = plan_wave(clock.day, &adapt, &mut rng.0, 4.0);
        wave.push(EKind::Titan);
        wave.push(EKind::Titan);
        plan.queue.append(&mut wave);
        notify.write(Notify("THE NECROPOLIS WAKES. Defend the launch engine!".into()));
    }
    if launch.countdown.is_none() && *final_started {
        *final_started = false;
        plan.final_mode = false;
    }
}

#[allow(clippy::too_many_arguments)]
fn spawner(
    time: Res<Time>,
    mut acc: Local<f32>,
    clock: Res<GameClock>,
    evfx: Res<EventFx>,
    mut plan: ResMut<WavePlan>,
    mut rng: ResMut<RngRes>,
    mut commands: Commands,
    libs: Res<Libs>,
    qn: Query<(&Nest, &SurfPos)>,
    qe: Query<&Enemy>,
) {
    *acc += time.delta_secs();
    if *acc < 2.0 {
        return;
    }
    *acc = 0.0;
    let night = clock.phase == Phase::Night || plan.final_mode || evfx.aggress;
    if !night || plan.queue.is_empty() || qe.iter().count() >= 300 {
        return;
    }
    let nests: Vec<((i32, i32), f32, f32)> = qn
        .iter()
        .map(|(n, sp)| (n.cell, sp.u, sp.v))
        .collect();
    let batch = (2 + rng.0.below(3)) as usize;
    for _ in 0..batch.min(plan.queue.len()) {
        let kind = plan.queue.pop().unwrap();
        let (home, u, v) = if !nests.is_empty() && !plan.final_mode {
            let pick = nests[rng.0.below(nests.len() as i32) as usize];
            (pick.0, pick.1, pick.2)
        } else {
            // map edge spawns for the final assault (or when all nests died)
            let side = rng.0.below(4);
            let t = rng.0.range(-half_map() + 6.0, half_map() - 6.0);
            let m = half_map() - 4.0;
            let (u, v) = match side {
                0 => (t, -m),
                1 => (t, m),
                2 => (-m, t),
                _ => (m, t),
            };
            (cell_of(u, v), u, v)
        };
        spawn_enemy(
            &mut commands,
            &libs,
            kind,
            u + rng.0.range(-2.0, 2.0),
            v + rng.0.range(-2.0, 2.0),
            home,
            clock.day,
        );
    }
}

// ---------------- AI ----------------
#[allow(clippy::too_many_arguments)]
fn enemy_decide(
    time: Res<Time>,
    grid: Res<Grid>,
    clock: Res<GameClock>,
    evfx: Res<EventFx>,
    plan: Res<WavePlan>,
    mut rng: ResMut<RngRes>,
    mut qe: Query<(&mut Enemy, &SurfPos, &Health)>,
    qb: Query<(Entity, &Building, &SurfPos), Without<Enemy>>,
    qd: Query<(Entity, &SurfPos), (With<Drone>, Without<Enemy>)>,
    qp: Query<(Entity, &SurfPos), (With<PlayerTag>, Without<Enemy>)>,
) {
    let dt = time.delta_secs();
    let player = qp.single().ok().map(|(e, sp)| (e, sp.u, sp.v));
    let calm = matches!(clock.phase, Phase::Dawn | Phase::Day) && !plan.final_mode && !evfx.aggress;

    for (mut en, sp, hp) in qe.iter_mut() {
        en.decide -= dt;
        if en.decide > 0.0 {
            continue;
        }
        en.decide = 0.5 + rng.0.f32() * 0.3;
        en.retreat = calm;
        if en.retreat {
            let (hu, hv) = cell_center(en.home);
            en.mv = Vec2::new(hu - sp.u, hv - sp.v).normalize_or_zero();
            en.target = None;
            continue;
        }

        let nearest_b = |f: &dyn Fn(&Building) -> bool, range: f32| -> Option<(Entity, f32, f32, f32)> {
            let mut best: Option<(Entity, f32, f32, f32)> = None;
            for (e, b, bsp) in qb.iter() {
                if b.built < 0.0 || !f(b) {
                    continue;
                }
                let d = bsp.dist(sp);
                if d < range && best.map_or(true, |(_, bd, _, _)| d < bd) {
                    best = Some((e, d, bsp.u, bsp.v));
                }
            }
            best
        };
        let nearest_drone = |range: f32| -> Option<(Entity, f32, f32, f32)> {
            let mut best: Option<(Entity, f32, f32, f32)> = None;
            for (e, dsp) in qd.iter() {
                let d = dsp.dist(sp);
                if d < range && best.map_or(true, |(_, bd, _, _)| d < bd) {
                    best = Some((e, d, dsp.u, dsp.v));
                }
            }
            best
        };
        let toward = |u: f32, v: f32| Vec2::new(u - sp.u, v - sp.v).normalize_or_zero();
        let flow = flow_dir(&grid, sp.cell());

        match en.kind {
            EKind::Mite | EKind::Knight | EKind::Titan => {
                let mut tgt = None;
                if let Some((pe, pu, pv)) = player {
                    let d = sp.dist_uv(pu, pv);
                    if d < 10.0 {
                        tgt = Some((pe, d, pu, pv));
                    }
                }
                if tgt.is_none() && en.kind != EKind::Titan {
                    tgt = nearest_drone(8.0);
                }
                if tgt.is_none() {
                    tgt = nearest_b(&|b| b.built > 0.0, 6.0);
                }
                match tgt {
                    Some((e, _, u, v)) => {
                        en.target = Some(e);
                        en.mv = toward(u, v);
                    }
                    None => {
                        en.target = None;
                        en.mv = flow.unwrap_or(Vec2::ZERO);
                    }
                }
            }
            EKind::Spitter => {
                let hurt = hp.hp < hp.max * 0.4;
                let mut tgt = nearest_b(
                    &|b| matches!(b.kind, BKind::Rail | BKind::Arc | BKind::Mortar),
                    15.0,
                );
                if tgt.is_none() {
                    if let Some((pe, pu, pv)) = player {
                        let d = sp.dist_uv(pu, pv);
                        if d < 15.0 {
                            tgt = Some((pe, d, pu, pv));
                        }
                    }
                }
                if tgt.is_none() {
                    tgt = nearest_b(&|b| b.built > 0.0, 14.0);
                }
                match tgt {
                    Some((e, d, u, v)) => {
                        en.target = Some(e);
                        en.mv = if hurt {
                            -toward(u, v)
                        } else if d > 11.0 {
                            toward(u, v)
                        } else if d < 7.0 {
                            -toward(u, v)
                        } else {
                            Vec2::ZERO
                        };
                    }
                    None => {
                        en.target = None;
                        en.mv = flow.unwrap_or(Vec2::ZERO);
                    }
                }
            }
            EKind::Priest => {
                en.target = None;
                en.mv = flow.unwrap_or(Vec2::ZERO);
            }
            EKind::Leech => {
                let tgt = nearest_b(&|b| b.powered > 0.2 && b.built >= 1.0, 22.0)
                    .or_else(|| nearest_drone(12.0));
                match tgt {
                    Some((e, _, u, v)) => {
                        en.target = Some(e);
                        en.mv = toward(u, v);
                    }
                    None => {
                        en.target = None;
                        en.mv = flow.unwrap_or(Vec2::ZERO);
                    }
                }
            }
            EKind::Bomber => {
                let tgt = nearest_b(
                    &|b| matches!(b.kind, BKind::Conveyor | BKind::Sorter | BKind::Silo),
                    34.0,
                );
                match tgt {
                    Some((e, _, u, v)) => {
                        en.target = Some(e);
                        en.mv = toward(u, v);
                    }
                    None => {
                        en.target = None;
                        en.mv = flow.unwrap_or(Vec2::ZERO);
                    }
                }
            }
            EKind::Parasite => {
                let tgt = nearest_b(&|b| b.kind == BKind::Pylon && b.disabled <= 0.0, 60.0);
                match tgt {
                    Some((e, _, u, v)) => {
                        en.target = Some(e);
                        en.mv = toward(u, v);
                    }
                    None => {
                        en.target = None;
                        en.mv = flow.unwrap_or(Vec2::ZERO);
                    }
                }
            }
        }
        // slight flanking jitter so swarms spread out
        let j = rng.0.range(-0.4, 0.4);
        en.mv = Vec2::new(en.mv.x - en.mv.y * j, en.mv.y + en.mv.x * j).normalize_or_zero()
            * en.mv.length().min(1.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn enemy_act(
    time: Res<Time>,
    grid: Res<Grid>,
    mut commands: Commands,
    libs: Res<Libs>,
    mut qe: Query<(Entity, &mut Enemy, &mut SurfPos, &mut SurfVel, &mut Yaw, &StatusFx), With<Enemy>>,
    qpos: Query<&SurfPos, Without<Enemy>>,
    qb: Query<&Building, Without<Enemy>>,
    mut dmg: EventWriter<DmgEvent>,
    mut boom: EventWriter<ExplodeEvent>,
    mut disable: EventWriter<DisablePylon>,
) {
    let dt = time.delta_secs();
    for (me, mut en, mut sp, mut vel, mut yaw, st) in qe.iter_mut() {
        let def = edef(en.kind);
        let mut speed = def.speed;
        if st.slow > 0.0 {
            speed *= 0.55;
        }
        if st.shock > 0.0 {
            speed *= 0.25;
        }
        let imp = Vec2::new(vel.0.x, vel.0.z);
        let step = en.mv * speed + imp;
        let (nu, nv) = clamp_uv(sp.u + step.x * dt, sp.v + step.y * dt);
        let phasing = en.kind == EKind::Leech;
        let walk_u = if phasing {
            Grid::inb(cell_of(nu, sp.v)) && grid.at(cell_of(nu, sp.v)).walk
        } else {
            grid.walkable(cell_of(nu, sp.v))
        };
        let walk_v = if phasing {
            Grid::inb(cell_of(sp.u, nv)) && grid.at(cell_of(sp.u, nv)).walk
        } else {
            grid.walkable(cell_of(sp.u, nv))
        };
        let mut blocked = false;
        if walk_u {
            sp.u = nu;
        } else {
            blocked = true;
        }
        if walk_v {
            sp.v = nv;
        } else {
            blocked = true;
        }
        vel.0.x *= (1.0f32 - 6.0 * dt).max(0.0);
        vel.0.z *= (1.0f32 - 6.0 * dt).max(0.0);
        sp.h = grid.elev_uv(sp.u, sp.v) + 0.1;
        if en.mv.length_squared() > 0.01 {
            yaw.0 = (-en.mv.x).atan2(-en.mv.y);
        }
        if en.retreat {
            let (hu, hv) = cell_center(en.home);
            if sp.dist_uv(hu, hv) < 3.0 {
                commands.entity(me).despawn();
            }
            continue;
        }
        // wall-breaking: if blocked, attack whatever building is in the way
        if blocked && en.target.is_none() && !phasing {
            let ahead = cell_of(sp.u + en.mv.x * 2.0, sp.v + en.mv.y * 2.0);
            if Grid::inb(ahead) {
                if let Some(e) = grid.at(ahead).occ {
                    en.target = Some(e);
                }
            }
        }
        en.atk -= dt;
        let Some(tgt) = en.target else { continue };
        let Ok(tsp) = qpos.get(tgt) else {
            en.target = None;
            continue;
        };
        let d = tsp.dist(&sp);
        if d > def.range || en.atk > 0.0 {
            continue;
        }
        en.atk = def.cd;
        match en.kind {
            EKind::Spitter => {
                let dir = Vec2::new(tsp.u - sp.u, tsp.v - sp.v).normalize_or_zero();
                spawn_proj(
                    &mut commands,
                    &libs,
                    sp.u,
                    sp.v,
                    sp.h + 1.2,
                    dir,
                    22.0,
                    def.dmg,
                    false,
                    DmgSrc::Enemy,
                    Cid::Green,
                    0.0,
                    false,
                    0.0,
                );
            }
            EKind::Bomber => {
                boom.write(ExplodeEvent {
                    u: sp.u,
                    v: sp.v,
                    radius: 4.0,
                    dmg: def.dmg,
                    src: DmgSrc::Enemy,
                    friendly: false,
                });
                dmg.write(DmgEvent::new(me, 9999.0, DmgSrc::Hazard));
            }
            EKind::Parasite => {
                if let Ok(b) = qb.get(tgt) {
                    if b.kind == BKind::Pylon {
                        disable.write(DisablePylon(tgt));
                    }
                }
                dmg.write(DmgEvent::new(me, 9999.0, DmgSrc::Hazard));
            }
            EKind::Titan => {
                // sweeping blow hits everything around it
                dmg.write(DmgEvent::new(tgt, def.dmg, DmgSrc::Enemy));
                boom.write(ExplodeEvent {
                    u: sp.u,
                    v: sp.v,
                    radius: 3.2,
                    dmg: 14.0,
                    src: DmgSrc::Enemy,
                    friendly: false,
                });
            }
            _ => {
                let mut ev = DmgEvent::new(tgt, def.dmg, DmgSrc::Enemy);
                if en.kind == EKind::Leech {
                    ev.slow = true;
                }
                dmg.write(ev);
                // thorned walls wound melee attackers
                if let Ok(b) = qb.get(tgt) {
                    if matches!(b.kind, BKind::Wall | BKind::Gate) {
                        dmg.write(DmgEvent::new(me, 2.5, DmgSrc::Wall));
                    }
                }
            }
        }
    }
}

/// Null priests project shields onto nearby enemies.
fn priest_aura(
    time: Res<Time>,
    mut acc: Local<f32>,
    mut commands: Commands,
    qp: Query<(&Enemy, &SurfPos)>,
    mut qe: Query<(Entity, &Enemy, &SurfPos, Option<&mut ShieldC>)>,
) {
    *acc += time.delta_secs();
    if *acc < 1.0 {
        return;
    }
    *acc = 0.0;
    let priests: Vec<(f32, f32)> = qp
        .iter()
        .filter(|(e, _)| e.kind == EKind::Priest)
        .map(|(_, sp)| (sp.u, sp.v))
        .collect();
    if priests.is_empty() {
        return;
    }
    for (e, en, sp, shield) in qe.iter_mut() {
        if en.kind == EKind::Priest {
            continue;
        }
        let near = priests
            .iter()
            .any(|(u, v)| Vec2::new(sp.u - u, sp.v - v).length() < 8.0);
        if !near {
            continue;
        }
        match shield {
            Some(mut s) => {
                s.fed = 2.5;
                s.max = s.max.max(20.0);
            }
            None => {
                commands.entity(e).insert(ShieldC {
                    hp: 10.0,
                    max: 20.0,
                    fed: 2.5,
                });
            }
        }
    }
}

fn pylon_disable(
    mut ev: EventReader<DisablePylon>,
    mut grid: ResMut<Grid>,
    mut sim: ResMut<SimStats>,
    mut q: Query<&mut Building>,
) {
    for DisablePylon(e) in ev.read() {
        if let Ok(mut b) = q.get_mut(*e) {
            b.disabled = 14.0;
            grid.power_dirty = true;
            sim.pylon_dmg += 5.0;
        }
    }
}

// ---------------- corruption & nests ----------------
fn corruption_spread(
    time: Res<Time>,
    mut acc: Local<f32>,
    clock: Res<GameClock>,
    evfx: Res<EventFx>,
    mut grid: ResMut<Grid>,
    mut rng: ResMut<RngRes>,
    mut qn: Query<&mut Nest>,
) {
    *acc += time.delta_secs();
    if *acc < 2.0 {
        return;
    }
    *acc = 0.0;
    for mut nest in qn.iter_mut() {
        nest.level = 1 + (clock.day.saturating_sub(1)) / 3;
        let r = 4 + nest.level as i32;
        for _ in 0..2 {
            let c = (
                nest.cell.0 + rng.0.below(r * 2 + 1) - r,
                nest.cell.1 + rng.0.below(r * 2 + 1) - r,
            );
            if !Grid::inb(c) {
                continue;
            }
            let cell = grid.at_mut(c);
            if cell.occ.is_some() {
                continue;
            }
            cell.corrupt = (cell.corrupt + 0.12 * evfx.corrupt).min(1.0);
            if cell.corrupt > 0.9 && cell.terrain != Terrain::Growth {
                cell.terrain = Terrain::Growth;
            }
            grid.mark(c);
        }
    }
}

// ---------------- adaptation report ----------------
fn adaptation_report(
    mut adapt: ResMut<Adapt>,
    mut sim: ResMut<SimStats>,
    clock: Res<GameClock>,
    qb: Query<&Building>,
) {
    let k = adapt.kills;
    let total: f32 = k.iter().sum::<f32>().max(1.0);
    let player_share = (k[DmgSrc::PKin.idx()] + k[DmgSrc::PArc.idx()] + k[DmgSrc::PHam.idx()]) / total;
    let turret_share = k[DmgSrc::Turret.idx()] / total;
    let mortar_share = k[DmgSrc::Mortar.idx()] / total;
    let wall_share = k[DmgSrc::Wall.idx()] / total;
    let conveyors = qb
        .iter()
        .filter(|b| matches!(b.kind, BKind::Conveyor | BKind::Sorter))
        .count();

    let mut w = [1.0f32; 8];
    let mut lines = vec![format!(
        "NIGHT {} ANALYSIS — {} machines destroyed",
        clock.day, total as u32
    )];
    if turret_share > 0.35 {
        w[1] *= 1.8; // armored knights vs turrets
        lines.push(format!(
            "Turrets dominate ({}%). Response: armored Ossuary Knights.",
            (turret_share * 100.0) as u32
        ));
    }
    if wall_share > 0.10 {
        w[4] *= 2.0; // phase leeches vs walls
        lines.push(format!(
            "Wall attrition noted ({}%). Response: Phase Leeches.",
            (wall_share * 100.0) as u32
        ));
    }
    if sim.outage > 15.0 {
        w[7] *= 2.0;
        lines.push(format!(
            "Power grid fragile ({:.0}s of brownout). Response: Signal Parasites.",
            sim.outage
        ));
    }
    if conveyors > 14 {
        w[5] *= 1.8;
        lines.push(format!(
            "Dense logistics ({} nodes). Response: Lattice Bombers.",
            conveyors
        ));
    }
    if player_share > 0.45 {
        w[2] *= 1.7;
        lines.push(format!(
            "Commander lethal at close range ({}%). Response: Choir Spitters.",
            (player_share * 100.0) as u32
        ));
    }
    if mortar_share > 0.30 {
        w[0] *= 1.6;
        lines.push("Mortar clusters effective. Response: dispersed Needle Mite swarms.".into());
    }
    if sim.drone_deaths > 3.0 {
        lines.push(format!("{} worker drones lost. Vulnerability logged.", sim.drone_deaths as u32));
    }
    if lines.len() == 1 {
        lines.push("No dominant defense detected. Standard assault doctrine.".into());
    }
    adapt.w = w;
    adapt.report = lines.join("\n");
    for v in adapt.kills.iter_mut() {
        *v *= 0.25;
    }
    sim.outage = 0.0;
    sim.conveyor_dmg = 0.0;
    sim.pylon_dmg = 0.0;
    sim.drone_deaths = 0.0;
}

pub struct EnemyPlugin;
impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Adapt>()
            .init_resource::<WavePlan>()
            .add_event::<DisablePylon>()
            .add_systems(
                FixedUpdate,
                (
                    flow_recompute,
                    phase_watch,
                    spawner,
                    enemy_decide,
                    priest_aura,
                    pylon_disable,
                    corruption_spread,
                )
                    .run_if(playing),
            )
            .add_systems(Update, enemy_act.run_if(playing))
            .add_systems(OnEnter(AppState::Report), adaptation_report);
    }
}
