use layers::platform::surface::{NativeSurface, NativeGraphicsMetadata, NativePaintingGraphicsContext, NativeCompositingGraphicsContext};
use layers::texturegl::Texture;
use geom::Size2D;

/// A surface wrapper that owns the surface,
/// and thus destroys it on drop
/// We need a graphics context to create the surface
/// And a compositing context to bind it to a texture
///
/// Note that the GraphicsContext/CompositingContext
/// structs are not really GL context, just metadata
pub struct LayersSurfaceWrapper {
    graphics_context: NativePaintingGraphicsContext,
    compositing_context: NativeCompositingGraphicsContext,
    surface: NativeSurface,
    size: Size2D<i32>,
}

#[cfg(target_os="linux")]
#[inline(always)]
fn create_compositing_context(metadata: &NativeGraphicsMetadata) -> NativeCompositingGraphicsContext {
    NativeCompositingGraphicsContext::from_display(metadata.display)
}

#[cfg(not(target_os="linux"))]
#[inline(always)]
fn create_compositing_context(_: &NativeGraphicsMetadata) -> NativeCompositingGraphicsContext {
    NativeCompositingGraphicsContext::new()
}

impl LayersSurfaceWrapper {
    pub fn new(metadata: NativeGraphicsMetadata, size: Size2D<i32>, stride: i32) -> LayersSurfaceWrapper {
        let graphics_ctx = NativePaintingGraphicsContext::from_metadata(&metadata);

        let compositing_ctx = create_compositing_context(&metadata);

        // TODO(ecoal95): Check if size.width is the stride we must use
        let mut surf = NativeSurface::new(&graphics_ctx, size, stride);
        surf.mark_will_leak();

        LayersSurfaceWrapper {
            graphics_context: graphics_ctx,
            compositing_context: compositing_ctx,
            surface: surf,
            size: size,
        }
    }

    pub fn bind_to_texture(&self, texture: &Texture) {
        let size = Size2D::new(self.size.width as isize, self.size.height as isize);
        self.surface.bind_to_texture(&self.compositing_context, texture, size)
    }

    pub fn borrow_surface(&self) -> &NativeSurface {
        &self.surface 
    }

    pub fn get_surface_id(&self) -> isize {
        self.surface.get_id()
    }
}

impl Drop for LayersSurfaceWrapper {
    fn drop(&mut self) {
        self.surface.destroy(&self.graphics_context);
    }
}
