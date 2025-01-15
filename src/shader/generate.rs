use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSet,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::{RenderAssetUsages, RenderAssets},
        render_resource::{
            AsBindGroup, BindGroup, ComputePipelineDescriptor, Extent3d, PipelineCache,
            TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::RenderDevice,
        storage::GpuShaderStorageBuffer,
        texture::{FallbackImage, GpuImage},
    },
};

use crate::debug::DebugResource;

use super::{CHUNK_SIZE, render::PipelineGroup};

const SPHERE_ASSET_PATH: &str = "shaders/sphere.wgsl";
const SLICE_ASSET_PATH: &str = "shaders/slice.wgsl";

pub struct GenerateVoxelsPlugin;
impl Plugin for GenerateVoxelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractResourcePlugin::<SphereBuffers>::default(),
            ExtractResourcePlugin::<SliceBuffers>::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, update_layer);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<ComputePipeline>().add_systems(
            Render,
            (
                prepare_bind_groups
                    .in_set(RenderSet::PrepareBindGroups)
                    .run_if(not(resource_exists::<GpuBufferBindGroup>)),
                update_bind_groups
                    .in_set(RenderSet::PrepareBindGroups)
                    .run_if(resource_exists::<GpuBufferBindGroup>),
            ),
        );
    }
}

fn update_layer(mut buffer: ResMut<SliceBuffers>, debug: Res<DebugResource>) {
    if debug.is_changed() {
        buffer.layer = debug.value;
    }
}

#[derive(Resource, ExtractResource, Clone, AsBindGroup)]
pub struct SphereBuffers {
    #[storage_texture(0, dimension="3d", image_format=R32Float, access=WriteOnly)]
    pub voxels: Handle<Image>,
}

#[derive(Resource, ExtractResource, Clone, AsBindGroup)]
struct SliceBuffers {
    #[storage_texture(0, dimension="3d", image_format=R32Float, access=ReadOnly)]
    voxels: Handle<Image>,
    #[storage_texture(1, dimension="2d", image_format=R32Float, access=WriteOnly)]
    slice: Handle<Image>,
    #[uniform(2)]
    layer: u32,
}

pub fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut size = Extent3d {
        width: CHUNK_SIZE,
        height: CHUNK_SIZE,
        depth_or_array_layers: 1,
    };

    let format = TextureFormat::R32Float;
    let d2_image_data_size = CHUNK_SIZE as usize
        * CHUNK_SIZE as usize
        * format.block_copy_size(None).unwrap_or(0) as usize;
    let mut slice = Image::new(
        size,
        TextureDimension::D2,
        vec![0; d2_image_data_size],
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    slice.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC;
    let slice = images.add(slice);

    commands
        .spawn((Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn((ImageNode::new(slice.clone()), Node {
                width: Val::Px(500.),
                ..default()
            }));
        });

    size.depth_or_array_layers = CHUNK_SIZE;
    let mut voxels = Image::new(
        size,
        TextureDimension::D3,
        vec![0; d2_image_data_size * CHUNK_SIZE as usize],
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    voxels.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;
    let voxels = images.add(voxels);

    let layer = 0u32;

    commands.insert_resource(SphereBuffers {
        voxels: voxels.clone(),
    });
    commands.insert_resource(SliceBuffers {
        voxels,
        slice,
        layer,
    });
}

#[derive(Resource)]
pub struct GpuBufferBindGroup {
    pub sphere: BindGroup,
    pub slice: BindGroup,
}

#[allow(clippy::too_many_arguments)]
fn prepare_bind_groups(
    mut commands: Commands,
    pipeline: Res<ComputePipeline>,
    render_device: Res<RenderDevice>,
    sphere_buffers: Res<SphereBuffers>,
    slice_buffers: Res<SliceBuffers>,
    buffers: Res<RenderAssets<GpuShaderStorageBuffer>>,
    fallback: Res<FallbackImage>,
    images: Res<RenderAssets<GpuImage>>,
) {
    let mut param = (images, fallback, buffers);
    let slice = slice_buffers
        .as_bind_group(&pipeline.slice.layout, &render_device, &mut param)
        .unwrap()
        .bind_group;
    let sphere = sphere_buffers
        .as_bind_group(&pipeline.sphere.layout, &render_device, &mut param)
        .unwrap()
        .bind_group;
    commands.insert_resource(GpuBufferBindGroup { sphere, slice });
}

fn update_bind_groups(
    mut bind_group: ResMut<GpuBufferBindGroup>,
    render_device: Res<RenderDevice>,
    pipeline: Res<ComputePipeline>,
    slice_buffers: Res<SliceBuffers>,
    buffers: Res<RenderAssets<GpuShaderStorageBuffer>>,
    fallback: Res<FallbackImage>,
    images: Res<RenderAssets<GpuImage>>,
) {
    if slice_buffers.is_changed() {
        let mut param = (images, fallback, buffers);
        let slice = slice_buffers
            .as_bind_group(&pipeline.slice.layout, &render_device, &mut param)
            .unwrap()
            .bind_group;
        bind_group.slice = slice;
    }
}

#[derive(Resource)]
pub struct ComputePipeline {
    pub sphere: PipelineGroup,
    pub slice: PipelineGroup,
}

impl FromWorld for ComputePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let sphere_shader = world.load_asset(SPHERE_ASSET_PATH);
        let sphere_layout = SphereBuffers::bind_group_layout(&render_device);
        let sphere_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("Sphere pipeline".into()),
            layout: vec![sphere_layout.clone()],
            push_constant_ranges: Vec::new(),
            shader: sphere_shader.clone(),
            shader_defs: Vec::new(),
            entry_point: "main".into(),
            zero_initialize_workgroup_memory: false,
        });
        let sphere = PipelineGroup {
            pipeline: sphere_pipeline,
            layout: sphere_layout,
        };

        let slice_shader = world.load_asset(SLICE_ASSET_PATH);
        let slice_layout = SliceBuffers::bind_group_layout(&render_device);
        let slice_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("Slice pipeline".into()),
            layout: vec![slice_layout.clone()],
            push_constant_ranges: Vec::new(),
            shader: slice_shader.clone(),
            shader_defs: Vec::new(),
            entry_point: "main".into(),
            zero_initialize_workgroup_memory: false,
        });
        let slice = PipelineGroup {
            pipeline: slice_pipeline,
            layout: slice_layout,
        };

        ComputePipeline { sphere, slice }
    }
}
