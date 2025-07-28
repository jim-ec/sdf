struct Uniforms {
    model: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
}

struct FragmentInput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vertex(in: VertexInput) -> FragmentInput {
    var out: FragmentInput;
    out.position = uniforms.projection * uniforms.view * uniforms.model * vec4<f32>(in.position, 1.0);
    out.color = vec4<f32>(in.color, 1.0);
    return out;
}

@fragment
fn fragment(in: FragmentInput) -> @location(0) vec4<f32> {
    return in.color;
    // return vec4<f32>(vec3(1.0 / in.position.w), 1.0);
}
