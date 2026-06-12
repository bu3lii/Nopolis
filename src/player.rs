//! Player: armored salvage commander. Movement on the curved hull, weapons,
//! mining/interaction, vital stats.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::building::{BKind, BuildSel, Building};
use crate::camera::cursor_uv;
use crate::combat::{spawn_beam, spawn_burst, spawn_proj, ParticleCount};
use crate::components::*;
use crate::enemies::Enemy;
use crate::events::EventFx;
use crate::research::TechFx;
use crate::resources::*;
use crate::ui::UiHover;

pub fn spawn_player(commands: &mut Commands, libs: &Libs, u: f32, v: f32) -> Entity {
    let e = commands
        .spawn((
            PlayerTag,
            SurfPos::new(u, v, 0.4),
            Yaw(0.0),
            SurfVel::default(),
            Health::new(110.0),
            Armor(2.0),
            StatusFx::default(),
            Transform::default(),
            Visibility::default(),
            GameEntity,
            Name::new("commander"),
        ))
        .id();
    commands.entity(e).with_children(|p| {
        p.spawn((
            Mesh3d(libs.cap.clone()),
            MeshMaterial3d(libs.mat(Cid::Steel)),
            Transform::from_xyz(0.0, 0.85, 0.0).with_scale(Vec3::new(0.9, 0.85, 0.9)),
        ));
        p.spawn((
            Mesh3d(libs.sph.clone()),
            MeshMaterial3d(libs.mat(Cid::Cyan)),
            Transform::from_xyz(0.0, 1.62, 0.0).with_scale(Vec3::splat(0.5)),
        ));
        p.spawn((
            Mesh3d(libs.cube.clone()),
            MeshMaterial3d(libs.mat(Cid::Dark)),
            Transform::from_xyz(0.0, 1.0, 0.3).with_scale(Vec3::new(0.6, 0.7, 0.3)),
        ));
        p.spawn((
            Mesh3d(libs.cone.clone()),
            MeshMaterial3d(libs.mat(Cid::Orange)),
            Transform::from_xyz(0.0, 0.55, 0.42)
                .with_scale(Vec3::new(0.25, 0.5, 0.25))
                .with_rotation(Quat::from_rotation_x(2.6)),
        ));
    });
    e
}

fn cam_basis(yaw: f32) -> (Vec2, Vec2) {
    // forward / right in (u, v) space for the current camera yaw
    (
        Vec2::new(-yaw.sin(), -yaw.cos()),
        Vec2::new(yaw.cos(), -yaw.sin()),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    cam: Res<CamCtl>,
    grid: Res<Grid>,
    evfx: Res<EventFx>,
    fx: Res<TechFx>,
    mut stats: ResMut<PStats>,
    mut q: Query<(&mut SurfPos, &mut SurfVel, &mut Yaw, &StatusFx), With<PlayerTag>>,
) {
    let Ok((mut sp, mut vel, mut yaw, st)) = q.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let (fwd, right) = cam_basis(cam.yaw);
    let mut wish = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        wish += fwd;
    }
    if keys.pressed(KeyCode::KeyS) {
        wish -= fwd;
    }
    if keys.pressed(KeyCode::KeyD) {
        wish += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        wish -= right;
    }
    wish = wish.normalize_or_zero();

    let cell = sp.cell();
    let mut speed = 8.5;
    if Grid::inb(cell) {
        let c = grid.at(cell);
        if c.terrain == Terrain::Marsh {
            speed *= if evfx.flood_slow { 0.4 } else { 0.55 };
        }
        if c.corrupt > 0.4 {
            speed *= 0.55 + 0.25 * fx.corrupt_res;
        }
    }
    if st.slow > 0.0 {
        speed *= 0.55;
    }
    if stats.carry_amt > stats.carry_cap * 0.6 {
        speed *= 0.85;
    }

    stats.dash_cd = (stats.dash_cd - dt).max(0.0);
    stats.jet_cd = (stats.jet_cd - dt).max(0.0);
    if keys.just_pressed(KeyCode::ShiftLeft)
        && stats.dash_cd <= 0.0
        && stats.stam >= 25.0
        && wish.length_squared() > 0.0
    {
        stats.stam -= 25.0;
        stats.dash_cd = 1.3;
        vel.0.x += wish.x * 26.0;
        vel.0.z += wish.y * 26.0;
    }
    let grounded = sp.h <= grid.elev_uv(sp.u, sp.v) + 0.45;
    if keys.just_pressed(KeyCode::Space) && stats.jet_cd <= 0.0 && stats.stam >= 35.0 && grounded {
        stats.stam -= 35.0;
        stats.jet_cd = 2.5;
        vel.0.y = 11.0;
    }

    // integrate: gravity toward the hull
    vel.0.y -= 26.0 * dt;
    let imp = Vec2::new(vel.0.x, vel.0.z);
    let total = wish * speed + imp;
    let (nu, nv) = clamp_uv(sp.u + total.x * dt, sp.v + total.y * dt);
    // axis-separated collision against unwalkable cells
    if grid.walkable(cell_of(nu, sp.v)) || sp.h > grid.elev_uv(nu, sp.v) + 1.5 {
        sp.u = nu;
    }
    if grid.walkable(cell_of(sp.u, nv)) || sp.h > grid.elev_uv(sp.u, nv) + 1.5 {
        sp.v = nv;
    }
    sp.h += vel.0.y * dt;
    let floor = grid.elev_uv(sp.u, sp.v) + 0.4;
    if sp.h < floor {
        sp.h = floor;
        vel.0.y = 0.0;
    }
    vel.0.x *= (1.0f32 - 7.0 * dt).max(0.0);
    vel.0.z *= (1.0f32 - 7.0 * dt).max(0.0);

    if wish.length_squared() > 0.01 {
        yaw.0 = (-wish.x).atan2(-wish.y);
    }
    if wish.length_squared() < 0.01 || !keys.pressed(KeyCode::ShiftLeft) {
        stats.stam = (stats.stam + 14.0 * dt).min(stats.max_stam * fx.stam);
    }
    stats.max_stam = 100.0 * fx.stam;
}

fn vitals(
    time: Res<Time>,
    fx: Res<TechFx>,
    mut stats: ResMut<PStats>,
    qb: Query<(&Building, &SurfPos)>,
    mut qp: Query<(&mut Health, &SurfPos), With<PlayerTag>>,
) {
    let dt = time.delta_secs();
    stats.heat = (stats.heat - 9.0 * dt).max(0.0);
    let Ok((mut hp, sp)) = qp.single_mut() else {
        return;
    };
    // oxygen: drains in the field, recovers near the core obelisk
    let near_core = qb.iter().any(|(b, bsp)| {
        b.kind == BKind::Core && bsp.dist(sp) < 14.0
    });
    if near_core {
        stats.oxy = (stats.oxy + 7.0 * dt).min(stats.max_oxy);
        hp.hp = (hp.hp + 2.0 * dt).min(hp.max);
    } else {
        stats.oxy -= 0.45 * fx.oxy * dt;
    }
    stats.hp = hp.hp;
    stats.max_hp = hp.max;
}

fn respawn(
    mut qp: Query<(&mut Health, &mut SurfPos), With<PlayerTag>>,
    qb: Query<(&Building, &SurfPos), Without<PlayerTag>>,
    mut stats: ResMut<PStats>,
    mut notify: EventWriter<Notify>,
) {
    let Ok((mut hp, mut sp)) = qp.single_mut() else {
        return;
    };
    if hp.hp > 0.0 {
        return;
    }
    let home = qb
        .iter()
        .find(|(b, _)| b.kind == BKind::Core)
        .map(|(_, s)| (s.u, s.v))
        .unwrap_or((0.0, 0.0));
    sp.u = home.0 + 3.0;
    sp.v = home.1 + 3.0;
    sp.h = 1.0;
    hp.hp = hp.max * 0.5;
    stats.oxy = (stats.oxy - 15.0).max(5.0);
    stats.carry_amt *= 0.5;
    notify.write(Notify("You were torn apart. Emergency reconstruction at core (-15 oxygen)".into()));
}

// ---------------- interaction ----------------
pub fn deposit_carry(bank: &mut Bank, sim: &mut SimStats, kind: usize, amt: f32) -> f32 {
    let got = bank.add(kind, amt);
    if kind == R_RELIC && got > 0.0 {
        sim.relic_acc += got;
        let cores = (sim.relic_acc / 8.0).floor();
        if cores >= 1.0 {
            sim.relic_acc -= cores * 8.0;
            bank.add(R_CORE, cores);
        }
    }
    got
}

#[allow(clippy::too_many_arguments)]
fn interact_tap(
    keys: Res<ButtonInput<KeyCode>>,
    mut bank: ResMut<Bank>,
    mut sim: ResMut<SimStats>,
    mut stats: ResMut<PStats>,
    mut ui: ResMut<UiState>,
    qp: Query<&SurfPos, With<PlayerTag>>,
    mut qb: Query<(&mut Building, &SurfPos)>,
    mut notify: EventWriter<Notify>,
) {
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Ok(psp) = qp.single() else { return };

    // priority: reliquary > sorter/loom toggles > collect > deposit
    let mut best: Option<(f32, Mut<Building>)> = None;
    for (b, sp) in qb.iter_mut() {
        let d = sp.dist(psp);
        if d < 5.0 && b.built >= 1.0 && best.as_ref().map_or(true, |(bd, _)| d < *bd) {
            best = Some((d, b));
        }
    }
    if let Some((_, mut b)) = best {
        match b.kind {
            BKind::Reliquary => {
                ui.research_open = !ui.research_open;
                return;
            }
            BKind::Sorter => {
                b.filter = if b.filter >= NRES { 0 } else { b.filter + 1 };
                let label = if b.filter >= NRES {
                    "ANY"
                } else {
                    RES_NAMES[b.filter]
                };
                notify.write(Notify(format!("Sorter filter: {label}")));
                return;
            }
            BKind::Loom => {
                b.recipe = 1 - b.recipe;
                notify.write(Notify(format!(
                    "Loom recipe: {}",
                    if b.recipe == 0 { "Circuits" } else { "Launch Parts" }
                )));
                return;
            }
            _ => {
                let total: f32 = b.buf_out.iter().sum();
                if total > 0.2 {
                    let mut msg = String::from("Collected:");
                    for k in 0..NRES {
                        if b.buf_out[k] > 0.0 {
                            let got = deposit_carry(&mut bank, &mut sim, k, b.buf_out[k]);
                            b.buf_out[k] -= got;
                            msg.push_str(&format!(" +{:.0} {}", got, RES_NAMES[k]));
                        }
                    }
                    notify.write(Notify(msg));
                    return;
                }
                if matches!(b.kind, BKind::Core | BKind::Silo) && stats.carry_amt > 0.0 {
                    let got = deposit_carry(&mut bank, &mut sim, stats.carry_kind, stats.carry_amt);
                    notify.write(Notify(format!(
                        "Deposited {:.0} {}",
                        got, RES_NAMES[stats.carry_kind]
                    )));
                    stats.carry_amt -= got;
                    return;
                }
            }
        }
    } else if stats.carry_amt > 0.0 {
        notify.write(Notify("Deposit at the core or a silo (walk closer, press E)".into()));
    }
}

fn mine_hold(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut grid: ResMut<Grid>,
    mut stats: ResMut<PStats>,
    fx: Res<TechFx>,
    qp: Query<&SurfPos, With<PlayerTag>>,
    mut commands: Commands,
    libs: Res<Libs>,
    mut pc: ResMut<ParticleCount>,
) {
    if !keys.pressed(KeyCode::KeyE) {
        return;
    }
    let Ok(psp) = qp.single() else { return };
    let dt = time.delta_secs();
    let pc0 = psp.cell();
    let mut target: Option<(i32, i32)> = None;
    'outer: for dy in -1..=1 {
        for dx in -1..=1 {
            let c = (pc0.0 + dx, pc0.1 + dy);
            if Grid::inb(c) && grid.at(c).res > 0.0 && grid.at(c).occ.is_none() {
                target = Some(c);
                break 'outer;
            }
        }
    }
    let Some(c) = target else { return };
    let kind = grid.at(c).res_kind;
    if stats.carry_amt > 0.05 && stats.carry_kind != kind {
        return;
    }
    let rate = if grid.at(c).terrain == Terrain::Ruin {
        3.0 * fx.ruin_yield
    } else {
        5.0
    };
    let amt = (rate * dt)
        .min(grid.at(c).res)
        .min(stats.carry_cap - stats.carry_amt);
    if amt <= 0.0 {
        return;
    }
    grid.at_mut(c).res -= amt;
    stats.carry_kind = kind;
    stats.carry_amt += amt;
    if grid.at(c).res <= 0.01 {
        grid.at_mut(c).res = 0.0;
        grid.mark(c);
    }
    if (time.elapsed_secs() * 6.0) as u32 % 3 == 0 {
        let (u, v) = cell_center(c);
        spawn_burst(&mut commands, &libs, &mut pc, u, v, grid.at(c).elev + 0.6, 1, Cid::Yellow, 2.5);
    }
}

#[allow(clippy::too_many_arguments)]
fn command_click(
    buttons: Res<ButtonInput<MouseButton>>,
    hover: Res<UiHover>,
    sel: Res<BuildSel>,
    mut ui: ResMut<UiState>,
    mut rally: ResMut<RallyPoint>,
    grid: Res<Grid>,
    qc: Query<(&Camera, &GlobalTransform), With<CamTag>>,
    qw: Query<&Window, With<PrimaryWindow>>,
    mut qm: Query<&mut SurfPos, With<RallyMarker>>,
    mut commands: Commands,
    libs: Res<Libs>,
) {
    if !buttons.just_pressed(MouseButton::Right) || hover.0 || sel.kind.is_some() {
        return;
    }
    let Some((u, v)) = cursor_uv(&qc, &qw) else {
        return;
    };
    let c = cell_of(u, v);
    if Grid::inb(c) {
        if let Some(e) = grid.at(c).occ {
            ui.sel = Some(e);
            return;
        }
    }
    ui.sel = None;
    rally.0 = Some((u, v));
    if let Ok(mut sp) = qm.single_mut() {
        sp.u = u;
        sp.v = v;
        sp.h = grid.elev_uv(u, v);
    } else {
        let e = commands
            .spawn((
                RallyMarker,
                SurfPos::new(u, v, grid.elev_uv(u, v)),
                Transform::default(),
                Visibility::default(),
                GameEntity,
            ))
            .id();
        commands.entity(e).with_children(|p| {
            p.spawn((
                Mesh3d(libs.cyl.clone()),
                MeshMaterial3d(libs.mat(Cid::Steel)),
                Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::new(0.1, 2.0, 0.1)),
            ));
            p.spawn((
                Mesh3d(libs.sph.clone()),
                MeshMaterial3d(libs.mat(Cid::Orange)),
                Transform::from_xyz(0.0, 2.1, 0.0).with_scale(Vec3::splat(0.5)),
            ));
        });
    }
}

// ---------------- weapons ----------------
fn fire_allowed(ui: &UiState, hover: &UiHover, sel: &BuildSel) -> bool {
    !ui.research_open && !ui.trade_open && !hover.0 && sel.kind.is_none()
}

fn weapon_switch(keys: Res<ButtonInput<KeyCode>>, mut ws: ResMut<WeaponState>, mut notify: EventWriter<Notify>) {
    for (key, idx) in [
        (KeyCode::Digit1, 0usize),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
    ] {
        if keys.just_pressed(key) && ws.cur != idx {
            ws.cur = idx;
            ws.cd = 0.2;
            notify.write(Notify(format!("Equipped {}", WEAPON_NAMES[idx])));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fire_kinetic(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    ui: Res<UiState>,
    hover: Res<UiHover>,
    sel: Res<BuildSel>,
    mut ws: ResMut<WeaponState>,
    mut rng: ResMut<RngRes>,
    mut cam: ResMut<CamCtl>,
    qp: Query<(&SurfPos, &Yaw), With<PlayerTag>>,
    qc: Query<(&Camera, &GlobalTransform), With<CamTag>>,
    qw: Query<&Window, With<PrimaryWindow>>,
    mut commands: Commands,
    libs: Res<Libs>,
    mut pc: ResMut<ParticleCount>,
) {
    let dt = time.delta_secs();
    ws.cd = (ws.cd - dt).max(0.0);
    if ws.reload > 0.0 {
        ws.reload -= dt;
        if ws.reload <= 0.0 {
            ws.ammo = ws.mag;
        }
    }
    if ws.cur != 0 || !fire_allowed(&ui, &hover, &sel) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyR) && ws.ammo < ws.mag && ws.reload <= 0.0 {
        ws.reload = 1.4;
    }
    if !buttons.pressed(MouseButton::Left) || ws.cd > 0.0 || ws.reload > 0.0 {
        return;
    }
    if ws.ammo <= 0 {
        ws.reload = 1.4;
        return;
    }
    let Ok((psp, _)) = qp.single() else { return };
    let Some((au, av)) = cursor_uv(&qc, &qw) else {
        return;
    };
    let mut dir = Vec2::new(au - psp.u, av - psp.v).normalize_or_zero();
    if dir == Vec2::ZERO {
        return;
    }
    let spread = rng.0.range(-0.05, 0.05);
    let (s, c) = spread.sin_cos();
    dir = Vec2::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c);
    spawn_proj(
        &mut commands,
        &libs,
        psp.u,
        psp.v,
        psp.h + 1.2,
        dir,
        46.0,
        9.0,
        true,
        DmgSrc::PKin,
        Cid::Yellow,
        1.5,
        false,
        0.0,
    );
    spawn_burst(&mut commands, &libs, &mut pc, psp.u + dir.x, psp.v + dir.y, psp.h + 1.2, 1, Cid::Orange, 1.0);
    ws.ammo -= 1;
    ws.cd = 0.13;
    cam.pitch = (cam.pitch + 0.006).min(1.35); // recoil
}

#[allow(clippy::too_many_arguments)]
fn fire_arc(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    ui: Res<UiState>,
    hover: Res<UiHover>,
    sel: Res<BuildSel>,
    ws: Res<WeaponState>,
    fx: Res<TechFx>,
    mut stats: ResMut<PStats>,
    mut locked: Local<bool>,
    qp: Query<&SurfPos, With<PlayerTag>>,
    qe: Query<(Entity, &SurfPos), With<Enemy>>,
    qc: Query<(&Camera, &GlobalTransform), With<CamTag>>,
    qw: Query<&Window, With<PrimaryWindow>>,
    mut dmg: EventWriter<DmgEvent>,
    mut commands: Commands,
    libs: Res<Libs>,
) {
    if stats.heat >= 99.0 {
        *locked = true;
    }
    if stats.heat < 35.0 {
        *locked = false;
    }
    if ws.cur != 1
        || !buttons.pressed(MouseButton::Left)
        || *locked
        || !fire_allowed(&ui, &hover, &sel)
    {
        return;
    }
    let Ok(psp) = qp.single() else { return };
    let Some((au, av)) = cursor_uv(&qc, &qw) else {
        return;
    };
    let dt = time.delta_secs();
    stats.heat = (stats.heat + 17.0 * fx.arc_heat * dt).min(100.0);
    let aim = Vec2::new(au - psp.u, av - psp.v).normalize_or_zero();

    // nearest enemy roughly along the aim direction
    let mut best: Option<(Entity, f32, f32, f32)> = None;
    for (e, sp) in qe.iter() {
        let to = Vec2::new(sp.u - psp.u, sp.v - psp.v);
        let d = to.length();
        if d > 24.0 || d < 0.5 {
            continue;
        }
        if to.normalize().dot(aim) < 0.86 {
            continue;
        }
        if best.map_or(true, |(_, bd, _, _)| d < bd) {
            best = Some((e, d, sp.u, sp.v));
        }
    }
    let from = surf_to_world(psp.u, psp.v, psp.h + 1.4);
    match best {
        Some((e, _, eu, ev)) => {
            let mut ev0 = DmgEvent::new(e, 30.0 * dt, DmgSrc::PArc);
            ev0.shock = true;
            dmg.write(ev0);
            let hitw = surf_to_world(eu, ev, 1.0);
            spawn_beam(&mut commands, &libs, from, hitw, Cid::Cyan, 0.10);
            // chain to nearby enemies
            let mut prev = (eu, ev);
            let mut chained = 0u32;
            for (ce, sp) in qe.iter() {
                if chained >= fx.arc_chain || ce == e {
                    continue;
                }
                let d = Vec2::new(sp.u - prev.0, sp.v - prev.1).length();
                if d < 7.0 {
                    let mut cv = DmgEvent::new(ce, 18.0 * dt, DmgSrc::PArc);
                    cv.shock = true;
                    dmg.write(cv);
                    spawn_beam(
                        &mut commands,
                        &libs,
                        surf_to_world(prev.0, prev.1, 1.0),
                        surf_to_world(sp.u, sp.v, 1.0),
                        Cid::Cyan,
                        0.07,
                    );
                    prev = (sp.u, sp.v);
                    chained += 1;
                }
            }
        }
        None => {
            let end = (psp.u + aim.x * 16.0, psp.v + aim.y * 16.0);
            spawn_beam(
                &mut commands,
                &libs,
                from,
                surf_to_world(end.0, end.1, 0.4),
                Cid::Cyan,
                0.05,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fire_hammer(
    buttons: Res<ButtonInput<MouseButton>>,
    ui: Res<UiState>,
    hover: Res<UiHover>,
    sel: Res<BuildSel>,
    mut ws: ResMut<WeaponState>,
    fx: Res<TechFx>,
    mut stats: ResMut<PStats>,
    grid: Res<Grid>,
    qp: Query<&SurfPos, With<PlayerTag>>,
    qe: Query<(Entity, &SurfPos), With<Enemy>>,
    qc: Query<(&Camera, &GlobalTransform), With<CamTag>>,
    qw: Query<&Window, With<PrimaryWindow>>,
    mut dmg: EventWriter<DmgEvent>,
    mut commands: Commands,
    libs: Res<Libs>,
    mut pc: ResMut<ParticleCount>,
) {
    if ws.cur != 2
        || !buttons.just_pressed(MouseButton::Left)
        || ws.cd > 0.0
        || !fire_allowed(&ui, &hover, &sel)
    {
        return;
    }
    let Ok(psp) = qp.single() else { return };
    let aim = cursor_uv(&qc, &qw)
        .map(|(au, av)| Vec2::new(au - psp.u, av - psp.v).normalize_or_zero())
        .unwrap_or(Vec2::Y);
    ws.cd = 1.1;
    stats.heat = (stats.heat + 8.0).min(100.0);
    let radius = 5.5 * fx.hammer;
    for (e, sp) in qe.iter() {
        let to = Vec2::new(sp.u - psp.u, sp.v - psp.v);
        let d = to.length();
        if d < radius && (d < 1.2 || to.normalize().dot(aim) > 0.55) {
            let mut ev = DmgEvent::new(e, 34.0, DmgSrc::PHam);
            ev.knock = to.normalize_or_zero() * 16.0 * fx.hammer;
            ev.slow = true;
            dmg.write(ev);
        }
    }
    // careless swings damage buildings in the blast cone
    let mut hit: Vec<Entity> = Vec::new();
    let pcell = psp.cell();
    for dy in -3i32..=3 {
        for dx in -3i32..=3 {
            let c = (pcell.0 + dx, pcell.1 + dy);
            if !Grid::inb(c) {
                continue;
            }
            let (cu, cv) = cell_center(c);
            let to = Vec2::new(cu - psp.u, cv - psp.v);
            if to.length() < 3.6 && to.normalize_or_zero().dot(aim) > 0.5 {
                if let Some(e) = grid.at(c).occ {
                    if !hit.contains(&e) {
                        hit.push(e);
                        dmg.write(DmgEvent::new(e, 18.0, DmgSrc::PHam));
                    }
                }
            }
        }
    }
    spawn_burst(
        &mut commands,
        &libs,
        &mut pc,
        psp.u + aim.x * 2.0,
        psp.v + aim.y * 2.0,
        psp.h + 0.6,
        16,
        Cid::Orange,
        7.0,
    );
}

fn repair_hold(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    sel: Res<BuildSel>,
    mut bank: ResMut<Bank>,
    qp: Query<&SurfPos, With<PlayerTag>>,
    mut qb: Query<(&Building, &SurfPos, &mut Health)>,
) {
    if !keys.pressed(KeyCode::KeyR) || sel.kind.is_some() {
        return;
    }
    let Ok(psp) = qp.single() else { return };
    let dt = time.delta_secs();
    for (b, sp, mut hp) in qb.iter_mut() {
        if b.built >= 1.0 && hp.hp < hp.max && sp.dist(psp) < 6.0 {
            let heal = (25.0 * dt).min(hp.max - hp.hp);
            if bank.take(R_SCRAP, heal * 0.12) > 0.0 || heal * 0.12 < 0.01 {
                hp.hp += heal;
            }
            return;
        }
    }
}

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                move_player,
                vitals,
                respawn,
                interact_tap,
                mine_hold,
                command_click,
                weapon_switch,
                fire_kinetic,
                fire_arc,
                fire_hammer,
                repair_hold,
            )
                .run_if(playing),
        );
    }
}
