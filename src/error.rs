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
    /// No surface was attached to this context.
    NoSurfaceAttached,
    /// This context renders to a window (not a surface).
    WindowAttached,
    /// No suitable adapter could be found.
    NoAdapterFound,
    /// The device couldn't be opened.
    DeviceOpenFailed,
}

/// Abstraction of the errors that EGL, CGL, GLX, CGL, etc. return.
///
/// They all tend to follow the same pattern.
#[derive(Clone, Copy, Debug)]
pub enum WindowingApiError {
    /// Miscellaneous error.
    Failed,
    /// Invalid pixel format attribute.
	BadAttribute,
    /// Invalid renderer property.
	BadProperty,
    /// Invalid pixel format object.
	BadPixelFormat,
    /// Invalid renderer information object.
	BadRendererInfo,
    /// Invalid context object.
	BadContext,
    /// Invalid drawable.
	BadDrawable,
    /// Invalid display.
	BadDisplay,
    /// Invalid context state.
	BadState,
    /// Invalid numerical value.
	BadValue,
    /// Invalid share context.
	BadMatch,
    /// Invalid enumerant (constant).
	BadEnumeration,
    /// Invalid off-screen drawable.
	BadOffScreen,
    /// Invalid full-screen drawable.
	BadFullScreen,
    /// Invalid window.
	BadWindow,
    /// Invalid address; e.g. null pointer passed to function requiring a non-null pointer
    /// argument.
	BadAddress,
    /// Invalid code module.
	BadCodeModule,
    /// Invalid memory allocation; i.e. CGL couldn't allocate memory.
	BadAlloc,
    /// Invalid Core Graphics connection.
	BadConnection,
}
