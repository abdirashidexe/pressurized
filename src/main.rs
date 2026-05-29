use bevy::prelude::*;
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::render::view::Hdr;
use bevy::transform::TransformSystems;
use std::fs;
use std::io;

const SCREEN_WIDTH: f32 = 900.0;
const SCREEN_HEIGHT: f32 = 600.0;
const GAP_WIDTH: f32 = 250.0;
const BUBBLE_RADIUS: f32 = 30.0;
const BUBBLE_START: Vec3 = Vec3::new(0.0, -150.0, 0.0);
const SEGMENT_HEIGHT: f32 = 120.0;
const BASE_SCROLL_SPEED: f32 = 150.0;
const SCROLL_RAMP_RATE: f32 = 9.0;
const MAX_SCROLL_SPEED: f32 = 450.0;
const GAP_DRIFT_PER_SEGMENT: f32 = 70.0;
const GAP_MARGIN: f32 = 80.0;
const CENTERED_START_SEGMENTS: u32 = 4;
const PIXELS_PER_METER: f32 = 100.0;
const HORIZONTAL_ACCELERATION: f32 = 900.0;
const HORIZONTAL_MAX_SPEED: f32 = 420.0;
const TOOTH_DEPTH: f32 = 35.0;
const TOOTH_HEIGHT: f32 = SEGMENT_HEIGHT * 0.5;
const WALL_VISUAL_HEIGHT: f32 = SEGMENT_HEIGHT + 8.0;
const POP_PARTICLE_COUNT: u32 = 16;
const POP_PARTICLE_LIFETIME: f32 = 0.5;
const SCREEN_SHAKE_MAX_OFFSET: f32 = 18.0;
const SCREEN_SHAKE_DURATION: f32 = 0.3;
const MENU_BUBBLE_COUNT: u32 = 20;
const FADE_DURATION: f32 = 0.18;
const DEPTH_COLOR_RAMP_METERS: f32 = 100.0;
const METERS_PER_SEGMENT: f32 = SEGMENT_HEIGHT / PIXELS_PER_METER;
const MAX_RECENT_RUNS: usize = 5;
const SCORES_FILE: &str = "pressurized_scores.txt";

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
enum GameState {
    #[default]
    Menu,
    Playing,
    GameOver,
}

#[derive(Component)]
struct RisingCircle;

#[derive(Component, Default)]
struct HorizontalVelocity(f32);

#[derive(Component)]
struct CaveSegment {
    gap_center_x: f32,
}

#[derive(Component)]
struct CaveWall;

#[derive(Component)]
struct GameplayEntity;

#[derive(Component)]
struct DepthHud;

#[derive(Component)]
struct DepthHudValue;

#[derive(Component)]
struct MenuUi;

#[derive(Component)]
struct GameOverUi;

#[derive(Component)]
struct GameOverText;

#[derive(Component)]
struct GameOverNewBest;

#[derive(Component)]
struct GameOverBestText;

#[derive(Component)]
struct GameOverRunHistory;

#[derive(Resource, Default)]
struct RunRecords {
    best_depth_m: i32,
    recent_depths: Vec<i32>,
}

impl RunRecords {
    fn load() -> Self {
        let Ok(content) = fs::read_to_string(SCORES_FILE) else {
            return Self::default();
        };

        let mut lines = content.lines();
        let best_depth_m = lines
            .next()
            .and_then(|line| line.strip_prefix("best="))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0);
        let recent_depths = lines
            .next()
            .map(|line| {
                line.split(',')
                    .filter_map(|value| value.trim().parse().ok())
                    .take(MAX_RECENT_RUNS)
                    .collect()
            })
            .unwrap_or_default();

        Self {
            best_depth_m,
            recent_depths,
        }
    }

    fn save(&self) -> io::Result<()> {
        let recent = self
            .recent_depths
            .iter()
            .map(|depth| depth.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let content = format!("best={}\n{}", self.best_depth_m, recent);
        fs::write(SCORES_FILE, content)?;
        Ok(())
    }

    fn record_run(&mut self, depth_m: i32) -> io::Result<bool> {
        let new_best = depth_m > self.best_depth_m;
        if new_best {
            self.best_depth_m = depth_m;
        }

        self.recent_depths.insert(0, depth_m);
        self.recent_depths.truncate(MAX_RECENT_RUNS);
        self.save()?;
        Ok(new_best)
    }
}

fn format_run_history(recent_depths: &[i32]) -> String {
    if recent_depths.is_empty() {
        return "Recent runs: —".to_string();
    }

    let depths = recent_depths
        .iter()
        .map(|depth| format!("{depth}m"))
        .collect::<Vec<_>>()
        .join(" · ");
    format!("Recent: {depths}")
}

#[derive(Component)]
struct MenuStartButton;

#[derive(Component)]
struct RestartButton;

#[derive(Component)]
struct MenuHowToButton;

#[derive(Component)]
struct HowToModal;

#[derive(Component)]
struct CloseHowToButton;

#[derive(Component)]
struct MainMenuButton;

#[derive(Resource)]
struct UiTheme {
    menu_backdrop: Color,
    panel_fill: Color,
    panel_shadow: Color,
    text_primary: Color,
    text_secondary: Color,
    hud_panel: Color,
    button_fill: Color,
    button_hover: Color,
    button_pressed: Color,
    button_text: Color,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self {
            menu_backdrop: Color::srgba(0.149, 0.294, 0.412, 0.88), // astronaut
            panel_fill: Color::srgba(0.239, 0.451, 0.522, 0.86),    // ming
            panel_shadow: Color::srgba(0.149, 0.294, 0.412, 0.48),  // astronaut
            text_primary: Color::linear_rgba(1.15, 1.45, 1.55, 1.0), // casper — HDR for bloom
            text_secondary: Color::linear_rgba(0.85, 1.2, 1.25, 1.0), // neptune
            hud_panel: Color::srgba(0.345, 0.557, 0.655, 0.62),     // horizon
            button_fill: Color::linear_rgba(1.0, 1.35, 1.45, 1.0),  // casper — HDR for bloom
            button_hover: Color::linear_rgba(0.9, 1.25, 1.3, 1.0), // neptune
            button_pressed: Color::srgb(0.345, 0.557, 0.655),       // horizon
            button_text: Color::srgb(0.149, 0.294, 0.412),          // astronaut
        }
    }
}

#[derive(Component)]
struct MenuBubble {
    top_px: f32,
    speed: f32,
    size: f32,
}

#[derive(Component)]
struct PopParticle {
    velocity: Vec2,
    remaining: f32,
    base_color: Vec3,
    start_alpha: f32,
}

#[derive(Resource, Default)]
struct CaveGeneration {
    last_gap_center_x: f32,
    spawned_segment_count: u32,
}

#[derive(Resource, Default)]
struct DepthState {
    pixels_scrolled: f32,
}

#[derive(Resource, Default)]
struct RunState {
    time_alive_secs: f32,
}

#[derive(Resource, Default)]
struct ScreenShake {
    trauma: f32,
    elapsed_secs: f32,
}

#[derive(Component)]
struct FadeOverlay;

#[derive(Component)]
struct WorldRoot;

#[derive(Resource)]
struct WorldRootEntity(Entity);

#[derive(Clone, Copy, PartialEq, Eq)]
enum FadePhase {
    FadeOut,
    FadeIn,
}

struct FadeTransitionActive {
    phase: FadePhase,
    target: GameState,
    elapsed: f32,
}

#[derive(Resource, Default)]
struct FadeTransition {
    active: Option<FadeTransitionActive>,
}

fn fade_transition_active(fade: Res<FadeTransition>) -> bool {
    fade.active.is_some()
}

fn gameplay_allowed(state: Res<State<GameState>>, fade: Res<FadeTransition>) -> bool {
    *state.get() == GameState::Playing && fade.active.is_none()
}

fn depth_blend_t(depth_meters: f32) -> f32 {
    (depth_meters / DEPTH_COLOR_RAMP_METERS).clamp(0.0, 1.0)
}

fn lerp_color(from: Color, to: Color, t: f32) -> Color {
    from.mix(&to, t)
}

fn cave_wall_color_for_depth(depth_meters: f32) -> Color {
    let t = depth_blend_t(depth_meters);
    let shallow = Color::srgb(0.45, 0.72, 0.82);
    let mid = Color::srgb(0.24, 0.14, 0.40);
    let abyss = Color::srgb(0.06, 0.03, 0.10);
    if t < 0.55 {
        lerp_color(shallow, mid, t / 0.55)
    } else {
        lerp_color(mid, abyss, (t - 0.55) / 0.45)
    }
}

fn atmosphere_clear_color(depth_meters: f32) -> Color {
    let shallow = Color::srgb(0.149, 0.294, 0.412);
    let deep = Color::srgb(0.03, 0.02, 0.07);
    lerp_color(shallow, deep, depth_blend_t(depth_meters))
}

fn spawn_depth_meters(spawn_index: u32) -> f32 {
    spawn_index as f32 * METERS_PER_SEGMENT
}

fn request_game_state_change(fade: &mut FadeTransition, target: GameState) {
    if fade.active.is_some() {
        return;
    }
    fade.active = Some(FadeTransitionActive {
        phase: FadePhase::FadeOut,
        target,
        elapsed: 0.0,
    });
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Pressurized".into(),
                resolution: (SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.149, 0.294, 0.412)))
        .init_state::<GameState>()
        .init_resource::<CaveGeneration>()
        .init_resource::<DepthState>()
        .init_resource::<RunState>()
        .init_resource::<ScreenShake>()
        .init_resource::<FadeTransition>()
        .insert_resource(RunRecords::load())
        .init_resource::<UiTheme>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Menu), enter_menu)
        .add_systems(OnEnter(GameState::Playing), (enter_playing, reset_new_best_banner))
        .add_systems(
            OnEnter(GameState::GameOver),
            (record_game_over_run, enter_game_over).chain(),
        )
        .add_systems(
            Update,
            (
                steer_circle,
                bubble_wobble,
                detect_wall_collision,
                scroll_cave_segments,
                update_depth_ui,
                update_depth_atmosphere,
            )
                .chain()
                .run_if(gameplay_allowed),
        )
        .add_systems(Update, update_fade_transition)
        .add_systems(
            Update,
            menu_input.run_if(in_state(GameState::Menu).and(not(fade_transition_active))),
        )
        .add_systems(
            Update,
            menu_button_input.run_if(in_state(GameState::Menu).and(not(fade_transition_active))),
        )
        .add_systems(Update, menu_how_to_button_input.run_if(in_state(GameState::Menu)))
        .add_systems(Update, menu_close_how_to_button_input.run_if(in_state(GameState::Menu)))
        .add_systems(Update, update_menu_bubbles.run_if(in_state(GameState::Menu)))
        .add_systems(
            Update,
            game_over_input.run_if(in_state(GameState::GameOver).and(not(fade_transition_active))),
        )
        .add_systems(
            Update,
            game_over_button_input
                .run_if(in_state(GameState::GameOver).and(not(fade_transition_active))),
        )
        .add_systems(
            Update,
            game_over_menu_button_input
                .run_if(in_state(GameState::GameOver).and(not(fade_transition_active))),
        )
        .add_systems(
            Update,
            update_pop_particles.run_if(in_state(GameState::GameOver)),
        )
        .add_systems(
            PostUpdate,
            update_screen_shake.after(TransformSystems::Propagate),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut cave_generation: ResMut<CaveGeneration>,
    asset_server: Res<AssetServer>,
    ui_theme: Res<UiTheme>,
) {
    let comic_bold = asset_server.load("fonts/ComicNeue-Bold.ttf");
    let comic_bold_italic = asset_server.load("fonts/ComicNeue-BoldItalic.ttf");

    commands.spawn((
        Camera2d,
        Hdr,
        Bloom {
            intensity: 0.32,
            low_frequency_boost: 0.82,
            low_frequency_boost_curvature: 0.92,
            high_pass_frequency: 0.85,
            prefilter: BloomPrefilter {
                threshold: 0.72,
                threshold_softness: 0.4,
            },
            composite_mode: BloomCompositeMode::Additive,
            ..Bloom::default()
        },
    ));
    let world_root = commands
        .spawn((WorldRoot, Transform::default(), Visibility::Visible))
        .id();
    commands.insert_resource(WorldRootEntity(world_root));
    reset_and_spawn_cave(
        &mut commands,
        &mut meshes,
        &mut materials,
        cave_generation.as_mut(),
        world_root,
    );
    commands.spawn((
        ChildOf(world_root),
        Mesh2d(meshes.add(Circle::new(BUBBLE_RADIUS))),
        MeshMaterial2d(materials.add(Color::linear_rgba(1.35, 1.75, 1.9, 0.88))),
        Transform::from_translation(BUBBLE_START),
        RisingCircle,
        HorizontalVelocity::default(),
        GameplayEntity,
        Visibility::Visible,
    ))
    .with_children(|parent| {
        parent.spawn((
            Mesh2d(meshes.add(Circle::new(BUBBLE_RADIUS * 0.2))),
            // Below bloom threshold so the specular stays visible inside the glowing shell.
            MeshMaterial2d(materials.add(Color::srgba(0.94, 0.99, 1.0, 0.9))),
            Transform::from_xyz(-BUBBLE_RADIUS * 0.25, BUBBLE_RADIUS * 0.25, 0.15),
        ));
    });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        GameOverUi,
        Visibility::Hidden,
    ))
    .with_children(|parent| {
        parent
            .spawn((
                Node {
                    width: Val::Px(520.0),
                    height: Val::Px(430.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                    left: Val::Px(2.0),
                    top: Val::Px(2.0),
                        width: Val::Px(520.0),
                        height: Val::Px(430.0),
                        border_radius: BorderRadius::all(Val::Px(18.0)),
                        ..default()
                    },
                    BackgroundColor(ui_theme.panel_shadow),
                ));
                panel.spawn((
                    Node {
                        width: Val::Px(520.0),
                        height: Val::Px(430.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(18.0)),
                        ..default()
                    },
                    BackgroundColor(ui_theme.panel_fill),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("You Popped!"),
                        TextFont {
                            font_size: 38.0,
                            font: comic_bold.clone(),
                            ..default()
                        },
                        TextColor(ui_theme.text_primary),
                    ));
                    card.spawn((
                        Text::new("Depth: 0m"),
                        TextFont {
                            font_size: 38.0,
                            font: comic_bold.clone(),
                            ..default()
                        },
                        TextColor(ui_theme.text_primary),
                        GameOverText,
                    ));
                    card.spawn((
                        Text::new("New Best!"),
                        TextFont {
                            font_size: 26.0,
                            font: comic_bold_italic.clone(),
                            ..default()
                        },
                        TextColor(ui_theme.text_primary),
                        GameOverNewBest,
                        Visibility::Hidden,
                    ));
                    card.spawn((
                        Text::new("Best: 0m"),
                        TextFont {
                            font_size: 20.0,
                            font: comic_bold.clone(),
                            ..default()
                        },
                        TextColor(ui_theme.text_secondary),
                        GameOverBestText,
                    ));
                    card.spawn((
                        Text::new("Recent: —"),
                        TextFont {
                            font_size: 17.0,
                            font: comic_bold.clone(),
                            ..default()
                        },
                        TextColor(ui_theme.text_secondary),
                        GameOverRunHistory,
                    ));
                    card.spawn((
                        Button,
                        Node {
                            width: Val::Px(240.0),
                            height: Val::Px(68.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(14.0)),
                            margin: UiRect::top(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(ui_theme.button_fill),
                        RestartButton,
                    ))
                    .with_children(|button| {
                        button
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    row_gap: Val::Px(2.0),
                                    ..default()
                                },
                            ))
                            .with_children(|stack| {
                                stack.spawn((
                                    Text::new("Restart"),
                                    TextFont {
                                        font_size: 24.0,
                                        font: comic_bold.clone(),
                                        ..default()
                                    },
                                    TextColor(ui_theme.button_text),
                                ));
                                stack.spawn((
                                    Text::new("or press R"),
                                    TextFont {
                                        font_size: 14.0,
                                        font: comic_bold.clone(),
                                        ..default()
                                    },
                                    TextColor(ui_theme.button_text),
                                ));
                            });
                    });
                    card.spawn((
                        Button,
                        Node {
                            width: Val::Px(240.0),
                            height: Val::Px(54.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(14.0)),
                            margin: UiRect::top(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(ui_theme.button_fill),
                        MainMenuButton,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Main Menu"),
                            TextFont {
                                font_size: 24.0,
                                font: comic_bold.clone(),
                                ..default()
                            },
                            TextColor(ui_theme.button_text),
                        ));
                    });
                });
            });
    });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(0.0),
            ..default()
        },
        BackgroundColor(ui_theme.menu_backdrop),
        MenuUi,
        Visibility::Visible,
    ))
    .with_children(|parent| {
        for _ in 0..MENU_BUBBLE_COUNT {
            let size = fastrand::f32() * 14.0 + 8.0;
            let left_px = fastrand::f32() * SCREEN_WIDTH;
            let top_px = fastrand::f32() * SCREEN_HEIGHT;
            let speed = fastrand::f32() * 28.0 + 18.0;

            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left_px),
                    top: Val::Px(top_px),
                    width: Val::Px(size),
                    height: Val::Px(size),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.525, 0.729, 0.757, 0.26)),
                MenuBubble {
                    top_px,
                    speed,
                    size,
                },
            ));
        }
        parent.spawn((
            Node {
                width: Val::Px(760.0),
                height: Val::Px(340.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|panel| {
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(2.0),
                    top: Val::Px(2.0),
                    width: Val::Px(760.0),
                    height: Val::Px(340.0),
                    border_radius: BorderRadius::all(Val::Px(26.0)),
                    ..default()
                },
                BackgroundColor(ui_theme.panel_shadow),
            ));
            panel
                .spawn((
                    Node {
                        width: Val::Px(760.0),
                        height: Val::Px(340.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(14.0),
                        border_radius: BorderRadius::all(Val::Px(26.0)),
                        ..default()
                    },
                    BackgroundColor(ui_theme.panel_fill),
                ))
                .with_children(|card| {
                    card.spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(14.0),
                            ..default()
                        },
                    ))
                    .with_children(|title_row| {
                        title_row
                            .spawn((
                                Node {
                                    width: Val::Px(32.0),
                                    height: Val::Px(32.0),
                                    border_radius: BorderRadius::MAX,
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.631, 0.773, 0.808, 0.95)),
                            ))
                            .with_children(|bubble| {
                                bubble.spawn((
                                    Node {
                                        width: Val::Px(10.0),
                                        height: Val::Px(10.0),
                                        border_radius: BorderRadius::MAX,
                                        position_type: PositionType::Absolute,
                                        top: Val::Px(4.0),
                                        left: Val::Px(6.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.89, 0.969, 0.949, 0.85)),
                                ));
                            });
                        title_row.spawn((
                            Text::new("PRESSURIZED"),
                            TextFont {
                                font_size: 72.0,
                                font: comic_bold.clone(),
                                ..default()
                            },
                            TextColor(ui_theme.text_primary),
                        ));
                    });
                    card.spawn((
                        Button,
                        Node {
                            width: Val::Px(280.0),
                            height: Val::Px(72.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(14.0)),
                            margin: UiRect::top(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(ui_theme.button_fill),
                        MenuStartButton,
                    ))
                    .with_children(|button| {
                        button
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    row_gap: Val::Px(2.0),
                                    ..default()
                                },
                            ))
                            .with_children(|stack| {
                                stack.spawn((
                                    Text::new("Play"),
                                    TextFont {
                                        font_size: 24.0,
                                        font: comic_bold.clone(),
                                        ..default()
                                    },
                                    TextColor(ui_theme.button_text),
                                ));
                                stack.spawn((
                                    Text::new("or press SPACE"),
                                    TextFont {
                                        font_size: 13.0,
                                        font: comic_bold.clone(),
                                        ..default()
                                    },
                                    TextColor(ui_theme.button_text),
                                ));
                            });
                    });
                    card.spawn((
                        Button,
                        Node {
                            width: Val::Px(280.0),
                            height: Val::Px(52.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(14.0)),
                            margin: UiRect::top(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(ui_theme.button_fill),
                        MenuHowToButton,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("How to Play"),
                            TextFont {
                                font_size: 22.0,
                                font: comic_bold.clone(),
                                ..default()
                            },
                            TextColor(ui_theme.button_text),
                        ));
                    });
                    card.spawn((
                        Text::new("This game will cost you a $499/mo subscription soon so enjoy will it lasts..."),
                        TextFont {
                            font_size: 13.0,
                            font: comic_bold_italic.clone(),
                            ..default()
                        },
                        TextColor(ui_theme.text_secondary),
                    ));
                });
        });

        parent
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.149, 0.294, 0.412)),
                HowToModal,
                Visibility::Hidden,
            ))
            .with_children(|overlay| {
                overlay
                    .spawn((
                        Node {
                            width: Val::Px(540.0),
                            height: Val::Px(280.0),
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::FlexStart,
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(24.0)),
                            row_gap: Val::Px(12.0),
                            border_radius: BorderRadius::all(Val::Px(18.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.239, 0.451, 0.522)),
                    ))
                    .with_children(|modal| {
                        modal.spawn((
                            Button,
                            Node {
                                position_type: PositionType::Absolute,
                                right: Val::Px(14.0),
                                top: Val::Px(14.0),
                                width: Val::Px(34.0),
                                height: Val::Px(34.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(Val::Px(10.0)),
                                ..default()
                            },
                            BackgroundColor(ui_theme.button_fill),
                            CloseHowToButton,
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("X"),
                                TextFont {
                                    font_size: 20.0,
                                    font: comic_bold.clone(),
                                    ..default()
                                },
                                TextColor(ui_theme.button_text),
                            ));
                        });

                        modal.spawn((
                            Text::new("How to Play"),
                            TextFont {
                                font_size: 30.0,
                                font: comic_bold.clone(),
                                ..default()
                            },
                            TextColor(ui_theme.text_primary),
                        ));
                        modal.spawn((
                            Text::new("1. Click Play (or press SPACE)."),
                            TextFont {
                                font_size: 20.0,
                                font: comic_bold.clone(),
                                ..default()
                            },
                            TextColor(ui_theme.text_secondary),
                        ));
                        modal.spawn((
                            Text::new("2. Use LEFT/RIGHT arrows to steer the bubble."),
                            TextFont {
                                font_size: 20.0,
                                font: comic_bold.clone(),
                                ..default()
                            },
                            TextColor(ui_theme.text_secondary),
                        ));
                        modal.spawn((
                            Text::new("3. Stay in the gap and go deeper."),
                            TextFont {
                                font_size: 20.0,
                                font: comic_bold.clone(),
                                ..default()
                            },
                            TextColor(ui_theme.text_secondary),
                        ));
                    });
            });
    });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            width: Val::Px(280.0),
            height: Val::Px(62.0),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            padding: UiRect::left(Val::Px(14.0)),
            border_radius: BorderRadius::all(Val::Px(14.0)),
            ..default()
        },
        BackgroundColor(ui_theme.hud_panel),
        DepthHud,
        GameplayEntity,
        Visibility::Hidden,
    ))
    .with_children(|hud| {
        hud.spawn((
            Text::new("Depth: 0m"),
            TextFont {
                font_size: 30.0,
                font: comic_bold,
                ..default()
            },
            TextColor(ui_theme.text_primary),
            DepthHudValue,
        ));
    });
    commands.spawn((
        FadeOverlay,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ZIndex(1000),
        Visibility::Hidden,
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
    ));
}

fn update_fade_transition(
    time: Res<Time>,
    mut fade: ResMut<FadeTransition>,
    mut overlay_query: Query<(&mut BackgroundColor, &mut Visibility), With<FadeOverlay>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok((mut overlay_color, mut overlay_visibility)) = overlay_query.single_mut() else {
        return;
    };

    let Some(active) = fade.active.as_mut() else {
        *overlay_visibility = Visibility::Hidden;
        overlay_color.0 = Color::srgba(0.0, 0.0, 0.0, 0.0);
        return;
    };

    *overlay_visibility = Visibility::Visible;

    active.elapsed += time.delta_secs();
    let t = (active.elapsed / FADE_DURATION).min(1.0);

    let alpha = match active.phase {
        FadePhase::FadeOut => t,
        FadePhase::FadeIn => 1.0 - t,
    };

    overlay_color.0 = Color::srgba(0.0, 0.0, 0.0, alpha);

    if t < 1.0 {
        return;
    }

    if active.phase == FadePhase::FadeOut {
        next_state.set(active.target);
        active.phase = FadePhase::FadeIn;
        active.elapsed = 0.0;
        return;
    }

    fade.active = None;
    *overlay_visibility = Visibility::Hidden;
    overlay_color.0 = Color::srgba(0.0, 0.0, 0.0, 0.0);
}

fn spawn_cave_segment(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    world_root: Entity,
    y: f32,
    gap_center_x: f32,
    wall_depth_meters: f32,
    teeth_in_upper_half: bool,
) {
    let wall_color = cave_wall_color_for_depth(wall_depth_meters);
    let half_screen = SCREEN_WIDTH * 0.5;
    let left_gap_edge = gap_center_x - GAP_WIDTH * 0.5;
    let right_gap_edge = gap_center_x + GAP_WIDTH * 0.5;
    let left_wall_width = left_gap_edge + half_screen;
    let right_wall_width = half_screen - right_gap_edge;
    let left_wall_x = (-half_screen + left_gap_edge) * 0.5;
    let right_wall_x = (right_gap_edge + half_screen) * 0.5;

    commands
        .spawn((
            ChildOf(world_root),
            CaveSegment { gap_center_x },
            Transform::from_xyz(0.0, y, 0.0),
            GlobalTransform::default(),
            GameplayEntity,
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh2d(meshes.add(Rectangle::new(left_wall_width, WALL_VISUAL_HEIGHT))),
                MeshMaterial2d(materials.add(wall_color)),
                Transform::from_xyz(left_wall_x, 0.0, 0.0),
                CaveWall,
            ));
            parent.spawn((
                Mesh2d(meshes.add(Rectangle::new(right_wall_width, WALL_VISUAL_HEIGHT))),
                MeshMaterial2d(materials.add(wall_color)),
                Transform::from_xyz(right_wall_x, 0.0, 0.0),
                CaveWall,
            ));

            let tooth_offsets = if teeth_in_upper_half {
                [6.0, 30.0]
            } else {
                [-6.0, -30.0]
            };

            for offset_y in tooth_offsets {
                parent.spawn((
                    Mesh2d(meshes.add(Triangle2d::new(
                        Vec2::new(0.0, -TOOTH_HEIGHT * 0.5),
                        Vec2::new(0.0, TOOTH_HEIGHT * 0.5),
                        Vec2::new(TOOTH_DEPTH, 0.0),
                    ))),
                    MeshMaterial2d(materials.add(wall_color)),
                    Transform::from_xyz(left_gap_edge, offset_y, 0.0),
                    CaveWall,
                ));
                parent.spawn((
                    Mesh2d(meshes.add(Triangle2d::new(
                        Vec2::new(0.0, -TOOTH_HEIGHT * 0.5),
                        Vec2::new(0.0, TOOTH_HEIGHT * 0.5),
                        Vec2::new(-TOOTH_DEPTH, 0.0),
                    ))),
                    MeshMaterial2d(materials.add(wall_color)),
                    Transform::from_xyz(right_gap_edge, offset_y, 0.0),
                    CaveWall,
                ));
            }
        });
}

fn reset_and_spawn_cave(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cave_generation: &mut CaveGeneration,
    world_root: Entity,
) {
    let segment_count = (SCREEN_HEIGHT / SEGMENT_HEIGHT).ceil() as i32 + 1;
    let first_segment_y = -SCREEN_HEIGHT * 0.5 + SEGMENT_HEIGHT * 0.5;

    cave_generation.last_gap_center_x = 0.0;
    cave_generation.spawned_segment_count = 0;

    for i in 0..segment_count {
        let y = first_segment_y + i as f32 * SEGMENT_HEIGHT;
        let (gap_center_x, spawn_index) = next_segment_gap_center(cave_generation);
        spawn_cave_segment(
            commands,
            meshes,
            materials,
            world_root,
            y,
            gap_center_x,
            spawn_depth_meters(spawn_index),
            spawn_index % 2 == 0,
        );
    }
}

fn next_segment_gap_center(cave_generation: &mut CaveGeneration) -> (f32, u32) {
    let spawn_index = cave_generation.spawned_segment_count;
    let gap_center_x = if spawn_index < CENTERED_START_SEGMENTS {
        0.0
    } else {
        next_gap_center(cave_generation.last_gap_center_x)
    };
    cave_generation.last_gap_center_x = gap_center_x;
    cave_generation.spawned_segment_count += 1;
    (gap_center_x, spawn_index)
}

fn next_gap_center(previous_gap_center_x: f32) -> f32 {
    let delta = fastrand::f32() * (GAP_DRIFT_PER_SEGMENT * 2.0) - GAP_DRIFT_PER_SEGMENT;
    let unclamped = previous_gap_center_x + delta;
    let min_center = -SCREEN_WIDTH * 0.5 + GAP_MARGIN + GAP_WIDTH * 0.5;
    let max_center = SCREEN_WIDTH * 0.5 - GAP_MARGIN - GAP_WIDTH * 0.5;
    unclamped.clamp(min_center, max_center)
}

fn enter_menu(
    mut clear_color: ResMut<ClearColor>,
    mut gameplay_query: Query<
        &mut Visibility,
        (With<GameplayEntity>, Without<MenuUi>, Without<GameOverUi>),
    >,
    mut menu_query: Query<
        &mut Visibility,
        (With<MenuUi>, Without<GameplayEntity>, Without<GameOverUi>),
    >,
    mut game_over_query: Query<
        &mut Visibility,
        (With<GameOverUi>, Without<GameplayEntity>, Without<MenuUi>),
    >,
    mut how_to_modal_query: Query<
        &mut Visibility,
        (
            With<HowToModal>,
            Without<GameplayEntity>,
            Without<MenuUi>,
            Without<GameOverUi>,
        ),
    >,
) {
    clear_color.0 = atmosphere_clear_color(0.0);
    for mut visibility in &mut gameplay_query {
        *visibility = Visibility::Hidden;
    }
    if let Ok(mut menu_visibility) = menu_query.single_mut() {
        *menu_visibility = Visibility::Visible;
    }
    if let Ok(mut game_over_visibility) = game_over_query.single_mut() {
        *game_over_visibility = Visibility::Hidden;
    }
    if let Ok(mut modal_visibility) = how_to_modal_query.single_mut() {
        *modal_visibility = Visibility::Hidden;
    }
}

fn enter_playing(
    mut clear_color: ResMut<ClearColor>,
    mut commands: Commands,
    mut cave_generation: ResMut<CaveGeneration>,
    mut depth_state: ResMut<DepthState>,
    mut run_state: ResMut<RunState>,
    mut screen_shake: ResMut<ScreenShake>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut gameplay_query: Query<
        &mut Visibility,
        (With<GameplayEntity>, Without<MenuUi>, Without<GameOverUi>),
    >,
    mut menu_query: Query<
        &mut Visibility,
        (With<MenuUi>, Without<GameplayEntity>, Without<GameOverUi>),
    >,
    mut game_over_query: Query<
        &mut Visibility,
        (With<GameOverUi>, Without<GameplayEntity>, Without<MenuUi>),
    >,
    mut world_root_query: Query<(Entity, &mut Transform), (With<WorldRoot>, Without<RisingCircle>)>,
    mut bubble_query: Query<
        (&mut Transform, &mut HorizontalVelocity),
        (With<RisingCircle>, Without<WorldRoot>),
    >,
    segment_query: Query<Entity, With<CaveSegment>>,
    particle_query: Query<Entity, With<PopParticle>>,
    mut depth_hud_query: Query<&mut Text, With<DepthHud>>,
) {
    let Ok((world_root_entity, mut world_transform)) = world_root_query.single_mut() else {
        return;
    };
    world_transform.translation = Vec3::ZERO;

    for entity in &segment_query {
        commands.entity(entity).despawn();
    }
    for entity in &particle_query {
        commands.entity(entity).despawn();
    }
    reset_and_spawn_cave(
        &mut commands,
        &mut meshes,
        &mut materials,
        cave_generation.as_mut(),
        world_root_entity,
    );

    if let Ok((mut transform, mut velocity)) = bubble_query.single_mut() {
        transform.translation = BUBBLE_START;
        transform.scale = Vec3::ONE;
        velocity.0 = 0.0;
    }
    if let Ok(mut depth_hud_text) = depth_hud_query.single_mut() {
        *depth_hud_text = Text::new("Depth: 0m");
    }

    depth_state.pixels_scrolled = 0.0;
    clear_color.0 = atmosphere_clear_color(0.0);
    run_state.time_alive_secs = 0.0;
    screen_shake.trauma = 0.0;
    screen_shake.elapsed_secs = 0.0;

    for mut visibility in &mut gameplay_query {
        *visibility = Visibility::Visible;
    }
    if let Ok(mut menu_visibility) = menu_query.single_mut() {
        *menu_visibility = Visibility::Hidden;
    }
    if let Ok(mut game_over_visibility) = game_over_query.single_mut() {
        *game_over_visibility = Visibility::Hidden;
    }
}

fn enter_game_over(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut screen_shake: ResMut<ScreenShake>,
    world_root: Res<WorldRootEntity>,
    mut game_over_query: Query<&mut Visibility, (With<GameOverUi>, Without<DepthHud>)>,
    mut depth_hud_query: Query<&mut Visibility, (With<DepthHud>, Without<GameOverUi>)>,
    bubble_query: Query<&Transform, With<RisingCircle>>,
) {
    if let Ok(mut game_over_visibility) = game_over_query.single_mut() {
        *game_over_visibility = Visibility::Visible;
    }
    if let Ok(mut depth_hud_visibility) = depth_hud_query.single_mut() {
        *depth_hud_visibility = Visibility::Hidden;
    }

    screen_shake.trauma = 1.0;
    screen_shake.elapsed_secs = 0.0;

    if let Ok(bubble_transform) = bubble_query.single() {
        let origin = bubble_transform.translation;
        for _ in 0..POP_PARTICLE_COUNT {
            let radius = fastrand::f32() * 4.0 + 4.0;
            let angle = fastrand::f32() * std::f32::consts::TAU;
            let speed = fastrand::f32() * 200.0 + 80.0;
            let velocity = Vec2::new(angle.cos(), angle.sin()) * speed;
            let start_alpha = fastrand::f32() * 0.4 + 0.5;
            let base_color = Vec3::new(
                0.525 + fastrand::f32() * 0.106, // neptune -> casper range
                0.729 + fastrand::f32() * 0.044,
                0.757 + fastrand::f32() * 0.051,
            );

            commands.spawn((
                ChildOf(world_root.0),
                Mesh2d(meshes.add(Circle::new(radius))),
                MeshMaterial2d(materials.add(Color::srgba(
                    base_color.x,
                    base_color.y,
                    base_color.z,
                    start_alpha,
                ))),
                Transform::from_xyz(origin.x, origin.y, 0.2),
                PopParticle {
                    velocity,
                    remaining: POP_PARTICLE_LIFETIME,
                    base_color,
                    start_alpha,
                },
                GameplayEntity,
            ));
        }
    }
}

fn record_game_over_run(
    depth_state: Res<DepthState>,
    mut run_records: ResMut<RunRecords>,
    mut depth_text_query: Query<&mut Text, With<GameOverText>>,
    mut best_text_query: Query<&mut Text, (With<GameOverBestText>, Without<GameOverText>)>,
    mut history_text_query: Query<
        &mut Text,
        (
            With<GameOverRunHistory>,
            Without<GameOverText>,
            Without<GameOverBestText>,
        ),
    >,
    mut new_best_query: Query<
        &mut Visibility,
        (
            With<GameOverNewBest>,
            Without<GameOverText>,
            Without<GameOverBestText>,
            Without<GameOverRunHistory>,
        ),
    >,
) {
    if let Ok(mut new_best_visibility) = new_best_query.single_mut() {
        *new_best_visibility = Visibility::Hidden;
    }

    let depth_m = (depth_state.pixels_scrolled / PIXELS_PER_METER).floor() as i32;
    let new_best = run_records.record_run(depth_m).unwrap_or(false);

    if let Ok(mut depth_text) = depth_text_query.single_mut() {
        *depth_text = Text::new(format!("Depth: {depth_m}m"));
    }
    if let Ok(mut best_text) = best_text_query.single_mut() {
        *best_text = Text::new(format!("Best: {}m", run_records.best_depth_m));
    }
    if let Ok(mut history_text) = history_text_query.single_mut() {
        *history_text = Text::new(format_run_history(&run_records.recent_depths));
    }
    if let Ok(mut new_best_visibility) = new_best_query.single_mut() {
        *new_best_visibility = if new_best {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn reset_new_best_banner(mut new_best_query: Query<&mut Visibility, With<GameOverNewBest>>) {
    for mut visibility in &mut new_best_query {
        *visibility = Visibility::Hidden;
    }
}

fn menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut fade: ResMut<FadeTransition>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        request_game_state_change(fade.as_mut(), GameState::Playing);
    }
}

fn menu_button_input(
    mut query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>, With<MenuStartButton>),
    >,
    mut fade: ResMut<FadeTransition>,
    ui_theme: Res<UiTheme>,
) {
    for (interaction, mut color) in &mut query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(ui_theme.button_pressed);
                request_game_state_change(fade.as_mut(), GameState::Playing);
            }
            Interaction::Hovered => *color = BackgroundColor(ui_theme.button_hover),
            Interaction::None => *color = BackgroundColor(ui_theme.button_fill),
        }
    }
}

fn menu_how_to_button_input(
    mut query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>, With<MenuHowToButton>),
    >,
    mut modal_query: Query<&mut Visibility, With<HowToModal>>,
    ui_theme: Res<UiTheme>,
) {
    for (interaction, mut color) in &mut query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(ui_theme.button_pressed);
                if let Ok(mut modal_visibility) = modal_query.single_mut() {
                    *modal_visibility = Visibility::Visible;
                }
            }
            Interaction::Hovered => *color = BackgroundColor(ui_theme.button_hover),
            Interaction::None => *color = BackgroundColor(ui_theme.button_fill),
        }
    }
}

fn menu_close_how_to_button_input(
    mut query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>, With<CloseHowToButton>),
    >,
    mut modal_query: Query<&mut Visibility, With<HowToModal>>,
    ui_theme: Res<UiTheme>,
) {
    for (interaction, mut color) in &mut query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(ui_theme.button_pressed);
                if let Ok(mut modal_visibility) = modal_query.single_mut() {
                    *modal_visibility = Visibility::Hidden;
                }
            }
            Interaction::Hovered => *color = BackgroundColor(ui_theme.button_hover),
            Interaction::None => *color = BackgroundColor(ui_theme.button_fill),
        }
    }
}

fn update_menu_bubbles(time: Res<Time>, mut query: Query<(&mut Node, &mut MenuBubble)>) {
    for (mut node, mut bubble) in &mut query {
        bubble.top_px -= bubble.speed * time.delta_secs();
        if bubble.top_px < -bubble.size {
            bubble.top_px = SCREEN_HEIGHT + bubble.size;
            node.left = Val::Px(fastrand::f32() * SCREEN_WIDTH);
        }
        node.top = Val::Px(bubble.top_px);
    }
}

fn game_over_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut fade: ResMut<FadeTransition>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        request_game_state_change(fade.as_mut(), GameState::Playing);
    }
}

fn game_over_button_input(
    mut query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>, With<RestartButton>),
    >,
    mut fade: ResMut<FadeTransition>,
    ui_theme: Res<UiTheme>,
) {
    for (interaction, mut color) in &mut query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(ui_theme.button_pressed);
                request_game_state_change(fade.as_mut(), GameState::Playing);
            }
            Interaction::Hovered => *color = BackgroundColor(ui_theme.button_hover),
            Interaction::None => *color = BackgroundColor(ui_theme.button_fill),
        }
    }
}

fn game_over_menu_button_input(
    mut query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>, With<MainMenuButton>),
    >,
    mut fade: ResMut<FadeTransition>,
    ui_theme: Res<UiTheme>,
) {
    for (interaction, mut color) in &mut query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(ui_theme.button_pressed);
                request_game_state_change(fade.as_mut(), GameState::Menu);
            }
            Interaction::Hovered => *color = BackgroundColor(ui_theme.button_hover),
            Interaction::None => *color = BackgroundColor(ui_theme.button_fill),
        }
    }
}

fn update_pop_particles(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut particles: Query<(Entity, &mut Transform, &MeshMaterial2d<ColorMaterial>, &mut PopParticle)>,
) {
    let dt = time.delta_secs();

    for (entity, mut transform, material_handle, mut particle) in &mut particles {
        particle.remaining -= dt;
        if particle.remaining <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }

        transform.translation.x += particle.velocity.x * dt;
        transform.translation.y += particle.velocity.y * dt;

        let alpha = particle.start_alpha * (particle.remaining / POP_PARTICLE_LIFETIME);
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.color = Color::srgba(
                particle.base_color.x,
                particle.base_color.y,
                particle.base_color.z,
                alpha.max(0.0),
            );
        }
    }
}

fn screen_shake_offset(trauma: f32) -> Vec2 {
    let shake_strength = trauma * trauma;
    Vec2::new(
        (fastrand::f32() * 2.0 - 1.0) * SCREEN_SHAKE_MAX_OFFSET * shake_strength,
        (fastrand::f32() * 2.0 - 1.0) * SCREEN_SHAKE_MAX_OFFSET * shake_strength,
    )
}

type WorldRootTransformQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Transform,
    (
        With<WorldRoot>,
        Without<RisingCircle>,
        Without<CaveSegment>,
    ),
>;

fn apply_screen_shake_offset(
    offset: Vec2,
    world_root_query: &mut WorldRootTransformQuery<'_, '_>,
    game_over_ui_query: &mut Query<&mut Node, With<GameOverUi>>,
) {
    if let Ok(mut world_transform) = world_root_query.single_mut() {
        world_transform.translation = Vec3::new(offset.x, offset.y, 0.0);
    }
    if let Ok(mut game_over_ui) = game_over_ui_query.single_mut() {
        game_over_ui.margin = UiRect {
            left: Val::Px(offset.x),
            top: Val::Px(offset.y),
            ..default()
        };
    }
}

fn clear_screen_shake_offset(
    world_root_query: &mut WorldRootTransformQuery<'_, '_>,
    game_over_ui_query: &mut Query<&mut Node, With<GameOverUi>>,
) {
    if let Ok(mut world_transform) = world_root_query.single_mut() {
        world_transform.translation = Vec3::ZERO;
    }
    if let Ok(mut game_over_ui) = game_over_ui_query.single_mut() {
        game_over_ui.margin = UiRect::ZERO;
    }
}

fn update_screen_shake(
    time: Res<Time>,
    mut screen_shake: ResMut<ScreenShake>,
    mut world_root_query: WorldRootTransformQuery,
    mut game_over_ui_query: Query<&mut Node, With<GameOverUi>>,
) {
    if screen_shake.trauma <= 0.0 {
        clear_screen_shake_offset(&mut world_root_query, &mut game_over_ui_query);
        return;
    }

    let dt = time.delta_secs();
    screen_shake.elapsed_secs += dt;
    let decay = dt / SCREEN_SHAKE_DURATION;
    screen_shake.trauma = (screen_shake.trauma - decay).max(0.0);

    apply_screen_shake_offset(
        screen_shake_offset(screen_shake.trauma),
        &mut world_root_query,
        &mut game_over_ui_query,
    );
}

fn bubble_hits_segment_wall(
    bubble_x: f32,
    bubble_y: f32,
    segment_transform: &Transform,
    segment: &CaveSegment,
) -> bool {
    let half_height = WALL_VISUAL_HEIGHT * 0.5;
    let min_y = segment_transform.translation.y - half_height;
    let max_y = segment_transform.translation.y + half_height;
    if bubble_y + BUBBLE_RADIUS < min_y || bubble_y - BUBBLE_RADIUS > max_y {
        return false;
    }

    let left_inner_edge = segment.gap_center_x - GAP_WIDTH * 0.5;
    let right_inner_edge = segment.gap_center_x + GAP_WIDTH * 0.5;
    bubble_x - BUBBLE_RADIUS <= left_inner_edge || bubble_x + BUBBLE_RADIUS >= right_inner_edge
}

fn steer_circle(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &mut HorizontalVelocity), With<RisingCircle>>,
) {
    let damping = 0.97;

    for (mut transform, mut velocity) in &mut query {
        let mut direction = 0.0;
        if keyboard.pressed(KeyCode::ArrowLeft) {
            direction -= 1.0;
        }
        if keyboard.pressed(KeyCode::ArrowRight) {
            direction += 1.0;
        }

        velocity.0 += direction * HORIZONTAL_ACCELERATION * time.delta_secs();
        velocity.0 = velocity
            .0
            .clamp(-HORIZONTAL_MAX_SPEED, HORIZONTAL_MAX_SPEED);
        velocity.0 *= damping;
        transform.translation.x += velocity.0 * time.delta_secs();
    }
}

fn bubble_wobble(mut query: Query<(&HorizontalVelocity, &mut Transform), With<RisingCircle>>) {
    for (velocity, mut transform) in &mut query {
        let stretch = (velocity.0.abs() / HORIZONTAL_MAX_SPEED) * 0.35;
        let scale_x = 1.0 + stretch;
        let scale_y = 1.0 / scale_x;
        transform.scale = Vec3::new(scale_x, scale_y, 1.0);
    }
}

fn scroll_cave_segments(
    time: Res<Time>,
    mut depth_state: ResMut<DepthState>,
    mut run_state: ResMut<RunState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut cave_generation: ResMut<CaveGeneration>,
    world_root: Res<WorldRootEntity>,
    segment_query: Query<(Entity, &Transform), With<CaveSegment>>,
) {
    run_state.time_alive_secs += time.delta_secs();
    let current_scroll_speed =
        (BASE_SCROLL_SPEED + run_state.time_alive_secs * SCROLL_RAMP_RATE).min(MAX_SCROLL_SPEED);
    let scroll_delta = current_scroll_speed * time.delta_secs();
    depth_state.pixels_scrolled += scroll_delta;
    let wall_depth_meters = depth_state.pixels_scrolled / PIXELS_PER_METER;

    let mut segments: Vec<(Entity, f32)> = segment_query
        .iter()
        .map(|(entity, transform)| (entity, transform.translation.y))
        .collect();
    segments.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let moves: Vec<(Entity, f32, bool)> = segments
        .iter()
        .map(|(entity, y)| {
            let new_y = y - scroll_delta;
            let below_screen = new_y + SEGMENT_HEIGHT * 0.5 < -SCREEN_HEIGHT * 0.5;
            (*entity, new_y, below_screen)
        })
        .collect();

    let mut ceiling = moves
        .iter()
        .filter(|(_, _, below)| !below)
        .map(|(_, new_y, _)| *new_y)
        .fold(f32::NEG_INFINITY, f32::max);
    if !ceiling.is_finite() {
        ceiling = -SCREEN_HEIGHT * 0.5 + SEGMENT_HEIGHT * 0.5;
    }

    for (entity, new_y, below_screen) in moves {
        if below_screen {
            commands.entity(entity).despawn();
            ceiling += SEGMENT_HEIGHT;
            let (next_gap_center_x, spawn_index) = next_segment_gap_center(cave_generation.as_mut());
            spawn_cave_segment(
                &mut commands,
                &mut meshes,
                &mut materials,
                world_root.0,
                ceiling,
                next_gap_center_x,
                wall_depth_meters,
                spawn_index % 2 == 0,
            );
        } else {
            commands
                .entity(entity)
                .insert(Transform::from_xyz(0.0, new_y, 0.0));
        }
    }
}

fn detect_wall_collision(
    mut next_state: ResMut<NextState<GameState>>,
    mut screen_shake: ResMut<ScreenShake>,
    bubble_query: Query<&Transform, (With<RisingCircle>, Without<WorldRoot>, Without<CaveSegment>)>,
    mut bubble_visibility_query: Query<&mut Visibility, With<RisingCircle>>,
    segment_query: Query<
        (&Transform, &CaveSegment),
        (Without<WorldRoot>, Without<RisingCircle>),
    >,
    mut world_root_query: WorldRootTransformQuery,
    mut game_over_ui_query: Query<&mut Node, With<GameOverUi>>,
) {
    let Ok(bubble_transform) = bubble_query.single() else {
        return;
    };

    let bubble_x = bubble_transform.translation.x;
    let bubble_y = bubble_transform.translation.y;

    let hit_wall = segment_query.iter().any(|(transform, segment)| {
        bubble_hits_segment_wall(bubble_x, bubble_y, transform, segment)
    });

    if hit_wall {
        screen_shake.trauma = 1.0;
        screen_shake.elapsed_secs = 0.0;
        apply_screen_shake_offset(
            screen_shake_offset(1.0),
            &mut world_root_query,
            &mut game_over_ui_query,
        );
        if let Ok(mut bubble_visibility) = bubble_visibility_query.single_mut() {
            *bubble_visibility = Visibility::Hidden;
        }
        next_state.set(GameState::GameOver);
    }
}

fn update_depth_atmosphere(
    depth_state: Res<DepthState>,
    mut clear_color: ResMut<ClearColor>,
) {
    let depth_meters = depth_state.pixels_scrolled / PIXELS_PER_METER;
    clear_color.0 = atmosphere_clear_color(depth_meters);
}

fn update_depth_ui(
    depth_state: Res<DepthState>,
    mut hud_query: Query<&mut Text, With<DepthHudValue>>,
) {
    let depth_meters = (depth_state.pixels_scrolled / PIXELS_PER_METER).floor() as i32;
    let depth_label = format!("Depth: {}m", depth_meters);

    for mut text in &mut hud_query {
        *text = Text::new(depth_label.clone());
    }
}
