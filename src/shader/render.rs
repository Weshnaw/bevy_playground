use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSet,
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::{
            BindGroupLayout, CachedComputePipelineId, CachedPipelineState, ComputePassDescriptor,
            PipelineCache,
        },
        renderer::RenderContext,
    },
};
use crossbeam_channel::{Receiver, Sender};

use super::{
    CHUNK_SIZE,
    generate::{self, GenerateVoxelsPlugin},
    vertices::{self, GenerateVerticesPlugin},
};

pub struct ShaderRenderPlugin;
impl Plugin for ShaderRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((GenerateVoxelsPlugin, GenerateVerticesPlugin));
    }

    fn finish(&self, app: &mut App) {
        let (s, r) = crossbeam_channel::unbounded();
        let render_app = app.sub_app_mut(RenderApp);

        render_app
            .insert_resource(ComputeShaderStateSender(s))
            .insert_resource(ComputeShaderStateReceiver(r))
            .add_systems(Render, test_state.after(RenderSet::Render))
            .world_mut()
            .resource_mut::<RenderGraph>()
            .add_node(ComputeNodeLabel, ComputeNode::default());
    }
}

#[derive(Resource)]
struct ComputeShaderStateReceiver(Receiver<ComputeShaderStage>);
#[derive(Resource)]
struct ComputeShaderStateSender(Sender<ComputeShaderStage>);

fn test_state(state: Res<ComputeShaderStateReceiver>) {
    if let Ok(state) = state.0.try_recv() {
        info!(?state);
        // TODO render objects here / or send buffer data to main land / or  move this to main world and see if the buffers already contain the correct data
    }
}

pub struct PipelineGroup {
    pub pipeline: CachedComputePipelineId,
    pub layout: BindGroupLayout,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct ComputeNodeLabel;

#[derive(Debug)]
enum ComputeShaderStage {
    Starting,
    StartupComplete,
}

#[derive(Default)]
pub(super) enum ShaderStage {
    #[default]
    Loading,
    GenerateVoxels,
    GenerateMeshVertices,
    Update,
}

#[derive(Default)]
struct ComputeNode {
    stage: ShaderStage,
}

impl render_graph::Node for ComputeNode {
    fn update(&mut self, world: &mut World) {
        let pipeline_cache = world.resource::<PipelineCache>();
        let gen_pipeline = world.resource::<generate::ComputePipeline>();
        let vert_pipeline = world.resource::<vertices::ComputePipeline>();
        let compute_shader_state = world.resource::<ComputeShaderStateSender>();

        match self.stage {
            ShaderStage::Loading => {
                let state = pipeline_cache.get_compute_pipeline_state(gen_pipeline.sphere.pipeline);

                match state {
                    CachedPipelineState::Ok(_) => {
                        self.stage = ShaderStage::GenerateVoxels;
                        compute_shader_state
                            .0
                            .try_send(ComputeShaderStage::Starting)
                            .expect("Failed to send shader stage");
                    }
                    CachedPipelineState::Err(err) => panic!("Unable to load pipeline\n{}", err),
                    _ => {}
                }
            }
            ShaderStage::GenerateVoxels => {
                let state = pipeline_cache.get_compute_pipeline_state(vert_pipeline.0.pipeline);

                match state {
                    CachedPipelineState::Ok(_) => self.stage = ShaderStage::GenerateMeshVertices,
                    CachedPipelineState::Err(err) => {
                        panic!("Unable to load vert pipeline\n{}", err)
                    }
                    _ => {}
                }
            }
            ShaderStage::GenerateMeshVertices => {
                let state = pipeline_cache.get_compute_pipeline_state(gen_pipeline.slice.pipeline);

                if let CachedPipelineState::Ok(_) = state {
                    compute_shader_state
                        .0
                        .try_send(ComputeShaderStage::StartupComplete)
                        .expect("Failed to send shader stage");
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
        let gen_pipeline = world.resource::<generate::ComputePipeline>();
        let gen_bind_group = world.resource::<generate::GpuBufferBindGroup>();
        let vert_pipeline = world.resource::<vertices::ComputePipeline>();
        let vert_bind_group = world.resource::<vertices::GpuBufferBindGroup>();

        let mut pass =
            render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: None,
                    ..Default::default()
                });

        match self.stage {
            ShaderStage::GenerateVoxels => {
                if let Some(init_pipeline) =
                    pipeline_cache.get_compute_pipeline(gen_pipeline.sphere.pipeline)
                {
                    pass.set_bind_group(0, &gen_bind_group.sphere, &[]);
                    pass.set_pipeline(init_pipeline);
                    pass.dispatch_workgroups(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE);
                }
            }
            ShaderStage::GenerateMeshVertices => {
                if let Some(init_pipeline) =
                    pipeline_cache.get_compute_pipeline(vert_pipeline.0.pipeline)
                {
                    pass.set_bind_group(0, &vert_bind_group.0, &[]);
                    pass.set_pipeline(init_pipeline);
                    pass.dispatch_workgroups(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE);
                }
            }
            ShaderStage::Update => {
                if let Some(init_pipeline) =
                    pipeline_cache.get_compute_pipeline(gen_pipeline.slice.pipeline)
                {
                    pass.set_bind_group(0, &gen_bind_group.slice, &[]);
                    pass.set_pipeline(init_pipeline);
                    pass.dispatch_workgroups(CHUNK_SIZE, CHUNK_SIZE, 1);
                }
            }
            _ => {}
        }

        Ok(())
    }
}
