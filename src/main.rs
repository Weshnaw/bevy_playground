use bevy::{prelude::*, render::{settings::{RenderCreation, WgpuFeatures, WgpuSettings}, RenderPlugin}};
use bevy_playground::{
    assets::AssetsPlugin, camera::CameraPlugin, debug::DebugPlugin, marching_cube::MarchingCubePlugin
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
        .add_plugins(AssetsPlugin)
        .add_plugins(CameraPlugin)
        // .add_plugins(TerrainPlugin)
        .add_plugins(MarchingCubePlugin)
        .run();
}
