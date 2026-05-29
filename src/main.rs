use bevy::prelude::*;

const SCREEN_WIDTH: f32 = 900.0;
const SCREEN_HEIGHT: f32 = 600.0;
const GAP_WIDTH: f32 = 250.0;
const BUBBLE_RADIUS: f32 = 30.0;
const BUBBLE_START: Vec3 = Vec3::new(0.0, -150.0, 0.0);
const SEGMENT_HEIGHT: f32 = 120.0;
const SCROLL_SPEED: f32 = 150.0;
const GAP_DRIFT_PER_SEGMENT: f32 = 70.0;
const GAP_MARGIN: f32 = 80.0;
const CENTERED_START_SEGMENTS: u32 = 4;
const PIXELS_PER_METER: f32 = 100.0;

#[derive(Component)]
struct RisingCircle;

#[derive(Component, Default)]
struct HorizontalVelocity(f32);

#[derive(Component)]
struct CaveSegment {
    gap_center_x: f32,
}

#[derive(Component)]
struct PopMessage;

#[derive(Component)]
struct DepthHud;

#[derive(Resource, Default)]
struct GameStatus {
    popped: bool,
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
        .init_resource::<GameStatus>()
        .init_resource::<CaveGeneration>()
        .init_resource::<DepthState>()
        .add_systems(Startup, setup)
        .add_systems(Update, steer_circle)
        .add_systems(Update, scroll_cave_segments)
        .add_systems(Update, detect_wall_collision)
        .add_systems(Update, update_depth_ui)
        .add_systems(Update, restart_game)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut cave_generation: ResMut<CaveGeneration>,
) {
    commands.spawn(Camera2d);
    reset_and_spawn_cave(
        &mut commands,
        &mut meshes,
        &mut materials,
        cave_generation.as_mut(),
    );
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(BUBBLE_RADIUS))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_translation(BUBBLE_START),
        RisingCircle,
        HorizontalVelocity::default(),
        Visibility::Visible,
    ));
    commands.spawn((
        Text::new("You Popped!\nDepth: 0m\nPress R to restart"),
        TextFont {
            font_size: 56.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(42.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        PopMessage,
        Visibility::Hidden,
    ));
    commands.spawn((
        Text::new("Depth: 0m"),
        TextFont {
            font_size: 36.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(5.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        DepthHud,
        Visibility::Visible,
    ));
}

fn spawn_cave_segment(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    y: f32,
    gap_center_x: f32,
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
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh2d(meshes.add(Rectangle::new(left_wall_width, SEGMENT_HEIGHT))),
                MeshMaterial2d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
                Transform::from_xyz(left_wall_x, 0.0, 0.0),
            ));
            parent.spawn((
                Mesh2d(meshes.add(Rectangle::new(right_wall_width, SEGMENT_HEIGHT))),
                MeshMaterial2d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
                Transform::from_xyz(right_wall_x, 0.0, 0.0),
            ));
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
        let gap_center_x = next_segment_gap_center(cave_generation);
        spawn_cave_segment(commands, meshes, materials, y, gap_center_x);
    }
}

fn next_segment_gap_center(cave_generation: &mut CaveGeneration) -> f32 {
    let gap_center_x = if cave_generation.spawned_segment_count < CENTERED_START_SEGMENTS {
        0.0
    } else {
        next_gap_center(cave_generation.last_gap_center_x)
    };
    cave_generation.last_gap_center_x = gap_center_x;
    cave_generation.spawned_segment_count += 1;
    gap_center_x
}

fn next_gap_center(previous_gap_center_x: f32) -> f32 {
    let delta = fastrand::f32() * (GAP_DRIFT_PER_SEGMENT * 2.0) - GAP_DRIFT_PER_SEGMENT;
    let unclamped = previous_gap_center_x + delta;
    let min_center = -SCREEN_WIDTH * 0.5 + GAP_MARGIN + GAP_WIDTH * 0.5;
    let max_center = SCREEN_WIDTH * 0.5 - GAP_MARGIN - GAP_WIDTH * 0.5;
    unclamped.clamp(min_center, max_center)
}

fn steer_circle(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    game_status: Res<GameStatus>,
    mut query: Query<(&mut Transform, &mut HorizontalVelocity), With<RisingCircle>>,
) {
    if game_status.popped {
        return;
    }

    let acceleration = 220.0;
    let damping = 0.92;

    for (mut transform, mut velocity) in &mut query {
        let mut direction = 0.0;
        if keyboard.pressed(KeyCode::ArrowLeft) {
            direction -= 1.0;
        }
        if keyboard.pressed(KeyCode::ArrowRight) {
            direction += 1.0;
        }

        velocity.0 += direction * acceleration * time.delta_secs();
        velocity.0 *= damping;
        transform.translation.x += velocity.0 * time.delta_secs();
    }
}

fn scroll_cave_segments(
    time: Res<Time>,
    game_status: Res<GameStatus>,
    mut depth_state: ResMut<DepthState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut cave_generation: ResMut<CaveGeneration>,
    segment_query: Query<(Entity, &Transform), With<CaveSegment>>,
) {
    if game_status.popped {
        return;
    }

    let scroll_delta = SCROLL_SPEED * time.delta_secs();
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
            let next_gap_center_x = next_segment_gap_center(cave_generation.as_mut());
            spawn_cave_segment(
                &mut commands,
                &mut meshes,
                &mut materials,
                spawn_y,
                next_gap_center_x,
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
    mut game_status: ResMut<GameStatus>,
    depth_state: Res<DepthState>,
    mut bubble_query: Query<(&Transform, &mut Visibility), With<RisingCircle>>,
    segment_query: Query<(&Transform, &CaveSegment)>,
    mut pop_text_query: Query<(&mut Text, &mut Visibility), (With<PopMessage>, Without<RisingCircle>)>,
) {
    if game_status.popped {
        return;
    }

    let Ok((bubble_transform, mut bubble_visibility)) = bubble_query.single_mut() else {
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
        game_status.popped = true;
        *bubble_visibility = Visibility::Hidden;
        if let Ok((mut pop_text, mut text_visibility)) = pop_text_query.single_mut() {
            let depth_meters = (depth_state.pixels_scrolled / PIXELS_PER_METER).floor() as i32;
            *pop_text = Text::new(format!(
                "You Popped!\nDepth: {}m\nPress R to restart",
                depth_meters
            ));
            *text_visibility = Visibility::Visible;
        }
    }
}

fn update_depth_ui(
    game_status: Res<GameStatus>,
    depth_state: Res<DepthState>,
    mut hud_query: Query<(&mut Text, &mut Visibility), With<DepthHud>>,
) {
    if let Ok((mut hud_text, mut hud_visibility)) = hud_query.single_mut() {
        let depth_meters = (depth_state.pixels_scrolled / PIXELS_PER_METER).floor() as i32;
        *hud_text = Text::new(format!("Depth: {}m", depth_meters));
        *hud_visibility = if game_status.popped {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

fn restart_game(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut game_status: ResMut<GameStatus>,
    mut depth_state: ResMut<DepthState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut cave_generation: ResMut<CaveGeneration>,
    mut bubble_query: Query<(&mut Transform, &mut HorizontalVelocity, &mut Visibility), With<RisingCircle>>,
    segment_query: Query<Entity, With<CaveSegment>>,
    mut pop_text_query: Query<(&mut Text, &mut Visibility), (With<PopMessage>, Without<RisingCircle>)>,
) {
    if !game_status.popped || !keyboard.just_pressed(KeyCode::KeyR) {
        return;
    }

    if let Ok((mut transform, mut velocity, mut visibility)) = bubble_query.single_mut() {
        transform.translation = BUBBLE_START;
        velocity.0 = 0.0;
        *visibility = Visibility::Visible;
    }
    if let Ok((mut pop_text, mut text_visibility)) = pop_text_query.single_mut() {
        *pop_text = Text::new("You Popped!\nDepth: 0m\nPress R to restart");
        *text_visibility = Visibility::Hidden;
    }

    for entity in &segment_query {
        commands.entity(entity).despawn();
    }
    reset_and_spawn_cave(
        &mut commands,
        &mut meshes,
        &mut materials,
        cave_generation.as_mut(),
    );

    depth_state.pixels_scrolled = 0.0;
    game_status.popped = false;
}
