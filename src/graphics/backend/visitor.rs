use std::str;
use std::os::raw::c_void;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use gl;
use gl::types::*;

use super::*;
use super::super::color::Color;
use super::super::pipeline::*;
use super::super::resource::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct VAOPair(GLuint, GLuint);

pub struct OpenGLVisitor {
    cull_face: Cell<CullFace>,
    front_face_order: Cell<FrontFaceOrder>,
    depth_test: Cell<Comparison>,
    depth_write: Cell<bool>,
    depth_write_offset: Cell<Option<(f32, f32)>>,
    color_blend: Cell<Option<(Equation, BlendFactor, BlendFactor)>>,
    color_write: Cell<(bool, bool, bool, bool)>,
    viewport: Cell<((u16, u16), (u16, u16))>,

    active_bufs: RefCell<HashMap<GLenum, GLuint>>,
    active_program: Cell<Option<GLuint>>,
    active_vao: Cell<Option<GLuint>>,
    program_attribute_locations: RefCell<HashMap<GLuint, HashMap<String, GLint>>>,
    program_uniform_locations: RefCell<HashMap<GLuint, HashMap<String, GLint>>>,
    vertex_array_objects: RefCell<HashMap<VAOPair, GLuint>>,
}

impl OpenGLVisitor {
    pub fn new() -> OpenGLVisitor {
        OpenGLVisitor {
            cull_face: Cell::new(CullFace::Back),
            front_face_order: Cell::new(FrontFaceOrder::CounterClockwise),
            depth_test: Cell::new(Comparison::Always),
            depth_write: Cell::new(false),
            depth_write_offset: Cell::new(None),
            color_blend: Cell::new(None),
            color_write: Cell::new((false, false, false, false)),
            viewport: Cell::new(((0, 0), (128, 128))),

            active_bufs: RefCell::new(HashMap::new()),
            active_program: Cell::new(None),
            active_vao: Cell::new(None),
            program_attribute_locations: RefCell::new(HashMap::new()),
            program_uniform_locations: RefCell::new(HashMap::new()),
            vertex_array_objects: RefCell::new(HashMap::new()),
        }
    }

    pub unsafe fn bind_buffer(&self, tp: GLenum, id: GLuint) -> Result<()> {
        assert!(tp == gl::ARRAY_BUFFER || tp == gl::ELEMENT_ARRAY_BUFFER);

        if let Some(record) = self.active_bufs.borrow().get(&tp) {
            if *record == id {
                return Ok(());
            }
        }

        gl::BindBuffer(tp, id);
        self.active_bufs.borrow_mut().insert(tp, id);
        check()
    }

    pub unsafe fn bind_program(&self, id: GLuint) -> Result<()> {
        if let Some(record) = self.active_program.get() {
            if record == id {
                return Ok(());
            }
        }

        gl::UseProgram(id);
        self.active_program.set(Some(id));
        check()
    }

    pub unsafe fn bind_attribute_layout(&self,
                                        attributes: &[(GLint, VertexAttributeDesc)],
                                        layout: &VertexLayout)
                                        -> Result<()> {
        let pid = self.active_program.get().ok_or(ErrorKind::InvalidHandle)?;
        let vid =
            *self.active_bufs.borrow().get(&gl::ARRAY_BUFFER).ok_or(ErrorKind::InvalidHandle)?;

        if let Some(vao) = self.vertex_array_objects.borrow().get(&VAOPair(pid, vid)) {
            if let Some(v) = self.active_vao.get() {
                if *vao == v {
                    return Ok(());
                }
            }

            gl::BindVertexArray(*vao);
            self.active_vao.set(Some(*vao));
            return check();
        }

        let mut vao = 0;
        gl::GenVertexArrays(1, &mut vao);
        gl::BindVertexArray(vao);
        self.active_vao.set(Some(vao));

        for &(location, desc) in attributes {
            if let Some(element) = layout.element(desc.name) {
                if element.format != desc.format || element.size != desc.size {
                    let name: &'static str = desc.name.into();
                    bail!(format!("vertex buffer has incompatible attribute {:?} format.",
                                  name));
                }

                let offset = layout.offset(desc.name)
                    .unwrap() as *const u8 as *const c_void;
                gl::EnableVertexAttribArray(location as GLuint);
                gl::VertexAttribPointer(location as GLuint,
                                        element.size as GLsizei,
                                        element.format.into(),
                                        element.normalized as u8,
                                        layout.stride() as GLsizei,
                                        offset);
            } else {
                let name: &'static str = desc.name.into();
                bail!(format!("can't find attribute {:?} description in vertex buffer.",
                              name));
            }
        }

        check()?;
        self.vertex_array_objects.borrow_mut().insert(VAOPair(pid, vid), vao);
        Ok(())
    }

    pub unsafe fn bind_uniform(&self, location: GLint, variable: &UniformVariable) -> Result<()> {
        match *variable {
            UniformVariable::Vector1(v) => gl::Uniform1f(location, v[0]),
            UniformVariable::Vector2(v) => gl::Uniform2f(location, v[0], v[1]),
            UniformVariable::Vector3(v) => gl::Uniform3f(location, v[0], v[1], v[2]),
            UniformVariable::Vector4(v) => gl::Uniform4f(location, v[0], v[1], v[2], v[3]),
            _ => (),
        }

        check()
    }

    pub unsafe fn get_uniform_location(&self, id: GLuint, name: &str) -> Result<GLint> {
        let mut cache = self.program_uniform_locations.borrow_mut();
        if let Some(uniforms) = cache.get_mut(&id) {
            match uniforms.get(name).map(|v| *v) {
                Some(location) => Ok(location),
                None => {
                    let c_name = ::std::ffi::CString::new(name.as_bytes()).unwrap();
                    let location = gl::GetUniformLocation(id, c_name.as_ptr());
                    check()?;

                    uniforms.insert(name.to_string(), location);
                    Ok(location)
                }
            }
        } else {
            bail!(ErrorKind::InvalidHandle)
        }
    }

    pub unsafe fn get_attribute_location(&self, id: GLuint, name: &str) -> Result<GLint> {
        let mut cache = self.program_uniform_locations.borrow_mut();
        if let Some(attributes) = cache.get_mut(&id) {
            match attributes.get(name).map(|v| *v) {
                Some(location) => Ok(location),
                None => {
                    let c_name = ::std::ffi::CString::new(name.as_bytes()).unwrap();
                    let location = gl::GetAttribLocation(id, c_name.as_ptr());
                    check()?;

                    attributes.insert(name.to_string(), location);
                    Ok(location)
                }
            }
        } else {
            bail!(ErrorKind::InvalidHandle)
        }
    }

    pub unsafe fn clear(&self,
                        color: Option<Color>,
                        depth: Option<f32>,
                        stencil: Option<i32>)
                        -> Result<()> {

        let mut bits = 0;
        if let Some(v) = color {
            bits |= gl::COLOR_BUFFER_BIT;
            gl::ClearColor(v.0, v.1, v.2, v.3);
        }

        if let Some(v) = depth {
            bits |= gl::DEPTH_BUFFER_BIT;
            gl::ClearDepth(v as f64);
        }

        if let Some(v) = stencil {
            bits |= gl::STENCIL_BUFFER_BIT;
            gl::ClearStencil(v);
        }

        gl::Clear(bits);
        check()
    }

    /// Set the viewport relative to the top-lef corner of th window, in pixels.
    pub unsafe fn set_viewport(&self, position: (u16, u16), size: (u16, u16)) -> Result<()> {
        if self.viewport.get().0 != position || self.viewport.get().1 != size {
            gl::Viewport(position.0 as i32,
                         position.1 as i32,
                         size.0 as i32,
                         size.1 as i32);
            self.viewport.set((position, size));
            check()
        } else {
            Ok(())
        }
    }

    /// Specify whether front- or back-facing polygons can be culled.
    pub unsafe fn set_cull_face(&self, face: CullFace) -> Result<()> {
        if self.cull_face.get() != face {
            if face != CullFace::Nothing {
                gl::Enable(gl::CULL_FACE);
                gl::CullFace(match face {
                    CullFace::Front => gl::FRONT,
                    CullFace::Back => gl::BACK,
                    CullFace::Nothing => unreachable!(""),
                });
            } else {
                gl::Disable(gl::CULL_FACE);
            }

            self.cull_face.set(face);
            check()
        } else {
            Ok(())
        }
    }

    /// Define front- and back-facing polygons.
    pub unsafe fn set_front_face_order(&self, front: FrontFaceOrder) -> Result<()> {
        if self.front_face_order.get() != front {
            gl::FrontFace(match front {
                FrontFaceOrder::Clockwise => gl::CW,
                FrontFaceOrder::CounterClockwise => gl::CCW,
            });
            self.front_face_order.set(front);
            check()
        } else {
            Ok(())
        }
    }

    /// Specify the value used for depth buffer comparisons.
    pub unsafe fn set_depth_test(&self, comparsion: Comparison) -> Result<()> {
        if self.depth_test.get() != comparsion {
            if comparsion != Comparison::Always {
                gl::Enable(gl::DEPTH_TEST);
                gl::DepthFunc(comparsion.into());
            } else {
                gl::Disable(gl::DEPTH_TEST);
            }

            self.depth_test.set(comparsion);
            check()
        } else {
            Ok(())
        }
    }

    /// Enable or disable writing into the depth buffer.
    ///
    /// Optional `offset` to address the scale and units used to calculate depth values.
    pub unsafe fn set_depth_write(&self, enable: bool, offset: Option<(f32, f32)>) -> Result<()> {
        if self.depth_write.get() != enable {
            if enable {
                gl::DepthMask(gl::TRUE);
            } else {
                gl::DepthMask(gl::FALSE);
            }
            self.depth_write.set(enable);
        }

        if self.depth_write_offset.get() != offset {
            if let Some(v) = offset {
                if v.0 != 0.0 || v.1 != 0.0 {
                    gl::Enable(gl::POLYGON_OFFSET_FILL);
                    gl::PolygonOffset(v.0, v.1);
                } else {
                    gl::Disable(gl::POLYGON_OFFSET_FILL);
                }
            }
            self.depth_write_offset.set(offset);
        }

        check()
    }

    // Specifies how source and destination are combined.
    pub unsafe fn set_color_blend(&self,
                                  blend: Option<(Equation, BlendFactor, BlendFactor)>)
                                  -> Result<()> {

        if self.color_blend.get() != blend {
            if let Some((equation, src, dst)) = blend {
                if self.color_blend.get() == None {
                    gl::Enable(gl::BLEND);
                }

                gl::BlendFunc(src.into(), dst.into());
                gl::BlendEquation(equation.into());

            } else {
                if self.color_blend.get() != None {
                    gl::Disable(gl::BLEND);
                }
            }

            self.color_blend.set(blend);
            check()
        } else {
            Ok(())
        }
    }

    /// Enable or disable writing color elements into the color buffer.
    pub unsafe fn set_color_write(&self,
                                  red: bool,
                                  green: bool,
                                  blue: bool,
                                  alpha: bool)
                                  -> Result<()> {
        let cw = self.color_write.get();
        if cw.0 != red || cw.1 != green || cw.2 != blue || cw.3 != alpha {

            self.color_write.set((red, green, blue, alpha));
            gl::ColorMask(red as u8, green as u8, blue as u8, alpha as u8);
            check()
        } else {
            Ok(())
        }
    }

    pub unsafe fn create_program(&self, vs: &str, fs: &str) -> Result<GLuint> {
        let vs = self.compile(gl::VERTEX_SHADER, vs)?;
        let fs = self.compile(gl::FRAGMENT_SHADER, fs)?;
        let id = self.link(vs, fs)?;

        gl::DetachShader(id, vs);
        gl::DeleteShader(vs);
        gl::DetachShader(id, fs);
        gl::DeleteShader(fs);

        check()?;

        let mut cache = self.program_uniform_locations.borrow_mut();
        if cache.contains_key(&id) {
            cache.get_mut(&id).unwrap().clear();
        } else {
            cache.insert(id, HashMap::new());
        }

        let mut cache = self.program_attribute_locations.borrow_mut();
        if cache.contains_key(&id) {
            cache.get_mut(&id).unwrap().clear();
        } else {
            cache.insert(id, HashMap::new());
        }

        Ok(id)
    }

    pub unsafe fn delete_program(&self, id: GLuint) -> Result<()> {
        if let Some(v) = self.active_program.get() {
            if v == id {
                self.active_program.set(None);
            }
        }
        gl::DeleteProgram(id);

        let mut cache = self.program_uniform_locations.borrow_mut();
        if let Some(v) = cache.get_mut(&id) {
            v.clear();
        }

        let mut cache = self.program_attribute_locations.borrow_mut();
        if let Some(v) = cache.get_mut(&id) {
            v.clear();
        }

        check()
    }

    pub unsafe fn create_buffer(&self,
                                buf: Resource,
                                hint: ResourceHint,
                                size: u32,
                                data: Option<&[u8]>)
                                -> Result<GLuint> {
        let mut id = 0;
        gl::GenBuffers(1, &mut id);
        if id == 0 {
            bail!("failed to create vertex buffer object.");
        }

        self.bind_buffer(buf.into(), id)?;

        let value = match data {
            Some(v) if v.len() > 0 => ::std::mem::transmute(&v[0]),
            _ => ::std::ptr::null(),
        };

        gl::BufferData(buf.into(), size as isize, value, hint.into());
        check()?;
        Ok(id)
    }

    pub unsafe fn update_buffer(&self,
                                id: GLuint,
                                buf: Resource,
                                offset: u32,
                                data: &[u8])
                                -> Result<()> {
        self.bind_buffer(buf.into(), id)?;
        gl::BufferSubData(buf.into(),
                          offset as isize,
                          data.len() as isize,
                          ::std::mem::transmute(&data[0]));
        check()
    }

    pub unsafe fn delete_buffer(&self, id: GLuint) -> Result<()> {
        gl::DeleteBuffers(1, &id);
        check()
    }

    pub unsafe fn compile(&self, shader: GLenum, src: &str) -> Result<GLuint> {
        let shader = gl::CreateShader(shader);
        // Attempt to compile the shader
        let c_str = ::std::ffi::CString::new(src.as_bytes()).unwrap();
        gl::ShaderSource(shader, 1, &c_str.as_ptr(), ::std::ptr::null());
        gl::CompileShader(shader);

        // Get the compile status
        let mut status = gl::FALSE as GLint;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

        // Fail on error
        if status != (gl::TRUE as GLint) {
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = Vec::with_capacity(len as usize);
            buf.set_len((len as usize) - 1); // subtract 1 to skip the trailing null character
            gl::GetShaderInfoLog(shader,
                                 len,
                                 ::std::ptr::null_mut(),
                                 buf.as_mut_ptr() as *mut GLchar);

            let error = format!("{}. with source:\n{}\n", str::from_utf8(&buf).unwrap(), src);
            bail!(ErrorKind::FailedCompilePipeline(error));
        }
        Ok(shader)
    }

    pub unsafe fn link(&self, vs: GLuint, fs: GLuint) -> Result<GLuint> {
        let program = gl::CreateProgram();
        gl::AttachShader(program, vs);
        gl::AttachShader(program, fs);

        gl::LinkProgram(program);
        // Get the link status
        let mut status = gl::FALSE as GLint;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);

        // Fail on error
        if status != (gl::TRUE as GLint) {
            let mut len: GLint = 0;
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = Vec::with_capacity(len as usize);
            buf.set_len((len as usize) - 1); // subtract 1 to skip the trailing null character
            gl::GetProgramInfoLog(program,
                                  len,
                                  ::std::ptr::null_mut(),
                                  buf.as_mut_ptr() as *mut GLchar);

            let error = format!("{}. ", str::from_utf8(&buf).unwrap());
            bail!(ErrorKind::FailedCompilePipeline(error));
        }
        Ok(program)
    }
}

pub unsafe fn check() -> Result<()> {
    match gl::GetError() {
        gl::NO_ERROR => Ok(()),
        gl::INVALID_ENUM => Err(ErrorKind::InvalidEnum.into()),
        gl::INVALID_VALUE => Err(ErrorKind::InvalidValue.into()),
        gl::INVALID_OPERATION => Err(ErrorKind::InvalidOperation.into()),
        gl::INVALID_FRAMEBUFFER_OPERATION => Err(ErrorKind::InvalidFramebufferOperation.into()),
        gl::OUT_OF_MEMORY => Err(ErrorKind::OutOfBounds.into()),
        _ => Err(ErrorKind::Unknown.into()),
    }
}

impl From<ResourceHint> for GLenum {
    fn from(hint: ResourceHint) -> Self {
        match hint {
            ResourceHint::Static => gl::STATIC_DRAW,
            ResourceHint::Dynamic => gl::DYNAMIC_DRAW,
        }
    }
}

impl From<Resource> for GLuint {
    fn from(res: Resource) -> Self {
        match res {
            Resource::Vertex => gl::ARRAY_BUFFER,
            Resource::Index => gl::ELEMENT_ARRAY_BUFFER,
        }
    }
}

impl From<Comparison> for GLenum {
    fn from(cmp: Comparison) -> Self {
        match cmp {
            Comparison::Never => gl::NEVER,
            Comparison::Less => gl::LESS,
            Comparison::LessOrEqual => gl::LEQUAL,
            Comparison::Greater => gl::GREATER,
            Comparison::GreaterOrEqual => gl::GEQUAL,
            Comparison::Equal => gl::EQUAL,
            Comparison::NotEqual => gl::NOTEQUAL,
            Comparison::Always => gl::ALWAYS,
        }
    }
}

impl From<Equation> for GLenum {
    fn from(eq: Equation) -> Self {
        match eq {
            Equation::Add => gl::FUNC_ADD,
            Equation::Subtract => gl::FUNC_SUBTRACT,
            Equation::ReverseSubtract => gl::FUNC_REVERSE_SUBTRACT,
        }
    }
}

impl From<BlendFactor> for GLenum {
    fn from(factor: BlendFactor) -> Self {
        match factor {
            BlendFactor::Zero => gl::ZERO,
            BlendFactor::One => gl::ONE,
            BlendFactor::Value(BlendValue::SourceColor) => gl::SRC_COLOR,
            BlendFactor::Value(BlendValue::SourceAlpha) => gl::SRC_ALPHA,
            BlendFactor::Value(BlendValue::DestinationColor) => gl::DST_COLOR,
            BlendFactor::Value(BlendValue::DestinationAlpha) => gl::DST_ALPHA,
            BlendFactor::OneMinusValue(BlendValue::SourceColor) => gl::ONE_MINUS_SRC_COLOR,
            BlendFactor::OneMinusValue(BlendValue::SourceAlpha) => gl::ONE_MINUS_SRC_ALPHA,
            BlendFactor::OneMinusValue(BlendValue::DestinationColor) => gl::ONE_MINUS_DST_COLOR,
            BlendFactor::OneMinusValue(BlendValue::DestinationAlpha) => gl::ONE_MINUS_DST_ALPHA,
        }
    }
}

impl From<VertexFormat> for GLenum {
    fn from(format: VertexFormat) -> Self {
        match format {
            VertexFormat::Byte => gl::BYTE,
            VertexFormat::UByte => gl::UNSIGNED_BYTE,
            VertexFormat::Short => gl::SHORT,
            VertexFormat::UShort => gl::UNSIGNED_SHORT,
            VertexFormat::Fixed => gl::FIXED,
            VertexFormat::Float => gl::FLOAT,
        }
    }
}

impl From<Primitive> for GLenum {
    fn from(primitive: Primitive) -> Self {
        match primitive {
            Primitive::Points => gl::POINTS,
            Primitive::Lines => gl::LINES,
            Primitive::LineLoop => gl::LINE_LOOP,
            Primitive::LineStrip => gl::LINE_STRIP,
            Primitive::Triangles => gl::TRIANGLES,
            Primitive::TriangleFan => gl::TRIANGLE_FAN,
            Primitive::TriangleStrip => gl::TRIANGLE_STRIP,
        }
    }
}

impl From<IndexFormat> for GLenum {
    fn from(format: IndexFormat) -> Self {
        match format {
            IndexFormat::UByte => gl::UNSIGNED_BYTE,
            IndexFormat::UShort => gl::UNSIGNED_SHORT,
        }
    }
}