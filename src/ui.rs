//! All game UI: HUD, build menu, research screen, selection panel, minimap,
//! trade panel, adaptation report, pause/end screens, tutorial, notifications.

use bevy::app::AppExit;
use bevy::prelude::*;

use crate::building::{bdef, BKind, BuildSel, Building, ALL_KINDS};
use crate::components::*;
use crate::enemies::Adapt;
use crate::events::{EvDirector, EventFx, TRADES};
use crate::research::{tdef, try_start, ResearchSt, NTECH};
use crate::resources::*;
use crate::workers::Drone;
use crate::AppState;

#[derive(Resource, Default)]
pub struct UiHover(pub bool);

// markers
#[derive(Component)]
struct ResText;
#[derive(Component)]
struct PhaseText;
#[derive(Component)]
struct BannerText;
#[derive(Component)]
struct MiniUi;
#[derive(Component)]
struct BarFill(usize);
#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct NoteText;
#[derive(Component)]
struct TutPanel;
#[derive(Component)]
struct TutText;
#[derive(Component)]
struct BuildMenu;
#[derive(Component)]
struct BuildBtn(BKind);
#[derive(Component)]
struct SelPanel;
#[derive(Component)]
struct SelText;
#[derive(Component)]
struct ResearchPanel;
#[derive(Component)]
struct ResearchHdr;
#[derive(Component)]
struct TechBtn(usize);
#[derive(Component)]
struct TradePanel;
#[derive(Component)]
struct TradeBtn(usize);
#[derive(Component)]
struct ReportPanel;
#[derive(Component)]
struct ReportText;
#[derive(Component)]
struct ContinueBtn;
#[derive(Component)]
struct PausePanel;
#[derive(Component)]
struct PauseBtn(u8);
#[derive(Component)]
struct EndPanel;
#[derive(Component)]
struct EndText;

const PANEL_BG: Color = Color::srgba(0.01, 0.04, 0.08, 0.88);
const BTN_BG: Color = Color::srgba(0.05, 0.12, 0.18, 0.95);
const CYAN: Color = Color::srgb(0.3, 0.9, 1.0);

fn setup_ui(mut commands: Commands, minimap: Res<MinimapImg>) {
    let font = |s: f32| TextFont {
        font_size: s,
        ..default()
    };

    // resource bar
    commands.spawn((
        Text::new(""),
        font(14.0),
        TextColor(Color::srgb(0.85, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Px(6.0),
            ..default()
        },
        ResText,
    ));
    // phase / day
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(6.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn((Text::new(""), font(17.0), TextColor(CYAN), PhaseText));
        });
    // event banner
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(30.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                font(16.0),
                TextColor(Color::srgb(1.0, 0.6, 0.2)),
                BannerText,
            ));
        });
    // minimap
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(34.0),
                right: Val::Px(8.0),
                padding: UiRect::all(Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor(CYAN),
        ))
        .with_children(|p| {
            p.spawn((
                ImageNode::new(minimap.0.clone()),
                Node {
                    width: Val::Px(168.0),
                    height: Val::Px(168.0),
                    ..default()
                },
                MiniUi,
            ));
        });
    // vital bars
    let bar_colors = [
        Color::srgb(0.9, 0.2, 0.2),
        Color::srgb(0.2, 0.9, 0.4),
        Color::srgb(1.0, 0.6, 0.1),
        Color::srgb(0.3, 0.7, 1.0),
    ];
    let bar_names = ["HP", "STA", "HEAT", "O2"];
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                bottom: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
        ))
        .with_children(|p| {
            for i in 0..4 {
                p.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Text::new(bar_names[i]),
                        font(11.0),
                        TextColor(Color::WHITE),
                        Node {
                            width: Val::Px(36.0),
                            ..default()
                        },
                    ));
                    row.spawn((
                        Node {
                            width: Val::Px(170.0),
                            height: Val::Px(11.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.1, 0.1, 0.12, 0.9)),
                    ))
                    .with_children(|bg| {
                        bg.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(bar_colors[i]),
                            BarFill(i),
                        ));
                    });
                });
            }
        });
    // weapon/carry status
    commands.spawn((
        Text::new(""),
        font(13.0),
        TextColor(Color::srgb(0.9, 0.9, 0.8)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(8.0),
            bottom: Val::Px(10.0),
            ..default()
        },
        StatusText,
    ));
    // notifications
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(120.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                font(14.0),
                TextColor(Color::srgb(1.0, 0.95, 0.7)),
                NoteText,
            ));
        });
    // tutorial
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(120.0),
                width: Val::Px(300.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor(Color::srgb(1.0, 0.8, 0.2)),
            TutPanel,
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), font(13.0), TextColor(Color::srgb(1.0, 0.95, 0.8)), TutText));
        });
    // build menu
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(64.0),
                left: Val::Percent(4.0),
                width: Val::Percent(92.0),
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BuildMenu,
        ))
        .with_children(|p| {
            for kind in ALL_KINDS {
                if kind == BKind::Core {
                    continue;
                }
                let def = bdef(kind);
                let cost: Vec<String> = def
                    .cost
                    .iter()
                    .map(|(k, a)| format!("{}{}", a, &RES_NAMES[*k][..2]))
                    .collect();
                p.spawn((
                    Button,
                    Node {
                        width: Val::Px(148.0),
                        height: Val::Px(40.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BTN_BG),
                    BuildBtn(kind),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new(format!("{}\n{}", def.name, cost.join(" "))),
                        font(11.0),
                        TextColor(Color::WHITE),
                    ));
                });
            }
        });
    // selection panel
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                top: Val::Px(230.0),
                width: Val::Px(250.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor(CYAN),
            SelPanel,
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), font(12.0), TextColor(Color::WHITE), SelText));
        });
    // research panel
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(8.0),
                top: Val::Percent(8.0),
                width: Val::Percent(84.0),
                height: Val::Percent(82.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor(Color::srgb(0.7, 0.3, 1.0)),
            ResearchPanel,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("RESEARCH RELIQUARY"),
                font(18.0),
                TextColor(Color::srgb(0.85, 0.5, 1.0)),
            ));
            p.spawn((Text::new(""), font(13.0), TextColor(Color::WHITE), ResearchHdr));
            p.spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(5.0),
                row_gap: Val::Px(5.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .with_children(|wrap| {
                for i in 0..NTECH {
                    wrap.spawn((
                        Button,
                        Node {
                            width: Val::Px(252.0),
                            height: Val::Px(64.0),
                            padding: UiRect::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(BTN_BG),
                        TechBtn(i),
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new(""), font(11.0), TextColor(Color::WHITE)));
                    });
                }
            });
        });
    // trade panel
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(36.0),
                top: Val::Percent(25.0),
                width: Val::Px(360.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor(Color::srgb(1.0, 0.85, 0.3)),
            TradePanel,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("MERCHANT ARK — trades while docked"),
                font(15.0),
                TextColor(Color::srgb(1.0, 0.85, 0.3)),
            ));
            for (i, (gk, ga, tk, ta)) in TRADES.iter().enumerate() {
                p.spawn((
                    Button,
                    Node {
                        height: Val::Px(30.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BTN_BG),
                    TradeBtn(i),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new(format!(
                            "{} {} -> {} {}",
                            ga, RES_NAMES[*gk], ta, RES_NAMES[*tk]
                        )),
                        font(13.0),
                        TextColor(Color::WHITE),
                    ));
                });
            }
        });
    // adaptation report
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(28.0),
                top: Val::Percent(20.0),
                width: Val::Percent(44.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor(Color::srgb(1.0, 0.2, 0.2)),
            ReportPanel,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("ENEMY ADAPTATION REPORT"),
                font(18.0),
                TextColor(Color::srgb(1.0, 0.3, 0.3)),
            ));
            p.spawn((Text::new(""), font(13.0), TextColor(Color::WHITE), ReportText));
            p.spawn((
                Button,
                Node {
                    height: Val::Px(34.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(BTN_BG),
                ContinueBtn,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("CONTINUE TO DAWN (autosaves)"),
                    font(14.0),
                    TextColor(CYAN),
                ));
            });
        });
    // pause panel
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(38.0),
                top: Val::Percent(28.0),
                width: Val::Px(320.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor(CYAN),
            PausePanel,
        ))
        .with_children(|p| {
            p.spawn((Text::new("PAUSED"), font(20.0), TextColor(CYAN)));
            for (i, label) in ["Resume (Esc)", "Save (F5)", "Load (F9)", "Quit"]
                .iter()
                .enumerate()
            {
                p.spawn((
                    Button,
                    Node {
                        height: Val::Px(32.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(BTN_BG),
                    PauseBtn(i as u8),
                ))
                .with_children(|b| {
                    b.spawn((Text::new(*label), font(14.0), TextColor(Color::WHITE)));
                });
            }
        });
    // end screen
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            EndPanel,
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), font(26.0), TextColor(Color::WHITE), EndText));
            p.spawn((
                Text::new("Press ENTER for a new expedition"),
                font(16.0),
                TextColor(CYAN),
            ));
        });
}

// ---------------- input toggles ----------------
#[allow(clippy::too_many_arguments)]
fn key_toggles(
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<UiState>,
    mut sel: ResMut<BuildSel>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    mut newgame: EventWriter<NewGame>,
    mut tut: ResMut<Tutorial>,
) {
    if keys.just_pressed(KeyCode::KeyB) && *state.get() == AppState::Playing {
        ui.build_open = !ui.build_open;
        ui.research_open = false;
    }
    if keys.just_pressed(KeyCode::Tab) {
        ui.tactical = !ui.tactical;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        ui.map_big = !ui.map_big;
    }
    if keys.just_pressed(KeyCode::KeyT) && *state.get() == AppState::Playing {
        ui.research_open = !ui.research_open;
        ui.build_open = false;
    }
    if keys.just_pressed(KeyCode::F1) && !tut.done {
        tut.step += 1;
    }
    if keys.just_pressed(KeyCode::Escape) {
        match state.get() {
            AppState::Playing => {
                if ui.research_open || ui.build_open || sel.kind.is_some() || ui.sel.is_some() {
                    ui.research_open = false;
                    ui.build_open = false;
                    ui.sel = None;
                    sel.kind = None;
                } else {
                    next.set(AppState::Paused);
                }
            }
            AppState::Paused => next.set(AppState::Playing),
            _ => {}
        }
    }
    if keys.just_pressed(KeyCode::Enter)
        && matches!(state.get(), AppState::GameOver | AppState::Victory)
    {
        newgame.write(NewGame);
    }
}

// ---------------- HUD updates ----------------
fn hud_update(
    time: Res<Time>,
    mut acc: Local<f32>,
    bank: Res<Bank>,
    clock: Res<GameClock>,
    launch: Res<LaunchState>,
    stats: Res<PStats>,
    ws: Res<WeaponState>,
    qd: Query<&Drone>,
    mut texts: ParamSet<(
        Query<&mut Text, With<ResText>>,
        Query<&mut Text, With<PhaseText>>,
        Query<&mut Text, With<StatusText>>,
    )>,
) {
    *acc += time.delta_secs();
    if *acc < 0.2 {
        return;
    }
    *acc = 0.0;
    if let Ok(mut t) = texts.p0().single_mut() {
        let mut s = String::new();
        for k in 0..NRES {
            s.push_str(&format!("{} {:.0}/{:.0}   ", RES_NAMES[k], bank.amt[k], bank.cap[k]));
        }
        t.0 = s;
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        let left = (clock.phase_len() - clock.t).max(0.0) as i32;
        let mut s = format!(
            "DAY {}  —  {}  {}:{:02}   |   LAUNCH {}/{}",
            clock.day,
            clock.phase.name(),
            left / 60,
            left % 60,
            launch.built,
            LAUNCH_SEGMENTS
        );
        if let Some(cd) = launch.countdown {
            s.push_str(&format!("   |   IGNITION IN {:.0}s — SURVIVE!", cd));
        }
        t.0 = s;
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        let ammo = if ws.cur == 0 {
            if ws.reload > 0.0 {
                " [reloading]".to_string()
            } else {
                format!(" [{}/{}]", ws.ammo, ws.mag)
            }
        } else {
            String::new()
        };
        t.0 = format!(
            "{}{}  |  Carry: {:.0} {}  |  Drones {}/{}  |  [B]uild [T]ech [Tab]overlay [M]ap",
            WEAPON_NAMES[ws.cur],
            ammo,
            stats.carry_amt,
            RES_NAMES[stats.carry_kind],
            qd.iter().count(),
            stats.bandwidth,
        );
    }
}

fn bars_update(stats: Res<PStats>, mut q: Query<(&BarFill, &mut Node)>) {
    for (b, mut n) in q.iter_mut() {
        let f = match b.0 {
            0 => stats.hp / stats.max_hp.max(1.0),
            1 => stats.stam / stats.max_stam.max(1.0),
            2 => stats.heat / 100.0,
            _ => stats.oxy / stats.max_oxy.max(1.0),
        };
        n.width = Val::Percent((f.clamp(0.0, 1.0)) * 100.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn panel_vis(
    ui: Res<UiState>,
    evfx: Res<EventFx>,
    state: Res<State<AppState>>,
    tut: Res<Tutorial>,
    mut q: ParamSet<(
        Query<&mut Node, With<BuildMenu>>,
        Query<&mut Node, With<ResearchPanel>>,
        Query<&mut Node, With<TradePanel>>,
        Query<&mut Node, With<ReportPanel>>,
        Query<&mut Node, With<PausePanel>>,
        Query<&mut Node, With<EndPanel>>,
        Query<&mut Node, With<SelPanel>>,
        Query<&mut Node, With<TutPanel>>,
    )>,
) {
    let show = |on: bool| if on { Display::Flex } else { Display::None };
    let playing = *state.get() == AppState::Playing;
    if let Ok(mut n) = q.p0().single_mut() {
        n.display = show(ui.build_open && playing);
    }
    if let Ok(mut n) = q.p1().single_mut() {
        n.display = show(ui.research_open && playing);
    }
    if let Ok(mut n) = q.p2().single_mut() {
        n.display = show(evfx.trade && playing);
    }
    if let Ok(mut n) = q.p3().single_mut() {
        n.display = show(*state.get() == AppState::Report);
    }
    if let Ok(mut n) = q.p4().single_mut() {
        n.display = show(*state.get() == AppState::Paused);
    }
    if let Ok(mut n) = q.p5().single_mut() {
        n.display = show(matches!(state.get(), AppState::GameOver | AppState::Victory));
    }
    if let Ok(mut n) = q.p6().single_mut() {
        n.display = show(ui.sel.is_some() && playing);
    }
    if let Ok(mut n) = q.p7().single_mut() {
        n.display = show(!tut.done && playing);
    }
}

fn minimap_size(ui: Res<UiState>, mut q: Query<&mut Node, With<MiniUi>>) {
    if let Ok(mut n) = q.single_mut() {
        let s = if ui.map_big { 384.0 } else { 168.0 };
        n.width = Val::Px(s);
        n.height = Val::Px(s);
    }
}

// ---------------- buttons ----------------
fn hover_track(mut hover: ResMut<UiHover>, q: Query<&Interaction, With<Button>>) {
    hover.0 = q
        .iter()
        .any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
}

fn button_colors(mut q: Query<(&Interaction, &mut BackgroundColor), (With<Button>, Changed<Interaction>)>) {
    for (i, mut bg) in q.iter_mut() {
        bg.0 = match i {
            Interaction::Pressed => Color::srgba(0.2, 0.5, 0.7, 1.0),
            Interaction::Hovered => Color::srgba(0.1, 0.25, 0.38, 1.0),
            Interaction::None => BTN_BG,
        };
    }
}

fn build_buttons(
    mut sel: ResMut<BuildSel>,
    q: Query<(&Interaction, &BuildBtn), Changed<Interaction>>,
    mut notify: EventWriter<Notify>,
) {
    for (i, b) in q.iter() {
        if *i == Interaction::Pressed {
            sel.kind = Some(b.0);
            notify.write(Notify(format!(
                "{}: {} — click to place, R rotates, RMB cancels",
                bdef(b.0).name,
                bdef(b.0).desc
            )));
        }
    }
}

fn tech_buttons(
    mut research: ResMut<ResearchSt>,
    mut bank: ResMut<Bank>,
    q: Query<(&Interaction, &TechBtn), Changed<Interaction>>,
    mut notify: EventWriter<Notify>,
) {
    for (i, b) in q.iter() {
        if *i == Interaction::Pressed {
            match try_start(&mut research, &mut bank, b.0) {
                Ok(()) => notify.write(Notify(format!("Researching {}", tdef(b.0).name))),
                Err(e) => notify.write(Notify(e.to_string())),
            };
        }
    }
}

fn tech_text_update(
    time: Res<Time>,
    mut acc: Local<f32>,
    research: Res<ResearchSt>,
    qb: Query<(&TechBtn, &Children)>,
    mut qt: Query<(&mut Text, &mut TextColor), Without<ResearchHdr>>,
    mut hdr: Query<&mut Text, With<ResearchHdr>>,
    qbld: Query<&Building>,
) {
    *acc += time.delta_secs();
    if *acc < 0.4 {
        return;
    }
    *acc = 0.0;
    let has_rel = qbld
        .iter()
        .any(|b| b.kind == BKind::Reliquary && b.built >= 1.0);
    if let Ok(mut h) = hdr.single_mut() {
        h.0 = match (&research.active, has_rel) {
            (_, false) => "Build a Research Reliquary to unlock research.".into(),
            (Some((i, t)), _) => format!("In progress: {} — {:.0}s remaining", tdef(*i).name, t),
            (None, true) => "Select a technology to research.".into(),
        };
    }
    for (b, children) in qb.iter() {
        let def = tdef(b.0);
        let cost: Vec<String> = def
            .cost
            .iter()
            .map(|(k, a)| format!("{} {}", a, RES_NAMES[*k]))
            .collect();
        let (status, color) = if research.unlocked[b.0] {
            ("DONE", Color::srgb(0.3, 1.0, 0.4))
        } else if research.active.map_or(false, |(i, _)| i == b.0) {
            ("ACTIVE", Color::srgb(1.0, 0.9, 0.3))
        } else {
            ("", Color::WHITE)
        };
        for c in children.iter() {
            if let Ok((mut t, mut tc)) = qt.get_mut(c) {
                t.0 = format!(
                    "[{}] {} {}\n{} ({:.0}s)\n{}",
                    def.cat,
                    def.name,
                    status,
                    cost.join(", "),
                    def.time,
                    def.desc
                );
                tc.0 = color;
            }
        }
    }
}

fn trade_buttons(
    mut bank: ResMut<Bank>,
    q: Query<(&Interaction, &TradeBtn), Changed<Interaction>>,
    mut notify: EventWriter<Notify>,
) {
    for (i, b) in q.iter() {
        if *i == Interaction::Pressed {
            let (gk, ga, tk, ta) = TRADES[b.0];
            if bank.pay(&[(gk, ga)]) {
                bank.add(tk, ta);
                notify.write(Notify(format!("Traded for {} {}", ta, RES_NAMES[tk])));
            } else {
                notify.write(Notify("Cannot afford that trade".into()));
            }
        }
    }
}

fn pause_buttons(
    q: Query<(&Interaction, &PauseBtn), Changed<Interaction>>,
    mut next: ResMut<NextState<AppState>>,
    mut save: EventWriter<DoSave>,
    mut load: EventWriter<DoLoad>,
    mut exit: EventWriter<AppExit>,
) {
    for (i, b) in q.iter() {
        if *i != Interaction::Pressed {
            continue;
        }
        match b.0 {
            0 => next.set(AppState::Playing),
            1 => {
                save.write(DoSave);
            }
            2 => {
                load.write(DoLoad);
            }
            _ => {
                exit.write(AppExit::Success);
            }
        }
    }
}

fn report_ui(
    adapt: Res<Adapt>,
    mut qt: Query<&mut Text, With<ReportText>>,
    q: Query<&Interaction, (With<ContinueBtn>, Changed<Interaction>)>,
    mut next: ResMut<NextState<AppState>>,
    mut save: EventWriter<DoSave>,
) {
    if let Ok(mut t) = qt.single_mut() {
        if t.0 != adapt.report {
            t.0 = adapt.report.clone();
        }
    }
    for i in q.iter() {
        if *i == Interaction::Pressed {
            next.set(AppState::Playing);
            save.write(DoSave);
        }
    }
}

fn end_screen(endmsg: Res<EndMsg>, mut q: Query<&mut Text, With<EndText>>) {
    if let Ok(mut t) = q.single_mut() {
        if t.0 != endmsg.0 {
            t.0 = endmsg.0.clone();
        }
    }
}

// ---------------- notifications & banner ----------------
fn notes_update(
    time: Res<Time>,
    mut ui: ResMut<UiState>,
    mut ev: EventReader<Notify>,
    mut q: Query<&mut Text, With<NoteText>>,
) {
    for n in ev.read() {
        ui.notes.push((n.0.clone(), 4.5));
        if ui.notes.len() > 5 {
            ui.notes.remove(0);
        }
    }
    let dt = time.delta_secs();
    for n in ui.notes.iter_mut() {
        n.1 -= dt;
    }
    ui.notes.retain(|n| n.1 > 0.0);
    if let Ok(mut t) = q.single_mut() {
        let s = ui
            .notes
            .iter()
            .map(|(s, _)| s.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if t.0 != s {
            t.0 = s;
        }
    }
}

fn banner_update(
    dir: Res<EvDirector>,
    clock: Res<GameClock>,
    mut q: Query<&mut Text, With<BannerText>>,
) {
    let Ok(mut t) = q.single_mut() else { return };
    let s = if !dir.banner.is_empty() {
        dir.banner.clone()
    } else if clock.phase == Phase::Dusk {
        format!(
            "DUSK — assault in {:.0}s. Man the defenses.",
            (clock.phase_len() - clock.t).max(0.0)
        )
    } else {
        String::new()
    };
    if t.0 != s {
        t.0 = s;
    }
}

// ---------------- selection panel ----------------
fn sel_panel(
    mut ui: ResMut<UiState>,
    qb: Query<(&Building, &Health)>,
    mut qt: Query<&mut Text, With<SelText>>,
) {
    let Some(e) = ui.sel else { return };
    let Ok((b, h)) = qb.get(e) else {
        ui.sel = None;
        return;
    };
    let Ok(mut t) = qt.single_mut() else { return };
    let def = bdef(b.kind);
    let mut s = format!("{}\nHP {:.0}/{:.0}", def.name, h.hp, h.max);
    if b.built < 1.0 {
        s.push_str(&format!("\nUnder construction: {:.0}%", b.built * 100.0));
    } else if def.power < 0.0 {
        s.push_str(&format!("\nPower: {:.0}%", b.powered * 100.0));
    }
    if b.disabled > 0.0 {
        s.push_str(&format!("\nDISABLED {:.0}s (parasite)", b.disabled));
    }
    if b.jam {
        s.push_str("\nOUTPUT JAMMED — no route/conveyor");
    }
    if b.missing {
        s.push_str("\nMISSING INPUTS");
    }
    if b.kind == BKind::Sorter {
        let f = if b.filter >= NRES { "ANY" } else { RES_NAMES[b.filter] };
        s.push_str(&format!("\nFilter: {} (E to cycle)", f));
    }
    if b.kind == BKind::Loom {
        s.push_str(&format!(
            "\nRecipe: {} (E to toggle)",
            if b.recipe == 0 { "Circuits" } else { "Launch Parts" }
        ));
    }
    for k in 0..NRES {
        if b.buf_in[k] > 0.05 {
            s.push_str(&format!("\nIN  {} {:.1}", RES_NAMES[k], b.buf_in[k]));
        }
        if b.buf_out[k] > 0.05 {
            s.push_str(&format!("\nOUT {} {:.1} (E to collect)", RES_NAMES[k], b.buf_out[k]));
        }
    }
    s.push('\n');
    s.push_str(def.desc);
    if t.0 != s {
        t.0 = s;
    }
}

// ---------------- tutorial ----------------
const TUT_STEPS: [&str; 11] = [
    "1/11 MOVEMENT\nWASD to move. Shift = dash, Space = jet-hop.\n(F1 skips any step)",
    "2/11 CAMERA\nHold MIDDLE MOUSE (or Ctrl) and move the mouse to orbit. Scroll to zoom.",
    "3/11 MINING\nFind a glowing deposit (brown regolith = scrap) and hold E next to it to mine.",
    "4/11 DEPOSIT\nCarry your scrap back to the Core Obelisk and press E to bank it.",
    "5/11 BUILDING\nPress B, choose a Scrap Extractor and place it ON a scrap deposit.",
    "6/11 POWER\nExtractors need power. Place a Power Pylon linking the Core to the extractor (B menu). Solar sails add daytime power.",
    "7/11 AUTOMATION\nPlace Conveyor Nodes from the extractor to a Storage Silo. Packets will flow automatically.",
    "8/11 DRONES\nBuild a Drone Cradle. Drones need drone cores: salvage the black ruins (hold E there) for relic dust.",
    "9/11 DEFENSE\nNight is coming. Build a Rail Turret (and walls) near your base. Right-click sets a rally point.",
    "10/11 RESEARCH\nBuild a Research Reliquary, then press T (or E at it) and start a technology.",
    "11/11 ESCAPE\nBuild 6 Launch Engine Segments (needs Launch Parts from the Circuit Loom). Survive the final assault and ignite!",
];

#[allow(clippy::too_many_arguments)]
fn tutorial_tick(
    mut tut: ResMut<Tutorial>,
    keys: Res<ButtonInput<KeyCode>>,
    cam: Res<CamCtl>,
    stats: Res<PStats>,
    bank: Res<Bank>,
    sim: Res<SimStats>,
    launch: Res<LaunchState>,
    research: Res<ResearchSt>,
    qb: Query<&Building>,
    qd: Query<&Drone>,
    mut qt: Query<&mut Text, With<TutText>>,
) {
    if tut.done {
        return;
    }
    let advance = match tut.step {
        0 => [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD]
            .iter()
            .any(|k| keys.pressed(*k)),
        1 => cam.orbited,
        2 => stats.carry_amt > 0.5,
        3 => bank.lifetime.iter().sum::<f32>() > 0.5,
        4 => qb.iter().any(|b| b.kind == BKind::Extractor),
        5 => qb.iter().any(|b| b.kind == BKind::Extractor && b.powered > 0.3),
        6 => sim.packets_delivered > 0,
        7 => qd.iter().count() > 0,
        8 => qb
            .iter()
            .any(|b| matches!(b.kind, BKind::Rail | BKind::Arc | BKind::Mortar) && b.built >= 1.0),
        9 => research.active.is_some() || research.unlocked.iter().any(|u| *u),
        _ => launch.built >= LAUNCH_SEGMENTS,
    };
    if advance {
        tut.step += 1;
    }
    if tut.step >= TUT_STEPS.len() {
        tut.done = true;
        return;
    }
    if let Ok(mut t) = qt.single_mut() {
        if t.0 != TUT_STEPS[tut.step] {
            t.0 = TUT_STEPS[tut.step].to_string();
        }
    }
}

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiHover>()
            .add_systems(PostStartup, setup_ui)
            .add_systems(
                Update,
                (
                    key_toggles,
                    hud_update,
                    bars_update,
                    panel_vis,
                    minimap_size,
                    hover_track,
                    button_colors,
                    build_buttons,
                    tech_buttons,
                    tech_text_update,
                    trade_buttons,
                    pause_buttons,
                    report_ui,
                    end_screen,
                ),
            )
            .add_systems(
                Update,
                (notes_update, banner_update, sel_panel, tutorial_tick.run_if(playing)),
            );
    }
}
