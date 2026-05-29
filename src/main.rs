use bevy::prelude::*;

const SCREEN_WIDTH: f32 = 900.0;
const SCREEN_HEIGHT: f32 = 600.0;
const GAP_WIDTH: f32 = SCREEN_WIDTH * 0.5;
const BUBBLE_RADIUS: f32 = 30.0;
const BUBBLE_START: Vec3 = Vec3::new(0.0, -150.0, 0.0);
const SEGMENT_HEIGHT: f32 = 120.0;
const SCROLL_SPEED: f32 = 150.0;

#[derive(Component)]
struct RisingCircle;

#[derive(Component, Default)]
struct HorizontalVelocity(f32);

#[derive(Component)]
struct CaveSegment;

#[derive(Component)]
struct PopMessage;

#[derive(Resource, Default)]
struct GameStatus {
    popped: bool,
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
        .add_systems(Startup, setup)
        .add_systems(Update, steer_circle)
        .add_systems(Update, scroll_cave_segments)
        .add_systems(Update, detect_wall_collision)
        .add_systems(Update, restart_game)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let segment_count = (SCREEN_HEIGHT / SEGMENT_HEIGHT).ceil() as i32 + 1;
    let first_segment_y = -SCREEN_HEIGHT * 0.5 + SEGMENT_HEIGHT * 0.5;

    commands.spawn(Camera2d);
    for i in 0..segment_count {
        let y = first_segment_y + i as f32 * SEGMENT_HEIGHT;
        spawn_cave_segment(&mut commands, &mut meshes, &mut materials, y);
    }
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(BUBBLE_RADIUS))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_translation(BUBBLE_START),
        RisingCircle,
        HorizontalVelocity::default(),
        Visibility::Visible,
    ));
    commands.spawn((
        Text::new("You Popped!"),
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
}

fn spawn_cave_segment(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    y: f32,
) {
    let wall_width = (SCREEN_WIDTH - GAP_WIDTH) * 0.5;
    let left_wall_x = -(GAP_WIDTH * 0.5 + wall_width * 0.5);
    let right_wall_x = GAP_WIDTH * 0.5 + wall_width * 0.5;

    commands
        .spawn((
            CaveSegment,
            Transform::from_xyz(0.0, y, 0.0),
            GlobalTransform::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh2d(meshes.add(Rectangle::new(wall_width, SEGMENT_HEIGHT))),
                MeshMaterial2d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
                Transform::from_xyz(left_wall_x, 0.0, 0.0),
            ));
            parent.spawn((
                Mesh2d(meshes.add(Rectangle::new(wall_width, SEGMENT_HEIGHT))),
                MeshMaterial2d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
                Transform::from_xyz(right_wall_x, 0.0, 0.0),
            ));
        });
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
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    segment_query: Query<(Entity, &Transform), With<CaveSegment>>,
) {
    if game_status.popped {
        return;
    }

    let mut top_y = f32::NEG_INFINITY;
    for (_, transform) in &segment_query {
        top_y = top_y.max(transform.translation.y);
    }

    for (entity, transform) in &segment_query {
        let new_y = transform.translation.y - SCROLL_SPEED * time.delta_secs();
        let below_screen = new_y + SEGMENT_HEIGHT * 0.5 < -SCREEN_HEIGHT * 0.5;

        if below_screen {
            commands.entity(entity).despawn();
            let spawn_y = top_y + SEGMENT_HEIGHT;
            spawn_cave_segment(&mut commands, &mut meshes, &mut materials, spawn_y);
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
    mut bubble_query: Query<(&Transform, &mut Visibility), With<RisingCircle>>,
    segment_query: Query<&Transform, With<CaveSegment>>,
    mut pop_text_query: Query<&mut Visibility, (With<PopMessage>, Without<RisingCircle>)>,
) {
    if game_status.popped {
        return;
    }

    let left_inner_edge = -GAP_WIDTH * 0.5;
    let right_inner_edge = GAP_WIDTH * 0.5;

    let Ok((bubble_transform, mut bubble_visibility)) = bubble_query.single_mut() else {
        return;
    };

    let bubble_x = bubble_transform.translation.x;
    let bubble_y = bubble_transform.translation.y;

    let has_segment_at_bubble_height = segment_query.iter().any(|segment_transform| {
        let half_height = SEGMENT_HEIGHT * 0.5;
        let min_y = segment_transform.translation.y - half_height;
        let max_y = segment_transform.translation.y + half_height;
        bubble_y >= min_y && bubble_y <= max_y
    });
    if !has_segment_at_bubble_height {
        return;
    }

    let hit_left_wall = bubble_x - BUBBLE_RADIUS <= left_inner_edge;
    let hit_right_wall = bubble_x + BUBBLE_RADIUS >= right_inner_edge;

    if hit_left_wall || hit_right_wall {
        game_status.popped = true;
        *bubble_visibility = Visibility::Hidden;
        if let Ok(mut text_visibility) = pop_text_query.single_mut() {
            *text_visibility = Visibility::Visible;
        }
    }
}

fn restart_game(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut game_status: ResMut<GameStatus>,
    mut bubble_query: Query<(&mut Transform, &mut HorizontalVelocity, &mut Visibility), With<RisingCircle>>,
    mut pop_text_query: Query<&mut Visibility, (With<PopMessage>, Without<RisingCircle>)>,
) {
    if !game_status.popped || !keyboard.just_pressed(KeyCode::KeyR) {
        return;
    }

    if let Ok((mut transform, mut velocity, mut visibility)) = bubble_query.single_mut() {
        transform.translation = BUBBLE_START;
        velocity.0 = 0.0;
        *visibility = Visibility::Visible;
    }
    if let Ok(mut text_visibility) = pop_text_query.single_mut() {
        *text_visibility = Visibility::Hidden;
    }
    game_status.popped = false;
}
