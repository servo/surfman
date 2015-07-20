use layers::platform::surface::{NativeSurface, NativeDisplay};
use layers::texturegl::Texture;
use euclid::Size2D;

/// A surface wrapper that owns the surface,
/// and thus destroys it on drop. We need a display
/// to create the surface and to bind it to a texture.
///
/// Note that the GraphicsContext/CompositingContext
/// structs are not really GL context, just metadata
pub struct LayersSurfaceWrapper {
    display: NativeDisplay,
    surface: NativeSurface,
}

impl LayersSurfaceWrapper {
    pub fn new(display: NativeDisplay, size: Size2D<i32>) -> LayersSurfaceWrapper {
        let mut surf = NativeSurface::new(&display, size);
        surf.mark_will_leak();

        LayersSurfaceWrapper {
            display: display,
            surface: surf,
        }
    }

    pub fn bind_to_texture(&self, texture: &Texture) {
        self.surface.bind_to_texture(&self.display, texture)
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
        self.surface.destroy(&self.display);
    }
}
