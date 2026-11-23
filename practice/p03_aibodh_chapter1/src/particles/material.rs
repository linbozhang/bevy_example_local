use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
        RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPipeline},
};

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(super) struct Glow {
    #[uniform(0)]
    pub color: LinearRgba,
}

impl Material2d for Glow {
    fn fragment_shader() -> ShaderRef {
        "shaders/glow_2d.wesl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        _: &Material2dPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(Some(target)) = descriptor
            .fragment
            .as_mut()
            .and_then(|f| f.targets.first_mut())
        {
            target.blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            });
        }
        Ok(())
    }
}

impl Material for Glow {
    fn fragment_shader() -> ShaderRef {
        "shaders/glow_3d.wesl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}
