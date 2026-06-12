//! ORBITAL NECROPOLIS — tactical survival colony-sim on the inner surface of
//! a rotating orbital tomb. Mine, automate, defend, research, escape.

mod automation;
mod building;
mod camera;
mod combat;
mod components;
mod debug;
mod enemies;
mod events;
mod player;
mod procgen;
mod research;
mod resources;
mod save;
mod ui;
mod workers;

use bevy::prelude::*;

use building::{BKind, Building};
use components::*;
use resources::*;

// ---------------- world constants ----------------
pub const MAP: i32 = 96;
pub const CELL: f32 = 2.0;
pub const RADIUS: f32 = 230.0;
pub const TILE: i32 = 16;
pub const TILES: i32 = MAP / TILE;

#[derive(States, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum AppState {
    #[default]
    Playing,
    Paused,
    Report,
    GameOver,
    Victory,
}

/// Set once the first world has been generated (guards win/lose checks).
#[derive(Resource, Default)]
pub struct WorldReady(pub bool);

/// 0..1 daylight factor, written by the lighting system, read by solar power.
#[derive(Resource)]
pub struct DayLight(pub f32);
impl Default for DayLight {
    fn default() -> Self {
        Self(1.0)
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.005, 0.006, 0.012)))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.5, 0.6, 0.9),
            brightness: 140.0,
            ..default()
        })
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "ORBITAL NECROPOLIS".into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .insert_resource(Time::<Fixed>::from_hz(10.0))
        .init_state::<AppState>()
        .init_resource::<WorldReady>()
        .init_resource::<DayLight>()
        .add_plugins((
            resources::CorePlugin,
            procgen::ProcgenPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            building::BuildingPlugin,
            automation::AutomationPlugin,
            workers::WorkerPlugin,
            enemies::EnemyPlugin,
            combat::CombatPlugin,
            ui::UiPlugin,
            research::ResearchPlugin,
            events::EventsPlugin,
            save::SavePlugin,
            debug::DebugPlugin,
        ))
        .add_systems(Startup, spawn_sun)
        .add_systems(
            Update,
            (
                phase_clock.run_if(playing),
                launch_logic.run_if(playing),
                lose_check.run_if(playing),
                daynight_lighting,
            ),
        )
        .run();
}

fn spawn_sun(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.95, 0.85),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.4, 0.0)),
        SunTag,
        Name::new("sun"),
    ));
}

/// Advances dawn -> day -> dusk -> night; after night, enters the adaptation
/// report state and rolls the calendar over to the next dawn.
fn phase_clock(
    time: Res<Time>,
    ready: Res<WorldReady>,
    mut clock: ResMut<GameClock>,
    mut next: ResMut<NextState<AppState>>,
    mut notify: EventWriter<Notify>,
) {
    if !ready.0 {
        return;
    }
    clock.t += time.delta_secs();
    if clock.t < clock.phase_len() {
        return;
    }
    clock.t = 0.0;
    clock.phase = match clock.phase {
        Phase::Dawn => {
            notify.write(Notify("Day phase: explore and expand.".into()));
            Phase::Day
        }
        Phase::Day => {
            notify.write(Notify("DUSK: ancient machinery is waking up.".into()));
            Phase::Dusk
        }
        Phase::Dusk => Phase::Night,
        Phase::Night => {
            clock.day += 1;
            next.set(AppState::Report);
            Phase::Dawn
        }
    };
}

fn launch_logic(
    time: Res<Time>,
    ready: Res<WorldReady>,
    mut launch: ResMut<LaunchState>,
    qb: Query<&Building>,
    mut next: ResMut<NextState<AppState>>,
    mut endmsg: ResMut<EndMsg>,
    mut notify: EventWriter<Notify>,
) {
    if !ready.0 {
        return;
    }
    let built = qb
        .iter()
        .filter(|b| b.kind == BKind::Launch && b.built >= 1.0)
        .count() as u32;
    launch.built = built;
    match launch.countdown {
        None => {
            if built >= LAUNCH_SEGMENTS {
                launch.countdown = Some(150.0);
                notify.write(Notify("ALL ENGINE SEGMENTS ONLINE. IGNITION IN 150 SECONDS.".into()));
            }
        }
        Some(t) => {
            if built < LAUNCH_SEGMENTS {
                launch.countdown = None;
                notify.write(Notify("Launch engine damaged — ignition aborted!".into()));
                return;
            }
            let t = t - time.delta_secs();
            if t <= 0.0 {
                endmsg.0 = "IGNITION. The salvage expedition tears free of the necropolis.\nYou escaped with the relics of a dead civilization.".into();
                next.set(AppState::Victory);
                launch.countdown = None;
            } else {
                launch.countdown = Some(t);
            }
        }
    }
}

fn lose_check(
    ready: Res<WorldReady>,
    stats: Res<PStats>,
    mut endmsg: ResMut<EndMsg>,
    mut next: ResMut<NextState<AppState>>,
) {
    if !ready.0 {
        return;
    }
    if stats.oxy <= 0.0 {
        endmsg.0 = "Oxygen depleted. The commander's visor fogs, then frosts over.".into();
        next.set(AppState::GameOver);
    }
    // core destruction is handled by the damage pipeline
}

#[allow(clippy::type_complexity)]
fn daynight_lighting(
    clock: Res<GameClock>,
    mut day: ResMut<DayLight>,
    mut ambient: ResMut<AmbientLight>,
    mut qs: Query<(&mut DirectionalLight, &mut Transform), With<SunTag>>,
    mut qf: Query<&mut bevy::pbr::DistanceFog>,
) {
    let c = clock.cycle01();
    // daylight arc occupies the first 80% of the cycle
    let alt = (std::f32::consts::PI * (c / 0.8).min(1.25)).sin();
    let dl = alt.clamp(0.0, 1.0);
    day.0 = dl;
    if let Ok((mut sun, mut tf)) = qs.single_mut() {
        sun.illuminance = 60.0 + 9500.0 * dl;
        sun.color = Color::srgb(1.0, 0.6 + 0.4 * dl, 0.45 + 0.5 * dl);
        let ang = 0.25 + alt.max(-0.3) * 1.1;
        *tf = Transform::from_rotation(
            Quat::from_rotation_z(0.35) * Quat::from_rotation_x(-ang),
        );
    }
    ambient.brightness = 25.0 + 170.0 * dl;
    ambient.color = if dl > 0.3 {
        Color::srgb(0.55, 0.65, 0.9)
    } else {
        Color::srgb(0.25, 0.3, 0.65)
    };
    for mut fog in qf.iter_mut() {
        let k = 0.15 + 0.85 * dl;
        fog.color = Color::srgb(0.02 + 0.025 * k, 0.025 + 0.03 * k, 0.05 + 0.04 * k);
    }
}
