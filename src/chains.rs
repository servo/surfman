/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! An implementation of thread-safe swap chains for the `surfman` surface manager.
//!
//! The role of a swap chain is to allow surfaces to be communicated between contexts,
//! often in different threads. Each swap chain has a *producer* context,
//! responsible for creating and destroying surfaces, and a number of *consumer* contexts,
//! (usually just one) which take surfaces from the swap chain, and return them for recycling.
//!
//! Each swap chain has a *back buffer*, that is the current surface that the producer context may draw to.
//! Each swap chain has a *front buffer*, that is the most recent surface the producer context finished drawing to.
//!
//! The producer may *swap* these buffers when it has finished drawing and has a surface ready to display.
//!
//! The consumer may *take* the front buffer, display it, then *recycle* it.
//!
//! Each producer context has one *attached* swap chain, whose back buffer is the current surface of the context.
//! The producer may change the attached swap chain, attaching a currently unattached swap chain,
//! and detaching the currently attached one.

#![allow(missing_docs)]

use crate::device::Device as DeviceAPI;
use crate::{ContextID, Error, SurfaceAccess, SurfaceInfo, SurfaceType};
use euclid::default::Size2D;
use fnv::{FnvHashMap, FnvHashSet};
use glow as gl;
use glow::Context as Gl;
use glow::HasContext;
use log::debug;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::hash::Hash;
use std::mem;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

// The data stored for each swap chain.
struct SwapChainData<Device: DeviceAPI> {
    // The size of the back buffer
    size: Size2D<i32>,
    // The id of the producer context
    context_id: ContextID,
    // The surface access mode for the context.
    surface_access: SurfaceAccess,
    // The back buffer of the swap chain.
    back_buffer: BackBuffer<Device>,
    // Some if the producing context has finished drawing a new front buffer, ready to be displayed.
    pending_surface: Option<Device::Surface>,
    // All of the surfaces that have already been displayed, ready to be recycled.
    recycled_surfaces: Vec<Device::Surface>,
}

pub enum PreserveBuffer<'a> {
    Yes(&'a Gl),
    No,
}

enum BackBuffer<Device: DeviceAPI> {
    Attached,
    Detached(Device::Surface),
    TakenAttached,
    TakenDetached,
}

impl<Device: DeviceAPI> BackBuffer<Device> {
    fn take_surface(
        &mut self,
        device: &Device,
        context: &mut Device::Context,
    ) -> Result<Device::Surface, Error> {
        let new_back_buffer = match self {
            BackBuffer::Attached => BackBuffer::TakenAttached,
            BackBuffer::Detached(_) => BackBuffer::TakenDetached,
            _ => return Err(Error::Failed),
        };
        let surface = match mem::replace(self, new_back_buffer) {
            BackBuffer::Attached => device.unbind_surface_from_context(context)?.unwrap(),
            BackBuffer::Detached(surface) => surface,
            _ => unreachable!(),
        };
        Ok(surface)
    }
    fn take_surface_texture(
        &mut self,
        device: &Device,
        context: &mut Device::Context,
    ) -> Result<Device::SurfaceTexture, Error> {
        let surface = self.take_surface(device, context)?;
        device
            .create_surface_texture(context, surface)
            .map_err(|(err, surface)| {
                let _ = self.replace_surface(device, context, surface);
                err
            })
    }
    fn replace_surface(
        &mut self,
        device: &Device,
        context: &mut Device::Context,
        surface: Device::Surface,
    ) -> Result<(), Error> {
        let new_back_buffer = match self {
            BackBuffer::TakenAttached => {
                if let Err((err, mut surface)) = device.bind_surface_to_context(context, surface) {
                    debug!("Oh no, destroying surface");
                    let _ = device.destroy_surface(context, &mut surface);
                    return Err(err);
                }
                BackBuffer::Attached
            }
            BackBuffer::TakenDetached => BackBuffer::Detached(surface),
            _ => return Err(Error::Failed),
        };
        *self = new_back_buffer;
        Ok(())
    }
    fn replace_surface_texture(
        &mut self,
        device: &Device,
        context: &mut Device::Context,
        surface_texture: Device::SurfaceTexture,
    ) -> Result<(), Error> {
        let surface = device
            .destroy_surface_texture(context, surface_texture)
            .map_err(|(err, _)| err)?;
        self.replace_surface(device, context, surface)
    }
}

impl<Device: DeviceAPI> SwapChainData<Device> {
    // Returns `Ok` if `context` is the producer context for this swap chain.
    fn validate_context(&self, device: &Device, context: &Device::Context) -> Result<(), Error> {
        if self.context_id == device.context_id(context) {
            Ok(())
        } else {
            Err(Error::IncompatibleContext)
        }
    }

    // Swap the back and front buffers.
    // Called by the producer.
    // Returns an error if `context` is not the producer context for this swap chain.
    fn swap_buffers(
        &mut self,
        device: &Device,
        context: &mut Device::Context,
        preserve_buffer: PreserveBuffer<'_>,
    ) -> Result<(), Error> {
        debug!("Swap buffers on context {:?}", self.context_id);
        self.validate_context(device, context)?;

        // Recycle the old front buffer
        if let Some(old_front_buffer) = self.pending_surface.take() {
            let SurfaceInfo { id, size, .. } = device.surface_info(&old_front_buffer);
            debug!(
                "Recycling surface {:?} ({:?}) for context {:?}",
                id, size, self.context_id
            );
            self.recycle_surface(old_front_buffer);
        }

        // Fetch a new back buffer, recycling presented buffers if possible.
        let new_back_buffer = self
            .recycled_surfaces
            .iter()
            .position(|surface| device.surface_info(surface).size == self.size)
            .map(|index| {
                debug!("Recycling surface for context {:?}", self.context_id);
                Ok(self.recycled_surfaces.swap_remove(index))
            })
            .unwrap_or_else(|| {
                debug!(
                    "Creating a new surface ({:?}) for context {:?}",
                    self.size, self.context_id
                );
                let surface_type = SurfaceType::Generic { size: self.size };
                device.create_surface(context, self.surface_access, surface_type)
            })?;

        let back_info = device.surface_info(&new_back_buffer);

        // Swap the buffers
        debug!(
            "Surface {:?} is the new back buffer for context {:?}",
            device.surface_info(&new_back_buffer).id,
            self.context_id
        );
        let new_front_buffer = self.back_buffer.take_surface(device, context)?;
        self.back_buffer
            .replace_surface(device, context, new_back_buffer)?;

        if let PreserveBuffer::Yes(gl) = preserve_buffer {
            let front_info = device.surface_info(&new_front_buffer);
            unsafe {
                gl.bind_framebuffer(gl::READ_FRAMEBUFFER, front_info.framebuffer_object);
                debug_assert_eq!(gl.get_error(), gl::NO_ERROR);
                gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, back_info.framebuffer_object);
                debug_assert_eq!(gl.get_error(), gl::NO_ERROR);
                gl.blit_framebuffer(
                    0,
                    0,
                    front_info.size.width,
                    front_info.size.height,
                    0,
                    0,
                    back_info.size.width,
                    back_info.size.height,
                    gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT,
                    gl::NEAREST,
                );
                debug_assert_eq!(gl.get_error(), gl::NO_ERROR);
            }
        }

        // Update the state
        debug!(
            "Surface {:?} is the new front buffer for context {:?}",
            device.surface_info(&new_front_buffer).id,
            self.context_id
        );
        self.pending_surface = Some(new_front_buffer);
        for mut surface in self.recycled_surfaces.drain(..) {
            debug!("Destroying a surface for context {:?}", self.context_id);
            device.destroy_surface(context, &mut surface)?;
        }

        Ok(())
    }

    // Swap the attached swap chain.
    // Called by the producer.
    // Returns an error if `context` is not the producer context for both swap chains.
    // Returns an error if this swap chain is attached, or the other swap chain is detached.
    fn take_attachment_from(
        &mut self,
        device: &Device,
        context: &mut Device::Context,
        other: &mut SwapChainData<Device>,
    ) -> Result<(), Error> {
        self.validate_context(device, context)?;
        other.validate_context(device, context)?;
        let our_surface = self.back_buffer.take_surface(device, context)?;
        let their_surface = other.back_buffer.take_surface(device, context)?;
        mem::swap(&mut self.back_buffer, &mut other.back_buffer);
        self.back_buffer
            .replace_surface(device, context, our_surface)?;
        other
            .back_buffer
            .replace_surface(device, context, their_surface)?;
        Ok(())
    }

    // Resize the swap chain.
    // This creates a new back buffer of the appropriate size,
    // and destroys the old one.
    // Called by the producer.
    // Returns an error if `context` is not the producer context for this swap chain.
    // Returns an error if `size` is smaller than (1, 1).
    fn resize(
        &mut self,
        device: &Device,
        context: &mut Device::Context,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        debug!(
            "Resizing context {:?} to {:?}",
            device.context_id(context),
            size
        );
        self.validate_context(device, context)?;
        if (size.width < 1) || (size.height < 1) {
            return Err(Error::Failed);
        }
        let surface_type = SurfaceType::Generic { size };
        let new_back_buffer = device.create_surface(context, self.surface_access, surface_type)?;
        let mut old_back_buffer = self.back_buffer.take_surface(device, context)?;
        self.back_buffer
            .replace_surface(device, context, new_back_buffer)?;
        device.destroy_surface(context, &mut old_back_buffer)?;
        self.size = size;
        Ok(())
    }

    // Get the current size.
    // Called by a consumer.
    fn size(&self) -> Size2D<i32> {
        self.size
    }

    // Take the current back buffer.
    // Called by a producer.
    fn take_surface_texture(
        &mut self,
        device: &Device,
        context: &mut Device::Context,
    ) -> Result<Device::SurfaceTexture, Error> {
        self.validate_context(device, context)?;
        self.back_buffer.take_surface_texture(device, context)
    }

    // Recycle the current back buffer.
    // Called by a producer.
    fn recycle_surface_texture(
        &mut self,
        device: &Device,
        context: &mut Device::Context,
        surface_texture: Device::SurfaceTexture,
    ) -> Result<(), Error> {
        self.validate_context(device, context)?;
        self.back_buffer
            .replace_surface_texture(device, context, surface_texture)
    }

    // Take the current front buffer.
    // Returns the most recent recycled surface if there is no current front buffer.
    // Called by a consumer.
    fn take_surface(&mut self) -> Option<Device::Surface> {
        self.pending_surface
            .take()
            .or_else(|| self.recycled_surfaces.pop())
    }

    // Take the current front buffer.
    // Returns `None` if there is no current front buffer.
    // Called by a consumer.
    fn take_pending_surface(&mut self) -> Option<Device::Surface> {
        self.pending_surface.take()
    }

    // Recycle the current front buffer.
    // Called by a consumer.
    fn recycle_surface(&mut self, surface: Device::Surface) {
        self.recycled_surfaces.push(surface)
    }

    // Clear the current back buffer.
    // Called by the producer.
    // Returns an error if `context` is not the producer context for this swap chain.
    fn clear_surface(
        &mut self,
        device: &Device,
        context: &mut Device::Context,
        gl: &Gl,
        color: [f32; 4],
    ) -> Result<(), Error> {
        self.validate_context(device, context)?;

        // Save the current GL state
        let draw_fbo;
        let read_fbo;
        let mut clear_color = [0., 0., 0., 0.];
        let mut clear_depth = [0.];
        let mut clear_stencil = [0];
        let color_mask;
        let depth_mask;
        let mut stencil_mask = [0];
        let scissor_enabled = unsafe { gl.is_enabled(gl::SCISSOR_TEST) };
        let rasterizer_enabled = unsafe { gl.is_enabled(gl::RASTERIZER_DISCARD) };
        unsafe {
            draw_fbo = gl.get_parameter_framebuffer(gl::DRAW_FRAMEBUFFER_BINDING);
            read_fbo = gl.get_parameter_framebuffer(gl::READ_FRAMEBUFFER_BINDING);
            gl.get_parameter_f32_slice(gl::COLOR_CLEAR_VALUE, &mut clear_color[..]);
            gl.get_parameter_f32_slice(gl::DEPTH_CLEAR_VALUE, &mut clear_depth[..]);
            gl.get_parameter_i32_slice(gl::STENCIL_CLEAR_VALUE, &mut clear_stencil[..]);
            depth_mask = gl.get_parameter_bool(gl::DEPTH_WRITEMASK);
            gl.get_parameter_i32_slice(gl::STENCIL_WRITEMASK, &mut stencil_mask[..]);
            color_mask = gl.get_parameter_bool_array::<4>(gl::COLOR_WRITEMASK);
        }

        // Make the back buffer the current surface
        let reattach = if self.is_attached() {
            None
        } else {
            let surface = self.back_buffer.take_surface(device, context)?;
            let mut reattach = device.unbind_surface_from_context(context)?;
            if let Err((err, mut surface)) = device.bind_surface_to_context(context, surface) {
                debug!("Oh no, destroying surfaces");
                let _ = device.destroy_surface(context, &mut surface);
                if let Some(ref mut reattach) = reattach {
                    let _ = device.destroy_surface(context, reattach);
                }
                return Err(err);
            }
            reattach
        };

        // Clear it
        let fbo = device
            .context_surface_info(context)
            .unwrap()
            .unwrap()
            .framebuffer_object;
        unsafe {
            gl.bind_framebuffer(gl::FRAMEBUFFER, fbo);
            gl.clear_color(color[0], color[1], color[2], color[3]);
            gl.clear_depth(1.);
            gl.clear_stencil(0);
            gl.disable(gl::SCISSOR_TEST);
            gl.disable(gl::RASTERIZER_DISCARD);
            gl.depth_mask(true);
            gl.stencil_mask(0xFFFFFFFF);
            gl.color_mask(true, true, true, true);
            gl.clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
        }
        // Reattach the old surface
        if let Some(surface) = reattach {
            let mut old_surface = device.unbind_surface_from_context(context)?.unwrap();
            if let Err((err, mut surface)) = device.bind_surface_to_context(context, surface) {
                debug!("Oh no, destroying surface");
                let _ = device.destroy_surface(context, &mut surface);
                let _ = device.destroy_surface(context, &mut old_surface);
                return Err(err);
            }
            self.back_buffer
                .replace_surface(device, context, old_surface)?;
        }

        // Restore the GL state
        unsafe {
            gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, draw_fbo);
            gl.bind_framebuffer(gl::READ_FRAMEBUFFER, read_fbo);
            gl.clear_color(
                clear_color[0],
                clear_color[1],
                clear_color[2],
                clear_color[3],
            );
            gl.color_mask(color_mask[0], color_mask[1], color_mask[2], color_mask[3]);
            gl.clear_depth(clear_depth[0] as f64);
            gl.clear_stencil(clear_stencil[0]);
            gl.depth_mask(depth_mask);
            gl.stencil_mask(stencil_mask[0] as _);
            if scissor_enabled {
                gl.enable(gl::SCISSOR_TEST);
            }
            if rasterizer_enabled {
                gl.enable(gl::RASTERIZER_DISCARD);
            }
        }

        Ok(())
    }

    /// Is this the attached swap chain?
    fn is_attached(&self) -> bool {
        match self.back_buffer {
            BackBuffer::Attached | BackBuffer::TakenAttached => true,
            BackBuffer::Detached(_) | BackBuffer::TakenDetached => false,
        }
    }

    // Destroy the swap chain.
    // Called by the producer.
    // Returns an error if `context` is not the producer context for this swap chain.
    fn destroy(&mut self, device: &Device, context: &mut Device::Context) -> Result<(), Error> {
        self.validate_context(device, context)?;
        let surfaces = self
            .pending_surface
            .take()
            .into_iter()
            .chain(self.back_buffer.take_surface(device, context).into_iter())
            .chain(self.recycled_surfaces.drain(..));
        for mut surface in surfaces {
            device.destroy_surface(context, &mut surface)?;
        }
        Ok(())
    }
}

/// A thread-safe swap chain.
pub struct SwapChain<Device: DeviceAPI>(Arc<Mutex<SwapChainData<Device>>>);

// We can't derive Clone unfortunately
impl<Device: DeviceAPI> Clone for SwapChain<Device> {
    fn clone(&self) -> Self {
        SwapChain(self.0.clone())
    }
}

impl<Device: DeviceAPI> SwapChain<Device> {
    // Guarantee unique access to the swap chain data
    fn lock(&self) -> MutexGuard<SwapChainData<Device>> {
        self.0.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// Swap the back and front buffers.
    /// Called by the producer.
    /// Returns an error if `context` is not the producer context for this swap chain.
    pub fn swap_buffers(
        &self,
        device: &Device,
        context: &mut Device::Context,
        preserve_buffer: PreserveBuffer<'_>,
    ) -> Result<(), Error> {
        self.lock().swap_buffers(device, context, preserve_buffer)
    }

    /// Swap the attached swap chain.
    /// Called by the producer.
    /// Returns an error if `context` is not the producer context for both swap chains.
    /// Returns an error if this swap chain is attached, or the other swap chain is detached.
    pub fn take_attachment_from(
        &self,
        device: &Device,
        context: &mut Device::Context,
        other: &SwapChain<Device>,
    ) -> Result<(), Error> {
        self.lock()
            .take_attachment_from(device, context, &mut *other.lock())
    }

    /// Resize the swap chain.
    /// This creates a new back buffer of the appropriate size,
    /// and destroys the old one.
    /// Called by the producer.
    /// Returns an error if `context` is not the producer context for this swap chain.
    pub fn resize(
        &self,
        device: &Device,
        context: &mut Device::Context,
        size: Size2D<i32>,
    ) -> Result<(), Error> {
        self.lock().resize(device, context, size)
    }

    /// Get the current size.
    /// Called by a consumer.
    pub fn size(&self) -> Size2D<i32> {
        self.lock().size()
    }

    /// Take the current back buffer.
    /// Called by a producer.
    pub fn take_surface_texture(
        &self,
        device: &Device,
        context: &mut Device::Context,
    ) -> Result<Device::SurfaceTexture, Error> {
        self.lock().take_surface_texture(device, context)
    }

    /// Recycle the current back buffer.
    /// Called by a producer.
    pub fn recycle_surface_texture(
        &self,
        device: &Device,
        context: &mut Device::Context,
        surface_texture: Device::SurfaceTexture,
    ) -> Result<(), Error> {
        self.lock()
            .recycle_surface_texture(device, context, surface_texture)
    }

    /// Take the current front buffer.
    /// Returns `None` if there is no current front buffer.
    /// Called by a consumer.
    pub fn take_pending_surface(&self) -> Option<Device::Surface> {
        self.lock().take_pending_surface()
    }

    /// Clear the current back buffer.
    /// Called by the producer.
    /// Returns an error if `context` is not the producer context for this swap chain.
    pub fn clear_surface(
        &self,
        device: &Device,
        context: &mut Device::Context,
        gl: &Gl,
        color: [f32; 4],
    ) -> Result<(), Error> {
        self.lock().clear_surface(device, context, gl, color)
    }

    /// Is this the attached swap chain?
    pub fn is_attached(&self) -> bool {
        self.lock().is_attached()
    }

    /// Destroy the swap chain.
    /// Called by the producer.
    /// Returns an error if `context` is not the producer context for this swap chain.
    pub fn destroy(&self, device: &Device, context: &mut Device::Context) -> Result<(), Error> {
        self.lock().destroy(device, context)
    }

    /// Create a new attached swap chain
    pub fn create_attached(
        device: &Device,
        context: &mut Device::Context,
        surface_access: SurfaceAccess,
    ) -> Result<SwapChain<Device>, Error> {
        let size = device.context_surface_info(context).unwrap().unwrap().size;
        Ok(SwapChain(Arc::new(Mutex::new(SwapChainData {
            size,
            context_id: device.context_id(context),
            surface_access,
            back_buffer: BackBuffer::Attached,
            pending_surface: None,
            recycled_surfaces: Vec::new(),
        }))))
    }

    /// Create a new detached swap chain
    pub fn create_detached(
        device: &Device,
        context: &mut Device::Context,
        surface_access: SurfaceAccess,
        size: Size2D<i32>,
    ) -> Result<SwapChain<Device>, Error> {
        let surface_type = SurfaceType::Generic { size };
        let surface = device.create_surface(context, surface_access, surface_type)?;
        Ok(SwapChain(Arc::new(Mutex::new(SwapChainData {
            size,
            context_id: device.context_id(context),
            surface_access,
            back_buffer: BackBuffer::Detached(surface),
            pending_surface: None,
            recycled_surfaces: Vec::new(),
        }))))
    }
}

impl<Device> SwapChainAPI for SwapChain<Device>
where
    Device: 'static + DeviceAPI,
    Device::Surface: Send,
{
    type Surface = Device::Surface;

    /// Take the current front buffer.
    /// Returns the most recent recycled surface if there is no current front buffer.
    /// Called by a consumer.
    fn take_surface(&self) -> Option<Device::Surface> {
        self.lock().take_surface()
    }

    /// Recycle the current front buffer.
    /// Called by a consumer.
    fn recycle_surface(&self, surface: Device::Surface) {
        self.lock().recycle_surface(surface)
    }
}

/// A thread-safe collection of swap chains.
#[derive(Default)]
pub struct SwapChains<SwapChainID: Eq + Hash, Device: DeviceAPI> {
    // The swap chain ids, indexed by context id
    ids: Arc<Mutex<FnvHashMap<ContextID, FnvHashSet<SwapChainID>>>>,
    // The swap chains, indexed by swap chain id
    table: Arc<RwLock<FnvHashMap<SwapChainID, SwapChain<Device>>>>,
}

// We can't derive Clone unfortunately
impl<SwapChainID: Eq + Hash, Device: DeviceAPI> Clone for SwapChains<SwapChainID, Device> {
    fn clone(&self) -> Self {
        SwapChains {
            ids: self.ids.clone(),
            table: self.table.clone(),
        }
    }
}

impl<SwapChainID, Device> SwapChains<SwapChainID, Device>
where
    SwapChainID: Clone + Eq + Hash + Debug,
    Device: DeviceAPI,
{
    /// Create a new collection.
    pub fn new() -> SwapChains<SwapChainID, Device> {
        SwapChains {
            ids: Arc::new(Mutex::new(FnvHashMap::default())),
            table: Arc::new(RwLock::new(FnvHashMap::default())),
        }
    }

    // Lock the ids
    fn ids(&self) -> MutexGuard<FnvHashMap<ContextID, FnvHashSet<SwapChainID>>> {
        self.ids.lock().unwrap_or_else(|err| err.into_inner())
    }

    // Lock the lookup table
    fn table(&self) -> RwLockReadGuard<FnvHashMap<SwapChainID, SwapChain<Device>>> {
        self.table.read().unwrap_or_else(|err| err.into_inner())
    }

    // Lock the lookup table for writing
    fn table_mut(&self) -> RwLockWriteGuard<FnvHashMap<SwapChainID, SwapChain<Device>>> {
        self.table.write().unwrap_or_else(|err| err.into_inner())
    }

    /// Create a new attached swap chain and insert it in the table.
    /// Returns an error if the `id` is already in the table.
    pub fn create_attached_swap_chain(
        &self,
        id: SwapChainID,
        device: &Device,
        context: &mut Device::Context,
        surface_access: SurfaceAccess,
    ) -> Result<(), Error> {
        match self.table_mut().entry(id.clone()) {
            Entry::Occupied(_) => Err(Error::Failed)?,
            Entry::Vacant(entry) => {
                entry.insert(SwapChain::create_attached(device, context, surface_access)?)
            }
        };
        self.ids()
            .entry(device.context_id(context))
            .or_insert_with(Default::default)
            .insert(id);
        Ok(())
    }

    /// Create a new dettached swap chain and insert it in the table.
    /// Returns an error if the `id` is already in the table.
    pub fn create_detached_swap_chain(
        &self,
        id: SwapChainID,
        size: Size2D<i32>,
        device: &Device,
        context: &mut Device::Context,
        surface_access: SurfaceAccess,
    ) -> Result<(), Error> {
        match self.table_mut().entry(id.clone()) {
            Entry::Occupied(_) => Err(Error::Failed)?,
            Entry::Vacant(entry) => entry.insert(SwapChain::create_detached(
                device,
                context,
                surface_access,
                size,
            )?),
        };
        self.ids()
            .entry(device.context_id(context))
            .or_insert_with(Default::default)
            .insert(id);
        Ok(())
    }

    /// Destroy a swap chain.
    /// Called by the producer.
    /// Returns an error if `context` is not the producer context for the swap chain.
    pub fn destroy(
        &self,
        id: SwapChainID,
        device: &Device,
        context: &mut Device::Context,
    ) -> Result<(), Error> {
        if let Some(swap_chain) = self.table_mut().remove(&id) {
            swap_chain.destroy(device, context)?;
        }
        if let Some(ids) = self.ids().get_mut(&device.context_id(context)) {
            ids.remove(&id);
        }
        Ok(())
    }

    /// Iterate over all the swap chains for a particular producer context.
    /// Called by the producer.
    pub fn iter(
        &self,
        device: &Device,
        context: &mut Device::Context,
    ) -> impl Iterator<Item = (SwapChainID, SwapChain<Device>)> {
        self.ids()
            .get(&device.context_id(context))
            .iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| Some((id.clone(), self.table().get(id)?.clone())))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl<SwapChainID, Device> SwapChainsAPI<SwapChainID> for SwapChains<SwapChainID, Device>
where
    SwapChainID: 'static + Clone + Eq + Hash + Debug + Sync + Send,
    Device: 'static + DeviceAPI,
    Device::Surface: Send,
{
    type Surface = Device::Surface;
    type SwapChain = SwapChain<Device>;

    /// Get a swap chain
    fn get(&self, id: SwapChainID) -> Option<SwapChain<Device>> {
        debug!("Getting swap chain {:?}", id);
        self.table().get(&id).cloned()
    }
}

/// The consumer's view of a swap chain
pub trait SwapChainAPI: 'static + Clone + Send {
    type Surface;

    /// Take the current front buffer.
    fn take_surface(&self) -> Option<Self::Surface>;

    /// Recycle the current front buffer.
    fn recycle_surface(&self, surface: Self::Surface);
}

/// The consumer's view of a collection of swap chains
pub trait SwapChainsAPI<SwapChainID>: 'static + Clone + Send {
    type Surface;
    type SwapChain: SwapChainAPI<Surface = Self::Surface>;

    /// Get a swap chain
    fn get(&self, id: SwapChainID) -> Option<Self::SwapChain>;
}
