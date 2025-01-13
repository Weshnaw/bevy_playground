use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        gpu_readback::Readback,
        render_asset::RenderAssets,
        render_graph::RenderLabel,
        render_resource::{
            BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries, CachedComputePipelineId,
            IntoBinding, PipelineCache, ShaderStages, ShaderType, StorageTextureAccess,
            TextureDimension, TextureFormat,
            binding_types::{storage_buffer, storage_buffer_read_only, texture_storage_2d},
        },
        storage::{GpuShaderStorageBuffer, ShaderStorageBuffer},
        texture::GpuImage,
    },
};

use super::slib::{
    BufferReader, ComputePipeline, GenericBindGroup, HelperBufferData,
    HelperBufferGroup, HelperEntry, HelperStorageBuffer, HelperTextureBuffer, ImageBuilder,
    ShaderPlugin,
};

pub type HelloShaderPlugin =
    ShaderPlugin<HelloData, HelloEntries, HelloBuffers, HelloComputePipeline, HelloShader, 4>;
pub type HelloComputePipeline = ComputePipeline<4, 2, HelloData>;
pub type HelloBindGroup = GenericBindGroup<HelloComputePipeline>;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct HelloShader;

// Above can probably be abstracted out with some phantom data types
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum HelloEntries {
    Main,
    Update,
}

impl HelperEntry for HelloEntries {
    fn as_key(&self) -> usize {
        match self {
            HelloEntries::Main => 0,
            HelloEntries::Update => 1,
        }
    }
}

#[derive(Clone, ShaderType)]
pub struct Foo {
    pub bar: u32,
    pub bazz: f32,
}

#[derive(Clone)]
pub struct HelloData {
    // for buffer in buffers
    pub a: Vec<u32>,
    pub b: Foo,
    pub c: Vec3,
    pub d: ImageBuilder,
}

impl HelperBufferData<4, 2> for HelloData {
    fn buffer_entries(stage: ShaderStages) -> BindGroupLayoutEntries<4> {
        BindGroupLayoutEntries::sequential(
            stage,
            (
                storage_buffer::<Vec<u32>>(false),
                storage_buffer_read_only::<Foo>(false),
                storage_buffer_read_only::<Vec3>(false),
                texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::ReadWrite),
            ),
        )
    }

    fn entries(
        pipeline_cache: &PipelineCache,
        layout: BindGroupLayout,
        shader: Handle<Shader>,
    ) -> [CachedComputePipelineId; 2] {
        [
            Self::create_entry(pipeline_cache, layout.clone(), shader.clone(), "main", None),
            Self::create_entry(
                pipeline_cache,
                layout.clone(),
                shader.clone(),
                "update",
                None,
            ),
        ]
    }
}

// I don't like this but I do not know how to improve it
#[derive(Resource, ExtractResource, Clone)]
pub struct HelloBuffers {
    a: ABuffer,
    b: BBuffer,
    c: CBuffer,
    d: DBuffer,
}

impl HelperBufferGroup<HelloData, 4> for HelloBuffers {
    fn get_bindings<'a>(
        &'a self,
        buffers: &'a RenderAssets<GpuShaderStorageBuffer>,
        images: &'a RenderAssets<GpuImage>,
    ) -> BindGroupEntries<'a, 4> {
        BindGroupEntries::sequential((
            buffers
                .get(&self.a.0)
                .unwrap()
                .buffer
                .as_entire_buffer_binding(),
            buffers
                .get(&self.b.0)
                .unwrap()
                .buffer
                .as_entire_buffer_binding(),
            buffers
                .get(&self.c.0)
                .unwrap()
                .buffer
                .as_entire_buffer_binding(),
            images.get(&self.d.0).unwrap().texture_view.into_binding(),
        ))
    }

    fn insert_resources(
        commands: &mut Commands,
        buffers: &mut Assets<ShaderStorageBuffer>,
        images: &mut Assets<Image>,
        d: HelloData,
    ) {
        let a = Self::insert_buffer::<ABuffer, _>(commands, buffers, d.a, true);
        let b = Self::insert_buffer::<BBuffer, _>(commands, buffers, d.b, false);
        let c = Self::insert_buffer::<CBuffer, _>(commands, buffers, d.c, false);
        let d = Self::insert_texture::<DBuffer>(commands, images, d.d, true);

        commands.insert_resource(Self {
            a: ABuffer(a),
            b: BBuffer(b),
            c: CBuffer(c),
            d: DBuffer(d),
        });
    }

    fn create_resource_extractor_plugins(app: &mut App) {
        app.add_plugins((
            ExtractResourcePlugin::<ABuffer>::default(),
            ExtractResourcePlugin::<BBuffer>::default(),
            ExtractResourcePlugin::<CBuffer>::default(),
            ExtractResourcePlugin::<DBuffer>::default(),
            ExtractResourcePlugin::<Self>::default(),
        ));
    }
}

#[derive(Resource, ExtractResource, Clone)]
pub struct ABuffer(Handle<ShaderStorageBuffer>);
impl HelperStorageBuffer for ABuffer {
    fn from_handle(value: Handle<ShaderStorageBuffer>) -> Self {
        Self(value)
    }
}

impl BufferReader for ABuffer {
    fn readback(&self) -> Readback {
        Readback::buffer(self.0.clone())
    }
}
// impl BufferWriter for ABuffer {
//     type T = ShaderStorageBuffer;

//     fn get_mut<'a>(&'a self, buffers: &'a mut ResMut<Assets<Self::T>>) -> &'a mut Self::T where Self::T: Asset {
//         todo!()
//     }

// }

#[derive(Resource, ExtractResource, Clone)]
pub struct BBuffer(Handle<ShaderStorageBuffer>);
impl HelperStorageBuffer for BBuffer {
    fn from_handle(value: Handle<ShaderStorageBuffer>) -> Self {
        Self(value)
    }
}
#[derive(Resource, ExtractResource, Clone)]
pub struct CBuffer(Handle<ShaderStorageBuffer>);
impl HelperStorageBuffer for CBuffer {
    fn from_handle(value: Handle<ShaderStorageBuffer>) -> Self {
        Self(value)
    }
}
#[derive(Resource, ExtractResource, Clone)]
pub struct DBuffer(Handle<Image>);
impl HelperTextureBuffer for DBuffer {
    fn texture_details() -> (TextureFormat, TextureDimension) {
        (TextureFormat::R32Float, TextureDimension::D2)
    }

    fn from_handle(value: Handle<Image>) -> Self {
        Self(value)
    }
}
