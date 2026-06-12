//! Dynamic event director: picks events based on colony state, runs a
//! warning phase then an active phase, and applies real gameplay effects.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::building::{BKind, Building};
use crate::components::*;
use crate::resources::*;
use crate::workers::Drone;
use crate::MAP;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum GEvent {
    Eclipse,
    Meteor,
    Flood,
    Bloom,
    Ghost,
    Merchant,
    Quake,
    RelicStorm,
}
impl GEvent {
    pub fn name(self) -> &'static str {
        match self {
            GEvent::Eclipse => "ECLIPSE",
            GEvent::Meteor => "METEOR RAIN",
            GEvent::Flood => "COOLANT FLOOD",
            GEvent::Bloom => "CORRUPTION BLOOM",
            GEvent::Ghost => "GHOST SIGNAL",
            GEvent::Merchant => "MERCHANT ARK",
            GEvent::Quake => "HULL QUAKE",
            GEvent::RelicStorm => "RELIC STORM",
        }
    }
    fn duration(self) -> f32 {
        match self {
            GEvent::Eclipse => 45.0,
            GEvent::Meteor => 25.0,
            GEvent::Flood => 30.0,
            GEvent::Bloom => 50.0,
            GEvent::Ghost => 25.0,
            GEvent::Merchant => 45.0,
            GEvent::Quake => 3.0,
            GEvent::RelicStorm => 45.0,
        }
    }
}

#[derive(Resource)]
pub struct EvDirector {
    /// (event, 0 = warning / 1 = active, seconds left in this phase)
    pub active: Option<(GEvent, u8, f32)>,
    pub cd: f32,
    pub banner: String,
}
impl Default for EvDirector {
    fn default() -> Self {
        Self {
            active: None,
            cd: 70.0,
            banner: String::new(),
        }
    }
}

#[derive(Resource)]
pub struct EventFx {
    pub solar: f32,
    pub corrupt: f32,
    pub scramble: bool,
    pub trade: bool,
    pub aggress: bool,
    pub flood_slow: bool,
}
impl Default for EventFx {
    fn default() -> Self {
        Self {
            solar: 1.0,
            corrupt: 1.0,
            scramble: false,
            trade: false,
            aggress: false,
            flood_slow: false,
        }
    }
}

pub const TRADES: [(usize, f32, usize, f32); 4] = [
    (R_SCRAP, 25.0, R_CIRCUIT, 1.0),
    (R_CRYSTAL, 6.0, R_COOLANT, 12.0),
    (R_RELIC, 4.0, R_LPART, 1.0),
    (R_SCRAP, 30.0, R_CORE, 1.0),
];

#[allow(clippy::too_many_arguments)]
fn director(
    time: Res<Time>,
    mut dir: ResMut<EvDirector>,
    mut evfx: ResMut<EventFx>,
    mut grid: ResMut<Grid>,
    mut rng: ResMut<RngRes>,
    clock: Res<GameClock>,
    qb: Query<(Entity, &Building, &Health)>,
    qn: Query<&Nest>,
    qd: Query<&Drone>,
    mut dmg: EventWriter<DmgEvent>,
    mut boom: EventWriter<ExplodeEvent>,
    mut notify: EventWriter<Notify>,
    mut meteor_acc: Local<f32>,
) {
    let dt = time.delta_secs();
    match dir.active {
        Some((ev, 0, mut t)) => {
            t -= dt;
            if t <= 0.0 {
                // --- activate: one-shot effects ---
                match ev {
                    GEvent::Quake => {
                        for (e, b, h) in qb.iter() {
                            if matches!(b.kind, BKind::Wall | BKind::Gate | BKind::Conveyor | BKind::Sorter) {
                                dmg.write(DmgEvent::new(e, h.max * 0.2, DmgSrc::Hazard));
                            }
                        }
                        notify.write(Notify("The hull shudders. Walls and conveyors damaged!".into()));
                    }
                    GEvent::Flood => {
                        let mut converted = 0;
                        for y in 1..MAP - 1 {
                            for x in 1..MAP - 1 {
                                if converted >= 70 {
                                    break;
                                }
                                if grid.at((x, y)).terrain != Terrain::Marsh {
                                    continue;
                                }
                                for d in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                                    let c = (x + d.0, y + d.1);
                                    let cell = grid.at(c);
                                    if cell.terrain != Terrain::Marsh
                                        && cell.walk
                                        && cell.occ.is_none()
                                        && rng.0.f32() < 0.25
                                    {
                                        let cc = grid.at_mut(c);
                                        cc.terrain = Terrain::Marsh;
                                        cc.res_kind = R_COOLANT;
                                        cc.res = 300.0;
                                        cc.elev = 0.0;
                                        grid.mark(c);
                                        converted += 1;
                                    }
                                }
                            }
                        }
                        notify.write(Notify("Coolant floods the lowlands — marshes expand.".into()));
                    }
                    GEvent::RelicStorm => {
                        for _ in 0..12 {
                            let c = (1 + rng.0.below(MAP - 2), 1 + rng.0.below(MAP - 2));
                            let cell = grid.at_mut(c);
                            if cell.occ.is_none() && cell.walk {
                                cell.res_kind = R_RELIC;
                                cell.res = rng.0.range(10.0, 25.0);
                                grid.mark(c);
                            }
                        }
                        notify.write(Notify("Relic shards rain across the hull — the machines stir!".into()));
                    }
                    _ => {}
                }
                dir.active = Some((ev, 1, ev.duration()));
                dir.banner = format!("{} — ACTIVE", ev.name());
            } else {
                dir.active = Some((ev, 0, t));
            }
        }
        Some((ev, _, mut t)) => {
            t -= dt;
            if ev == GEvent::Meteor {
                *meteor_acc += dt;
                if *meteor_acc > 1.8 {
                    *meteor_acc = 0.0;
                    // strike a random discovered cell
                    for _ in 0..10 {
                        let c = (1 + rng.0.below(MAP - 2), 1 + rng.0.below(MAP - 2));
                        if grid.at(c).disc {
                            let (u, v) = cell_center(c);
                            boom.write(ExplodeEvent {
                                u,
                                v,
                                radius: 3.0,
                                dmg: 16.0,
                                src: DmgSrc::Hazard,
                                friendly: true,
                            });
                            break;
                        }
                    }
                }
            }
            if t <= 0.0 {
                dir.active = None;
                dir.banner.clear();
                dir.cd = rng.0.range(50.0, 100.0);
                notify.write(Notify(format!("{} has passed.", ev.name())));
            } else {
                dir.active = Some((ev, 1, t));
            }
        }
        None => {
            dir.cd -= dt;
            if dir.cd <= 0.0 {
                let solars = qb.iter().filter(|(_, b, _)| b.kind == BKind::Solar).count();
                let walls = qb
                    .iter()
                    .filter(|(_, b, _)| matches!(b.kind, BKind::Wall | BKind::Gate))
                    .count();
                let marsh_any = true;
                let mut opts: Vec<(GEvent, f32)> = Vec::new();
                if solars >= 2 {
                    opts.push((GEvent::Eclipse, 1.0));
                }
                opts.push((GEvent::Meteor, 0.9));
                if marsh_any {
                    opts.push((GEvent::Flood, 0.7));
                }
                if qn.iter().count() > 0 {
                    opts.push((GEvent::Bloom, 1.0));
                }
                if qd.iter().count() >= 2 {
                    opts.push((GEvent::Ghost, 0.8));
                }
                if clock.day >= 2 {
                    opts.push((GEvent::Merchant, 1.0));
                }
                if clock.day >= 3 && walls >= 4 {
                    opts.push((GEvent::Quake, 0.8));
                }
                if clock.day >= 3 {
                    opts.push((GEvent::RelicStorm, 0.7));
                }
                let total: f32 = opts.iter().map(|(_, w)| w).sum();
                let mut roll = rng.0.f32() * total;
                let mut pick = GEvent::Meteor;
                for (e, w) in opts {
                    roll -= w;
                    if roll <= 0.0 {
                        pick = e;
                        break;
                    }
                }
                dir.active = Some((pick, 0, 12.0));
                dir.banner = format!("WARNING: {} imminent", pick.name());
                notify.write(Notify(format!("Sensors: {} approaching", pick.name())));
            }
        }
    }

    // recompute live effect flags
    let active = dir.active.and_then(|(e, ph, _)| if ph == 1 { Some(e) } else { None });
    evfx.solar = if active == Some(GEvent::Eclipse) { 0.15 } else { 1.0 };
    evfx.corrupt = if active == Some(GEvent::Bloom) { 4.0 } else { 1.0 };
    evfx.scramble = active == Some(GEvent::Ghost);
    evfx.trade = active == Some(GEvent::Merchant);
    evfx.aggress = active == Some(GEvent::RelicStorm);
    evfx.flood_slow = active == Some(GEvent::Flood);
}

pub struct EventsPlugin;
impl Plugin for EventsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EvDirector>()
            .init_resource::<EventFx>()
            .add_systems(FixedUpdate, director.run_if(playing));
    }
}
