//! Various errors that methods can produce.

#[derive(Debug)]
pub enum Error {
    /// The method failed for a miscellaneous reason.
    Failed,
    /// The platform doesn't support this method.
    UnsupportedOnThisPlatform,
    /// The platform supports this method in theory, but the functionality isn't implemented yet.
    Unimplemented,
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
    /// An extension necessary for this library to function isn't supported.
    RequiredExtensionUnavailable,
    /// Looking up an OpenGL function address failed.
    GLFunctionNotFound,
    /// This context renders to an externally-managed render target.
    ExternalRenderTarget,
    /// No suitable adapter could be found.
    NoAdapterFound,
    /// The device couldn't be opened.
    DeviceOpenFailed,
    /// The system couldn't create a surface.
    SurfaceCreationFailed(WindowingApiError),
    /// The system couldn't import a surface from another thread.
    SurfaceImportFailed(WindowingApiError),
    /// The system couldn't create a surface texture from a surface.
    SurfaceTextureCreationFailed(WindowingApiError),
    /// A context couldn't be created because there was no current context.
    NoCurrentContext,
    /// The surface was not created from this context.
    IncompatibleSurface,
    /// The context descriptor is from a hardware device, but this is a software device, or vice
    /// versa.
    IncompatibleContextDescriptor,
    /// The context is from a hardware device, but this is a software device, or vice versa.
    IncompatibleContext,
    /// The surface texture is from a hardware device, but this is a software device, or vice
    /// versa.
    IncompatibleSurfaceTexture,
    /// There is no hardware adapter available with this backend.
    NoHardwareAdapters,
    /// There is no software adapter available with this backend.
    NoSoftwareAdapters,
    /// The surface has no window attachment.
    NoWidgetAttached,
    /// The surface has a window attachement.
    WidgetAttached,
    /// The native widget is invalid.
    InvalidNativeWidget,
    /// The surface was not created with the `CPU_READ_WRITE` flag, so it cannot be accessed from
    /// the CPU.
    SurfaceDataInaccessible,
    /// The surface could not be locked for CPU reading due to an OS error.
    SurfaceLockFailed,
    /// A connection to the display server could not be opened.
    ConnectionFailed,
    /// A connection to the window server is required to open a hardware device.
    ConnectionRequired,
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
    /// X11: Attribute to get is bad.
	BadAttribute,
    /// CGL: Invalid renderer property.
	BadProperty,
    /// CGL: Invalid pixel format object.
	BadPixelFormat,
    /// CGL: Invalid renderer information object.
	BadRendererInfo,
    /// CGL: Invalid context object.
    /// EGL: An EGLContext argument does not name a valid EGL rendering context.
    /// X11: The context is invalid.
	BadContext,
    /// Invalid drawable.
	BadDrawable,
    /// CGL: Invalid display.
    /// EGL: An EGLDisplay argument does not name a valid EGL display connection. 
	BadDisplay,
    /// CGL: Invalid context state.
	BadState,
    /// CGL: Invalid numerical value.
    /// X11: Invalid value.
    /// GL: Given when a value parameter is not a legal value for that function.
	BadValue,
    /// CGL: Invalid share context.
    /// EGL: Arguments are inconsistent (for example, a valid context requires
    /// buffers not supplied by a valid surface). 
	BadMatch,
    /// CGL: Invalid enumerant (constant).
    /// X11: Invalid enum value.
    /// GL: Given when an enumeration parameter is not a legal enumeration for that function.
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
    /// X11: Screen number is bad.
    BadScreen,
    /// X11: The GLX extension is unavailable on the server.
    NoExtension,
    /// X11: Visual number not known by GLX.
    BadVisual,
    /// GL: Given when the set of state for a command is not legal for the parameters given to that
    /// command.
    BadOperation,
}
