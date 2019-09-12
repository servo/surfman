//! Various errors that methods can produce.

#[derive(Debug)]
pub enum Error {
    /// The method failed for a miscellaneous reason.
    Failed,
    /// The platform doesn't support this method.
    UnsupportedOnThisPlatform,
    /// The system doesn't support the requested OpenGL API type (OpenGL or OpenGL ES).
    UnsupportedGLType,
    /// Choosing an OpenGL pixel format failed.
    PixelFormatSelectionFailed(WindowingApiError),
    /// The system couldn't choose an OpenGL pixel format.
    NoPixelFormatFound,
    /// The system couldn't create an OpenGL context.
    ContextCreationFailed(WindowingApiError),
    /// The system couldn't destroy the OpenGL context.
    ContextDestructionFailed(WindowingApiError),
    /// The system couldn't make the OpenGL context current or not current.
    MakeCurrentFailed(WindowingApiError),
    /// The system OpenGL library couldn't be located.
    NoGLLibraryFound,
    /// Looking up an OpenGL function address failed.
    GLFunctionNotFound,
    /// This context renders to an externally-managed render target.
    ExternalRenderTarget,
    /// No suitable adapter could be found.
    NoAdapterFound,
    /// The device couldn't be opened.
    DeviceOpenFailed,
    /// An attempt was made to attach a surface to a context, but the surface was not created from
    /// that context.
    IncompatibleSurface,
}

/// Abstraction of the errors that EGL, CGL, GLX, CGL, etc. return.
///
/// They all tend to follow similar patterns.
#[derive(Clone, Copy, Debug)]
pub enum WindowingApiError {
    /// Miscellaneous error.
    Failed,
    /// CGL: Invalid pixel format attribute.
    /// EGL: An unrecognized attribute or attribute value was passed in the attribute list.
	BadAttribute,
    /// CGL: Invalid renderer property.
	BadProperty,
    /// CGL: Invalid pixel format object.
	BadPixelFormat,
    /// CGL: Invalid renderer information object.
	BadRendererInfo,
    /// CGL: Invalid context object.
    /// EGL: An EGLContext argument does not name a valid EGL rendering context. 
	BadContext,
    /// Invalid drawable.
	BadDrawable,
    /// CGL: Invalid display.
    /// EGL: An EGLDisplay argument does not name a valid EGL display connection. 
	BadDisplay,
    /// CGL: Invalid context state.
	BadState,
    /// CGL: Invalid numerical value.
	BadValue,
    /// CGL: Invalid share context.
    /// EGL: Arguments are inconsistent (for example, a valid context requires
    /// buffers not supplied by a valid surface). 
	BadMatch,
    /// CGL: Invalid enumerant (constant).
	BadEnumeration,
    /// CGL: Invalid off-screen drawable.
	BadOffScreen,
    /// CGL: Invalid full-screen drawable.
	BadFullScreen,
    /// CGL: Invalid window.
	BadWindow,
    /// CGL: Invalid address; e.g. null pointer passed to function requiring
    /// a non-null pointer argument.
	BadAddress,
    /// CGL: Invalid code module.
	BadCodeModule,
    /// CGL: Invalid memory allocation; i.e. CGL couldn't allocate memory.
    /// EGL: EGL failed to allocate resources for the requested operation.
	BadAlloc,
    /// CGL: Invalid Core Graphics connection.
	BadConnection,
    /// EGL: EGL is not initialized, or could not be initialized, for the
    /// specified EGL display connection. 
    NotInitialized,
    /// EGL: EGL cannot access a requested resource (for example a context is
    /// bound in another thread). 
    BadAccess,
    /// EGL: The current surface of the calling thread is a window, pixel
    /// buffer or pixmap that is no longer valid. 
    BadCurrentSurface,
    /// EGL: An EGLSurface argument does not name a valid surface (window,
    /// pixel buffer or pixmap) configured for GL rendering. 
    BadSurface,
    /// EGL: One or more argument values are invalid.
    BadParameter,
    /// EGL: A NativePixmapType argument does not refer to a valid native
    /// pixmap.
    BadNativePixmap,
    /// EGL: A NativeWindowType argument does not refer to a valid native
    /// window.
    BadNativeWindow,
    /// EGL: A power management event has occurred. The application must
    /// destroy all contexts and reinitialise OpenGL ES state and objects to
    /// continue rendering. 
    ContextLost,
}
