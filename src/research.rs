//! Research tree: 24 techs across 6 branches, researched at the reliquary.
//! Every unlock feeds a multiplier in `TechFx` that live systems read.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::building::{BKind, Building};
use crate::components::*;
use crate::resources::*;
use crate::workers::Drone;

pub const NTECH: usize = 24;

pub struct TechDef {
    pub name: &'static str,
    pub cat: &'static str,
    pub cost: &'static [(usize, f32)],
    pub time: f32,
    pub desc: &'static str,
}

pub fn tdef(i: usize) -> TechDef {
    let t = |name, cat, cost, time, desc| TechDef {
        name,
        cat,
        cost,
        time,
        desc,
    };
    match i {
        0 => t("Mag-Rail Packets", "Automation", &[(R_SCRAP, 30.0), (R_CIRCUIT, 4.0)], 25.0, "Conveyor packets move 60% faster."),
        1 => t("Compression Silos", "Automation", &[(R_SCRAP, 40.0), (R_CIRCUIT, 4.0)], 25.0, "Storage caps +80%."),
        2 => t("Heuristic Sorters", "Automation", &[(R_CIRCUIT, 8.0), (R_CRYSTAL, 6.0)], 30.0, "Sorters draw no power; packet limit raised."),
        3 => t("Overclock Protocols", "Automation", &[(R_CIRCUIT, 10.0), (R_ALLOY, 20.0)], 35.0, "Machines 30% faster, +25% power draw."),
        4 => t("Long-Span Pylons", "Power", &[(R_SCRAP, 30.0), (R_CRYSTAL, 4.0)], 22.0, "Pylon link radius +40%."),
        5 => t("Fusion Catalysis", "Power", &[(R_ALLOY, 25.0), (R_COOLANT, 10.0)], 35.0, "Fusion kettles +25% output, half coolant."),
        6 => t("Photonic Banks", "Power", &[(R_CIRCUIT, 6.0), (R_CRYSTAL, 8.0)], 30.0, "Solar sails store light: 35% output at night."),
        7 => t("Brownout Dampers", "Power", &[(R_CIRCUIT, 8.0), (R_ALLOY, 10.0)], 28.0, "Connected machines never drop below 55% speed."),
        8 => t("Rail Autoloaders", "Defense", &[(R_ALLOY, 20.0), (R_CIRCUIT, 6.0)], 30.0, "Rail turrets fire 50% faster."),
        9 => t("Arc Cascade", "Defense", &[(R_CRYSTAL, 12.0), (R_CIRCUIT, 6.0)], 30.0, "Arc weapons chain to 2 extra targets."),
        10 => t("Cluster Shells", "Defense", &[(R_ALLOY, 18.0), (R_CIRCUIT, 8.0)], 35.0, "Mortars fire 3-shell clusters."),
        11 => t("Aegis Capacitors", "Defense", &[(R_CRYSTAL, 10.0), (R_CIRCUIT, 10.0)], 35.0, "Shield domes hold double capacity."),
        12 => t("Drone Servos", "Workers", &[(R_ALLOY, 12.0), (R_CIRCUIT, 4.0)], 22.0, "Drones move 40% faster."),
        13 => t("Drone Plating", "Workers", &[(R_ALLOY, 16.0)], 22.0, "Drones gain 3 armor."),
        14 => t("Bandwidth Expansion", "Workers", &[(R_CIRCUIT, 8.0), (R_RELIC, 4.0)], 30.0, "Command bandwidth +4 drones."),
        15 => t("Emergency Protocols", "Workers", &[(R_CIRCUIT, 6.0), (R_ALLOY, 10.0)], 25.0, "Drones repair critical buildings twice as fast."),
        16 => t("Myofiber Weave", "Player", &[(R_ALLOY, 10.0), (R_CRYSTAL, 4.0)], 22.0, "Stamina +50%."),
        17 => t("Closed-Loop O2", "Player", &[(R_CIRCUIT, 6.0), (R_COOLANT, 12.0)], 28.0, "Oxygen drain halved."),
        18 => t("Superconductor Coils", "Player", &[(R_CRYSTAL, 10.0)], 25.0, "Arc lance heat -40%."),
        19 => t("Resonant Hammer", "Player", &[(R_ALLOY, 14.0), (R_RELIC, 4.0)], 30.0, "Grav hammer radius & knockback +60%."),
        20 => t("Radar Sweep", "Exploration", &[(R_SCRAP, 25.0), (R_CIRCUIT, 4.0)], 22.0, "Radar masts reveal 60% further."),
        21 => t("Ruin Scanner", "Exploration", &[(R_RELIC, 6.0), (R_CIRCUIT, 4.0)], 25.0, "Ruins shown on minimap; salvage +50%."),
        22 => t("Corruption Sheathing", "Exploration", &[(R_RELIC, 8.0), (R_COOLANT, 10.0)], 28.0, "Corrupted ground barely slows you."),
        _ => t("Nest Triangulation", "Exploration", &[(R_RELIC, 10.0), (R_CIRCUIT, 8.0)], 30.0, "All enemy nests revealed on the minimap."),
    }
}

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct ResearchSt {
    pub unlocked: Vec<bool>,
    pub active: Option<(usize, f32)>,
}
impl Default for ResearchSt {
    fn default() -> Self {
        Self {
            unlocked: vec![false; NTECH],
            active: None,
        }
    }
}

#[derive(Resource)]
pub struct TechFx {
    pub packet: f32,
    pub storage: f32,
    pub sorter: bool,
    pub overclock: f32,
    pub pylon: f32,
    pub fusion: f32,
    pub solar_night: f32,
    pub brownout: bool,
    pub rail: f32,
    pub arc_chain: u32,
    pub mortar_cluster: bool,
    pub shield_cap: f32,
    pub drone_speed: f32,
    pub drone_armor: f32,
    pub emerg: bool,
    pub stam: f32,
    pub oxy: f32,
    pub arc_heat: f32,
    pub hammer: f32,
    pub radar: f32,
    pub ruin_scan: bool,
    pub ruin_yield: f32,
    pub corrupt_res: f32,
    pub nest_loc: bool,
}
impl Default for TechFx {
    fn default() -> Self {
        Self {
            packet: 1.0,
            storage: 1.0,
            sorter: false,
            overclock: 1.0,
            pylon: 1.0,
            fusion: 1.0,
            solar_night: 0.0,
            brownout: false,
            rail: 1.0,
            arc_chain: 1,
            mortar_cluster: false,
            shield_cap: 1.0,
            drone_speed: 1.0,
            drone_armor: 0.0,
            emerg: false,
            stam: 1.0,
            oxy: 1.0,
            arc_heat: 1.0,
            hammer: 1.0,
            radar: 1.0,
            ruin_scan: false,
            ruin_yield: 1.0,
            corrupt_res: 0.0,
            nest_loc: false,
        }
    }
}

pub fn recompute_fx(r: &ResearchSt) -> TechFx {
    let mut f = TechFx::default();
    let u = |i: usize| r.unlocked.get(i).copied().unwrap_or(false);
    if u(0) {
        f.packet = 1.6;
    }
    if u(1) {
        f.storage = 1.8;
    }
    if u(2) {
        f.sorter = true;
    }
    if u(3) {
        f.overclock = 1.3;
    }
    if u(4) {
        f.pylon = 1.4;
    }
    if u(5) {
        f.fusion = 1.25;
    }
    if u(6) {
        f.solar_night = 0.35;
    }
    if u(7) {
        f.brownout = true;
    }
    if u(8) {
        f.rail = 1.5;
    }
    if u(9) {
        f.arc_chain = 3;
    }
    if u(10) {
        f.mortar_cluster = true;
    }
    if u(11) {
        f.shield_cap = 2.0;
    }
    if u(12) {
        f.drone_speed = 1.4;
    }
    if u(13) {
        f.drone_armor = 3.0;
    }
    if u(15) {
        f.emerg = true;
    }
    if u(16) {
        f.stam = 1.5;
    }
    if u(17) {
        f.oxy = 0.5;
    }
    if u(18) {
        f.arc_heat = 0.6;
    }
    if u(19) {
        f.hammer = 1.6;
    }
    if u(20) {
        f.radar = 1.6;
    }
    if u(21) {
        f.ruin_scan = true;
        f.ruin_yield = 1.5;
    }
    if u(22) {
        f.corrupt_res = 1.0;
    }
    if u(23) {
        f.nest_loc = true;
    }
    f
}

/// Called from the research UI. Returns an error string on failure.
pub fn try_start(research: &mut ResearchSt, bank: &mut Bank, i: usize) -> Result<(), &'static str> {
    if research.unlocked[i] {
        return Err("Already researched");
    }
    if research.active.is_some() {
        return Err("Research already in progress");
    }
    if !bank.pay(tdef(i).cost) {
        return Err("Insufficient resources");
    }
    research.active = Some((i, tdef(i).time));
    Ok(())
}

fn research_progress(
    time: Res<Time>,
    mut research: ResMut<ResearchSt>,
    mut fx: ResMut<TechFx>,
    mut stats: ResMut<PStats>,
    qb: Query<&Building>,
    mut notify: EventWriter<Notify>,
) {
    let Some((i, mut left)) = research.active else {
        return;
    };
    let reliquary = qb
        .iter()
        .filter(|b| b.kind == BKind::Reliquary && b.built >= 1.0)
        .map(|b| b.powered)
        .fold(0.0f32, f32::max);
    if reliquary <= 0.05 {
        return; // needs a powered reliquary
    }
    left -= time.delta_secs() * reliquary;
    if left <= 0.0 {
        research.unlocked[i] = true;
        research.active = None;
        *fx = recompute_fx(&research);
        if i == 14 {
            stats.bandwidth += 4;
        }
        notify.write(Notify(format!("RESEARCH COMPLETE: {}", tdef(i).name)));
    } else {
        research.active = Some((i, left));
    }
}

fn apply_drone_armor(
    time: Res<Time>,
    mut acc: Local<f32>,
    fx: Res<TechFx>,
    mut q: Query<&mut Armor, With<Drone>>,
) {
    *acc += time.delta_secs();
    if *acc < 2.0 {
        return;
    }
    *acc = 0.0;
    for mut a in q.iter_mut() {
        a.0 = fx.drone_armor;
    }
}

pub struct ResearchPlugin;
impl Plugin for ResearchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ResearchSt>()
            .init_resource::<TechFx>()
            .add_systems(
                FixedUpdate,
                (research_progress, apply_drone_armor).run_if(playing),
            );
    }
}
