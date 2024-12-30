use bevy::{
    prelude::*,
    render::{
        RenderPlugin,
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
    },
};
use bevy_playground::{
    camera::CameraPlugin, debug::DebugPlugin, loading::LoadingPlugin,
    marching_cube::MarchingCubePlugin,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(RenderPlugin {
            render_creation: RenderCreation::Automatic(WgpuSettings {
                features: WgpuFeatures::POLYGON_MODE_LINE,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(DebugPlugin)
        .add_plugins(LoadingPlugin)
        .add_plugins(CameraPlugin)
        // .add_plugins(TerrainPlugin)
        .add_plugins(MarchingCubePlugin)
        .run();
}
