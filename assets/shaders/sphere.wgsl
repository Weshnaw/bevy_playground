@group(0) @binding(0) var sphere: texture_storage_3d<r32float, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let pos = vec3<f32>(invocation_id);
    let dim = vec3<f32>(textureDimensions(sphere));
    let x = pos - dim / 2.;
    let y = max(length(x), 0.001); // avoids the zero and should avoid branching
    let value = vec4<f32>(5. / y);  // there's probably a nice way to compute this magic number
    textureStore(sphere, invocation_id, value);
}
