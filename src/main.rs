use bevy::prelude::*;

#[derive(Component)]
struct RisingCircle;

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
        .add_systems(Update, rise_circle)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(30.0))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, -200.0, 0.0),
        RisingCircle,
    ));
}

fn rise_circle(time: Res<Time>, mut query: Query<&mut Transform, With<RisingCircle>>) {
    let speed = 40.0;
    for mut transform in &mut query {
        transform.translation.y += speed * time.delta_secs();
    }
}
