//! Combat: projectiles, beams, mortars, explosions, the unified damage
//! pipeline (shields -> armor -> hp -> death), statuses, particles and
//! floating damage numbers.

use bevy::prelude::*;

use crate::building::{bdef, BKind, Building};
use crate::components::*;
use crate::enemies::Enemy;
use crate::research::TechFx;
use crate::resources::*;
use crate::workers::Drone;
use crate::AppState;

#[derive(Resource, Default)]
pub struct ParticleCount(pub i32);

// ---------------- spawn helpers ----------------
#[allow(clippy::too_many_arguments)]
pub fn spawn_proj(
    commands: &mut Commands,
    libs: &Libs,
    u: f32,
    v: f32,
    h: f32,
    dir: Vec2,
    speed: f32,
    dmg: f32,
    friendly: bool,
    src: DmgSrc,
    cid: Cid,
    knock: f32,
    burn: bool,
    aoe: f32,
) {
    commands.spawn((
        Projectile {
            dir,
            speed,
            dmg,
            life: 1.2,
            friendly,
            aoe,
            knock,
            src,
            burn,
        },
        SurfPos::new(u, v, h),
        Yaw((-dir.x).atan2(-dir.y)),
        Mesh3d(libs.cube_s.clone()),
        MeshMaterial3d(libs.mat(cid)),
        Transform::from_scale(Vec3::new(0.45, 0.45, 1.6)),
        Visibility::default(),
        GameEntity,
    ));
}

pub fn spawn_beam(commands: &mut Commands, libs: &Libs, a: Vec3, b: Vec3, cid: Cid, w: f32) {
    let mid = (a + b) * 0.5;
    let d = b - a;
    let len = d.length().max(0.01);
    commands.spawn((
        BeamVis { life: 0.07 },
        Mesh3d(libs.cube.clone()),
        MeshMaterial3d(libs.mat(cid)),
        Transform::from_translation(mid)
            .with_rotation(Quat::from_rotation_arc(Vec3::Z, d / len))
            .with_scale(Vec3::new(w, w, len)),
        Visibility::default(),
        GameEntity,
    ));
}

pub fn spawn_burst(
    commands: &mut Commands,
    libs: &Libs,
    pc: &mut ParticleCount,
    u: f32,
    v: f32,
    h: f32,
    n: usize,
    cid: Cid,
    spd: f32,
) {
    if pc.0 > 420 {
        return;
    }
    for i in 0..n {
        pc.0 += 1;
        let a = i as f32 / n.max(1) as f32 * std::f32::consts::TAU + u * 0.37 + v * 0.71;
        commands.spawn((
            Particle {
                vel: Vec3::new(a.cos() * spd, spd * 0.8 + (a * 3.0).sin().abs() * spd * 0.5, a.sin() * spd),
                life: 0.65,
                grav: 16.0,
            },
            SurfPos::new(u, v, h),
            Mesh3d(libs.cube_s.clone()),
            MeshMaterial3d(libs.mat(cid)),
            Transform::from_scale(Vec3::splat(0.5)),
            Visibility::default(),
            GameEntity,
        ));
    }
}

pub fn spawn_float(commands: &mut Commands, world: Vec3, txt: String, color: Color) {
    commands.spawn((
        Text::new(txt),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(color),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(-100.0),
            top: Val::Px(-100.0),
            ..default()
        },
        FloatDmg { world, t: 0.0 },
        GameEntity,
    ));
}

// ---------------- spatial hash ----------------
fn egrid_rebuild(
    mut egrid: ResMut<EGrid>,
    q: Query<(Entity, &SurfPos), Or<(With<Enemy>, With<Nest>)>>,
) {
    egrid.clear();
    for (e, sp) in q.iter() {
        egrid.insert(e, sp.u, sp.v);
    }
}

// ---------------- turrets ----------------
#[allow(clippy::too_many_arguments)]
fn turret_ai(
    time: Res<Time>,
    fx: Res<TechFx>,
    egrid: Res<EGrid>,
    grid: Res<Grid>,
    mut commands: Commands,
    libs: Res<Libs>,
    mut q: Query<(&mut Building, &SurfPos)>,
    qe: Query<&SurfPos, With<Enemy>>,
    mut dmg: EventWriter<DmgEvent>,
) {
    let dt = time.delta_secs();
    for (mut b, sp) in q.iter_mut() {
        if b.built < 1.0 || b.powered < 0.25 {
            continue;
        }
        match b.kind {
            BKind::Rail => {
                b.cd -= dt * b.powered * fx.rail;
                if b.cd > 0.0 {
                    continue;
                }
                let foes = egrid.near(sp.u, sp.v, 24.0);
                let Some((_, fu, fv)) = foes
                    .iter()
                    .min_by(|a, c| {
                        let da = Vec2::new(a.1 - sp.u, a.2 - sp.v).length_squared();
                        let dc = Vec2::new(c.1 - sp.u, c.2 - sp.v).length_squared();
                        da.total_cmp(&dc)
                    })
                    .copied()
                else {
                    continue;
                };
                b.cd = 0.8;
                let dir = Vec2::new(fu - sp.u, fv - sp.v).normalize_or_zero();
                spawn_proj(
                    &mut commands,
                    &libs,
                    sp.u,
                    sp.v,
                    sp.h + 1.6,
                    dir,
                    55.0,
                    16.0,
                    true,
                    DmgSrc::Turret,
                    Cid::Red,
                    1.0,
                    false,
                    0.0,
                );
            }
            BKind::Arc => {
                b.cd -= dt * b.powered;
                if b.cd > 0.0 {
                    continue;
                }
                let foes = egrid.near(sp.u, sp.v, 13.0);
                if foes.is_empty() {
                    continue;
                }
                b.cd = 1.6;
                let mut prev = surf_to_world(sp.u, sp.v, sp.h + 1.7);
                let chains = 2 + fx.arc_chain as usize;
                for (i, (e, fu, fv)) in foes.iter().enumerate() {
                    if i >= chains {
                        break;
                    }
                    let mut ev = DmgEvent::new(*e, 13.0, DmgSrc::Turret);
                    ev.shock = true;
                    dmg.write(ev);
                    let w = surf_to_world(*fu, *fv, 1.0);
                    spawn_beam(&mut commands, &libs, prev, w, Cid::Cyan, 0.09);
                    prev = w;
                }
            }
            BKind::Mortar => {
                b.cd -= dt * b.powered;
                if b.cd > 0.0 {
                    continue;
                }
                let foes = egrid.near(sp.u, sp.v, 28.0);
                // prefer the densest cluster, ignore foes inside min range
                let mut best: Option<((f32, f32), usize)> = None;
                for (_, fu, fv) in foes.iter() {
                    if Vec2::new(fu - sp.u, fv - sp.v).length() < 9.0 {
                        continue;
                    }
                    let n = foes
                        .iter()
                        .filter(|(_, ou, ov)| Vec2::new(ou - fu, ov - fv).length() < 4.0)
                        .count();
                    if best.map_or(true, |(_, bn)| n > bn) {
                        best = Some(((*fu, *fv), n));
                    }
                }
                let Some(((tu, tv), _)) = best else { continue };
                b.cd = 5.0;
                let shots: i32 = if fx.mortar_cluster { 3 } else { 1 };
                for s in 0..shots {
                    let off = s as f32 * 2.2;
                    let (ou, ov) = if s == 0 {
                        (0.0, 0.0)
                    } else {
                        ((s as f32 * 1.7).sin() * off, (s as f32 * 2.3).cos() * off)
                    };
                    let to = (
                        (tu + ou).clamp(-half_map(), half_map()),
                        (tv + ov).clamp(-half_map(), half_map()),
                    );
                    let d = Vec2::new(to.0 - sp.u, to.1 - sp.v).length();
                    commands.spawn((
                        MortarShell {
                            from: (sp.u, sp.v),
                            to,
                            t: 0.0,
                            dur: (d / 14.0).max(0.6),
                            dmg: 26.0,
                            radius: 4.0,
                            src: DmgSrc::Mortar,
                        },
                        SurfPos::new(sp.u, sp.v, sp.h + 1.5),
                        Mesh3d(libs.sph.clone()),
                        MeshMaterial3d(libs.mat(Cid::Orange)),
                        Transform::from_scale(Vec3::splat(0.5)),
                        Visibility::default(),
                        GameEntity,
                    ));
                }
            }
            _ => {}
        }
        let _ = (&grid, &qe);
    }
}

// ---------------- projectiles ----------------
#[allow(clippy::too_many_arguments)]
fn proj_move(
    time: Res<Time>,
    grid: Res<Grid>,
    egrid: Res<EGrid>,
    mut commands: Commands,
    libs: Res<Libs>,
    mut pc: ResMut<ParticleCount>,
    mut q: Query<(Entity, &mut Projectile, &mut SurfPos)>,
    qp: Query<(Entity, &SurfPos), (With<PlayerTag>, Without<Projectile>)>,
    qd: Query<(Entity, &SurfPos), (With<Drone>, Without<Projectile>)>,
    mut dmg: EventWriter<DmgEvent>,
    mut boom: EventWriter<ExplodeEvent>,
) {
    let dt = time.delta_secs();
    for (e, mut p, mut sp) in q.iter_mut() {
        p.life -= dt;
        let (nu, nv) = clamp_uv(sp.u + p.dir.x * p.speed * dt, sp.v + p.dir.y * p.speed * dt);
        sp.u = nu;
        sp.v = nv;
        sp.h = grid.elev_uv(nu, nv) + 1.2;
        let mut hit: Option<Entity> = None;
        if p.friendly {
            let foes = egrid.near(sp.u, sp.v, 1.2);
            if let Some((fe, _, _)) = foes.first() {
                hit = Some(*fe);
            }
        } else {
            if let Ok((pe, psp)) = qp.single() {
                if psp.dist(&sp) < 1.4 {
                    hit = Some(pe);
                }
            }
            if hit.is_none() {
                for (de, dsp) in qd.iter() {
                    if dsp.dist(&sp) < 1.0 {
                        hit = Some(de);
                        break;
                    }
                }
            }
            if hit.is_none() {
                let c = sp.cell();
                if Grid::inb(c) && !grid.at(c).conv {
                    if let Some(be) = grid.at(c).occ {
                        hit = Some(be);
                    }
                }
            }
        }
        if let Some(t) = hit {
            let mut ev = DmgEvent::new(t, p.dmg, p.src);
            ev.burn = p.burn;
            ev.knock = p.dir * p.knock;
            dmg.write(ev);
            if p.aoe > 0.0 {
                boom.write(ExplodeEvent {
                    u: sp.u,
                    v: sp.v,
                    radius: p.aoe,
                    dmg: p.dmg * 0.6,
                    src: p.src,
                    friendly: true,
                });
            }
            spawn_burst(&mut commands, &libs, &mut pc, sp.u, sp.v, sp.h, 3, Cid::White, 3.0);
            commands.entity(e).despawn();
        } else if p.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

fn mortar_fly(
    time: Res<Time>,
    grid: Res<Grid>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut MortarShell, &mut SurfPos)>,
    mut boom: EventWriter<ExplodeEvent>,
) {
    let dt = time.delta_secs();
    for (e, mut m, mut sp) in q.iter_mut() {
        m.t += dt / m.dur;
        let t = m.t.min(1.0);
        sp.u = m.from.0 + (m.to.0 - m.from.0) * t;
        sp.v = m.from.1 + (m.to.1 - m.from.1) * t;
        sp.h = grid.elev_uv(sp.u, sp.v) + 1.0 + (std::f32::consts::PI * t).sin() * 9.0;
        if m.t >= 1.0 {
            boom.write(ExplodeEvent {
                u: m.to.0,
                v: m.to.1,
                radius: m.radius,
                dmg: m.dmg,
                src: m.src,
                friendly: true, // mortars do not discriminate
            });
            commands.entity(e).despawn();
        }
    }
}

// ---------------- explosions ----------------
#[allow(clippy::too_many_arguments)]
fn explosions(
    mut ev: EventReader<ExplodeEvent>,
    grid: Res<Grid>,
    egrid: Res<EGrid>,
    mut commands: Commands,
    libs: Res<Libs>,
    mut pc: ResMut<ParticleCount>,
    qp: Query<(Entity, &SurfPos), With<PlayerTag>>,
    qd: Query<(Entity, &SurfPos), With<Drone>>,
    mut dmg: EventWriter<DmgEvent>,
) {
    for ex in ev.read() {
        spawn_burst(
            &mut commands,
            &libs,
            &mut pc,
            ex.u,
            ex.v,
            grid.elev_uv(ex.u, ex.v) + 1.0,
            (ex.radius * 3.0) as usize,
            Cid::Orange,
            7.0,
        );
        commands.spawn((
            PointLight {
                intensity: 2_000_000.0,
                color: Color::srgb(1.0, 0.6, 0.2),
                range: 28.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(surf_to_world(ex.u, ex.v, 2.5)),
            FlashLight { t: 0.3 },
            GameEntity,
        ));
        // enemies & nests
        for (fe, _, _) in egrid.near(ex.u, ex.v, ex.radius) {
            let mut d = DmgEvent::new(fe, ex.dmg, ex.src);
            d.burn = ex.src == DmgSrc::Mortar;
            dmg.write(d);
        }
        // buildings
        if ex.friendly || ex.src == DmgSrc::Enemy || ex.src == DmgSrc::Hazard {
            let c0 = cell_of(ex.u, ex.v);
            let r = (ex.radius / crate::CELL).ceil() as i32;
            let mut seen: Vec<Entity> = Vec::new();
            for dy in -r..=r {
                for dx in -r..=r {
                    let c = (c0.0 + dx, c0.1 + dy);
                    if !Grid::inb(c) {
                        continue;
                    }
                    let (cu, cv) = cell_center(c);
                    if Vec2::new(cu - ex.u, cv - ex.v).length() > ex.radius {
                        continue;
                    }
                    if let Some(be) = grid.at(c).occ {
                        if !seen.contains(&be) {
                            seen.push(be);
                            dmg.write(DmgEvent::new(be, ex.dmg * 0.8, ex.src));
                        }
                    }
                }
            }
        }
        // player & drones
        if let Ok((pe, psp)) = qp.single() {
            if psp.dist_uv(ex.u, ex.v) < ex.radius {
                dmg.write(DmgEvent::new(pe, ex.dmg * 0.8, ex.src));
            }
        }
        for (de, dsp) in qd.iter() {
            if dsp.dist_uv(ex.u, ex.v) < ex.radius {
                dmg.write(DmgEvent::new(de, ex.dmg, ex.src));
            }
        }
    }
}

// ---------------- damage pipeline ----------------
type DmgQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Health,
        Option<&'static mut ShieldC>,
        Option<&'static Armor>,
        Option<&'static mut StatusFx>,
        Option<&'static mut SurfVel>,
        &'static SurfPos,
        Option<&'static Enemy>,
        Option<&'static Building>,
        Option<&'static Drone>,
        Option<&'static PlayerTag>,
        Option<&'static Nest>,
    ),
>;

#[allow(clippy::too_many_arguments)]
fn apply_damage(
    mut ev: EventReader<DmgEvent>,
    mut q: DmgQuery,
    mut commands: Commands,
    libs: Res<Libs>,
    mut pc: ResMut<ParticleCount>,
    mut grid: ResMut<Grid>,
    mut adapt: ResMut<crate::enemies::Adapt>,
    mut sim: ResMut<SimStats>,
    mut bank: ResMut<Bank>,
    mut notify: EventWriter<Notify>,
    mut endmsg: ResMut<EndMsg>,
    mut next: ResMut<NextState<AppState>>,
) {
    for e in ev.read() {
        let Ok((ent, mut hp, shield, armor, status, vel, sp, enemy, building, drone, player, nest)) =
            q.get_mut(e.target)
        else {
            continue;
        };
        if hp.hp <= 0.0 {
            continue;
        }
        let mut d = e.dmg;
        if let Some(mut s) = shield {
            if s.hp > 0.0 {
                let absorb = s.hp.min(d);
                s.hp -= absorb;
                d -= absorb;
            }
        }
        if d > 0.0 {
            if let Some(a) = armor {
                if a.0 > 0.0 {
                    d = (d - a.0).max(e.dmg * 0.15);
                }
            }
        }
        hp.hp -= d;
        if let Some(mut st) = status {
            if e.burn {
                st.burn = 3.0;
            }
            if e.shock {
                st.shock = 0.8;
            }
            if e.slow {
                st.slow = 2.0;
            }
        }
        if let Some(mut v) = vel {
            v.0.x += e.knock.x;
            v.0.z += e.knock.y;
        }
        if d >= 1.0 && (enemy.is_some() || player.is_some() || nest.is_some()) {
            let color = if player.is_some() {
                Color::srgb(1.0, 0.3, 0.3)
            } else {
                Color::srgb(1.0, 0.9, 0.5)
            };
            spawn_float(&mut commands, sp.world() + Vec3::Y * 2.0, format!("{}", d as i32), color);
        }
        if d >= 1.0 {
            spawn_burst(&mut commands, &libs, &mut pc, sp.u, sp.v, sp.h + 1.0, 1, Cid::White, 2.0);
        }
        if let Some(b) = building {
            if matches!(b.kind, BKind::Conveyor | BKind::Sorter) {
                sim.conveyor_dmg += d;
            }
            if b.kind == BKind::Pylon {
                sim.pylon_dmg += d;
            }
        }

        if hp.hp > 0.0 {
            continue;
        }
        // ---- death ----
        if enemy.is_some() {
            adapt.kills[e.src.idx()] += 1.0;
            bank.add(R_SCRAP, 0.5);
            spawn_burst(&mut commands, &libs, &mut pc, sp.u, sp.v, sp.h + 1.0, 6, Cid::Red, 5.0);
            commands.entity(ent).despawn();
        } else if nest.is_some() {
            bank.add(R_RELIC, 25.0);
            notify.write(Notify("Nest destroyed! +25 relic dust".into()));
            spawn_burst(&mut commands, &libs, &mut pc, sp.u, sp.v, sp.h + 1.0, 18, Cid::Purple, 8.0);
            commands.entity(ent).despawn();
        } else if drone.is_some() {
            sim.drone_deaths += 1.0;
            spawn_burst(&mut commands, &libs, &mut pc, sp.u, sp.v, sp.h + 1.0, 5, Cid::Yellow, 4.0);
            commands.entity(ent).despawn();
        } else if let Some(b) = building {
            for c in crate::building::foot_cells(b.kind, b.cell) {
                if Grid::inb(c) && grid.at(c).occ == Some(ent) {
                    let cell = grid.at_mut(c);
                    cell.occ = None;
                    cell.conv = false;
                    grid.mark(c);
                }
            }
            grid.flow_dirty = true;
            grid.power_dirty = true;
            spawn_burst(&mut commands, &libs, &mut pc, sp.u, sp.v, sp.h + 1.0, 12, Cid::Orange, 7.0);
            notify.write(Notify(format!("{} DESTROYED", bdef(b.kind).name)));
            if b.kind == BKind::Core {
                endmsg.0 = "The Core Obelisk has fallen. The necropolis claims another expedition.".into();
                next.set(AppState::GameOver);
            }
            commands.entity(ent).despawn();
        }
        // player death handled by the respawn system
    }
}

// ---------------- statuses ----------------
fn status_tick(
    time: Res<Time>,
    grid: Res<Grid>,
    mut q: Query<(Entity, &mut StatusFx, &SurfPos, Option<&PlayerTag>, Option<&Drone>)>,
    mut dmg: EventWriter<DmgEvent>,
) {
    let dt = time.delta_secs();
    for (e, mut st, sp, player, drone) in q.iter_mut() {
        if (player.is_some() || drone.is_some()) && Grid::inb(sp.cell()) {
            if grid.at(sp.cell()).corrupt > 0.5 {
                st.corrupt = 1.0;
            }
        }
        if st.burn > 0.0 {
            st.burn -= dt;
            dmg.write(DmgEvent::new(e, 4.0 * dt, DmgSrc::Hazard));
        }
        if st.corrupt > 0.0 {
            st.corrupt -= dt;
            dmg.write(DmgEvent::new(e, 1.5 * dt, DmgSrc::Hazard));
        }
        st.slow = (st.slow - dt).max(0.0);
        st.shock = (st.shock - dt).max(0.0);
    }
}

// ---------------- visuals ----------------
fn particle_update(
    time: Res<Time>,
    mut pc: ResMut<ParticleCount>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Particle, &mut SurfPos)>,
    qp: Query<&SurfPos, (With<PlayerTag>, Without<Particle>)>,
) {
    let dt = time.delta_secs();
    let ppos = qp.single().ok().map(|s| (s.u, s.v));
    for (e, mut p, mut sp) in q.iter_mut() {
        p.life -= dt;
        p.vel.y -= p.grav * dt;
        sp.u += p.vel.x * dt;
        sp.h = (sp.h + p.vel.y * dt).max(0.05);
        sp.v += p.vel.z * dt;
        let far = ppos.map_or(false, |(pu, pv)| sp.dist_uv(pu, pv) > 90.0);
        if p.life <= 0.0 || far {
            pc.0 -= 1;
            commands.entity(e).despawn();
        }
    }
}

fn beam_fade(time: Res<Time>, mut commands: Commands, mut q: Query<(Entity, &mut BeamVis)>) {
    for (e, mut b) in q.iter_mut() {
        b.life -= time.delta_secs();
        if b.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

fn flash_fade(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut FlashLight, &mut PointLight)>,
) {
    for (e, mut f, mut l) in q.iter_mut() {
        f.t -= time.delta_secs();
        l.intensity *= 0.82;
        if f.t <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

fn float_update(
    time: Res<Time>,
    mut commands: Commands,
    qc: Query<(&Camera, &GlobalTransform), With<CamTag>>,
    mut q: Query<(Entity, &mut FloatDmg, &mut Node, &mut TextColor)>,
) {
    let Ok((cam, gt)) = qc.single() else { return };
    let dt = time.delta_secs();
    for (e, mut f, mut node, mut color) in q.iter_mut() {
        f.t += dt;
        if f.t > 0.9 {
            commands.entity(e).despawn();
            continue;
        }
        match cam.world_to_viewport(gt, f.world) {
            Ok(pos) => {
                node.left = Val::Px(pos.x);
                node.top = Val::Px(pos.y - f.t * 40.0);
                color.0 = color.0.with_alpha(1.0 - f.t);
            }
            Err(_) => {
                node.left = Val::Px(-200.0);
            }
        }
    }
}

pub struct CombatPlugin;
impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleCount>()
            .add_systems(
                Update,
                (
                    egrid_rebuild,
                    proj_move,
                    mortar_fly,
                    explosions,
                    apply_damage,
                    particle_update,
                    beam_fade,
                    flash_fade,
                    float_update,
                )
                    .chain()
                    .run_if(playing),
            )
            .add_systems(FixedUpdate, (turret_ai, status_tick).run_if(playing));
    }
}
