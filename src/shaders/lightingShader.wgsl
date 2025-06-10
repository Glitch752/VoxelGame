@group(0) @binding(0)
var normalSampler: sampler;
@group(0) @binding(1)
var normalTexture: texture_2d<f32>;

@group(0) @binding(2)
var colorSampler: sampler;
@group(0) @binding(3)
var colorTexture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
};

@vertex
fn vs_main(
    @builtin(vertex_index) id: u32,
) -> VertexOutput {
    var out: VertexOutput;
	var uv = vec2<f32>((id << 1) & 2, id & 2);
    out.clip_position = vec4<f32>(uv * vec2<f32>(2, -2) + vec2<f32>(-1, 1), 0.0, 1.0);
    return out;
}

struct GBufferOutput {
  @location(0) normal: vec4f, // a: smoothness?
  @location(1) color: vec4f // a: emissive?
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    var input: GBufferOutput;
    input.normal = textureSample(normalTexture, normalSampler, in.clip_position);
    input.color = textureSample(colorTexture, colorSampler, in.clip_position);

    // TODO

    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}