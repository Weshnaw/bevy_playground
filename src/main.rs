use bevy::{
    math::vec3,
    prelude::*,
    render::{
        RenderPlugin,
        gpu_readback::ReadbackComplete,
        render_resource::Extent3d,
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
    },
};
use bevy_playground::*;
use shader::{
    exm::{ABuffer, Foo, HelloData, HelloEntries, HelloShaderPlugin},
    slib::{BufferReader, ImageBuilder, ImageData, ShaderBuilder, ShaderEntries, ShaderEntry},
};

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
        // .add_plugins(TerrainPlugin)
        // .add_plugins(marching_cube::MarchingCubePlugin)
        // .add_plugins(compute::ComputePlugin)
        .add_plugins(TestPlugin)
        // .add_plugins(readback::GpuReadbackPlugin)
        .run();
}

struct TestPlugin;

impl Plugin for TestPlugin {
    fn build(&self, app: &mut App) {
        let a: HelloShaderPlugin = ShaderBuilder::default()
            .initial_data(HelloData {
                a: vec![1, 2, 3],
                b: Foo { bar: 1, bazz: 2. },
                c: vec3(1., 2., 3.),
                d: ImageBuilder {
                    size: Extent3d {
                        width: 2,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    data: ImageData::Zeros,
                },
            })
            .dispatches(ShaderEntries {
                on_startup: vec![ShaderEntry {
                    entry: HelloEntries::Main,
                    workgroup: (3, 1, 1),
                }],
                on_update: vec![],
            })
            .build();

        app.add_plugins(a);

        app.add_systems(PostStartup, setup);
    }
}

fn setup(
    mut commands: bevy::prelude::Commands,
    a_buffer: Res<ABuffer>,
    // b_buffer: Res<BBuffer>,
    // mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    // d_buffer: Res<temp::DBuffer>,
) {
    // b_buffer.set_data(&mut buffers, temp::Foo { bar: 5, baz: 5. });

    commands
        .spawn(a_buffer.as_ref().readback())
        .observe(|t: Trigger<ReadbackComplete>| {
            let data: Vec<u32> = t.event().to_shader_type();
            info!(?data);
        });
    // d_buffer.spawn_readback(&mut commands, |t: Trigger<ReadbackComplete>| {
    //     let data: Vec<f32> = t.event().to_shader_type();
    //     info!(?data);
    // });
}
