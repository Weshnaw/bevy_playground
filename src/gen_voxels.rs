use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSet,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::{RenderAssetUsages, RenderAssets},
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::{binding_types::texture_storage_2d, *},
        renderer::{RenderContext, RenderDevice},
        storage::{GpuShaderStorageBuffer, ShaderStorageBuffer},
        texture::GpuImage,
    },
};
use binding_types::uniform_buffer;

use crate::debug::DebugResource;

const SPHERE_ASSET_PATH: &str = "shaders/sphere.wgsl";
const SLICE_ASSET_PATH: &str = "shaders/slice.wgsl";
pub const CHUNK_SIZE: u32 = 100;

pub struct GenerateVoxelsPlugin;
impl Plugin for GenerateVoxelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((ExtractResourcePlugin::<Buffers>::default(),))
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

        render_app
            .world_mut()
            .resource_mut::<RenderGraph>()
            .add_node(ComputeNodeLabel, ComputeNode::default());
    }
}

fn update_layer(
    buffer: Res<Buffers>,
    debug: Res<DebugResource>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
) {
    if debug.is_changed() {
        let layer = buffers.get_mut(buffer.layer.id()).unwrap();
        layer.set_data(debug.value);
    }
}

#[derive(Resource, ExtractResource, Clone)]
struct Buffers {
    voxels: Handle<Image>,
    slice: Handle<Image>,
    layer: Handle<ShaderStorageBuffer>,
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
) {
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
    let mut layer = ShaderStorageBuffer::from(layer);
    // We need to enable the COPY_SRC usage so we can copy the buffer to the cpu
    layer.buffer_description.usage |= BufferUsages::UNIFORM;
    let layer = buffers.add(layer);

    commands.insert_resource(Buffers {
        voxels,
        slice,
        layer,
    });
}

#[derive(Resource)]
struct GpuBufferBindGroup {
    sphere: BindGroup,
    slice: BindGroup,
}

fn prepare_bind_groups(
    mut commands: Commands,
    pipeline: Res<ComputePipeline>,
    render_device: Res<RenderDevice>,
    buffer: Res<Buffers>,
    buffers: Res<RenderAssets<GpuShaderStorageBuffer>>,
    images: Res<RenderAssets<GpuImage>>,
) {
    let voxels = images.get(&buffer.voxels).unwrap();
    let slice = images.get(&buffer.slice).unwrap();
    let layer = buffers.get(&buffer.layer).unwrap();

    let sphere = render_device.create_bind_group(
        None,
        &pipeline.sphere_pipeline.layout,
        &BindGroupEntries::sequential((voxels.texture_view.into_binding(),)),
    );

    let slice = render_device.create_bind_group(
        None,
        &pipeline.slice_pipeline.layout,
        &BindGroupEntries::sequential((
            voxels.texture_view.into_binding(),
            slice.texture_view.into_binding(),
            layer.buffer.as_entire_buffer_binding(),
        )),
    );
    commands.insert_resource(GpuBufferBindGroup { sphere, slice });
}

fn update_bind_groups(
    mut bind_group: ResMut<GpuBufferBindGroup>,
    render_device: Res<RenderDevice>,
    pipeline: Res<ComputePipeline>,
    buffer: Res<Buffers>,
    buffers: Res<RenderAssets<GpuShaderStorageBuffer>>,
    images: Res<RenderAssets<GpuImage>>,
) {
    if buffers.is_changed() | images.is_changed() {
        info!("changed");
        let voxels = images.get(&buffer.voxels).unwrap();
        let slice = images.get(&buffer.slice).unwrap();
        let layer = buffers.get(&buffer.layer).unwrap();

        let slice = render_device.create_bind_group(
            None,
            &pipeline.slice_pipeline.layout,
            &BindGroupEntries::sequential((
                voxels.texture_view.into_binding(),
                slice.texture_view.into_binding(),
                layer.buffer.as_entire_buffer_binding(),
            )),
        );
        bind_group.slice = slice;
    }
}

#[derive(Resource)]
struct ComputePipeline {
    sphere_pipeline: PipelineGroup,
    slice_pipeline: PipelineGroup,
}

struct PipelineGroup {
    pipeline: CachedComputePipelineId,
    layout: BindGroupLayout,
}

pub fn texture_storage_3d(
    format: TextureFormat,
    access: StorageTextureAccess,
) -> BindGroupLayoutEntryBuilder {
    BindingType::StorageTexture {
        access,
        format,
        view_dimension: TextureViewDimension::D3,
    }
    .into_bind_group_layout_entry_builder()
}
impl FromWorld for ComputePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let shader = world.load_asset(SPHERE_ASSET_PATH);
        let layout = render_device.create_bind_group_layout(
            None,
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (texture_storage_3d(
                    TextureFormat::R32Float,
                    StorageTextureAccess::WriteOnly,
                ),),
            ),
        );
        let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("Sphere pipeline".into()),
            layout: vec![layout.clone()],
            push_constant_ranges: Vec::new(),
            shader: shader.clone(),
            shader_defs: Vec::new(),
            entry_point: "main".into(),
            zero_initialize_workgroup_memory: false,
        });

        let sphere_pipeline = PipelineGroup { pipeline, layout };

        let shader = world.load_asset(SLICE_ASSET_PATH);
        let layout = render_device.create_bind_group_layout(
            None,
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    texture_storage_3d(TextureFormat::R32Float, StorageTextureAccess::ReadOnly),
                    texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::WriteOnly),
                    uniform_buffer::<u32>(false),
                ),
            ),
        );
        let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("Slice pipeline".into()),
            layout: vec![layout.clone()],
            push_constant_ranges: Vec::new(),
            shader: shader.clone(),
            shader_defs: Vec::new(),
            entry_point: "main".into(),
            zero_initialize_workgroup_memory: false,
        });
        let slice_pipeline = PipelineGroup { pipeline, layout };

        ComputePipeline {
            sphere_pipeline,
            slice_pipeline,
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct ComputeNodeLabel;

#[derive(Default)]
pub(super) enum ShaderStage {
    #[default]
    Loading,
    Startup,
    Update,
}

#[derive(Default)]
struct ComputeNode {
    stage: ShaderStage,
}

impl render_graph::Node for ComputeNode {
    fn update(&mut self, world: &mut World) {
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ComputePipeline>();

        match self.stage {
            ShaderStage::Loading => {
                let state =
                    pipeline_cache.get_compute_pipeline_state(pipeline.sphere_pipeline.pipeline);

                match state {
                    CachedPipelineState::Ok(_) => self.stage = ShaderStage::Startup,
                    CachedPipelineState::Err(err) => panic!("Unable to load pipeline\n{}", err),
                    _ => {}
                }
            }
            ShaderStage::Startup => {
                let state =
                    pipeline_cache.get_compute_pipeline_state(pipeline.slice_pipeline.pipeline);

                if let CachedPipelineState::Ok(_) = state {
                    self.stage = ShaderStage::Update
                }
            }
            _ => {}
        }
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ComputePipeline>();
        let bind_group = world.resource::<GpuBufferBindGroup>();

        let mut pass =
            render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: None,
                    ..Default::default()
                });

        match self.stage {
            ShaderStage::Startup => {
                if let Some(init_pipeline) =
                    pipeline_cache.get_compute_pipeline(pipeline.sphere_pipeline.pipeline)
                {
                    pass.set_bind_group(0, &bind_group.sphere, &[]);
                    pass.set_pipeline(init_pipeline);
                    pass.dispatch_workgroups(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE);
                }
            }
            ShaderStage::Update => {
                if let Some(init_pipeline) =
                    pipeline_cache.get_compute_pipeline(pipeline.slice_pipeline.pipeline)
                {
                    pass.set_bind_group(0, &bind_group.slice, &[]);
                    pass.set_pipeline(init_pipeline);
                    pass.dispatch_workgroups(CHUNK_SIZE, CHUNK_SIZE, 1);
                }
            }
            _ => {}
        }

        Ok(())
    }
}
