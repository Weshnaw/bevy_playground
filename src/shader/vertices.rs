use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSet,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_resource::*,
        renderer::RenderDevice,
        storage::{GpuShaderStorageBuffer, ShaderStorageBuffer},
        texture::{FallbackImage, GpuImage},
    },
};

use super::{CHUNK_SIZE, generate, render::PipelineGroup};

const SHADER_PATH: &str = "shaders/voxels.wgsl";

pub struct GenerateVerticesPlugin;
impl Plugin for GenerateVerticesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((ExtractResourcePlugin::<VertexBuffers>::default(),))
            .add_systems(Startup, setup.after(generate::setup));
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<ComputePipeline>().add_systems(
            Render,
            (prepare_bind_groups
                .in_set(RenderSet::PrepareBindGroups)
                .run_if(not(resource_exists::<GpuBufferBindGroup>)),),
        );
    }
}

#[derive(Resource, ExtractResource, Clone, AsBindGroup)]
struct VertexBuffers {
    #[storage_texture(0, dimension="3d", image_format=R32Float, access=ReadOnly)]
    voxels: Handle<Image>,
    #[storage(1, visibility(compute))]
    data: Handle<ShaderStorageBuffer>,
    #[storage(2, visibility(compute))]
    vertices: Handle<ShaderStorageBuffer>,
    #[storage(3, visibility(compute))]
    indices: Handle<ShaderStorageBuffer>,
    #[storage(4, visibility(compute))]
    normals: Handle<ShaderStorageBuffer>,
    #[storage(5, visibility(compute))]
    uvs: Handle<ShaderStorageBuffer>,
}

const CHUNK_SIZE3: usize = CHUNK_SIZE as usize * CHUNK_SIZE as usize * CHUNK_SIZE as usize * 3 * 8;
#[derive(ShaderType, Debug, Default)]
struct VertexData {
    index_head: u32,
    vertex_head: u32,
}

#[derive(ShaderType, Debug, Default, Clone)]
struct PaddedVec3 {
    #[size(16)]
    vec: Vec3,
}

fn setup(
    mut commands: Commands,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    gen_buffers: Res<generate::SphereBuffers>,
) {
    let data = VertexData::default();
    let mut data = ShaderStorageBuffer::from(data);
    data.buffer_description.usage |= BufferUsages::COPY_SRC;
    let data = buffers.add(data);
    let vertices: Vec<PaddedVec3> = vec![PaddedVec3::default(); CHUNK_SIZE3];
    let mut vertices = ShaderStorageBuffer::from(vertices);
    vertices.buffer_description.usage |= BufferUsages::COPY_SRC;
    let vertices = buffers.add(vertices);
    let indices: Vec<u32> = vec![0; CHUNK_SIZE3];
    let mut indices = ShaderStorageBuffer::from(indices);
    indices.buffer_description.usage |= BufferUsages::COPY_SRC;
    let indices = buffers.add(indices);
    let normals: Vec<PaddedVec3> = vec![PaddedVec3::default(); CHUNK_SIZE3];
    let mut normals = ShaderStorageBuffer::from(normals);
    normals.buffer_description.usage |= BufferUsages::COPY_SRC;
    let normals = buffers.add(normals);
    let uvs: Vec<Vec2> = vec![Vec2::default(); CHUNK_SIZE3];
    let mut uvs = ShaderStorageBuffer::from(uvs);
    uvs.buffer_description.usage |= BufferUsages::COPY_SRC;
    let uvs = buffers.add(uvs);

    commands.spawn(Readback::buffer(data.clone())).observe(
        |trigger: Trigger<ReadbackComplete>, mut commands: Commands| {
            let data: VertexData = trigger.event().to_shader_type();
            if data.index_head != 0 {
                info!(?data.index_head);
                commands.entity(trigger.entity()).despawn();
            }
        },
    );

    commands.insert_resource(VertexBuffers {
        voxels: gen_buffers.voxels.clone(),
        data,
        vertices,
        indices,
        normals,
        uvs,
    });
}

#[derive(Resource)]
pub struct GpuBufferBindGroup(pub BindGroup);

fn prepare_bind_groups(
    mut commands: Commands,
    pipeline: Res<ComputePipeline>,
    render_device: Res<RenderDevice>,
    vert_buffers: Res<VertexBuffers>,
    buffers: Res<RenderAssets<GpuShaderStorageBuffer>>,
    fallback: Res<FallbackImage>,
    images: Res<RenderAssets<GpuImage>>,
) {
    let bind_group = vert_buffers
        .as_bind_group(
            &pipeline.0.layout,
            &render_device,
            &mut (images, fallback, buffers),
        )
        .unwrap()
        .bind_group;

    commands.insert_resource(GpuBufferBindGroup(bind_group));
}

#[derive(Resource)]
pub struct ComputePipeline(pub PipelineGroup);

impl FromWorld for ComputePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let shader = world.load_asset(SHADER_PATH);
        let layout = VertexBuffers::bind_group_layout(&render_device);
        let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("Voxel pipeline".into()),
            layout: vec![layout.clone()],
            push_constant_ranges: Vec::new(),
            shader: shader.clone(),
            shader_defs: Vec::new(),
            entry_point: "main".into(),
            zero_initialize_workgroup_memory: false,
        });

        let pipeline = PipelineGroup { pipeline, layout };

        ComputePipeline(pipeline)
    }
}
