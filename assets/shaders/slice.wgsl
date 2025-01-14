@group(0) @binding(0) var voxels: texture_storage_3d<r32float, read>;
@group(0) @binding(1) var slice: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> layer: u32;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let loc = vec2<u32>(u32(invocation_id.x), u32(invocation_id.y));
    let loc3 = vec3<u32>(loc.x, loc.y, layer);
    let value = textureLoad(voxels, loc3);

    if value.x > 0.49 && value.x < 0.51 {
        textureStore(slice, loc, vec4<f32>(0.));
    } else {
        textureStore(slice, loc, value);
    }

    // textureStore(slice, loc, vec4<f32>(f32(value) / 1024.));
}
