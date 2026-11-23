fn glow(uv: vec2<f32>, color: vec4<f32>) -> vec4<f32> {
    let local = uv * 2.0 - vec2<f32>(1.0);
    // A slightly stretched core makes the particles' spin visible.
    let radial = 1.0 - smoothstep(0.0, 1.0, length(local));
    let core = 1.0 - smoothstep(0.0, 0.65, length(local * vec2<f32>(1.65, 1.0)));
    let intensity = (0.60 * radial + 0.65 * core * core) * color.a;
    // Premultiplied RGB and zero output alpha also make Bevy's 3D
    // premultiplied transparent pipeline preserve the destination color.
    return vec4<f32>(color.rgb * intensity, 0.0);
}
