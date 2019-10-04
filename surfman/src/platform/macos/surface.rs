//! Surface management for macOS.

use crate::context::ContextID;
use crate::gl::types::{GLenum, GLint, GLuint};
use crate::renderbuffers::Renderbuffers;
use crate::{Error, HiDPIMode, SurfaceID, gl};
use super::context::{Context, GL_FUNCTIONS};
use super::device::Device;

use cocoa::appkit::{NSScreen, NSView as NSViewMethods, NSWindow};
use cocoa::base::{YES, id};
use cocoa::quartzcore::{CALayer, transaction};
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use display_link::macos::cvdisplaylink::{CVDisplayLink, CVTimeStamp, DisplayLink};
use euclid::default::Size2D;
use io_surface::{self, IOSurface, kIOSurfaceBytesPerElement, kIOSurfaceBytesPerRow};
use io_surface::{kIOSurfaceHeight, kIOSurfacePixelFormat, kIOSurfaceWidth};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::mem;
use std::os::raw::c_void;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

#[cfg(feature = "sm-winit")]
use winit::Window;
#[cfg(feature = "sm-winit")]
use winit::os::macos::WindowExt;

const BYTES_PER_PIXEL: i32 = 4;

const SURFACE_GL_TEXTURE_TARGET: GLenum = gl::TEXTURE_RECTANGLE;

const BGRA: i32 = 0x42475241;   // 'BGRA'

#[allow(non_upper_case_globals)]
const kCVReturnSuccess: i32 = 0;

pub struct Surface {
    pub(crate) io_surface: IOSurface,
    pub(crate) size: Size2D<i32>,
    pub(crate) context_id: ContextID,
    pub(crate) framebuffer_object: GLuint,
    pub(crate) texture_object: GLuint,
    pub(crate) renderbuffers: Renderbuffers,
    pub(crate) view_info: Option<ViewInfo>,
}

pub struct SurfaceTexture {
    pub(crate) surface: Surface,
    pub(crate) texture_object: GLuint,
    pub(crate) phantom: PhantomData<*const ()>,
}

unsafe impl Send for Surface {}

impl Debug for Surface {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "Surface({:x})", self.id().0)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if self.framebuffer_object != 0 && !thread::panicking() {
            panic!("Should have destroyed the surface first with `destroy_surface()`!")
        }
    }
}

pub(crate) struct ViewInfo {
    layer: CALayer,
    front_surface: IOSurface,
    display_link: DisplayLink,
    next_vblank: Arc<VblankCond>,
}

struct VblankCond {
    mutex: Mutex<()>,
    cond: Condvar,
}

pub enum SurfaceType {
    Generic { size: Size2D<i32> },
    Widget { native_widget: NativeWidget },
}

pub struct NSView(id);

pub struct NativeWidget {
    pub view: NSView,
    pub hidpi_mode: HiDPIMode,
}

impl Device {
    pub fn create_surface(&mut self, context: &Context, surface_type: &SurfaceType)
                          -> Result<Surface, Error> {
        let _guard = self.temporarily_make_context_current(context);
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                let size = match *surface_type {
                    SurfaceType::Generic { size } => size,
                    SurfaceType::Widget { ref native_widget } => {
                        let window: id = msg_send![native_widget.view.0, window];
                        let mut bounds = native_widget.view.0.bounds();
                        if native_widget.hidpi_mode == HiDPIMode::On {
                            bounds = window.convertRectToBacking(bounds);
                        }
                        Size2D::new(bounds.size.width.round(), bounds.size.height.round()).to_i32()
                    }
                };

                let io_surface = self.create_io_surface(&size);
                let texture_object = self.bind_to_gl_texture(&io_surface, &size);

                let mut framebuffer_object = 0;
                gl.GenFramebuffers(1, &mut framebuffer_object);
                gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer_object);

                gl.FramebufferTexture2D(gl::FRAMEBUFFER,
                                        gl::COLOR_ATTACHMENT0,
                                        SURFACE_GL_TEXTURE_TARGET,
                                        texture_object,
                                        0);

                let context_descriptor = self.context_descriptor(context);
                let context_attributes = self.context_descriptor_attributes(&context_descriptor);

                let renderbuffers = Renderbuffers::new(&size, &context_attributes);
                renderbuffers.bind_to_current_framebuffer();

                debug_assert_eq!(gl.CheckFramebufferStatus(gl::FRAMEBUFFER),
                                 gl::FRAMEBUFFER_COMPLETE);

                let view_info = match *surface_type {
                    SurfaceType::Generic { .. } => None,
                    SurfaceType::Widget { ref native_widget, .. } => {
                        Some(self.create_view_info(&size, native_widget))
                    }
                };

                Ok(Surface {
                    io_surface,
                    size,
                    context_id: context.id,
                    framebuffer_object,
                    texture_object,
                    renderbuffers,
                    view_info,
                })
            }
        })
    }

    unsafe fn create_view_info(&mut self, size: &Size2D<i32>, native_widget: &NativeWidget)
                               -> ViewInfo {
        let front_surface = self.create_io_surface(&size);

        let window: id = msg_send![native_widget.view.0, window];
        let device_description: CFDictionary<CFString, CFNumber> =
            CFDictionary::wrap_under_get_rule(window.screen().deviceDescription() as *const _);
        let description_key: CFString = CFString::from("NSScreenNumber");
        let display_id = device_description.get(description_key).to_i64().unwrap() as u32;
        println!("display_id={}", display_id);
        let mut display_link = DisplayLink::on_display(display_id).unwrap();
        let next_vblank = Arc::new(VblankCond { mutex: Mutex::new(()), cond: Condvar::new() });
        display_link.set_output_callback(display_link_output_callback,
                                         mem::transmute(next_vblank.clone()));
        display_link.start();

        transaction::begin();
        transaction::set_disable_actions(true);

        let layer = CALayer::new();
        layer.set_contents(front_surface.obj as id);
        native_widget.view.0.setLayer(layer.id());
        native_widget.view.0.setWantsLayer(YES);
        layer.set_opaque(true);
        layer.set_contents_opaque(true);

        transaction::commit();

        ViewInfo { layer, front_surface, display_link, next_vblank }
    }

    pub fn create_surface_texture(&self, _: &mut Context, surface: Surface)
                                  -> Result<SurfaceTexture, Error> {
        if surface.view_info.is_some() {
            return Err(Error::WindowAttached);
        }

        let texture_object = self.bind_to_gl_texture(&surface.io_surface, &surface.size);
        Ok(SurfaceTexture {
            surface,
            texture_object,
            phantom: PhantomData,
        })
    }

    fn bind_to_gl_texture(&self, io_surface: &IOSurface, size: &Size2D<i32>) -> GLuint {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                let mut texture = 0;
                gl.GenTextures(1, &mut texture);
                debug_assert_ne!(texture, 0);

                gl.BindTexture(gl::TEXTURE_RECTANGLE, texture);
                io_surface.bind_to_gl_texture(size.width, size.height, true);

                gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                 gl::TEXTURE_MAG_FILTER,
                                 gl::NEAREST as GLint);
                gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                 gl::TEXTURE_MIN_FILTER,
                                 gl::NEAREST as GLint);
                gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                 gl::TEXTURE_WRAP_S,
                                 gl::CLAMP_TO_EDGE as GLint);
                gl.TexParameteri(gl::TEXTURE_RECTANGLE,
                                 gl::TEXTURE_WRAP_T,
                                 gl::CLAMP_TO_EDGE as GLint);

                gl.BindTexture(gl::TEXTURE_RECTANGLE, 0);

                debug_assert_eq!(gl.GetError(), gl::NO_ERROR);

                texture
            }
        })
    }

    pub fn destroy_surface(&self, context: &mut Context, mut surface: Surface)
                           -> Result<(), Error> {
        if context.id != surface.context_id {
            // Leak the surface, and return an error.
            surface.framebuffer_object = 0;
            surface.renderbuffers.leak();
            return Err(Error::IncompatibleSurface)
        }

        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.BindFramebuffer(gl::FRAMEBUFFER, 0);
                gl.DeleteFramebuffers(1, &surface.framebuffer_object);
                surface.framebuffer_object = 0;
                surface.renderbuffers.destroy();
                gl.DeleteTextures(1, &surface.texture_object);
                surface.texture_object = 0;
            }
        });

        Ok(())
    }

    pub fn destroy_surface_texture(&self, _: &mut Context, mut surface_texture: SurfaceTexture)
                                   -> Result<Surface, Error> {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.DeleteTextures(1, &surface_texture.texture_object);
                surface_texture.texture_object = 0;
            }

            Ok(surface_texture.surface)
        })
    }

    #[inline]
    pub fn surface_gl_texture_target(&self) -> GLenum {
        SURFACE_GL_TEXTURE_TARGET
    }

    pub fn present_surface(&self, context: &Context, surface: &mut Surface) -> Result<(), Error> {
        let _guard = self.temporarily_make_context_current(context)?;
        surface.present()
    }

    fn create_io_surface(&self, size: &Size2D<i32>) -> IOSurface {
        unsafe {
            let properties = CFDictionary::from_CFType_pairs(&[
                (CFString::wrap_under_get_rule(kIOSurfaceWidth),
                 CFNumber::from(size.width).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceHeight),
                 CFNumber::from(size.height).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceBytesPerElement),
                 CFNumber::from(BYTES_PER_PIXEL).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfaceBytesPerRow),
                 CFNumber::from(size.width * BYTES_PER_PIXEL).as_CFType()),
                (CFString::wrap_under_get_rule(kIOSurfacePixelFormat),
                 CFNumber::from(BGRA).as_CFType()),
            ]);

            io_surface::new(&properties)
        }
    }
}

impl Surface {
    #[inline]
    pub fn size(&self) -> Size2D<i32> {
        self.size
    }

    #[inline]
    pub fn id(&self) -> SurfaceID {
        SurfaceID(self.io_surface.as_concrete_TypeRef() as usize)
    }

    // Assumes the context is current.
    pub(crate) fn present(&mut self) -> Result<(), Error> {
        GL_FUNCTIONS.with(|gl| {
            unsafe {
                gl.Flush();

                transaction::begin();
                transaction::set_disable_actions(true);

                let view_info = match self.view_info {
                    None => return Err(Error::NoWindowAttached),
                    Some(ref mut view_info) => view_info,
                };
                mem::swap(&mut view_info.front_surface, &mut self.io_surface);
                view_info.layer.set_contents(view_info.front_surface.obj as id);

                transaction::commit();

                let size = self.size;
                gl.BindTexture(gl::TEXTURE_RECTANGLE, self.texture_object);
                self.io_surface.bind_to_gl_texture(size.width, size.height, true);
                gl.BindTexture(gl::TEXTURE_RECTANGLE, 0);

                // Wait for the next swap interval.
                let next_vblank_mutex_guard = view_info.next_vblank.mutex.lock().unwrap();
                drop(view_info.next_vblank.cond.wait(next_vblank_mutex_guard).unwrap());

                Ok(())
            }
        })
    }
}

impl SurfaceTexture {
    #[inline]
    pub fn gl_texture(&self) -> GLuint {
        self.texture_object
    }
}

impl NativeWidget {
    #[cfg(feature = "sm-winit")]
    #[inline]
    pub fn from_winit_window(window: &Window, hidpi_mode: HiDPIMode) -> NativeWidget {
        unsafe {
            NativeWidget {
                view: NSView(msg_send![window.get_nsview() as id, retain]),
                hidpi_mode,
            }
        }
    }
}

impl Drop for NativeWidget {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            msg_send![self.view.0, release];
        }
    }
}

impl Drop for ViewInfo {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // FIXME(pcwalton): When this returns, are there absolutely guaranteed to be no more
            // callbacks? `CVDisplayLinkStop()` documentation doesn't say…
            //
            // If not, then this is a possible use-after-free!
            self.display_link.stop();

            // Drop the reference that the callback was holding onto.
            mem::transmute_copy::<Arc<VblankCond>, Arc<VblankCond>>(&self.next_vblank);
        }
    }
}

unsafe extern "C" fn display_link_output_callback(_: *mut CVDisplayLink,
                                                  _: *const CVTimeStamp,
                                                  _: *const CVTimeStamp,
                                                  _: i64,
                                                  _: *mut i64,
                                                  user_data: *mut c_void)
                                                  -> i32 {
    let next_vblank: Arc<VblankCond> = mem::transmute(user_data);
    {
        let _guard = next_vblank.mutex.lock().unwrap();
        next_vblank.cond.notify_all();
    }

    mem::forget(next_vblank);
    kCVReturnSuccess
}
