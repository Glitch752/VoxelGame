struct CameraUniform {
    view_proj: mat4x4f,
};
@group(0) @binding(0) 
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3f,
    @location(1) color: vec3f,
    @location(2) normal: vec3f
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) color: vec3f,
    @location(1) normal: vec3f // world-space normal
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color + model.position;
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    out.normal = model.normal;
    return out;
}

struct GBufferOutput {
  @location(0) normal: vec4f, // a: smoothness?
  @location(1) color: vec4f // a: emissive?
}

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
    var output: GBufferOutput;
    output.normal = vec4(normalize(in.normal), 1.0);
    output.color = vec4(in.color, 1.0);

    return output;
}