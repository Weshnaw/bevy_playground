use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        Render, RenderApp, RenderSet,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::{binding_types::storage_buffer, *},
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};
use binding_types::{texture_storage_2d, uniform_buffer};
use crossbeam_channel::{Receiver, Sender};
use encase::internal::WriteInto;
use std::{borrow::Cow, mem::size_of};
use zerocopy::FromBytes;

use crate::debug::DebugResource;

// TODO: maybe one day consider making a proc macro that transforms the Foo struct into the boilerplate

pub struct ComputePlugin;

impl Plugin for ComputePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GpuReadbackPlugin);
        app.add_plugins(GenerateTexturePlugin);
        app.add_systems(Update, receive);
    }
}

fn receive(receiver: Res<MainWorldReceiver>) {
    if let Ok(_data) = receiver.try_recv() {
        // info!("Received data from render world: {data:?}");
    }
}

// GPU compute shader buffer Example

struct GpuReadbackPlugin;
impl Plugin for GpuReadbackPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        let (s, r) = crossbeam_channel::unbounded();
        app.insert_resource(MainWorldReceiver(r));

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .insert_resource(RenderWorldSender(s))
            .init_resource::<ComputePipeline>()
            .init_resource::<FooBuffers>()
            .add_systems(
                Render,
                (
                    prepare_bind_group
                        .in_set(RenderSet::PrepareBindGroups)
                        .run_if(not(resource_exists::<GpuBufferBindGroup>)),
                    map_and_read_buffer.after(RenderSet::Render),
                ),
            );

        render_app
            .world_mut()
            .resource_mut::<RenderGraph>()
            .add_node(ComputeNodeLabel, ComputeNode);
    }
}

fn prepare_bind_group(
    mut commands: Commands,
    pipeline: Res<ComputePipeline>,
    render_device: Res<RenderDevice>,
    buffers: Res<FooBuffers>,
) {
    let bind_group = buffers.bind_group(&render_device, &pipeline);
    commands.insert_resource(GpuBufferBindGroup(bind_group));
}

fn map_and_read_buffer(
    render_device: Res<RenderDevice>,
    buffers: Res<FooBuffers>,
    sender: Res<RenderWorldSender>,
) {
    sender
        .send(buffers.read(&render_device))
        .expect("Failed to send buffer data to crossbeam");
}

#[derive(Debug)]
struct Foo {
    _data: Vec<u32>,
    _data1: Vec<u32>,
}

#[derive(Resource, Deref)]
struct MainWorldReceiver(Receiver<Foo>);

#[derive(Resource, Deref)]
struct RenderWorldSender(Sender<Foo>);

struct Buffers<T: ShaderType + WriteInto> {
    gpu: BufferVec<T>,
    cpu: Buffer,
}

impl<T> Buffers<T>
where
    T: ShaderType + WriteInto + Default + FromBytes,
{
    fn new(device: &RenderDevice, queue: &RenderQueue, length: usize) -> Self {
        let mut gpu = BufferVec::new(BufferUsages::STORAGE | BufferUsages::COPY_SRC);

        for _ in 0..length {
            gpu.push(T::default());
        }
        gpu.write_buffer(device, queue);

        let cpu = device.create_buffer(&BufferDescriptor {
            label: Some("readback_buffer"),
            size: (BUFFER_LEN * size_of::<T>()) as u64, // buffers with different data types???
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self { gpu, cpu }
    }

    fn binding(&self) -> BindingResource<'_> {
        self.gpu
            .binding()
            .expect("Buffer should have been uploaded to gpu")
    }

    fn read(&self, device: &RenderDevice) -> Vec<T> {
        let buffer_slice = self.cpu.slice(..);
        let (s, r) = crossbeam_channel::unbounded::<()>();
        buffer_slice.map_async(MapMode::Read, move |r| match r {
            // This will execute once the gpu is ready, so after the call to poll()
            Ok(_) => s.send(()).expect("Failed to send map update"),
            Err(err) => panic!("Failed to map buffer {err}"),
        });

        device.poll(Maintain::wait()).panic_on_timeout();
        r.recv().expect("Failed to receive the map_async message");

        let data = {
            let buffer_view = buffer_slice.get_mapped_range();
            let data = buffer_view
                .chunks(size_of::<T>())
                .map(|chunk| T::read_from_bytes(chunk).expect("Failed to read bytes"))
                .collect::<Vec<T>>();
            data
        };

        self.cpu.unmap();

        data
    }

    fn copy_buffer(&self, context: &mut RenderContext) {
        context.command_encoder().copy_buffer_to_buffer(
            self.gpu
                .buffer()
                .expect("Buffer should have already been uploaded to the gpu"),
            0,
            &self.cpu,
            0,
            (BUFFER_LEN * size_of::<T>()) as u64,
        );
    }
}

const SHADER_ASSET_PATH: &str = "shaders/compute.wgsl";
const BUFFER_LEN: usize = 16;
#[derive(Resource)]
struct FooBuffers {
    data: Buffers<u32>,
    data1: Buffers<u32>,
}

impl FooBuffers {
    fn bind_group(&self, device: &RenderDevice, pipeline: &ComputePipeline) -> BindGroup {
        device.create_bind_group(
            None,
            &pipeline.layout,
            &BindGroupEntries::sequential((self.data.binding(), self.data1.binding())), // TODO how can I automagically create these? macro?
        )
    }

    fn layout(device: &RenderDevice) -> BindGroupLayout {
        device.create_bind_group_layout(
            None,
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    storage_buffer::<Vec<u32>>(false),
                    storage_buffer::<Vec<u32>>(false),
                ),
            ),
        )
    }

    fn pipeline(
        pipeline_cache: &PipelineCache,
        layout: BindGroupLayout,
        shader: Handle<Shader>,
    ) -> CachedComputePipelineId {
        // considering making the label and entry_point parameters, but as the whole idea is
        // the 'ComputeBuffers' is suposed to be effectively a 1-1 of the wgsl it may be unneeded
        pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("GPU readback compute shader".into()),
            layout: vec![layout],
            push_constant_ranges: Vec::new(),
            shader,
            shader_defs: Vec::new(),
            entry_point: "main".into(),
            zero_initialize_workgroup_memory: false,
        })
    }

    fn read(&self, device: &RenderDevice) -> Foo {
        let data = self.data.read(device);
        let data1 = self.data1.read(device);

        Foo {
            _data: data,
            _data1: data1,
        }
    }

    fn copy_buffers(&self, context: &mut RenderContext) {
        self.data.copy_buffer(context);
        self.data1.copy_buffer(context);
    }
}

impl FromWorld for FooBuffers {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        let queue = world.resource::<RenderQueue>();

        Self {
            data: Buffers::new(device, queue, BUFFER_LEN),
            data1: Buffers::new(device, queue, BUFFER_LEN),
        }
    }
}

#[derive(Resource)]
struct GpuBufferBindGroup(BindGroup);

#[derive(Resource)]
struct ComputePipeline {
    layout: BindGroupLayout,
    pipeline: CachedComputePipelineId,
}

impl FromWorld for ComputePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let layout = FooBuffers::layout(render_device);
        let shader = world.load_asset(SHADER_ASSET_PATH);
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = FooBuffers::pipeline(pipeline_cache, layout.clone(), shader);

        ComputePipeline { layout, pipeline }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct ComputeNodeLabel;

#[derive(Default)]
struct ComputeNode;

fn dispatch(
    pipeline_cache: &PipelineCache,
    pipeline: &ComputePipeline,
    context: &mut RenderContext,
    bind_group: &GpuBufferBindGroup,
) {
    if let Some(init_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.pipeline) {
        let mut pass = context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("GPU readback compute pass"),
                ..default()
            });

        pass.set_bind_group(0, &bind_group.0, &[]);
        pass.set_pipeline(init_pipeline);
        pass.dispatch_workgroups(BUFFER_LEN as u32, 1, 1);
    }
}
impl render_graph::Node for ComputeNode {
    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ComputePipeline>();
        let bind_group = world.resource::<GpuBufferBindGroup>();

        dispatch(pipeline_cache, pipeline, render_context, bind_group);

        let buffers = world.resource::<FooBuffers>();
        buffers.copy_buffers(render_context);

        Ok(())
    }
}

// GPU texture compute shader example
struct GenerateTexturePlugin;
impl Plugin for GenerateTexturePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SphereComputePlugin);
        app.add_systems(Startup, prepare_3d_texture);
        // app.add_systems(Update, debug_image);
    }
}

fn prepare_3d_texture(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new(
        Extent3d {
            width: CHUNK_SIZE,
            height: CHUNK_SIZE,
            depth_or_array_layers: CHUNK_SIZE,
        },
        TextureDimension::D3,
        vec![0; (CHUNK_SIZE as usize).pow(3) * 4],
        TextureFormat::R32Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;

    let voxels = images.add(image);

    // Debug the texture:
    // TODO: move out and make toggleable
    let mut image_slice = Image::new(
        Extent3d {
            width: CHUNK_SIZE,
            height: CHUNK_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; (CHUNK_SIZE as usize).pow(2) * 4],
        TextureFormat::R32Float,
        RenderAssetUsages::RENDER_WORLD,
    );

    image_slice.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    let slice = images.add(image_slice);

    commands
        .spawn((
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            DebugVoxelImage,
        ))
        .with_children(|parent| {
            parent.spawn((ImageNode::new(slice.clone()), Node {
                width: Val::Px(500.),
                ..default()
            }));
        });

    commands.insert_resource(Voxels {
        image: voxels,
        debug_slice: slice,
    });
}

pub const CHUNK_SIZE: u32 = 32;
#[derive(Component)]
struct DebugVoxelImage;

struct SphereComputePlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct SphereComputeLabel;

impl Plugin for SphereComputePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractResourcePlugin::<Voxels>::default());
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<SpherePipeline>();
        render_app.init_resource::<LayerBuffer>();
        render_app.init_resource::<AtomicBuffer>();

        render_app.add_systems(
            Render,
            (
                prepare_bind_group_sphere.in_set(RenderSet::PrepareBindGroups),
                read_atomic_buffer.after(RenderSet::Render),
            ),
        );

        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();

        render_graph.add_node(SphereComputeLabel, SphereComputeNode::default());
        render_graph.add_node_edge(SphereComputeLabel, bevy::render::graph::CameraDriverLabel);
    }
}

#[derive(Resource, Clone, ExtractResource)]
struct Voxels {
    image: Handle<Image>, // image is just a convient way to get a 3d matrix
    debug_slice: Handle<Image>,
}

#[derive(Resource)]
struct SphereBindGroups(BindGroup);

fn prepare_bind_group_sphere(
    mut commands: Commands,
    pipeline: Res<SpherePipeline>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    game_of_life_images: Res<Voxels>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    debug_res: Res<DebugResource>,
    mut layer_buffer: ResMut<LayerBuffer>,
    atomic_buffer: Res<AtomicBuffer>,
) {
    let voxels = gpu_images.get(&game_of_life_images.image).unwrap();
    let debug_slice = gpu_images.get(&game_of_life_images.debug_slice).unwrap();
    let uniform_buffer: &mut UniformBuffer<u32> = &mut layer_buffer.0;
    uniform_buffer.set(debug_res.value);
    uniform_buffer.write_buffer(&render_device, &render_queue);
    let bind_group = render_device.create_bind_group(
        None,
        &pipeline.layout,
        &BindGroupEntries::sequential((
            &voxels.texture_view,
            &debug_slice.texture_view,
            uniform_buffer
                .binding()
                .expect("Uniform buffer failed to bind"),
            atomic_buffer
                .0
                .binding()
                .expect("Atomic buffer failed to bind"),
        )),
    );
    commands.insert_resource(SphereBindGroups(bind_group));
}

#[derive(Resource)]
struct SpherePipeline {
    layout: BindGroupLayout,
    init: CachedComputePipelineId,
    update: CachedComputePipelineId,
}

impl FromWorld for SpherePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "Sphere",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    texture_storage_3d(TextureFormat::R32Float, StorageTextureAccess::ReadWrite),
                    texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::WriteOnly),
                    uniform_buffer::<u32>(false),
                    storage_buffer::<u32>(false),
                ),
            ),
        );
        let shader = world.load_asset(SPHERE_ASSET_PATH);
        let pipeline_cache = world.resource::<PipelineCache>();

        let init = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: None,
            layout: vec![layout.clone()],
            push_constant_ranges: Vec::new(),
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Cow::from("main"),
            zero_initialize_workgroup_memory: false,
        });
        let update = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: None,
            layout: vec![layout.clone()],
            push_constant_ranges: Vec::new(),
            shader,
            shader_defs: vec![],
            entry_point: Cow::from("get_slice"),
            zero_initialize_workgroup_memory: false,
        });

        SpherePipeline {
            layout,
            init,
            update,
        }
    }
}

#[derive(Default)]
enum SphereComputeState {
    #[default]
    Loading,
    Init,
    Update,
}

#[derive(Default)]
struct SphereComputeNode {
    state: SphereComputeState,
}

// const WORKGROUP_SIZE: u32 = CHUNK_SIZE / 64;
impl render_graph::Node for SphereComputeNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<SpherePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        match self.state {
            SphereComputeState::Loading => {
                match pipeline_cache.get_compute_pipeline_state(pipeline.init) {
                    CachedPipelineState::Ok(_) => self.state = SphereComputeState::Init,
                    CachedPipelineState::Err(err) => {
                        panic!("Initializing assets/{SPHERE_ASSET_PATH}\n{err}")
                    }
                    _ => {}
                }
            }
            SphereComputeState::Init => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.update)
                {
                    self.state = SphereComputeState::Update
                }
            }
            _ => {}
        }
    }

    fn run<'w>(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), render_graph::NodeRunError> {
        let bind_groups = &world.resource::<SphereBindGroups>().0;
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<SpherePipeline>();

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor::default());

        match self.state {
            SphereComputeState::Loading => {}
            SphereComputeState::Init => {
                let init_pipeline = pipeline_cache.get_compute_pipeline(pipeline.init).unwrap();
                pass.set_bind_group(0, bind_groups, &[]);
                pass.set_pipeline(init_pipeline);
                pass.dispatch_workgroups(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE);
            }
            SphereComputeState::Update => {
                let update_pipeline = pipeline_cache
                    .get_compute_pipeline(pipeline.update)
                    .unwrap();
                pass.set_bind_group(0, bind_groups, &[]);
                pass.set_pipeline(update_pipeline);
                pass.dispatch_workgroups(CHUNK_SIZE, CHUNK_SIZE, 1);
                let buffers = world.resource::<AtomicBuffer>();
                buffers.copy_buffer(render_context);
            }
        }

        Ok(())
    }
}

const SPHERE_ASSET_PATH: &str = "shaders/sphere.wgsl";
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

#[derive(Resource, Default)]
struct LayerBuffer(UniformBuffer<u32>);

#[derive(Resource)]
struct AtomicBuffer(StorageBuffer<u32>, Buffer);

impl FromWorld for AtomicBuffer {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        let queue = world.resource::<RenderQueue>();

        let mut gpu = StorageBuffer::default();

        gpu.write_buffer(device, queue);

        let cpu = device.create_buffer(&BufferDescriptor {
            label: Some("atomic readback_buffer"),
            size: (BUFFER_LEN * size_of::<u32>()) as u64, // buffers with different data types???
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self(gpu, cpu)
    }
}

impl AtomicBuffer {
    fn read(&self, device: &RenderDevice) -> u32 {
        let buffer_slice = self.1.slice(..);
        let (s, r) = crossbeam_channel::unbounded::<()>();
        buffer_slice.map_async(MapMode::Read, move |r| match r {
            // This will execute once the gpu is ready, so after the call to poll()
            Ok(_) => s.send(()).expect("Failed to send map update"),
            Err(err) => panic!("Failed to map buffer {err}"),
        });

        device.poll(Maintain::wait()).panic_on_timeout();
        r.recv().expect("Failed to receive the map_async message");

        let data = {
            let buffer_view = buffer_slice.get_mapped_range();
            let data = u32::read_from_bytes(&buffer_view[..size_of::<u32>()])
                .expect("Failed to read bytes");
            data
        };

        self.1.unmap();

        data
    }

    fn _reset(&mut self, device: &RenderDevice, queue: &RenderQueue) {
        self.0.set(0);
        self.0.write_buffer(device, queue);
    }

    fn copy_buffer(&self, context: &mut RenderContext) {
        context.command_encoder().copy_buffer_to_buffer(
            self.0
                .buffer()
                .expect("Buffer should have already been uploaded to the gpu"),
            0,
            &self.1,
            0,
            (size_of::<u32>()) as u64,
        );
    }
}

fn read_atomic_buffer(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut buffers: ResMut<AtomicBuffer>,
) {
    buffers._reset(&render_device, &render_queue);
    let num = buffers.read(&render_device);
    info!(?num);
}
