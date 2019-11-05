// surfman/src/platform/unix/default.rs
//
//! The default backend for Unix, which dynamically switches between Wayland and X11.

use crate::platform::generic::multi::adapter::Adapter as MultiAdapter;
use crate::platform::generic::multi::connection::Connection as MultiConnection;
use crate::platform::generic::multi::context::Context as MultiContext;
use crate::platform::generic::multi::surface::Surface as MultiSurface;
use crate::platform::generic::multi::surface::SurfaceTexture as MultiSurfaceTexture;
use super::wayland::adapter::Adapter as WaylandAdapter;
use super::wayland::connection::Connection as WaylandConnection;
use super::wayland::context::Context as WaylandContext;
use super::wayland::surface::{Surface as WaylandSurface, SurfaceTexture as WaylandSurfaceTexture};
use super::x11::adapter::Adapter as X11Adapter;
use super::x11::connection::Connection as X11Connection;
use super::x11::context::Context as X11Context;
use super::x11::surface::{Surface as X11Surface, SurfaceTexture as X11SurfaceTexture};

pub type Adapter = MultiAdapter<WaylandAdapter, X11Adapter>;
pub type Connection = MultiConnection<WaylandConnection, X11Connection>;
pub type Context = MultiContext<WaylandContext, X11Context>;
pub type Surface = MultiSurface<WaylandSurface, X11Surface>;
pub type SurfaceTexture = MultiSurfaceTexture<WaylandSurfaceTexture, X11SurfaceTexture>;
