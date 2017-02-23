use std::marker::PhantomData;
use std::borrow::Borrow;
use std::str;
use std::slice;
use std::mem;

use super::*;
use super::resource::{ResourceHint, IndexFormat, VertexLayout, VertexAttributeDesc, MAX_ATTRIBUTES};
use super::pipeline::{UniformVariable, Primitive};
use super::backend::Context;

#[derive(Debug, Clone, Copy)]
pub enum PreFrameTask {
    CreateView(ViewHandle, TaskBufferPtr<ViewDescriptor>),
    UpdateViewRect(ViewHandle, (u16, u16), (u16, u16)),
    UpdateViewScissor(ViewHandle, (u16, u16), (u16, u16)),
    UpdateViewClear(ViewHandle, Option<u32>, Option<f32>, Option<i32>),

    CreatePipeline(PipelineHandle, TaskBufferPtr<PipelineDescriptor>),
    UpdatePipelineState(PipelineHandle, TaskBufferPtr<RenderState>),
    UpdatePipelineUniform(PipelineHandle, TaskBufferPtr<str>, TaskBufferPtr<UniformVariable>),

    CreateVertexBuffer(VertexBufferHandle, TaskBufferPtr<VertexBufferDescriptor>),
    UpdateVertexBuffer(VertexBufferHandle, u32, TaskBufferPtr<[u8]>),

    CreateIndexBuffer(IndexBufferHandle, TaskBufferPtr<IndexBufferDescriptor>),
    UpdateIndexBuffer(IndexBufferHandle, u32, TaskBufferPtr<[u8]>),
}

#[derive(Debug, Clone, Copy)]
pub struct FrameTask {
    view: ViewHandle,
    pipeline: PipelineHandle,
    vb: VertexBufferHandle,
    ib: Option<IndexBufferHandle>,
    primitive: Primitive,
    from: u32,
    len: u32,
    uniforms: TaskBufferPtr<[(TaskBufferPtr<str>, UniformVariable)]>,
}

#[derive(Debug, Clone, Copy)]
pub enum PostFrameTask {
    DeleteView(ViewHandle),
    DeletePipeline(PipelineHandle),
    DeleteVertexBuffer(VertexBufferHandle),
    DeleteIndexBuffer(IndexBufferHandle),
}

#[derive(Debug, Clone, Copy)]
pub struct ViewDescriptor {
    clear_color: Option<u32>,
    clear_depth: Option<f32>,
    clear_stencil: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
pub struct PipelineDescriptor {
    vs: TaskBufferPtr<str>,
    fs: TaskBufferPtr<str>,
    state: RenderState,
    attributes: (u8, [VertexAttributeDesc; MAX_ATTRIBUTES]),
}

#[derive(Debug, Clone, Copy)]
pub struct VertexBufferDescriptor {
    layout: VertexLayout,
    hint: ResourceHint,
    size: u32,
    data: Option<TaskBufferPtr<[u8]>>,
}

#[derive(Debug, Clone, Copy)]
pub struct IndexBufferDescriptor {
    format: IndexFormat,
    hint: ResourceHint,
    size: u32,
    data: Option<TaskBufferPtr<[u8]>>,
}

pub struct Frame {
    pub pre: Vec<PreFrameTask>,
    pub drawcalls: Vec<FrameTask>,
    pub post: Vec<PostFrameTask>,
    pub buf: TaskBuffer,
}

impl Frame {
    /// Creates a new frame with specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Frame {
            pre: Vec::with_capacity(capacity),
            post: Vec::with_capacity(capacity),
            drawcalls: Vec::with_capacity(capacity),
            buf: TaskBuffer::with_capacity(capacity),
        }
    }

    pub unsafe fn clear(&mut self) {
        self.pre.clear();
        self.drawcalls.clear();
        self.post.clear();
        self.buf.clear();
    }

    pub unsafe fn dispatch(&mut self, context: &mut Context) {
        let mut device = &mut context.device();

        for v in &self.pre {
            match *v {
                PreFrameTask::CreateView(handle, desc) => {
                    let desc = &self.buf.as_ref(desc);
                    device.create_view(handle, desc.clear_color, desc.clear_depth, desc.clear_stencil).unwrap();
                },
                PreFrameTask::UpdateViewRect(handle, position, size) => {
                    device.update_view_rect(handle, position, size).unwrap();
                },
                PreFrameTask::UpdateViewScissor(handle, position, size) => {
                    device.update_view_scissor(handle, position, size).unwrap();
                },
                PreFrameTask::UpdateViewClear(handle, clear_color, clear_depth, clear_stencil) => {
                    device.update_view_clear(handle, clear_color, clear_depth, clear_stencil).unwrap();
                },
                PreFrameTask::CreatePipeline(handle, desc) => {
                    let desc = &self.buf.as_ref(desc);
                    device.create_pipeline(handle, &desc.state, self.buf.as_str(desc.vs), self.buf.as_str(desc.fs), desc.attributes).unwrap();
                },
                PreFrameTask::UpdatePipelineState(handle, state) => {
                    let state = &self.buf.as_ref(state);
                    device.update_pipeline_state(handle, &state).unwrap();
                },
                PreFrameTask::UpdatePipelineUniform(handle, name, variable) => {
                    let name = &self.buf.as_str(name);
                    let variable = &self.buf.as_ref(variable);
                    device.update_pipeline_uniform(handle, name, &variable).unwrap();
                },
                PreFrameTask::CreateVertexBuffer(handle, desc) => {
                    let desc = &self.buf.as_ref(desc);
                    let data = desc.data.map(|ptr| self.buf.as_bytes(ptr));
                    device.create_vertex_buffer(handle, &desc.layout, desc.hint, desc.size, data).unwrap();
                },
                PreFrameTask::UpdateVertexBuffer(handle, offset, data) => {
                    let data = &self.buf.as_bytes(data);
                    device.update_vertex_buffer(handle, offset, &data).unwrap();
                },
                PreFrameTask::CreateIndexBuffer(handle, desc) => {
                    let desc = &self.buf.as_ref(desc);
                    let data = desc.data.map(|ptr| self.buf.as_bytes(ptr));
                    device.create_index_buffer(handle, desc.format, desc.hint, desc.size, data).unwrap();
                },
                PreFrameTask::UpdateIndexBuffer(handle, offset, data) => {
                    let data = &self.buf.as_bytes(data);
                    device.update_index_buffer(handle, offset, &data).unwrap();
                },
            }
        }

        {
            let mut uniforms = vec![];
            self.drawcalls.sort_by_key(|dc| dc.view);

            for dc in &self.drawcalls {
                uniforms.clear();
                for &(name, variable) in self.buf.as_slice(dc.uniforms) {
                    let name = self.buf.as_str(name);
                    uniforms.push((name, variable));
                }

                device.bind_view(dc.view).unwrap();
                device.draw(dc.primitive, dc.pipeline, dc.vb, dc.ib, dc.from, dc.len, uniforms.as_slice()).unwrap();
            }
        }

        for v in &self.post {
            match *v {
                PostFrameTask::DeleteView(handle) => {
                    device.delete_view(handle).unwrap();
                },
                PostFrameTask::DeletePipeline(handle) => {
                    device.delete_pipeline(handle).unwrap();
                },
                PostFrameTask::DeleteVertexBuffer(handle) => {
                    device.delete_vertex_buffer(handle).unwrap();
                },
                PostFrameTask::DeleteIndexBuffer(handle) => {
                    device.delete_index_buffer(handle).unwrap();
                }
            }
        }
    }
}

/// Where we store all the intermediate bytes.
pub struct TaskBuffer(Vec<u8>);

impl TaskBuffer {
    /// Creates a new task buffer with specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        TaskBuffer(Vec::with_capacity(capacity))
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn extend<T>(&mut self, value: &T) -> TaskBufferPtr<T> where T: Copy {
        let data = unsafe {
            slice::from_raw_parts(value as *const T as *const u8, mem::size_of::<T>())  
        };

        self.0.extend_from_slice(data);
        TaskBufferPtr {
            position: (self.0.len() - data.len()) as u32,
            size: data.len() as u32,
            _phantom: PhantomData,
        }
    }

    /// Clones and appends all elements in a slice to the buffer.
    pub fn extend_from_slice<T>(&mut self, slice: &[T]) -> TaskBufferPtr<[T]>
        where T: Copy
    {
        let len = mem::size_of::<T>().wrapping_mul(slice.len());
        let u8_slice = unsafe { slice::from_raw_parts(slice.as_ptr() as *const u8, len) };
        self.0.extend_from_slice(u8_slice);
        TaskBufferPtr {
            position: (self.0.len() - len) as u32,
            size: len as u32,
            _phantom: PhantomData,
        }
    }

    /// Clones and append all bytes in a string slice to the buffer.
    pub fn extend_from_str<T>(&mut self, value: T) -> TaskBufferPtr<str> where T: Borrow<str>
    {
        let slice = self.extend_from_slice(value.borrow().as_bytes());
        TaskBufferPtr {
            position: slice.position,
            size: slice.size,
            _phantom: PhantomData,
        }
    }

    /// Returns reference to object indicated by `TaskBufferPtr`.
    #[inline]
    pub fn as_ref<T>(&self, ptr: TaskBufferPtr<T>) -> &T
        where T: Copy
    {
        let slice = self.as_bytes(ptr);
        assert_eq!(slice.len(), mem::size_of::<T>());
        unsafe { &*(slice.as_ptr() as *const _) }
    }

    /// Returns a object slice indicated by `TaskBufferPtr.
    #[inline]
    pub fn as_slice<T>(&self, ptr: TaskBufferPtr<[T]>) -> &[T]
        where T: Copy
    {
        let slice = self.as_bytes(ptr);
        let len = slice.len() / mem::size_of::<T>();
        assert_eq!(slice.len(), mem::size_of::<T>().wrapping_mul(len));
        unsafe { slice::from_raw_parts(slice.as_ptr() as *const T, len) }
    }

    /// Returns string slice indicated by `TaskBufferPtr`.
    #[inline]
    pub fn as_str(&self, ptr: TaskBufferPtr<str>) -> &str {
        str::from_utf8(self.as_bytes(ptr)).unwrap()
    }

    #[inline]
    pub fn as_bytes<T>(&self, slice:TaskBufferPtr<T>) -> &[u8] where T: ?Sized {
        &self.0[slice.position as usize..(slice.position + slice.size) as usize]
    }
}

/// A view into our `DataBuffer`, indicates where the object `T` stored.
#[derive(Debug)]
pub struct TaskBufferPtr<T> where T: ?Sized {
    position: u32,
    size: u32,
    _phantom: PhantomData<T>,
}

impl<T> Clone for TaskBufferPtr<T> where T: ?Sized {
    fn clone(&self) -> Self {
        TaskBufferPtr {
            position: self.position,
            size: self.size,
            _phantom: PhantomData,
        }
    }
}

impl<T> Copy for TaskBufferPtr<T> where T: ?Sized {}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
    struct UpdateViewRect {
        position: (u16, u16),
        size: (u16, u16),
    }

    #[test]
    fn buf() {
        let mut buffer = TaskBuffer::with_capacity(128);

        let mut uvp = UpdateViewRect::default();
        uvp.position = (256, 128);
        let slice_uvp = buffer.extend(&uvp);

        let int = 128 as u32;
        let slice_int = buffer.extend(&int);

        assert_eq!(*buffer.as_ref(slice_int), int);
        assert_eq!(*buffer.as_ref(slice_uvp), uvp);

        let arr = [1, 2, 3];
        let slice_arr = buffer.extend(&arr);
        assert_eq!(*buffer.as_ref(slice_arr), arr);

        let slice_arr_1_2 = buffer.extend_from_slice(&arr[0..2]);
        assert_eq!(buffer.as_slice(slice_arr_1_2), &arr[0..2]);

        let text = "string serialization";
        let slice_text = buffer.extend_from_str(text);
        assert_eq!(text, buffer.as_str(slice_text));
    }
}