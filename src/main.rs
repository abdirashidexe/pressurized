use bevy::prelude::*;
use bevy::post_process::bloom::Bloom;
use bevy::render::view::Hdr;

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

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
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
struct GameplayEntity;

#[derive(Component)]
struct DepthHud;

#[derive(Component)]
struct MenuUi;

#[derive(Component)]
struct GameOverUi;

#[derive(Component)]
struct GameOverText;

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
        .insert_resource(ClearColor(Color::srgb(0.04, 0.04, 0.12)))
        .init_state::<GameState>()
        .init_resource::<CaveGeneration>()
        .init_resource::<DepthState>()
        .init_resource::<RunState>()
        .init_resource::<ScreenShake>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Menu), enter_menu)
        .add_systems(OnEnter(GameState::Playing), enter_playing)
        .add_systems(OnEnter(GameState::GameOver), enter_game_over)
        .add_systems(
            Update,
            (
                steer_circle,
                bubble_wobble,
                scroll_cave_segments,
                detect_wall_collision,
                update_depth_ui,
            )
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, menu_input.run_if(in_state(GameState::Menu)))
        .add_systems(Update, update_menu_bubbles.run_if(in_state(GameState::Menu)))
        .add_systems(Update, game_over_input.run_if(in_state(GameState::GameOver)))
        .add_systems(
            Update,
            (update_pop_particles, update_screen_shake).run_if(in_state(GameState::GameOver)),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut cave_generation: ResMut<CaveGeneration>,
    asset_server: Res<AssetServer>,
) {
    let poppins_regular = asset_server.load("fonts/Poppins-Regular.ttf");
    let poppins_semibold = asset_server.load("fonts/Poppins-SemiBold.ttf");

    commands.spawn((
        Camera2d,
        Hdr,
        Bloom {
            intensity: 0.08,
            ..Bloom::default()
        },
    ));
    reset_and_spawn_cave(
        &mut commands,
        &mut meshes,
        &mut materials,
        cave_generation.as_mut(),
    );
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(BUBBLE_RADIUS))),
        MeshMaterial2d(materials.add(Color::linear_rgba(1.2, 1.6, 1.85, 0.75))),
        Transform::from_translation(BUBBLE_START),
        RisingCircle,
        HorizontalVelocity::default(),
        GameplayEntity,
        Visibility::Visible,
    ))
    .with_children(|parent| {
        parent.spawn((
            Mesh2d(meshes.add(Circle::new(BUBBLE_RADIUS * 0.3))),
            MeshMaterial2d(materials.add(Color::linear_rgba(1.9, 2.0, 2.0, 0.95))),
            Transform::from_xyz(-BUBBLE_RADIUS * 0.25, BUBBLE_RADIUS * 0.25, 0.1),
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
                    height: Val::Px(220.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.1, 0.88)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("You Popped!"),
                    TextFont {
                        font_size: 48.0,
                        font: poppins_semibold.clone(),
                        ..default()
                    },
                    TextColor(Color::linear_rgba(1.15, 1.15, 1.2, 1.0)),
                ));
                panel.spawn((
                    Text::new("Depth: 0m"),
                    TextFont {
                        font_size: 48.0,
                        font: poppins_regular.clone(),
                        ..default()
                    },
                    TextColor(Color::linear_rgba(1.15, 1.15, 1.2, 1.0)),
                    GameOverText,
                ));
                panel.spawn((
                    Text::new("Press R to restart"),
                    TextFont {
                        font_size: 48.0,
                        font: poppins_regular.clone(),
                        ..default()
                    },
                    TextColor(Color::linear_rgba(1.15, 1.15, 1.2, 1.0)),
                ));
            });
    });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            padding: UiRect::top(Val::Percent(18.0)),
            row_gap: Val::Px(22.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.01, 0.01, 0.015, 1.0)),
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
                BackgroundColor(Color::srgba(0.75, 0.9, 1.0, 0.22)),
                MenuBubble {
                    top_px,
                    speed,
                    size,
                },
            ));
        }
        parent.spawn((
            Text::new("PRESSURIZED"),
            TextFont {
                font_size: 86.0,
                font: poppins_semibold.clone(),
                ..default()
            },
            TextColor(Color::linear_rgba(1.15, 1.15, 1.2, 1.0)),
        ));
        parent.spawn((
            Text::new("Press SPACE to begin"),
            TextFont {
                font_size: 34.0,
                font: poppins_regular.clone(),
                ..default()
            },
            TextColor(Color::linear_rgba(1.15, 1.15, 1.2, 1.0)),
        ));
    });
    commands.spawn((
        Text::new("Depth: 0m"),
        TextFont {
            font_size: 36.0,
            font: poppins_regular.clone(),
            ..default()
        },
        TextColor(Color::linear_rgba(1.15, 1.15, 1.2, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(5.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        DepthHud,
        GameplayEntity,
        Visibility::Visible,
    ));
}

fn spawn_cave_segment(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    y: f32,
    gap_center_x: f32,
    teeth_in_upper_half: bool,
) {
    let half_screen = SCREEN_WIDTH * 0.5;
    let left_gap_edge = gap_center_x - GAP_WIDTH * 0.5;
    let right_gap_edge = gap_center_x + GAP_WIDTH * 0.5;
    let left_wall_width = left_gap_edge + half_screen;
    let right_wall_width = half_screen - right_gap_edge;
    let left_wall_x = (-half_screen + left_gap_edge) * 0.5;
    let right_wall_x = (right_gap_edge + half_screen) * 0.5;

    commands
        .spawn((
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
                MeshMaterial2d(materials.add(Color::srgb(0.08, 0.18, 0.22))),
                Transform::from_xyz(left_wall_x, 0.0, 0.0),
            ));
            parent.spawn((
                Mesh2d(meshes.add(Rectangle::new(right_wall_width, WALL_VISUAL_HEIGHT))),
                MeshMaterial2d(materials.add(Color::srgb(0.08, 0.18, 0.22))),
                Transform::from_xyz(right_wall_x, 0.0, 0.0),
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
                    MeshMaterial2d(materials.add(Color::srgb(0.08, 0.18, 0.22))),
                    Transform::from_xyz(left_gap_edge, offset_y, 0.0),
                ));
                parent.spawn((
                    Mesh2d(meshes.add(Triangle2d::new(
                        Vec2::new(0.0, -TOOTH_HEIGHT * 0.5),
                        Vec2::new(0.0, TOOTH_HEIGHT * 0.5),
                        Vec2::new(-TOOTH_DEPTH, 0.0),
                    ))),
                    MeshMaterial2d(materials.add(Color::srgb(0.08, 0.18, 0.22))),
                    Transform::from_xyz(right_gap_edge, offset_y, 0.0),
                ));
            }
        });
}

fn reset_and_spawn_cave(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cave_generation: &mut CaveGeneration,
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
            y,
            gap_center_x,
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
) {
    for mut visibility in &mut gameplay_query {
        *visibility = Visibility::Hidden;
    }
    if let Ok(mut menu_visibility) = menu_query.single_mut() {
        *menu_visibility = Visibility::Visible;
    }
    if let Ok(mut game_over_visibility) = game_over_query.single_mut() {
        *game_over_visibility = Visibility::Hidden;
    }
}

fn enter_playing(
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
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<RisingCircle>)>,
    mut bubble_query: Query<
        (&mut Transform, &mut HorizontalVelocity),
        (With<RisingCircle>, Without<Camera2d>),
    >,
    segment_query: Query<Entity, With<CaveSegment>>,
    particle_query: Query<Entity, With<PopParticle>>,
    mut depth_hud_query: Query<&mut Text, With<DepthHud>>,
) {
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
    );

    if let Ok((mut transform, mut velocity)) = bubble_query.single_mut() {
        transform.translation = BUBBLE_START;
        velocity.0 = 0.0;
    }
    if let Ok(mut depth_hud_text) = depth_hud_query.single_mut() {
        *depth_hud_text = Text::new("Depth: 0m");
    }

    depth_state.pixels_scrolled = 0.0;
    run_state.time_alive_secs = 0.0;
    screen_shake.trauma = 0.0;
    screen_shake.elapsed_secs = 0.0;
    if let Ok(mut camera_transform) = camera_query.single_mut() {
        camera_transform.translation.x = 0.0;
        camera_transform.translation.y = 0.0;
    }

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
            let base_color = Vec3::new(0.55 + fastrand::f32() * 0.1, 0.85 + fastrand::f32() * 0.1, 1.0);

            commands.spawn((
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

fn menu_input(keyboard: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<GameState>>) {
    if keyboard.just_pressed(KeyCode::Space) {
        next_state.set(GameState::Playing);
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
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        next_state.set(GameState::Playing);
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

fn update_screen_shake(
    time: Res<Time>,
    mut screen_shake: ResMut<ScreenShake>,
    mut camera_query: Query<&mut Transform, With<Camera2d>>,
) {
    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };

    if screen_shake.trauma <= 0.0 {
        camera_transform.translation.x = 0.0;
        camera_transform.translation.y = 0.0;
        return;
    }

    screen_shake.elapsed_secs += time.delta_secs();
    let decay = time.delta_secs() / SCREEN_SHAKE_DURATION;
    screen_shake.trauma = (screen_shake.trauma - decay).max(0.0);

    let shake_strength = screen_shake.trauma * screen_shake.trauma;
    let offset_x = (fastrand::f32() * 2.0 - 1.0) * SCREEN_SHAKE_MAX_OFFSET * shake_strength;
    let offset_y = (fastrand::f32() * 2.0 - 1.0) * SCREEN_SHAKE_MAX_OFFSET * shake_strength;
    camera_transform.translation.x = offset_x;
    camera_transform.translation.y = offset_y;

    if screen_shake.trauma <= 0.0 {
        camera_transform.translation.x = 0.0;
        camera_transform.translation.y = 0.0;
    }
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
    segment_query: Query<(Entity, &Transform), With<CaveSegment>>,
) {
    run_state.time_alive_secs += time.delta_secs();
    let current_scroll_speed =
        (BASE_SCROLL_SPEED + run_state.time_alive_secs * SCROLL_RAMP_RATE).min(MAX_SCROLL_SPEED);
    let scroll_delta = current_scroll_speed * time.delta_secs();
    depth_state.pixels_scrolled += scroll_delta;

    let mut top_y = f32::NEG_INFINITY;
    for (_, transform) in &segment_query {
        top_y = top_y.max(transform.translation.y);
    }

    for (entity, transform) in &segment_query {
        let new_y = transform.translation.y - scroll_delta;
        let below_screen = new_y + SEGMENT_HEIGHT * 0.5 < -SCREEN_HEIGHT * 0.5;

        if below_screen {
            commands.entity(entity).despawn();
            let spawn_y = top_y + SEGMENT_HEIGHT;
            let (next_gap_center_x, spawn_index) = next_segment_gap_center(cave_generation.as_mut());
            spawn_cave_segment(
                &mut commands,
                &mut meshes,
                &mut materials,
                spawn_y,
                next_gap_center_x,
                spawn_index % 2 == 0,
            );
            top_y = spawn_y;
        } else {
            commands
                .entity(entity)
                .insert(Transform::from_xyz(0.0, new_y, 0.0));
        }
    }
}

fn detect_wall_collision(
    mut next_state: ResMut<NextState<GameState>>,
    depth_state: Res<DepthState>,
    bubble_query: Query<&Transform, With<RisingCircle>>,
    mut bubble_visibility_query: Query<&mut Visibility, With<RisingCircle>>,
    segment_query: Query<(&Transform, &CaveSegment)>,
    mut game_over_depth_query: Query<&mut Text, With<GameOverText>>,
) {
    let Ok(bubble_transform) = bubble_query.single() else {
        return;
    };

    let bubble_x = bubble_transform.translation.x;
    let bubble_y = bubble_transform.translation.y;

    let segment_at_bubble = segment_query.iter().find(|(segment_transform, _)| {
        let half_height = SEGMENT_HEIGHT * 0.5;
        let min_y = segment_transform.translation.y - half_height;
        let max_y = segment_transform.translation.y + half_height;
        bubble_y >= min_y && bubble_y <= max_y
    });
    let Some((_, segment)) = segment_at_bubble else {
        return;
    };

    let left_inner_edge = segment.gap_center_x - GAP_WIDTH * 0.5;
    let right_inner_edge = segment.gap_center_x + GAP_WIDTH * 0.5;

    let hit_left_wall = bubble_x - BUBBLE_RADIUS <= left_inner_edge;
    let hit_right_wall = bubble_x + BUBBLE_RADIUS >= right_inner_edge;

    if hit_left_wall || hit_right_wall {
        if let Ok(mut bubble_visibility) = bubble_visibility_query.single_mut() {
            *bubble_visibility = Visibility::Hidden;
        }
        if let Ok(mut game_over_depth_text) = game_over_depth_query.single_mut() {
            let depth_meters = (depth_state.pixels_scrolled / PIXELS_PER_METER).floor() as i32;
            *game_over_depth_text = Text::new(format!("Depth: {}m", depth_meters));
        }
        next_state.set(GameState::GameOver);
    }
}

fn update_depth_ui(
    depth_state: Res<DepthState>,
    mut hud_query: Query<&mut Text, With<DepthHud>>,
) {
    if let Ok(mut hud_text) = hud_query.single_mut() {
        let depth_meters = (depth_state.pixels_scrolled / PIXELS_PER_METER).floor() as i32;
        *hud_text = Text::new(format!("Depth: {}m", depth_meters));
    }
}
