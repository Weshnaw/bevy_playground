use std::{borrow::Cow, hash::Hash, marker::PhantomData, sync::Arc};

use bevy::{
    asset::{Assets, RenderAssetUsages},
    image::Image,
    prelude::*,
    render::{
        Render, RenderSet, gpu_readback,
        render_asset::RenderAssets,
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, BufferUsages,
            CachedComputePipelineId, CachedPipelineState, ComputePass, ComputePassDescriptor,
            ComputePipelineDescriptor, Extent3d, PipelineCache, ShaderStages, ShaderType,
            TextureDimension, TextureFormat, encase::internal::WriteInto,
        },
        renderer::RenderDevice,
        storage::{GpuShaderStorageBuffer, ShaderStorageBuffer},
        texture::GpuImage,
    },
};
#[derive(Clone)]
pub enum ImageData {
    Fill([u8; 4]),
    Data(Vec<u8>),
    Zeros,
}

#[derive(Clone)]
pub struct ImageBuilder {
    pub size: Extent3d,
    pub data: ImageData,
}

pub trait HelperStorageBuffer {
    fn from_handle(value: Handle<ShaderStorageBuffer>) -> Self;
}

pub trait HelperTextureBuffer {
    fn texture_details() -> (TextureFormat, TextureDimension);
    fn from_handle(value: Handle<Image>) -> Self;
}

pub trait HelperComputeShader {}

pub trait HelperBufferGroup<DataTy: Clone, const B: usize> {
    fn create_resource_extractor_plugins(app: &mut App);

    fn get_bindings<'a>(
        &'a self,
        buffers: &'a RenderAssets<GpuShaderStorageBuffer>,
        images: &'a RenderAssets<GpuImage>,
    ) -> BindGroupEntries<'a, B>;
    fn label() -> Option<&'static str> {
        // TODO: make this the correct return type -> impl wgpu::Label<'a>
        None
    }

    // TODO: consider refactoring the buffer inserters
    fn insert_resources(
        commands: &mut Commands,
        buffers: &mut Assets<ShaderStorageBuffer>,
        images: &mut Assets<Image>,
        d: DataTy,
    );
    fn insert_buffer<BufType: Resource + HelperStorageBuffer, DataType: ShaderType + WriteInto>(
        commands: &mut Commands,
        buffers: &mut Assets<ShaderStorageBuffer>,
        data: DataType,
        writeable: bool,
    ) -> Handle<ShaderStorageBuffer> {
        let mut buffer = ShaderStorageBuffer::from(data);
        if writeable {
            buffer.buffer_description.usage |= BufferUsages::COPY_SRC;
        }
        let buffer = buffers.add(buffer);
        commands.insert_resource(BufType::from_handle(buffer.clone()));

        buffer
    }
    fn insert_texture<BufType: Resource + HelperTextureBuffer>(
        commands: &mut Commands,
        images: &mut Assets<Image>,
        image_builder: ImageBuilder,
        writeable: bool,
    ) -> Handle<Image> {
        let (format, dimension) = BufType::texture_details();
        let asset_usage = RenderAssetUsages::RENDER_WORLD;

        let mut image = match image_builder.data {
            ImageData::Fill(data) => bevy::image::Image::new_fill(
                image_builder.size,
                dimension,
                &data,
                format,
                asset_usage,
            ),
            ImageData::Data(vec) => {
                bevy::image::Image::new(image_builder.size, dimension, vec, format, asset_usage)
            }
            ImageData::Zeros => {
                let size = image_builder.size;
                let total = size.height * size.width * size.depth_or_array_layers;
                let total = total * format.block_copy_size(None).unwrap_or(0);
                bevy::image::Image::new(
                    size,
                    dimension,
                    vec![0; total as usize],
                    format,
                    asset_usage,
                )
            }
        };
        image.texture_descriptor.usage |=
            bevy::render::render_resource::TextureUsages::STORAGE_BINDING;
        if writeable {
            image.texture_descriptor.usage |=
                bevy::render::render_resource::TextureUsages::COPY_SRC;
        }

        let image = images.add(image);
        commands.insert_resource(BufType::from_handle(image.clone()));

        image
    }
}

pub trait HelperPipeline {
    fn layout(&self) -> &BindGroupLayout;
    fn get_id<EntryTy: HelperEntry>(&self, entry: &EntryTy) -> CachedComputePipelineId;
    fn compute_pass_label() -> Option<&'static str> {
        None
    }
}

pub trait HelperBindGroup {
    fn from_bind_group(bind_group: BindGroup) -> Self;
}

pub fn prepare_bind_group<
    const B: usize,
    BuffersDataTy: Clone,
    PipelineTy: Resource + HelperPipeline,
    BuffersTy: Resource + HelperBufferGroup<BuffersDataTy, B>,
>(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline: Res<PipelineTy>,
    // pipeline: Res<ComputePipeline<B, E, BuffersDataTy>>,
    buffer: Res<BuffersTy>,
    buffers: Res<RenderAssets<GpuShaderStorageBuffer>>,
    images: Res<RenderAssets<GpuImage>>,
) {
    debug!("Preparing bind group");
    let bind_group = render_device.create_bind_group(
        BuffersTy::label(),
        pipeline.layout(),
        &buffer.get_bindings(&buffers, &images),
    );

    let bind_group: GenericBindGroup<PipelineTy> = GenericBindGroup::from_bind_group(bind_group);
    commands.insert_resource(bind_group);
}

#[derive(Resource)]
pub struct ComputePipeline<const B: usize, const E: usize, DataTy> {
    pub layout: BindGroupLayout,
    pub entries: [CachedComputePipelineId; E],
    _phantom: PhantomData<DataTy>,
}
impl<const B: usize, const E: usize, DataTy> HelperPipeline for ComputePipeline<B, E, DataTy> {
    fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }

    fn get_id<EntryTy: HelperEntry>(&self, entry: &EntryTy) -> CachedComputePipelineId {
        self.entries[entry.as_key()]
    }
}

pub trait HelperEntry {
    fn as_key(&self) -> usize;
}

impl<const B: usize, const E: usize, DataTy: HelperBufferData<B, E>> FromWorld
    for ComputePipeline<B, E, DataTy>
{
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        let render_device = world.resource::<bevy::render::renderer::RenderDevice>();
        let layout = render_device.create_bind_group_layout(
            DataTy::bind_group_label(),
            &DataTy::buffer_entries(ShaderStages::COMPUTE),
        );

        let shader = world.load_asset("shaders/hello.wgsl");
        let pipeline_cache = world.resource::<PipelineCache>();
        let entries = DataTy::entries(pipeline_cache, layout.clone(), shader);
        Self {
            layout,
            entries,
            _phantom: Default::default(),
        }
    }
}

pub trait HelperBufferData<const B: usize, const E: usize> {
    fn buffer_entries(stage: ShaderStages) -> BindGroupLayoutEntries<B>;

    fn bind_group_label() -> Option<&'static str> {
        None
    }

    fn entries(
        pipeline_cache: &PipelineCache,
        layout: BindGroupLayout,
        shader: Handle<Shader>,
    ) -> [CachedComputePipelineId; E];

    fn create_entry(
        pipeline_cache: &PipelineCache,
        layout: BindGroupLayout,
        shader: Handle<Shader>,
        entry: &'static str,
        label: Option<Cow<'static, str>>,
    ) -> CachedComputePipelineId {
        pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label,
            layout: vec![layout],
            push_constant_ranges: vec![],
            shader,
            shader_defs: vec![],
            entry_point: entry.into(),
            zero_initialize_workgroup_memory: false,
        })
    }
}

#[derive(Resource)]
pub struct GenericBindGroup<T>(pub BindGroup, PhantomData<T>);
impl<T> HelperBindGroup for GenericBindGroup<T> {
    fn from_bind_group(bind_group: BindGroup) -> Self {
        Self(bind_group, Default::default())
    }
}

#[derive(Default)]
pub enum ShaderStage {
    #[default]
    Loading,
    Startup,
    Update, // TODO: somehow allow for end user to do fancy state things such as the gol example of buffer swapping
}

#[derive(Clone, Debug)]
pub struct ShaderEntry<EntryTy> {
    pub entry: EntryTy,
    pub workgroup: (u32, u32, u32),
}

impl<EntryTy: HelperEntry> ShaderEntry<EntryTy> {
    fn get_state<'a, PipelineTy: HelperPipeline>(
        &'a self,
        pipeline_cache: &'a PipelineCache,
        pipeline: &'a PipelineTy,
    ) -> &'a CachedPipelineState {
        pipeline_cache.get_compute_pipeline_state(pipeline.get_id(&self.entry))
    }

    fn dispatch<PipelineTy: HelperPipeline>(
        &self,
        pipeline_cache: &PipelineCache,
        pipeline: &PipelineTy,
        pass: &mut ComputePass,
        bind_group: &GenericBindGroup<PipelineTy>,
    ) {
        if let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline.get_id(&self.entry)) {
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.set_pipeline(pipeline);
            pass.dispatch_workgroups(self.workgroup.0, self.workgroup.1, self.workgroup.2);
        }
    }
}

#[derive(Clone)]
pub struct ShaderEntries<EntryTy> {
    pub on_startup: Vec<ShaderEntry<EntryTy>>,
    pub on_update: Vec<ShaderEntry<EntryTy>>,
    // TODO: on_request: Vec<(receiver, ShaderDispatch)>
}

// TODO: consider refactoring so that you pass in a enum to specify if on_update or on_startup
impl<EntryTy: HelperEntry> ShaderEntries<EntryTy> {
    pub fn on_startup_success<PipelineTy: HelperPipeline>(
        &self,
        pipeline_cache: &PipelineCache,
        pipeline: &PipelineTy,
    ) -> bool {
        self.on_startup
            .iter()
            .map(|entry| entry.get_state(pipeline_cache, pipeline))
            .all(|state| match state {
                CachedPipelineState::Ok(_) => true,
                CachedPipelineState::Err(e) => {
                    panic!("Failed to load shader: {e}")
                }
                _ => false,
            })
    }

    pub fn on_update_success<PipelineTy: HelperPipeline>(
        &self,
        pipeline_cache: &PipelineCache,
        pipeline: &PipelineTy,
    ) -> bool {
        self.on_startup
            .iter()
            .map(|entry| pipeline_cache.get_compute_pipeline_state(pipeline.get_id(&entry.entry)))
            .all(|state| matches!(state, CachedPipelineState::Ok(_)))
    }

    pub fn on_startup_dispatch<PipelineTy: HelperPipeline>(
        &self,
        pipeline_cache: &PipelineCache,
        pipeline: &PipelineTy,
        pass: &mut ComputePass,
        bind_group: &GenericBindGroup<PipelineTy>,
    ) {
        for entry in self.on_startup.iter() {
            entry.dispatch(pipeline_cache, pipeline, pass, bind_group);
        }
    }

    pub fn on_update_dispatch<PipelineTy: HelperPipeline>(
        &self,
        pipeline_cache: &PipelineCache,
        pipeline: &PipelineTy,
        pass: &mut ComputePass,
        bind_group: &GenericBindGroup<PipelineTy>,
    ) {
        for entry in self.on_update.iter() {
            entry.dispatch(pipeline_cache, pipeline, pass, bind_group);
        }
    }
}

pub struct ComputeNode<PipelineTy, EntryTy> {
    state: ShaderStage,
    dispatches: ShaderEntries<EntryTy>,
    _phantom: PhantomData<PipelineTy>,
}

impl<PipelineTy, EntryTy> ComputeNode<PipelineTy, EntryTy> {
    pub fn new(dispatches: ShaderEntries<EntryTy>) -> Self {
        Self {
            state: ShaderStage::Loading,
            dispatches,
            _phantom: Default::default(),
        }
    }
}

impl<PipelineTy: Resource + HelperPipeline, EntryTy: HelperEntry + Send + Sync + 'static>
    render_graph::Node for ComputeNode<PipelineTy, EntryTy>
{
    fn update(&mut self, world: &mut bevy::prelude::World) {
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<PipelineTy>();

        match self.state {
            ShaderStage::Loading => {
                if self.dispatches.on_startup_success(pipeline_cache, pipeline) {
                    self.state = ShaderStage::Startup
                }
            }
            ShaderStage::Startup => {
                if self.dispatches.on_update_success(pipeline_cache, pipeline) {
                    self.state = ShaderStage::Update
                }
            }
            _ => {}
        }
    }

    fn run(
        &self,
        _graph: &mut bevy::render::render_graph::RenderGraphContext,
        render_context: &mut bevy::render::renderer::RenderContext,
        world: &bevy::prelude::World,
    ) -> Result<(), bevy::render::render_graph::NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<PipelineTy>();
        let bind_group = world.resource::<GenericBindGroup<PipelineTy>>();
        let mut pass =
            render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: PipelineTy::compute_pass_label(),
                    ..default()
                });
        match self.state {
            ShaderStage::Startup => {
                self.dispatches.on_startup_dispatch(
                    pipeline_cache,
                    pipeline,
                    &mut pass,
                    bind_group,
                );
            }
            ShaderStage::Update => {
                self.dispatches
                    .on_update_dispatch(pipeline_cache, pipeline, &mut pass, bind_group);
            }
            _ => {}
        }

        Ok(())
    }
}

pub struct ShaderBuilder<T: ?Sized, DataTy, EntriesTy> {
    initial_data: Option<DataTy>,
    dispatches: Option<ShaderEntries<EntriesTy>>,
    _phantom: PhantomData<T>,
}

impl<DataTy, EntriesTy, T: BuildableShader<DataTy, EntriesTy>> Default
    for ShaderBuilder<T, DataTy, EntriesTy>
{
    fn default() -> Self {
        Self {
            initial_data: Default::default(),
            dispatches: Default::default(),
            _phantom: Default::default(),
        }
    }
}
impl<DataTy, EntriesTy, T: BuildableShader<DataTy, EntriesTy>> ShaderBuilder<T, DataTy, EntriesTy> {
    pub fn initial_data(self, data: DataTy) -> Self {
        Self {
            initial_data: Some(data),
            dispatches: self.dispatches,
            _phantom: self._phantom,
        }
    }

    pub fn dispatches(self, dispatches: ShaderEntries<EntriesTy>) -> Self {
        Self {
            initial_data: self.initial_data,
            dispatches: Some(dispatches),
            _phantom: self._phantom,
        }
    }

    pub fn build(self) -> T {
        T::from_builder(self)
    }
}

pub trait BuildableShader<DataTy, EntriesTy> {
    fn from_builder(builder: ShaderBuilder<Self, DataTy, EntriesTy>) -> Self;
}

impl<const B: usize, DataTy, EntriesTy, BuffersTy, PipelineTy, ShaderLabel>
    BuildableShader<DataTy, EntriesTy>
    for ShaderPlugin<DataTy, EntriesTy, BuffersTy, PipelineTy, ShaderLabel, B>
{
    fn from_builder(builder: ShaderBuilder<Self, DataTy, EntriesTy>) -> Self {
        let Some(initial_data) = builder.initial_data else {
            todo!()
        };
        let Some(entry_dispatches) = builder.dispatches else {
            todo!()
        };

        Self {
            initial_data: Arc::new(initial_data),
            entry_dispatches,
            _buffers_phantom: PhantomData,
            _pipeline_phantom: PhantomData,
            _label_phantom: PhantomData,
        }
    }
}

pub struct ShaderPlugin<DataTy, EntriesTy, BuffersTy, PipelineTy, ShaderLabel, const B: usize> {
    initial_data: Arc<DataTy>,
    entry_dispatches: ShaderEntries<EntriesTy>,
    _buffers_phantom: PhantomData<BuffersTy>,
    _pipeline_phantom: PhantomData<PipelineTy>,
    _label_phantom: PhantomData<ShaderLabel>,
}

fn create_setup<const B: usize, DataTy: Clone, BuffersTy: HelperBufferGroup<DataTy, B>>(
    d: Arc<DataTy>,
) -> impl Fn(Commands, ResMut<Assets<ShaderStorageBuffer>>, ResMut<Assets<Image>>) {
    move |mut commands, mut buffers, mut images| {
        BuffersTy::insert_resources(&mut commands, &mut buffers, &mut images, d.as_ref().clone());
    }
}
impl<
    const B: usize,
    DataTy: Send + Sync + 'static + Clone,
    EntriesTy: Send + Sync + 'static + HelperEntry + Clone,
    BuffersTy: Send + Sync + 'static + HelperBufferGroup<DataTy, B> + Resource,
    PipelineTy: Send + Sync + 'static + HelperPipeline + Resource + FromWorld,
    ShaderLabel: Send + Sync + 'static + RenderLabel + Clone + Eq + PartialEq + Hash,
> Plugin for ShaderPlugin<DataTy, EntriesTy, BuffersTy, PipelineTy, ShaderLabel, B>
{
    fn build(&self, app: &mut App) {
        BuffersTy::create_resource_extractor_plugins(app);
        app.add_systems(
            PreStartup,
            create_setup::<B, DataTy, BuffersTy>(self.initial_data.clone()),
        );
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(bevy::render::RenderApp);
        debug!("Preparing render resources");
        render_app.init_resource::<PipelineTy>().add_systems(
            Render,
            prepare_bind_group::<B, _, PipelineTy, BuffersTy>
                .in_set(RenderSet::PrepareBindGroups)
                .run_if(not(resource_exists::<GenericBindGroup<PipelineTy>>)),
        );

        render_app
            .world_mut()
            .resource_mut::<RenderGraph>()
            .add_node(
                GenericLabel::<ShaderLabel>::new(),
                ComputeNode::<PipelineTy, EntriesTy>::new(self.entry_dispatches.clone()),
            );
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct GenericLabel<T> {
    _phantom: PhantomData<T>,
}

impl<T> GenericLabel<T> {
    fn new() -> Self {
        Self {
            _phantom: Default::default(),
        }
    }
}

pub trait BufferReader {
    fn readback(&self) -> gpu_readback::Readback;
}
pub trait BufferWriter {
    type T;
    fn get_mut<'a>(&'a self, buffers: &'a mut ResMut<Assets<Self::T>>) -> &'a mut Self::T
    where
        Self::T: Asset;
}
