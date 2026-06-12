//! Worker drones: spawned by cradles, haul/build/repair/salvage/recharge/flee.

use bevy::prelude::*;

use crate::building::{bdef, BKind, Building};
use crate::components::*;
use crate::events::EventFx;
use crate::player::deposit_carry;
use crate::research::TechFx;
use crate::resources::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DState {
    Idle,
    Haul,
    Build,
    Repair,
    Recharge,
    Flee,
    Salvage,
}

#[derive(Component)]
pub struct Drone {
    pub state: DState,
    pub charge: f32,
    pub carry_kind: usize,
    pub carry_amt: f32,
    pub target: Option<Entity>,
    pub tcell: Option<(i32, i32)>,
    pub decide: f32,
}

#[derive(Component)]
pub struct DroneLight;

pub fn spawn_drone(commands: &mut Commands, libs: &Libs, u: f32, v: f32, charge: f32) -> Entity {
    let e = commands
        .spawn((
            Drone {
                state: DState::Idle,
                charge,
                carry_kind: 0,
                carry_amt: 0.0,
                target: None,
                tcell: None,
                decide: 0.0,
            },
            SurfPos::new(u, v, 1.0),
            Yaw(0.0),
            SurfVel::default(),
            Health::new(30.0),
            Armor(0.0),
            StatusFx::default(),
            Transform::default(),
            Visibility::default(),
            GameEntity,
            Name::new("drone"),
        ))
        .id();
    commands.entity(e).with_children(|p| {
        p.spawn((
            Mesh3d(libs.cap.clone()),
            MeshMaterial3d(libs.mat(Cid::Grey)),
            Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(0.5, 0.45, 0.5)),
        ));
        p.spawn((
            Mesh3d(libs.sph.clone()),
            MeshMaterial3d(libs.mat(Cid::White)),
            Transform::from_xyz(0.0, 0.45, 0.0).with_scale(Vec3::splat(0.4)),
            DroneLight,
        ));
    });
    e
}

fn state_color(s: DState) -> Cid {
    match s {
        DState::Idle => Cid::White,
        DState::Haul => Cid::Yellow,
        DState::Build => Cid::Cyan,
        DState::Repair => Cid::Green,
        DState::Recharge => Cid::Orange,
        DState::Flee => Cid::Red,
        DState::Salvage => Cid::Purple,
    }
}

fn drone_lights(
    libs: Res<Libs>,
    q: Query<(&Drone, &Children), Changed<Drone>>,
    mut ql: Query<&mut MeshMaterial3d<StandardMaterial>, With<DroneLight>>,
) {
    for (d, children) in q.iter() {
        for c in children.iter() {
            if let Ok(mut m) = ql.get_mut(c) {
                m.0 = libs.mat(state_color(d.state));
            }
        }
    }
}

fn cradle_spawn(
    time: Res<Time>,
    mut commands: Commands,
    libs: Res<Libs>,
    stats: Res<PStats>,
    mut bank: ResMut<Bank>,
    mut qc: Query<(&mut Building, &SurfPos)>,
    qd: Query<&Drone>,
) {
    let dt = time.delta_secs();
    let alive = qd.iter().count() as u32;
    let mut cradles = 0u32;
    for (b, _) in qc.iter() {
        if b.kind == BKind::Cradle && b.built >= 1.0 {
            cradles += 1;
        }
    }
    let cap = stats.bandwidth.min(cradles * 3);
    for (mut b, sp) in qc.iter_mut() {
        if b.kind != BKind::Cradle || b.built < 1.0 || b.powered < 0.3 {
            continue;
        }
        b.cd -= dt * b.powered;
        if b.cd <= 0.0 && alive < cap && bank.can_afford(&[(R_CORE, 1.0), (R_ALLOY, 4.0)]) {
            bank.pay(&[(R_CORE, 1.0), (R_ALLOY, 4.0)]);
            spawn_drone(&mut commands, &libs, sp.u + 1.5, sp.v + 1.5, 100.0);
            b.cd = 8.0;
            return; // one per tick keeps counts honest
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn drone_decide(
    time: Res<Time>,
    grid: Res<Grid>,
    bank: Res<Bank>,
    evfx: Res<EventFx>,
    rally: Res<RallyPoint>,
    egrid: Res<EGrid>,
    mut rng: ResMut<RngRes>,
    mut sim: ResMut<SimStats>,
    mut qd: Query<(&mut Drone, &SurfPos)>,
    qb: Query<(Entity, &Building, &SurfPos, &Health)>,
) {
    let dt = time.delta_secs();
    let mut jobs = 0u32;
    for (mut d, sp) in qd.iter_mut() {
        if d.state != DState::Idle {
            jobs += 1;
        }
        d.decide -= dt;
        if d.decide > 0.0 {
            continue;
        }
        d.decide = 0.6;

        if !egrid.near(sp.u, sp.v, 9.0).is_empty() {
            d.state = DState::Flee;
            continue;
        }
        if d.charge < 25.0 {
            if let Some((e, _)) = nearest(&qb, sp, |b, _| b.kind == BKind::Cradle && b.built >= 1.0)
            {
                d.state = DState::Recharge;
                d.target = Some(e);
                continue;
            }
        }
        if evfx.scramble {
            // ghost signal: drones wander uselessly
            d.state = DState::Idle;
            let c = sp.cell();
            d.tcell = Some((
                (c.0 + rng.0.below(11) - 5).clamp(1, crate::MAP - 2),
                (c.1 + rng.0.below(11) - 5).clamp(1, crate::MAP - 2),
            ));
            continue;
        }
        if d.carry_amt > 0.0 {
            if let Some((e, _)) = nearest(&qb, sp, |b, _| {
                matches!(b.kind, BKind::Silo | BKind::Core) && b.built >= 1.0
            }) {
                d.state = DState::Haul;
                d.target = Some(e);
                d.tcell = None;
                continue;
            }
        }
        if let Some((e, _)) = nearest(&qb, sp, |b, _| b.built < 1.0) {
            d.state = DState::Build;
            d.target = Some(e);
            continue;
        }
        if bank.amt[R_SCRAP] > 2.0 {
            if let Some((e, _)) = nearest(&qb, sp, |b, h| b.built >= 1.0 && h.hp < h.max * 0.65) {
                d.state = DState::Repair;
                d.target = Some(e);
                continue;
            }
        }
        if let Some((e, _)) = nearest(&qb, sp, |b, _| {
            b.built >= 1.0 && b.jam && b.buf_out.iter().sum::<f32>() >= 4.0
        }) {
            d.state = DState::Haul;
            d.target = Some(e);
            continue;
        }
        // salvage discovered ruins
        let c0 = sp.cell();
        let mut ruin = None;
        let mut bd = f32::MAX;
        for dy in -28i32..=28 {
            for dx in -28i32..=28 {
                let c = (c0.0 + dx, c0.1 + dy);
                if !Grid::inb(c) {
                    continue;
                }
                let cell = grid.at(c);
                if cell.terrain == Terrain::Ruin && cell.disc && cell.res > 0.0 {
                    let dd = (dx * dx + dy * dy) as f32;
                    if dd < bd {
                        bd = dd;
                        ruin = Some(c);
                    }
                }
            }
        }
        if let Some(c) = ruin {
            d.state = DState::Salvage;
            d.tcell = Some(c);
            d.target = None;
            continue;
        }
        d.state = DState::Idle;
        if let Some((u, v)) = rally.0 {
            let c = cell_of(u, v);
            d.tcell = Some((
                (c.0 + rng.0.below(7) - 3).clamp(1, crate::MAP - 2),
                (c.1 + rng.0.below(7) - 3).clamp(1, crate::MAP - 2),
            ));
        } else {
            d.tcell = None;
        }
    }
    sim.active_jobs = jobs;
}

fn nearest<F: Fn(&Building, &Health) -> bool>(
    qb: &Query<(Entity, &Building, &SurfPos, &Health)>,
    sp: &SurfPos,
    f: F,
) -> Option<(Entity, f32)> {
    let mut best = None;
    for (e, b, bsp, h) in qb.iter() {
        if !f(b, h) {
            continue;
        }
        let d = bsp.dist(sp);
        if best.map_or(true, |(_, bd)| d < bd) {
            best = Some((e, d));
        }
    }
    best
}

#[allow(clippy::too_many_arguments)]
fn drone_act(
    time: Res<Time>,
    fx: Res<TechFx>,
    egrid: Res<EGrid>,
    mut bank: ResMut<Bank>,
    mut sim: ResMut<SimStats>,
    mut qd: Query<(&mut Drone, &mut SurfPos, &mut Yaw), Without<Building>>,
    mut qb: Query<(&mut Building, &SurfPos, &mut Health), With<Building>>,
    mut qgrid: ResMut<Grid>,
) {
    let dt = time.delta_secs();
    for (mut d, mut sp, mut yaw) in qd.iter_mut() {
        let mut goal: Option<(f32, f32)> = None;
        let mut arrive = 2.2;
        match d.state {
            DState::Flee => {
                let foes = egrid.near(sp.u, sp.v, 12.0);
                if let Some((_, fu, fv)) = foes.first() {
                    let away = Vec2::new(sp.u - fu, sp.v - fv).normalize_or_zero();
                    goal = Some((sp.u + away.x * 8.0, sp.v + away.y * 8.0));
                    arrive = 0.5;
                } else {
                    d.state = DState::Idle;
                }
            }
            _ => {
                if let Some(t) = d.target {
                    if let Ok((_, bsp, _)) = qb.get(t) {
                        goal = Some((bsp.u, bsp.v));
                    } else {
                        d.target = None;
                        d.state = DState::Idle;
                    }
                } else if let Some(c) = d.tcell {
                    let (u, v) = cell_center(c);
                    goal = Some((u, v));
                    arrive = 1.2;
                }
            }
        }
        let Some((gu, gv)) = goal else {
            sp.h = qgrid.elev_uv(sp.u, sp.v) + 0.9 + (time.elapsed_secs() * 3.0 + sp.v).sin() * 0.1;
            continue;
        };
        let to = Vec2::new(gu - sp.u, gv - sp.v);
        let dist = to.length();
        if dist > arrive {
            let speed = 7.0 * fx.drone_speed * if d.charge <= 0.5 { 0.4 } else { 1.0 };
            let dir = to.normalize();
            let (nu, nv) = clamp_uv(sp.u + dir.x * speed * dt, sp.v + dir.y * speed * dt);
            if qgrid.walkable(cell_of(nu, sp.v)) {
                sp.u = nu;
            }
            if qgrid.walkable(cell_of(sp.u, nv)) {
                sp.v = nv;
            }
            yaw.0 = (-dir.x).atan2(-dir.y);
            if d.state != DState::Idle {
                d.charge = (d.charge - 0.5 * dt).max(0.0);
            }
        } else {
            // arrived: do the job
            match d.state {
                DState::Build => {
                    if let Some(t) = d.target {
                        if let Ok((mut b, _, mut hp)) = qb.get_mut(t) {
                            if b.built < 1.0 {
                                b.built += dt * 1.2 / bdef(b.kind).btime;
                                if b.built >= 1.0 {
                                    b.built = 1.0;
                                    hp.hp = hp.max;
                                }
                            } else {
                                d.state = DState::Idle;
                            }
                        }
                    }
                }
                DState::Repair => {
                    if let Some(t) = d.target {
                        if let Ok((b, _, mut hp)) = qb.get_mut(t) {
                            let mult = if fx.emerg && hp.hp < hp.max * 0.3 { 2.0 } else { 1.0 };
                            let _ = b;
                            let heal = (14.0 * mult * dt).min(hp.max - hp.hp);
                            if heal <= 0.01 {
                                d.state = DState::Idle;
                            } else if bank.take(R_SCRAP, heal * 0.1) > 0.0 || heal * 0.1 < 0.01 {
                                hp.hp += heal;
                            }
                        }
                    }
                }
                DState::Haul => {
                    if let Some(t) = d.target {
                        if let Ok((mut b, _, _)) = qb.get_mut(t) {
                            if matches!(b.kind, BKind::Silo | BKind::Core) {
                                if d.carry_amt > 0.0 {
                                    let got =
                                        deposit_carry(&mut bank, &mut sim, d.carry_kind, d.carry_amt);
                                    d.carry_amt -= got;
                                }
                                d.state = DState::Idle;
                            } else {
                                // pick up from a jammed machine
                                let mut bestk = None;
                                let mut bestv = 0.0;
                                for k in 0..NRES {
                                    if b.buf_out[k] > bestv {
                                        bestv = b.buf_out[k];
                                        bestk = Some(k);
                                    }
                                }
                                if let Some(k) = bestk {
                                    if d.carry_amt <= 0.0 {
                                        let amt = b.buf_out[k].min(10.0);
                                        b.buf_out[k] -= amt;
                                        d.carry_kind = k;
                                        d.carry_amt = amt;
                                    }
                                }
                                d.state = DState::Idle;
                                d.decide = 0.0;
                            }
                        }
                    }
                }
                DState::Recharge => {
                    d.charge += 14.0 * dt;
                    if d.charge >= 95.0 {
                        d.state = DState::Idle;
                    }
                }
                DState::Salvage => {
                    if let Some(c) = d.tcell {
                        let cell = qgrid.at_mut(c);
                        if cell.res > 0.0 && d.carry_amt < 10.0 {
                            let amt = (2.5 * fx.ruin_yield * dt).min(cell.res);
                            cell.res -= amt;
                            d.carry_kind = R_RELIC;
                            d.carry_amt += amt;
                            if cell.res <= 0.01 {
                                cell.res = 0.0;
                                qgrid.mark(c);
                            }
                        } else {
                            d.state = DState::Idle;
                            d.decide = 0.0;
                        }
                    }
                }
                _ => {}
            }
        }
        sp.h = qgrid.elev_uv(sp.u, sp.v) + 0.9 + (time.elapsed_secs() * 3.0 + sp.v).sin() * 0.1;
    }
}

pub struct WorkerPlugin;
impl Plugin for WorkerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (cradle_spawn, drone_decide).run_if(playing),
        )
        .add_systems(Update, (drone_act, drone_lights).run_if(playing));
    }
}
