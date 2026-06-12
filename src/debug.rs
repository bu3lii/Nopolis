//! F3 debug / profiling overlay.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

use crate::building::Building;
use crate::components::*;
use crate::enemies::Enemy;
use crate::resources::*;
use crate::workers::Drone;

#[derive(Resource, Default)]
pub struct DebugOn(pub bool);

#[derive(Component)]
struct DebugText;

fn setup(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.6, 1.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(320.0),
            top: Val::Px(28.0),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        DebugText,
    ));
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut on: ResMut<DebugOn>, mut q: Query<&mut Node, With<DebugText>>) {
    if keys.just_pressed(KeyCode::F3) {
        on.0 = !on.0;
        if let Ok(mut n) = q.single_mut() {
            n.display = if on.0 { Display::Flex } else { Display::None };
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update(
    time: Res<Time>,
    mut acc: Local<f32>,
    on: Res<DebugOn>,
    diags: Res<DiagnosticsStore>,
    grid: Res<Grid>,
    clock: Res<GameClock>,
    info: Res<PowerInfo>,
    sim: Res<SimStats>,
    pc: Res<crate::combat::ParticleCount>,
    counts: (
        Query<Entity>,
        Query<&Enemy>,
        Query<&Drone>,
        Query<&Building>,
        Query<&Packet>,
        Query<&Nest>,
    ),
    qply: Query<&SurfPos, With<PlayerTag>>,
    mut q: Query<&mut Text, With<DebugText>>,
) {
    if !on.0 {
        return;
    }
    *acc += time.delta_secs();
    if *acc < 0.25 {
        return;
    }
    *acc = 0.0;
    let fps = diags
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let ms = diags
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let (qe, qen, qd, qb, qp, qn) = &counts;
    let power: Vec<String> = info
        .nets
        .iter()
        .map(|(p, d, s)| format!("[{:.0}/{:.0} {:.0}%]", p, d, s * 100.0))
        .collect();
    let ploc = qply
        .single()
        .ok()
        .map(|sp| {
            let c = sp.cell();
            if Grid::inb(c) {
                format!("cell {:?} {}", c, grid.at(c).terrain.name())
            } else {
                "out of bounds".into()
            }
        })
        .unwrap_or_default();
    if let Ok(mut t) = q.single_mut() {
        t.0 = format!(
            "FPS {:.0}  frame {:.2} ms (update+render approx)\n\
             entities {}  enemies {}  drones {}  buildings {}  nests {}\n\
             packets {}  particles {}  active jobs {}  delivered {}\n\
             power nets {}: {}\n\
             phase {:?} t {:.1}  day {}  seed {}  {ploc}",
            fps,
            ms,
            qe.iter().count(),
            qen.iter().count(),
            qd.iter().count(),
            qb.iter().count(),
            qn.iter().count(),
            qp.iter().count(),
            pc.0,
            sim.active_jobs,
            sim.packets_delivered,
            info.nets.len(),
            power.join(" "),
            clock.phase,
            clock.t,
            clock.day,
            grid.seed,
        );
    }
}

pub struct DebugPlugin;
impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugOn>()
            .add_systems(Startup, setup)
            .add_systems(Update, (toggle, update));
    }
}
