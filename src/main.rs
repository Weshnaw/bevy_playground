use bevy::{
    prelude::*,
    render::{
        RenderPlugin,
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
    },
};
use bevy_playground::*;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        features: WgpuFeatures::POLYGON_MODE_LINE,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(debug::DebugPlugin)
        .add_plugins(loading::LoadingPlugin)
        .add_plugins(camera::CameraPlugin)
        // .add_plugins(marching_cube::MarchingCubePlugin)
        .add_plugins(gen_voxels::GenerateVoxelsPlugin)
        .run();
}
