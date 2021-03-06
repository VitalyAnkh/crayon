//! The backend of renderer, which should be responsible for only one thing:
//! submitting draw-calls using low-level OpenGL video APIs.

pub mod frame;
pub mod gl;
pub mod headless;

use super::assets::prelude::*;

use errors::*;
use math;
use utils::hash_value;

pub type UniformVar = (hash_value::HashValue<str>, UniformVariable);

pub trait Visitor {
    unsafe fn create_surface(&mut self, handle: SurfaceHandle, params: SurfaceParams)
        -> Result<()>;

    unsafe fn delete_surface(&mut self, handle: SurfaceHandle) -> Result<()>;

    unsafe fn create_shader(
        &mut self,
        handle: ShaderHandle,
        params: ShaderParams,
        vs: &str,
        fs: &str,
    ) -> Result<()>;

    unsafe fn delete_shader(&mut self, handle: ShaderHandle) -> Result<()>;

    unsafe fn create_texture(
        &mut self,
        handle: TextureHandle,
        params: TextureParams,
        bytes: Option<TextureData>,
    ) -> Result<()>;

    unsafe fn update_texture(
        &mut self,
        handle: TextureHandle,
        area: math::Aabb2<u32>,
        bytes: &[u8],
    ) -> Result<()>;

    unsafe fn delete_texture(&mut self, handle: TextureHandle) -> Result<()>;

    unsafe fn create_render_texture(
        &mut self,
        handle: RenderTextureHandle,
        params: RenderTextureParams,
    ) -> Result<()>;

    unsafe fn delete_render_texture(&mut self, handle: RenderTextureHandle) -> Result<()>;

    unsafe fn create_mesh(
        &mut self,
        handle: MeshHandle,
        ps: MeshParams,
        data: Option<MeshData>,
    ) -> Result<()>;

    unsafe fn update_vertex_buffer(
        &mut self,
        handle: MeshHandle,
        o: usize,
        bytes: &[u8],
    ) -> Result<()>;

    unsafe fn update_index_buffer(
        &mut self,
        handle: MeshHandle,
        o: usize,
        bytes: &[u8],
    ) -> Result<()>;

    unsafe fn delete_mesh(&mut self, handle: MeshHandle) -> Result<()>;

    unsafe fn bind(&mut self, surface: SurfaceHandle, dimensions: math::Vector2<u32>)
        -> Result<()>;

    unsafe fn draw(
        &mut self,
        shader: ShaderHandle,
        mesh: MeshHandle,
        mesh_index: MeshIndex,
        vars: &[UniformVar],
    ) -> Result<u32>;

    unsafe fn update_surface_scissor(&mut self, scissor: SurfaceScissor) -> Result<()>;

    unsafe fn update_surface_viewport(&mut self, vp: SurfaceViewport) -> Result<()>;

    /// Blocks until all execution is complete. Such effects include all changes to render state, all
    /// changes to connection state, and all changes to the frame buffer contents.
    unsafe fn flush(&mut self) -> Result<()>;

    /// Advance one frame, it will be called every frames.
    unsafe fn advance(&mut self) -> Result<()>;
}
