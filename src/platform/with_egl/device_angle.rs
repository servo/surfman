//! A wrapper for a Direct3D 11 device and associated EGL display.
//!
//! These are per-thread, because ANGLE is not thread-safe.

pub struct Device {
    egl_device: EGLDeviceEXT,
    pub egl_display: EGLDisplay,
    surfaces: Vec<SurfaceEntry>,
    owned_by_us: bool,
}

lazy_static! {
    static ref eglCreateDeviceANGLE: extern "C" fn(device_type: EGLint,
                                                   native_device: *mut c_void,
                                                   attrib_list: *const EGLAttrib)
                                                   -> EGLDeviceEXT = {
        unsafe {
            static NAME: &'static [u8] = b"eglCreateDeviceANGLE\0";
            let f = egl::GetProcAddress(&NAME[0] as *const u8 as *const c_char);
            assert_ne!(f as usize, 0);
            mem::transmute(f)
        }
    };
    static ref eglQuerySurfacePointerANGLE: extern "C" fn(dpy: EGLDisplay,
                                                          surface: EGLSurface,
                                                          attribute: EGLint,
                                                          value: *mut *mut c_void)
                                                          -> EGLBoolean = {
        unsafe {
            static NAME: &'static [u8] = b"eglQuerySurfacePointerANGLE\0";
            let f = egl::GetProcAddress(&NAME[0] as *const u8 as *const c_char);
            assert_ne!(f as usize, 0);
            mem::transmute(f)
        }
    };
}

impl Device {
    pub fn new() -> Device {
        unsafe {
            let mut d3d11_device = ptr::null_mut();
            let mut d3d11_feature_level = 0;
            let mut d3d11_device_context = ptr::null_mut();
            let result = D3D11CreateDevice(ptr::null_mut(),
                                            D3D_DRIVER_TYPE_HARDWARE,
                                            ptr::null_mut(),
                                            0,
                                            ptr::null_mut(),
                                            0,
                                            D3D11_SDK_VERSION,
                                            &mut d3d11_device,
                                            &mut d3d11_feature_level,
                                            &mut d3d11_device_context);
            assert!(winerror::SUCCEEDED(result));
            debug_assert!(d3d11_feature_level >= D3D_FEATURE_LEVEL_9_3);
            let d3d11_device = ComPtr::from_raw(d3d11_device);
            let d3d11_device_context = ComPtr::from_raw(d3d11_device_context);

            let mut dxgi_device: *mut IDXGIDevice = ptr::null_mut();
            let result = (*d3d11_device).QueryInterface(
                &IDXGIDevice::uuidof(),
                &mut dxgi_device as *mut *mut IDXGIDevice as *mut *mut c_void);
            assert!(winerror::SUCCEEDED(result));
            let dxgi_device = ComPtr::from_raw(dxgi_device);

            let mut dxgi_adapter = ptr::null_mut();
            let result = (*dxgi_device).GetAdapter(&mut dxgi_adapter);
            assert!(winerror::SUCCEEDED(result));
            let dxgi_adapter = ComPtr::from_raw(dxgi_adapter);

            let mut desc = mem::zeroed();
            let result = (*dxgi_adapter).GetDesc(&mut desc);
            assert!(winerror::SUCCEEDED(result));

            println!("Adapter name: {}", String::from_utf16_lossy(&desc.Description));

            let egl_device = (*eglCreateDeviceANGLE)(EGL_D3D11_DEVICE_ANGLE,
                                                        d3d11_device.as_raw() as *mut c_void,
                                                        ptr::null_mut());
            assert_ne!(egl_device, EGL_NO_DEVICE_EXT);

            let attribs = [egl::NONE as EGLAttrib, egl::NONE as EGLAttrib, 0, 0];
            let egl_display = egl::GetPlatformDisplay(EGL_PLATFORM_DEVICE_EXT,
                                                        egl_device as *mut c_void,
                                                        &attribs[0]);
            assert_ne!(egl_display, egl::NO_DISPLAY);

            let (mut major_version, mut minor_version) = (0, 0);
            let result = egl::Initialize(egl_display,
                                            &mut major_version,
                                            &mut minor_version);
            assert_ne!(result, egl::FALSE);

            Device {
                d3d11_device,
                egl_device,
                egl_display,
                surfaces: vec![],
                owned_by_us: true,
            }
        }
    }

    pub(crate) fn init_with_native_display(egl_display: EGLDisplay) {
        DISPLAY.with(|display| {
            let mut display = display.borrow_mut();
            if display.is_some() {
                panic!("Display already initialized for this thread!");
            }

            unsafe {
                // Get the underlying EGL device.
                let mut egl_device = EGL_NO_DEVICE_EXT;
                let result = egl::QueryDisplayAttribEXT(egl_display,
                                                        EGL_DEVICE_EXT,
                                                        &mut egl_device);
                assert_ne!(result, egl::FALSE);

                // Get the underlying D3D device.
                let mut d3d11_device = 0;
                let result = egl::QueryDeviceAttribEXT(egl_device,
                                                       EGL_D3D11_DEVICE_ANGLE,
                                                       &mut d3d11_device);
                assert_ne!(result, egl::FALSE);
                let d3d11_device = ComPtr::new(d3d11_device);

                // Finish up.
                *display = Some(Display {
                    d3d11_device,
                    egl_device,
                    egl_display,
                    surfaces: vec![],
                    owned_by_us: false,
                })
            }
        })
    }

    pub(crate) fn with<F, R>(callback: F) -> R where F: FnOnce(&mut Display) -> R {
        DISPLAY.with(|display| {
            match display.borrow_mut() {
                None => panic!("Display was not initialized yet!"),
                Some(display) => callback(display),
            }
        })
    }

    fn sweep_dead_surfaces(&mut self) {
        let egl_display = self.egl_display;
        self.surfaces.retain(|surface| {
            let dead = surface.handle.upgrade().is_none();
            if dead {
                unsafe {
                    let ok = egl::DestroySurface(egl_display, surface.angle_surface.egl_surface);
                    debug_assert_ne!(ok, egl::FALSE);
                }
            }
            dead
        })
    }

    fn api_to_config(&self, api_type: GlType, api_version: GLVersion) -> EGLConfig {
        let renderable_type = get_pbuffer_renderable_type(api_type, api_version);

        // FIXME(pcwalton): Convert the GL formats to an appropriate set of EGL attributes!
        let pbuffer_attributes = [
            egl::SURFACE_TYPE as EGLint,         egl::PBUFFER_BIT as EGLint,
            egl::RENDERABLE_TYPE as EGLint,      renderable_type as EGLint,
            egl::BIND_TO_TEXTURE_RGBA as EGLint, 1 as EGLint,
            egl::RED_SIZE as EGLint,             8,
            egl::GREEN_SIZE as EGLint,           8,
            egl::BLUE_SIZE as EGLint,            8,
            egl::ALPHA_SIZE as EGLint,           0,
            egl::NONE as EGLint,                 0,
            0,                                   0,
        ];

        unsafe {
            let (mut config, mut configs_found) = (ptr::null(), 0);
            if egl::ChooseConfig(self.egl_display,
                                 pbuffer_attributes.as_ptr(),
                                 &mut config,
                                 1,
                                 &mut configs_found) != egl::TRUE as u32 {
                panic!("Failed to choose an EGL configuration: {:x}!",
                       egl::GetError())
            }

            if configs_found == 0 {
                panic!("No valid EGL configurations found!")
            }

            config
        }
    }

    // TODO(pcwalton): This is O(n) in the number of surfaces. Might be a problem with many
    // surfaces.
    fn get_angle_surface(&mut self, query: &Arc<SurfaceHandle>) -> AngleSurface {
        // Find an existing surface if we have one.
        for surface in &self.surfaces {
            if let Some(handle) = surface.handle.upgrade() {
                if ptr::eq(&**query, &*handle) {
                    return surface.angle_surface.clone();
                }
            }
        }

        // We don't have an EGL surface yet. Create one from the D3D handle.
        let egl_config = self.api_to_config(query.api_type, query.api_version);
        let attributes = [
            egl::WIDTH as EGLint,          query.size.width,
            egl::HEIGHT as EGLint,         query.size.height,
            egl::TEXTURE_FORMAT as EGLint, egl::TEXTURE_RGBA as EGLint,
            egl::TEXTURE_TARGET as EGLint, egl::TEXTURE_2D as EGLint,
            egl::NONE as EGLint,           egl::NONE as EGLint,
            0,                             0,
        ];
        let egl_surface = unsafe {
            egl::CreatePbufferFromClientBuffer(self.egl_display,
                                               EGL_D3D_TEXTURE_2D_SHARE_HANDLE_ANGLE,
                                               query.share_handle,
                                               egl_config,
                                               attributes.as_ptr())
        };
        if egl_surface == egl::NO_SURFACE {
            unsafe {
                panic!("eglCreatePbufferFromClientBuffer failed: {:x}", egl::GetError());
            }
        }

        // Cache our new surface and return it.
        let angle_surface = AngleSurface { egl_surface, egl_config };
        self.surfaces.push(SurfaceEntry { handle: Arc::downgrade(query), angle_surface });
        self.surfaces.last().unwrap().angle_surface.clone()
    }
}
