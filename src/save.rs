//! Save / load (serde JSON) and world (re)construction for new game & load.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::building::{bdef, spawn_building, BKind, Building, BuildSel};
use crate::components::*;
use crate::enemies::{spawn_nest, Adapt, WavePlan};
use crate::events::{EvDirector, EventFx};
use crate::player::spawn_player;
use crate::procgen::{generate, reveal};
use crate::research::{recompute_fx, ResearchSt};
use crate::resources::*;
use crate::workers::{spawn_drone, Drone};
use crate::{AppState, WorldReady, MAP};

pub const SAVE_PATH: &str = "save.json";

#[derive(Serialize, Deserialize)]
struct BSave {
    kind: BKind,
    cell: (i32, i32),
    hp: f32,
    built: f32,
    filter: usize,
    recipe: u8,
    disabled: f32,
}

#[derive(Serialize, Deserialize)]
struct SaveData {
    seed: u64,
    day: u32,
    phase: u8,
    phase_t: f32,
    bank: Bank,
    pstats: PStats,
    weapon: WeaponState,
    player: (f32, f32, f32, f32),
    terrain: Vec<u8>,
    elev: Vec<f32>,
    res: Vec<f32>,
    res_kind: Vec<u8>,
    corrupt: Vec<f32>,
    disc: Vec<bool>,
    walk: Vec<bool>,
    buildings: Vec<BSave>,
    drones: Vec<(f32, f32, f32)>,
    nests: Vec<((i32, i32), u32, f32)>,
    research: ResearchSt,
    adapt: Adapt,
    launch: LaunchState,
    tutorial_step: usize,
    tutorial_done: bool,
    rng: u64,
}

fn save_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut save: EventWriter<DoSave>,
    mut load: EventWriter<DoLoad>,
) {
    if keys.just_pressed(KeyCode::F5) {
        save.write(DoSave);
    }
    if keys.just_pressed(KeyCode::F9) {
        load.write(DoLoad);
    }
}

#[allow(clippy::too_many_arguments)]
fn do_save(
    mut ev: EventReader<DoSave>,
    grid: Res<Grid>,
    bank: Res<Bank>,
    clock: Res<GameClock>,
    pstats: Res<PStats>,
    weapon: Res<WeaponState>,
    research: Res<ResearchSt>,
    adapt: Res<Adapt>,
    launch: Res<LaunchState>,
    tutorial: Res<Tutorial>,
    rng: Res<RngRes>,
    qp: Query<(&SurfPos, &Health), With<PlayerTag>>,
    qb: Query<(&Building, &Health)>,
    qd: Query<(&Drone, &SurfPos)>,
    qn: Query<(&Nest, &Health)>,
    mut notify: EventWriter<Notify>,
) {
    if ev.is_empty() {
        return;
    }
    ev.clear();
    let (psp, php) = match qp.single() {
        Ok((sp, hp)) => ((sp.u, sp.v, sp.h, hp.hp), hp.hp),
        Err(_) => ((0.0, 0.0, 1.0, 100.0), 100.0),
    };
    let _ = php;
    let n = (MAP * MAP) as usize;
    let mut data = SaveData {
        seed: grid.seed,
        day: clock.day,
        phase: clock.phase.to_u8(),
        phase_t: clock.t,
        bank: bank.clone(),
        pstats: pstats.clone(),
        weapon: weapon.clone(),
        player: psp,
        terrain: Vec::with_capacity(n),
        elev: Vec::with_capacity(n),
        res: Vec::with_capacity(n),
        res_kind: Vec::with_capacity(n),
        corrupt: Vec::with_capacity(n),
        disc: Vec::with_capacity(n),
        walk: Vec::with_capacity(n),
        buildings: qb
            .iter()
            .map(|(b, h)| BSave {
                kind: b.kind,
                cell: b.cell,
                hp: h.hp,
                built: b.built,
                filter: b.filter,
                recipe: b.recipe,
                disabled: b.disabled,
            })
            .collect(),
        drones: qd.iter().map(|(d, sp)| (sp.u, sp.v, d.charge)).collect(),
        nests: qn.iter().map(|(nst, h)| (nst.cell, nst.level, h.hp)).collect(),
        research: research.clone(),
        adapt: adapt.clone(),
        launch: launch.clone(),
        tutorial_step: tutorial.step,
        tutorial_done: tutorial.done,
        rng: rng.0 .0,
    };
    for c in grid.cells.iter() {
        data.terrain.push(c.terrain.to_u8());
        data.elev.push(c.elev);
        data.res.push(c.res);
        data.res_kind.push(c.res_kind as u8);
        data.corrupt.push(c.corrupt);
        data.disc.push(c.disc);
        data.walk.push(c.walk);
    }
    match serde_json::to_string(&data).map(|s| std::fs::write(SAVE_PATH, s)) {
        Ok(Ok(())) => notify.write(Notify("Game saved to save.json".into())),
        _ => notify.write(Notify("SAVE FAILED".into())),
    };
}

#[allow(clippy::too_many_arguments)]
fn do_load(
    mut ev: EventReader<DoLoad>,
    mut commands: Commands,
    libs: Res<Libs>,
    mut grid: ResMut<Grid>,
    qgame: Query<Entity, With<GameEntity>>,
    mut next: ResMut<NextState<AppState>>,
    mut notify: EventWriter<Notify>,
) {
    if ev.is_empty() {
        return;
    }
    ev.clear();
    let Ok(text) = std::fs::read_to_string(SAVE_PATH) else {
        notify.write(Notify("No save.json found".into()));
        return;
    };
    let Ok(data) = serde_json::from_str::<SaveData>(&text) else {
        notify.write(Notify("save.json is corrupted".into()));
        return;
    };
    for e in qgame.iter() {
        commands.entity(e).despawn();
    }

    *grid = Grid::new(data.seed);
    for (i, c) in grid.cells.iter_mut().enumerate() {
        c.terrain = Terrain::from_u8(data.terrain[i]);
        c.elev = data.elev[i];
        c.res = data.res[i];
        c.res_kind = data.res_kind[i] as usize;
        c.corrupt = data.corrupt[i];
        c.disc = data.disc[i];
        c.walk = data.walk[i];
    }
    grid.mark_all();

    for bs in data.buildings.iter() {
        let e = spawn_building(&mut commands, &libs, &mut grid, bs.kind, bs.cell, bs.built, 0);
        commands.entity(e).insert((
            Health {
                hp: bs.hp,
                max: bdef(bs.kind).hp,
            },
            Building {
                kind: bs.kind,
                cell: bs.cell,
                built: bs.built,
                powered: 0.0,
                buf_in: [0.0; NRES],
                buf_out: [0.0; NRES],
                craft: 0.0,
                cd: 0.0,
                filter: bs.filter,
                recipe: bs.recipe,
                disabled: bs.disabled,
                jam: false,
                missing: false,
            },
        ));
    }
    for (u, v, charge) in data.drones.iter() {
        spawn_drone(&mut commands, &libs, *u, *v, *charge);
    }
    for (cell, level, hp) in data.nests.iter() {
        let e = spawn_nest(&mut commands, &libs, *cell, *level);
        commands.entity(e).insert(Health {
            hp: *hp,
            max: 260.0,
        });
    }
    let pe = spawn_player(&mut commands, &libs, data.player.0, data.player.1);
    commands.entity(pe).insert(Health {
        hp: data.player.3,
        max: 110.0,
    });

    commands.insert_resource(GameClock {
        day: data.day,
        phase: Phase::from_u8(data.phase),
        t: data.phase_t,
    });
    commands.insert_resource(data.bank.clone());
    commands.insert_resource(data.pstats.clone());
    commands.insert_resource(data.weapon.clone());
    commands.insert_resource(recompute_fx(&data.research));
    commands.insert_resource(data.research.clone());
    commands.insert_resource(data.adapt.clone());
    commands.insert_resource(data.launch.clone());
    commands.insert_resource(Tutorial {
        step: data.tutorial_step,
        done: data.tutorial_done,
    });
    commands.insert_resource(RngRes(Rng(data.rng.max(1))));
    commands.insert_resource(SimStats::default());
    commands.insert_resource(WavePlan::default());
    commands.insert_resource(EvDirector::default());
    commands.insert_resource(EventFx::default());
    commands.insert_resource(BuildSel::default());
    commands.insert_resource(RallyPoint::default());
    commands.insert_resource(UiState::default());
    commands.insert_resource(crate::combat::ParticleCount::default());
    commands.insert_resource(WorldReady(true));
    next.set(AppState::Playing);
    notify.write(Notify("Save loaded".into()));
}

#[allow(clippy::too_many_arguments)]
fn new_game(
    mut ev: EventReader<NewGame>,
    mut commands: Commands,
    libs: Res<Libs>,
    mut grid: ResMut<Grid>,
    qgame: Query<Entity, With<GameEntity>>,
    mut next: ResMut<NextState<AppState>>,
    mut notify: EventWriter<Notify>,
) {
    if ev.is_empty() {
        return;
    }
    ev.clear();
    for e in qgame.iter() {
        commands.entity(e).despawn();
    }
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xC0FFEE)
        | 1;
    generate(&mut grid, seed);

    let ctr = (MAP / 2, MAP / 2);
    spawn_building(&mut commands, &libs, &mut grid, BKind::Core, ctr, 1.0, 0);
    let nests: Vec<(i32, i32)> = grid.nests_gen.clone();
    for c in nests {
        spawn_nest(&mut commands, &libs, c, 1);
    }
    let (cu, cv) = cell_center(ctr);
    spawn_player(&mut commands, &libs, cu + 6.0, cv + 6.0);
    reveal(&mut grid, ctr, 12);

    commands.insert_resource(Bank::default());
    commands.insert_resource(GameClock::default());
    commands.insert_resource(PStats::default());
    commands.insert_resource(WeaponState::default());
    commands.insert_resource(crate::research::ResearchSt::default());
    commands.insert_resource(crate::research::TechFx::default());
    commands.insert_resource(Adapt::default());
    commands.insert_resource(LaunchState::default());
    commands.insert_resource(SimStats::default());
    commands.insert_resource(WavePlan::default());
    commands.insert_resource(EvDirector::default());
    commands.insert_resource(EventFx::default());
    commands.insert_resource(BuildSel::default());
    commands.insert_resource(RallyPoint::default());
    commands.insert_resource(Tutorial::default());
    commands.insert_resource(UiState::default());
    commands.insert_resource(RngRes(Rng::new(seed)));
    commands.insert_resource(crate::combat::ParticleCount::default());
    commands.insert_resource(WorldReady(true));
    next.set(AppState::Playing);
    notify.write(Notify(format!("Expedition deployed. Seed {seed}")));
}

pub struct SavePlugin;
impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (save_keys, do_save, do_load, new_game));
    }
}
