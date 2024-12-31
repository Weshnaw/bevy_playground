use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSet,
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::{binding_types::storage_buffer, *},
        renderer::{RenderContext, RenderDevice, RenderQueue},
    },
};
use crossbeam_channel::{Receiver, Sender};
use encase::internal::WriteInto;
use std::mem::size_of;
use zerocopy::FromBytes;

// TODO: maybe one day consider making a proc macro that transforms the Foo struct into the boilerplate

pub struct ComputePlugin;

impl Plugin for ComputePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GpuReadbackPlugin);
        app.add_systems(Update, receive);
    }
}

fn receive(receiver: Res<MainWorldReceiver>) {
    if let Ok(data) = receiver.try_recv() {
        info!("Received data from render world: {data:?}");
    }
}

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
