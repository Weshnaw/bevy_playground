struct Foo {
    bar: u32,
    bazz: f32,
}

@group(0) @binding(0) var<storage, read_write> a: array<u32>;
@group(0) @binding(1) var<storage, read>       b: Foo;
@group(0) @binding(2) var<storage, read>       c: vec3<f32>;
@group(0) @binding(3) var                      d: texture_storage_2d<r32float, read_write>;



@compute @workgroup_size(1) fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    a[global_id.x] += 1u;
    let loc = vec2<u32>(global_id.x, global_id.y); 
    let x = textureLoad(d, loc);
    textureStore(d, loc, x + 1.);
}

@compute @workgroup_size(1) fn update(@builtin(global_invocation_id) global_id: vec3<u32>) {
    a[global_id.x] = b.bar;
    let loc = vec2<u32>(global_id.x, global_id.y); 
    let x = textureLoad(d, loc);
    textureStore(d, loc, x + 1.);
}