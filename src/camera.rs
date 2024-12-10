use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};

use crate::ApplicationStates;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PanOrbitCameraPlugin);
        app.add_systems(OnEnter(ApplicationStates::LoadingComplete), setup_camera);
        app.add_systems(Update, (control_player, sync_camera_to_player).chain());
    }
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct PlayerCamera;

pub fn setup_camera(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    info!("Setting up camera");

    // spawn a camera to be able to see anything
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0., 20., -75.).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
        PanOrbitCamera::default(),
        PlayerCamera, // OrderIndependentTransparencySettings::default(),
                      // Msaa::Off
    ));

    commands.spawn((
        Player,
        Mesh3d(meshes.add(Cuboid::default())),
        Transform::from_xyz(0., 10., 0.),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(images.add(uv_debug_texture())),
            ..default()
        })),
    ));

    // commands.spawn((
    //     Name::new("Character"),
    //     SceneRoot(
    //         player.player.clone()
    //     )
    // ));

    // commands.spawn((
    //     Name::new("Level"),
    //     SceneRoot(
    //         levels.level_01.clone()
    //     )
    // ));
}

fn control_player(
    input: Res<ButtonInput<KeyCode>>,
    mut players: Query<&mut Transform, With<Player>>,
) {
    let mut direction = Vec2::new(0., 0.);

    if input.pressed(KeyCode::KeyW) {
        direction.y += 1.;
    }
    if input.pressed(KeyCode::KeyS) {
        direction.y -= 1.;
    }
    if input.pressed(KeyCode::KeyA) {
        direction.x += 1.;
    }
    if input.pressed(KeyCode::KeyD) {
        direction.x -= 1.;
    }

    for mut player in &mut players {
        player.translation.x += direction.x * 1.;
        player.translation.z += direction.y * 5.;
    }
}

fn sync_camera_to_player(
    players: Query<
        &Transform,
        With<Player>,
    >,
    mut camera: Query<&mut PanOrbitCamera, With<PlayerCamera>>,
) {
    let Ok(player) = players.get_single() else {
        warn!("Multiple players detected, failing to sync camera");
        return;
    };

    let mut orbit = camera.single_mut();

    orbit.target_focus = Vec3::new(
        player.translation.x,
        player.translation.y,
        player.translation.z,
    );
}

fn uv_debug_texture() -> Image {
    const TEXTURE_SIZE: usize = 8;

    let mut palette: [u8; 32] = [
        255, 102, 159, 255, 255, 159, 102, 255, 236, 255, 102, 255, 121, 255, 102, 255, 102, 255,
        198, 255, 102, 198, 255, 255, 121, 102, 255, 255, 236, 102, 255, 255,
    ];

    let mut texture_data = [0; TEXTURE_SIZE * TEXTURE_SIZE * 4];
    for y in 0..TEXTURE_SIZE {
        let offset = TEXTURE_SIZE * y * 4;
        texture_data[offset..(offset + TEXTURE_SIZE * 4)].copy_from_slice(&palette);
        palette.rotate_right(4);
    }

    Image::new_fill(
        Extent3d {
            width: TEXTURE_SIZE as u32,
            height: TEXTURE_SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texture_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}
