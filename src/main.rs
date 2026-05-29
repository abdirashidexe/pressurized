use bevy::prelude::*;

#[derive(Component)]
struct RisingCircle;

#[derive(Component, Default)]
struct HorizontalVelocity(f32);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Pressurized".into(),
                resolution: (900, 600).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, steer_circle)
        .add_systems(Update, rise_circle)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let screen_width = 900.0;
    let screen_height = 600.0;
    let gap_width = screen_width * 0.5;
    let wall_width = (screen_width - gap_width) * 0.5;
    let left_wall_x = -(gap_width * 0.5 + wall_width * 0.5);
    let right_wall_x = gap_width * 0.5 + wall_width * 0.5;

    commands.spawn(Camera2d);
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(wall_width, screen_height))),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
        Transform::from_xyz(left_wall_x, 0.0, 0.0),
    ));
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(wall_width, screen_height))),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
        Transform::from_xyz(right_wall_x, 0.0, 0.0),
    ));
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(30.0))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, -200.0, 0.0),
        RisingCircle,
        HorizontalVelocity::default(),
    ));
}

fn steer_circle(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &mut HorizontalVelocity), With<RisingCircle>>,
) {
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

fn rise_circle(time: Res<Time>, mut query: Query<&mut Transform, With<RisingCircle>>) {
    let speed = 40.0;
    for mut transform in &mut query {
        transform.translation.y += speed * time.delta_secs();
    }
}
