use bevy::prelude::*;

const SCREEN_WIDTH: f32 = 900.0;
const SCREEN_HEIGHT: f32 = 600.0;
const GAP_WIDTH: f32 = SCREEN_WIDTH * 0.5;
const BUBBLE_RADIUS: f32 = 30.0;
const BUBBLE_START: Vec3 = Vec3::new(0.0, -200.0, 0.0);

#[derive(Component)]
struct RisingCircle;

#[derive(Component, Default)]
struct HorizontalVelocity(f32);

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
        .add_systems(Update, rise_circle)
        .add_systems(Update, detect_wall_collision)
        .add_systems(Update, restart_game)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let wall_width = (SCREEN_WIDTH - GAP_WIDTH) * 0.5;
    let left_wall_x = -(GAP_WIDTH * 0.5 + wall_width * 0.5);
    let right_wall_x = GAP_WIDTH * 0.5 + wall_width * 0.5;

    commands.spawn(Camera2d);
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(wall_width, SCREEN_HEIGHT))),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
        Transform::from_xyz(left_wall_x, 0.0, 0.0),
    ));
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(wall_width, SCREEN_HEIGHT))),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
        Transform::from_xyz(right_wall_x, 0.0, 0.0),
    ));
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

fn rise_circle(
    time: Res<Time>,
    game_status: Res<GameStatus>,
    mut query: Query<&mut Transform, With<RisingCircle>>,
) {
    if game_status.popped {
        return;
    }

    let speed = 40.0;
    for mut transform in &mut query {
        transform.translation.y += speed * time.delta_secs();
    }
}

fn detect_wall_collision(
    mut game_status: ResMut<GameStatus>,
    mut bubble_query: Query<(&Transform, &mut Visibility), With<RisingCircle>>,
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
