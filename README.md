# surfman [![Build Status](https://github.com/servo/surfman/workflows/Rust/badge.svg)](https://github.com/servo/surfman/actions)

![surfman logo](https://i.imgur.com/t0xcJ6D.png)

`surfman` is a low-level, cross-platform Rust library for managing *surfaces*, blocks of image data
in GPU memory. Using this library, you can:

* Draw to a window (perhaps created with `winit`) on the CPU.

* Render to a window (created via `winit` or otherwise) with OpenGL.

* Render to an off-screen surface with OpenGL.

* Use a surface created on one thread as an OpenGL texture on another thread.

* Draw to a surface with a platform-specific GPU API like Metal.

`surfman` forms the low-level graphics infrastructure of the
[Servo](https://github.com/servo/servo/) project, where it allows for easy porting of the
browser's WebGL and WebXR code to a variety of platforms.

## What `surfman` is not

`surfman` is not a full-featured GPU rendering API. It doesn't attempt to abstract over rendering
libraries like OpenGL, Metal, and Direct3D. For that, try [gfx-rs](https://github.com/gfx-rs/gfx).

`surfman` is also not a windowing solution. It can only render to a window that is already open
and needs to be paired with a crate like [winit](https://github.com/rust-windowing/winit) to
actually open the window. 

Likewise, `surfman` is not a UI toolkit. For that, see GTK+ and many other libraries. It's possible
to use `surfman` alongside any of these UI toolkits to efficiently integrate GPU rendering into an
application, however.

## Why `surfman`?

Most of this functionality can be achieved with other libraries, such as `glutin` and SDL. However,
for several use cases you can achieve better performance and/or correctness with `surfman`. For
example:

* On multi-GPU systems, games typically want to use the discrete GPU instead of the integrated GPU
  for better performance, while UI applications want the reverse for better energy consumption.
  However, most game-oriented OpenGL windowing libraries end up using the discrete GPU on Linux
  and macOS and the integrated GPU on Windows. On such systems, `surfman` explicitly allows you to
  choose which GPU you would like to render with.

* OpenGL's *share context* or *share lists* feature allows you to share textures across contexts.
  However, this often exposes driver bugs, and, even if it works, it causes most operations to
  take mutex locks. Efficient texture sharing requires the use of platform-specific APIs, which
  `surfman` abstracts over.

* The ANGLE implementation of OpenGL on Windows is not generally thread-safe, so attempts to render
  on background threads will generally segfault. `surfman` carefully works around all the safety
  issues so that the library is safe to use from any thread.

* Applications such as emulators and video players that draw to the CPU want to avoid copying
  pixels as much as possible. Classic APIs for transferring image data like `glTexImage2D()` and
  `XPutImage()` often cause the data to be copied several times. In contrast, `surfman` allows you
  to render to the screen with as few copies as feasible—sometimes even zero, depending on the
  platform.

## Platform support

The library supports the following platforms:

* Windows, with OpenGL via the native WGL framework.

* Windows, with OpenGL via Google's ANGLE library.

* macOS, with OpenGL via the native CGL framework.

* macOS, with Metal.

* Linux/other Unix, with OpenGL on Wayland.

* Linux/other Unix, with OpenGL on X11 via GLX.

* Android P and up, with OpenGL.

* Generic CPU rendering of OpenGL via the OSMesa framework.

## Future work

The following features may be added later:

* Support for Android Marshmallow, Nougat, and Oreo.

* Partial presentation, to allow the OS to composite only the region of the window that has
  changed.

* CPU rendering support on more platforms. (Right now, the CPU rendering features only work on
  macOS.)

* Vulkan support.

* Direct3D 11 support on Windows.

* YUV surfaces, for software video codecs.

* Support for running in a browser with WebAssembly.

## License

`surfman` is licensed under the same terms as Rust itself.

`surfman` abides by the same code of conduct as Rust itself.
