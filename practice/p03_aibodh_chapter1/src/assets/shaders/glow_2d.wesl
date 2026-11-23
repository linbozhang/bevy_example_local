import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;
import super::glow::glow;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> color: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    return glow(mesh.uv, color);
}
