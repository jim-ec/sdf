struct Uniforms {
    rotation: vec4<f32>,
    eye: vec3<f32>,
    viewport_aspect_ratio: f32,
    max_iterations: u32,
    min_step_size: f32,
}

var<push_constant> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Compute the normalized quad coordinates based on the vertex index.
    // #: 0 1 2 3 4 5
    // x: 1 1 0 0 0 1
    // y: 1 0 0 0 1 1
    let uv = vec2<u32>((vec2(1u, 2u) + vertex_index) % vec2(6u) < vec2(3u));

    var out: VertexOutput;
    out.position = vec4(vec2<f32>(uv << vec2(1u)) - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv) - 0.5;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let focal_length = 1.0;
    let pixel = vec2(uniforms.viewport_aspect_ratio, 1.0) * in.uv;
    let direction = quat_transform(uniforms.rotation, normalize(vec3(pixel, focal_length)));
    var point = quat_transform(uniforms.rotation, uniforms.eye);

    var i = 0u;
    loop {
        if (i >= uniforms.max_iterations) {
            discard;
        }

        let dist = sdf(point);
        if (dist < uniforms.min_step_size) {
            break;
        }
        point += dist * direction;

        i++;
    }

    let n = sdf_normal(point);
    // let color = vec3(0.9) * dot(direction, -n) + 0.1;
    // let color = vec3(f32(i)) / f32(uniforms.max_iterations);
    // return vec4(color, 1.0);
    return vec4(0.5 * n + 0.5, 1.0);
    // return vec4(linear_to_gamma_rgb(color), 1.0);
}

fn linear_to_gamma_rgb(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3(2.2));
}

fn sdf(point: vec3<f32>) -> f32 {
    // return sdf_sphere(point, vec3(0.0), 1.0);
    return sdf_cuboid(point, vec3(0.0), vec3(0.5)) - 0.2;
}

fn sdf_sphere(point: vec3<f32>, center: vec3<f32>, radius: f32) -> f32 {
    let dist = distance(center, point);
    return dist - radius;
}

fn sdf_cuboid(point: vec3<f32>, center: vec3<f32>, half_extent: vec3<f32>) -> f32 {
    let offset: vec3<f32> = abs(point - center) - half_extent;
    // dst from point outside box to edge (0 if inside box)
    let unsignedDst: f32 = length(max(offset, vec3(0.0)));
    // -dst from point inside box to edge (0 if outside box)
    let tmp: vec3<f32> = min(offset, vec3(0.0));
    let dstInsideBox: f32 = max(max(tmp.x, tmp.y), tmp.z);
    return unsignedDst + dstInsideBox;
}

fn modulo(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    return p - n * round(p / n);
}

fn sdf_empty() -> f32 {
    return 1.0;
}

fn sdf_union(d1: f32, d2: f32) -> f32 {
    return min(d1, d2);
}

fn sdf_intersection(d1: f32, d2: f32) -> f32 {
    return max(d1, d2);
}

fn sdf_invert(d: f32) -> f32 {
    return -d;
}

fn sdf_difference(d1: f32, d2: f32) -> f32 {
    return max(d1, -d2);
}

fn sdf_union_smooth(a: f32, b: f32, k_: f32) -> f32 {
    // let k = 1.0 * k_;
    // let r = exp2(-a/k) + exp2(-b/k);
    // return -k*log2(r);

    // let k = 2.0 * k_;
    // let x = b-a;
    // return 0.5*( a+b-sqrt(x*x+k*k) );

    let k = k_ * 1.0/(1.0-sqrt(0.5));
    let h = max( k-abs(a-b), 0.0 )/k;
    return min(a,b) - k*0.5*(1.0+h-sqrt(1.0-h*(h-2.0)));

    // let k = k_ * 1.0/(1.0-sqrt(0.5));
    // return max(k,min(a,b)) - length(max(k-vec2(a,b),vec2(0.0)));
}

fn sdf_smooth_min(dstA: f32, dstB: f32, k: f32) -> f32 {
    let h = max(k - abs(dstA - dstB), 0.0) / k;
    return min(dstA, dstB) - h * h * h * k / 6.0;
}

fn sdf_normal_numerical(p: vec3<f32>) -> vec3<f32> {
    // Small offset value for finite differences
    let eps = 0.001;

    // Sample the SDF at slightly offset positions
    let dx = sdf(vec3<f32>(p.x + eps, p.y, p.z)) - sdf(vec3<f32>(p.x - eps, p.y, p.z));
    let dy = sdf(vec3<f32>(p.x, p.y + eps, p.z)) - sdf(vec3<f32>(p.x, p.y - eps, p.z));
    let dz = sdf(vec3<f32>(p.x, p.y, p.z + eps)) - sdf(vec3<f32>(p.x, p.y, p.z - eps));

    return normalize(vec3<f32>(dx, dy, dz));
}

fn sdf_normal(p: vec3<f32>) -> vec3<f32> {
    // return normalize(sdf_gradient(p));
    return sdf_normal_numerical(p);
}

alias quat = vec4<f32>;

fn quat_transform(q: quat, v: vec3<f32>) -> vec3<f32> {
    let w = q.w;
    let u = q.xyz;
    return v + 2.0 * cross(u, cross(u, v) + w * v);
}

fn sq(x: f32) -> f32 {
    return x * x;
}

fn inf_norm(v: vec3<f32>) -> f32 {
    return max(max(v.x, v.y), v.z);
}

fn select_mat(f: mat3x3<f32>, t: mat3x3<f32>, cond: vec3<bool>) -> mat3x3<f32> {
    let x = select(f[0], t[0], cond[0]);
    let y = select(f[1], t[1], cond[1]);
    let z = select(f[2], t[2], cond[2]);
    return mat3x3(x, y, z);
}

fn diag_mat(v: vec3<f32>) -> mat3x3<f32> {
    return mat3x3(
        vec3(v.x, 0.0, 0.0),
        vec3(0.0, v.y, 0.0),
        vec3(0.0, 0.0, v.z),
    );
}

fn negate_mat(m: mat3x3<f32>) -> mat3x3<f32> {
    return mat3x3(-m[0], -m[1], -m[2]);
}
