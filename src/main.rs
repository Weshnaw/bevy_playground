use bevy::prelude::*;
use bevy_playground::{assets::AssetsPlugin, debug::DebugPlugin, setup_camera, MyStates};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(DebugPlugin)
        .add_plugins(AssetsPlugin)
        .add_systems(OnEnter(MyStates::Next), setup_camera)
        .run();
}
