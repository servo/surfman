use geom::Size2D;

pub struct DrawBuffer {
    size: Size2D<i32>,
    stencil_render_buffer: GLUint,
    depth_render_buffer: GLUint,
    color_render_buffer: GLUint,
    samples: GLSizei,
    framebuffer: GLUint
}


impl DrawBuffer {
    fn new(context: &GLContext, size: Size2D<i32>, attrs: GLContextAttributes) -> Result<DrawBuffer, &'static str> {
        if attrs.antialias {
            if context.capabilities.max_samples == 0 {
                return Err("Multisample antialising not supported");
            }
        }

        return Err("Not yet implemented");
    }
}
