use assets::{LevelAssets, PlayerAssets};
use bevy::prelude::*;

pub mod debug;
pub mod assets; 

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum MyStates {
    #[default]
    AssetLoading,
    Next,
}

pub fn setup_camera(mut commands: Commands, levels: Res<LevelAssets>, player: Res<PlayerAssets>) {
    info!("Setting up camera");
    // spawn a camera to be able to see anything
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(10., 7., 10.).looking_at(Vec3::new(0., 2., 0.), Vec3::Y),
        // OrderIndependentTransparencySettings::default(),
        // Msaa::Off
    ));

    commands.spawn((
        Name::new("Character"),
        SceneRoot(
            player.player.clone()
        )
    ));

    commands.spawn((
        Name::new("Level"),
        SceneRoot(
            levels.level_01.clone()
        )
    ));
}
