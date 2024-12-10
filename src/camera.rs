use bevy::prelude::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};

use crate::ApplicationStates;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PanOrbitCameraPlugin);
        app.add_systems(OnEnter(ApplicationStates::LoadingComplete), setup_camera);
    }
}

pub fn setup_camera(mut commands: Commands) {
    info!("Setting up camera");
    // spawn a camera to be able to see anything
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0., 20., 75.).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
        PanOrbitCamera::default()
        // OrderIndependentTransparencySettings::default(),
        // Msaa::Off
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