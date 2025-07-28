@group(0) @binding(0) var depth_sampler: sampler;
@group(0) @binding(1) var depth_texture: texture_depth_2d;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Compute the normalized quad coordinates based on the vertex index.
    // #: 0 1 2
    // x: 0 1 0
    // y: 0 0 1
    let uv = (vec2(vertex_index) & vec2(1u, 2u)) << vec2(1u, 0u);

    var out: VertexOutput;
    out.position = vec4(vec2<f32>(uv << vec2(1u)) - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // return vec4<f32>(in.uv, 0.0, 1.0);
    let depth = textureSample(depth_texture, depth_sampler, in.uv);
    let far = 100.0;
    let near = 0.1;
    let depth_range = far - near;
    let depth_normalized = (depth - near) / depth_range;
    let linear_depth = near * far / (far - depth * (far - near));
    return vec4<f32>(vec3(depth * 0.1), 1.0);
}
