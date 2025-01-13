@group(0) @binding(0) var voxels: texture_storage_3d<r32float, read_write>; // Does this need to be read_write?
// I only really need the 1 float; maybe in the future I could find a use for all rgba values

// debug the texture into a layer slice
@group(0) @binding(1) var debug: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> layer: u32;
@group(0) @binding(3) var<storage, read_write> count: atomic<u32>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let pos = vec3<f32>(invocation_id);
    let dim = vec3<f32>(textureDimensions(voxels));
    let x = pos - dim / 2.;
    let y = length(x);
    if y != 0. {
        let value = vec4<f32>(5. / y);  // there's probably a nice way to compute this magic number
        textureStore(voxels, invocation_id, value);
    } else {
        textureStore(voxels, invocation_id, vec4<f32>(1.));
    }
    // atomicStore(&count, 100u);
}

@compute @workgroup_size(8, 8, 1)
fn get_slice(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let loc = vec2<u32>(u32(invocation_id.x), u32(invocation_id.y));
    let loc3 = vec3<u32>(loc.x, loc.y, layer);
    let value = textureLoad(voxels, loc3);
    if value.x > 0.495 && value.x < 0.505 {
        textureStore(debug, loc, vec4<f32>(0.));
    } else {
        textureStore(debug, loc, value);
    }

    let val = atomicAdd(&count, 1u);
    textureStore(debug, loc, vec4<f32>(f32(val) / 1024.));
}