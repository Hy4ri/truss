use std::{path::PathBuf, sync::mpsc::Sender};

use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{DrmDevice, DrmDeviceFd, DrmEvent, GbmBufferedSurface},
        egl::{EGLContext, EGLDisplay},
        renderer::{
            gles::GlesRenderer, utils::draw_render_elements, Bind, Color32F, Frame, Renderer,
        },
        session::{libseat::LibSeatSession, Session},
    },
    output::Output,
    reexports::{
        calloop::LoopHandle,
        drm::control::{connector, crtc, Device as ControlDevice},
        rustix::fs::OFlags,
        wayland_server::DisplayHandle,
    },
    utils::{DeviceFd, Rectangle, Transform},
};
use tracing::{info, warn};

use crate::{
    backend::{collect_render_elements, DESKTOP_BG_COLOR},
    App,
};

/// A physical display output driven directly via DRM/KMS and GBM framebuffer page-flipping.
pub struct DrmDisplay {
    pub name: String,
    pub card_id: usize,
    pub crtc: crtc::Handle,
    pub output: Output,
    pub gbm_surface: GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    pub renderer: GlesRenderer,
    pub size: (i32, i32),
    pub cursor_manager: crate::backend::cursor::CursorManager,
}

impl DrmDisplay {
    /// Render current compositor scene (windows, background, layers) and scan out to the physical monitor.
    pub fn render_frame(&mut self, app: &App) -> Result<(), Box<dyn std::error::Error>> {
        let (mut dmabuf, _age) = self
            .gbm_surface
            .next_buffer()
            .map_err(|e| format!("DRM next_buffer failed: {e}"))?;

        let mut framebuffer = self
            .renderer
            .bind(&mut dmabuf)
            .map_err(|e| format!("GlesRenderer bind dmabuf failed: {e}"))?;

        let elements = collect_render_elements(app, &mut self.renderer, &mut self.cursor_manager);
        let size = (self.size.0, self.size.1).into();
        let damage = Rectangle::from_size(size);

        // Mirrored outputs share one logical scene laid out in the primary
        // output's coordinate space (windows, bar and cursor are all placed
        // within the primary's usable area). Scale the scene to this display's
        // own physical size so each panel shows the desktop at its native
        // resolution (e.g. eDP-1 at 1920x1080 while the primary HDMI is
        // 1366x768), instead of clipping it to the primary's size.
        let scale = app
            .output_manager
            .outputs
            .first()
            .and_then(|o| o.current_mode())
            .map(|m| {
                (self.size.0 as f64 / m.size.w as f64).min(self.size.1 as f64 / m.size.h as f64)
            })
            .unwrap_or(1.0);

        let bg = Color32F::new(
            DESKTOP_BG_COLOR.r(),
            DESKTOP_BG_COLOR.g(),
            DESKTOP_BG_COLOR.b(),
            DESKTOP_BG_COLOR.a(),
        );

        if let Ok(mut frame) = self.renderer.render(&mut framebuffer, size, Transform::Normal) {
            let _ = frame.clear(bg, &[damage]);
            let _ = draw_render_elements(&mut frame, scale, &elements, &[damage]);
            let _ = frame.finish();
        }

        self.gbm_surface
            .queue_buffer(None, None, ())
            .map_err(|e| format!("DRM queue_buffer failed: {e}"))?;
        Ok(())
    }

    /// Release the queued GBM buffer only after the DRM page-flip/vblank that
    /// displayed it. Calling this before the event exhausts the swapchain or
    /// causes repeated scanout of stale buffers.
    pub fn frame_submitted(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.gbm_surface
            .frame_submitted()
            .map(|_| ())
            .map_err(|e| format!("DRM frame submission completion failed: {e}").into())
    }

    /// Reset DRM surface state after VT switch resume.
    pub fn reset_state(&mut self) {
        let _ = self.gbm_surface.surface().reset_state();
    }
}

/// Discovers connected DRM cards and displays, initializing hardware rendering pipeline.
pub fn discover_and_init_drm_displays(
    session: &mut LibSeatSession,
    loop_handle: &LoopHandle<'static, App>,
    dh: &DisplayHandle,
    app: &mut App,
    vblank_tx: Sender<(usize, crtc::Handle)>,
) -> Vec<DrmDisplay> {
    let mut displays = Vec::new();

    // Discover every DRM primary node. GPU numbering is not stable: systems
    // with USB/display GPUs or hybrid graphics often expose the active card as
    // card3 or higher.
    let mut candidate_cards: Vec<PathBuf> = std::fs::read_dir("/dev/dri")
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.strip_prefix("card").is_some_and(|suffix| {
                        !suffix.is_empty() && suffix.chars().all(char::is_numeric)
                    })
                })
        })
        .collect();
    candidate_cards.sort();

    for (card_id, path) in candidate_cards.into_iter().enumerate() {
        let card_path = path.display();

        info!("truss: probing DRM card node at {}", card_path);
        let owned_fd = match session.open(&path, OFlags::RDWR | OFlags::CLOEXEC) {
            Ok(fd) => fd,
            Err(e) => {
                warn!("truss: unable to open DRM device {}: {}", card_path, e);
                continue;
            }
        };

        let device_fd = DeviceFd::from(owned_fd);
        let drm_device_fd = DrmDeviceFd::new(device_fd);

        let (mut drm_device, notifier) = match DrmDevice::new(drm_device_fd.clone(), false) {
            Ok(res) => res,
            Err(e) => {
                warn!("truss: failed to init DrmDevice for {}: {}", card_path, e);
                continue;
            }
        };

        let gbm = match GbmDevice::new(drm_device_fd.clone()) {
            Ok(g) => g,
            Err(e) => {
                warn!("truss: failed to init GbmDevice for {}: {}", card_path, e);
                continue;
            }
        };

        let egl_display = match unsafe { EGLDisplay::new(gbm.clone()) } {
            Ok(d) => d,
            Err(e) => {
                warn!(
                    "truss: failed to create EGLDisplay for {}: {}",
                    card_path, e
                );
                continue;
            }
        };

        let dmabuf_render_formats = egl_display.dmabuf_render_formats().clone();

        let resources = match drm_device_fd.resource_handles() {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "truss: failed to get DRM resources for {}: {}",
                    card_path, e
                );
                continue;
            }
        };

        let mut used_crtcs = Vec::new();

        for &conn_handle in resources.connectors() {
            let conn_info = match drm_device_fd.get_connector(conn_handle, false) {
                Ok(info) => info,
                Err(_) => continue,
            };

            if conn_info.state() != connector::State::Connected {
                continue;
            }

            let modes = conn_info.modes();
            if modes.is_empty() {
                continue;
            }

            // Pick preferred or first mode
            let mode = modes
                .iter()
                .find(|m| {
                    m.mode_type()
                        .contains(smithay::reexports::drm::control::ModeTypeFlags::PREFERRED)
                })
                .copied()
                .unwrap_or(modes[0]);

            let (width, height) = (mode.size().0 as i32, mode.size().1 as i32);
            let vrefresh = mode.vrefresh() as i32;

            // Find compatible CRTC
            let mut matched_crtc = None;
            for &enc_handle in conn_info.encoders() {
                if let Ok(enc_info) = drm_device_fd.get_encoder(enc_handle) {
                    for &crtc_handle in resources.crtcs() {
                        if !used_crtcs.contains(&crtc_handle)
                            && (resources
                                .filter_crtcs(enc_info.possible_crtcs())
                                .contains(&crtc_handle))
                        {
                            matched_crtc = Some(crtc_handle);
                            break;
                        }
                    }
                }
                if matched_crtc.is_some() {
                    break;
                }
            }

            // Fallback to any unused CRTC if encoder lookup didn't match
            if matched_crtc.is_none() {
                matched_crtc = resources
                    .crtcs()
                    .iter()
                    .find(|c| !used_crtcs.contains(c))
                    .copied();
            }

            let Some(crtc_handle) = matched_crtc else {
                warn!(
                    "truss: no compatible unused CRTC found for connector {:?}",
                    conn_handle
                );
                continue;
            };

            used_crtcs.push(crtc_handle);

            let drm_surface = match drm_device.create_surface(crtc_handle, mode, &[conn_handle]) {
                Ok(s) => s,
                Err(e) => {
                    warn!(
                        "truss: failed to create DrmSurface for connector {:?}: {}",
                        conn_handle, e
                    );
                    continue;
                }
            };

            let gbm_allocator = GbmAllocator::new(
                gbm.clone(),
                GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
            );

            let color_formats = [Fourcc::Argb8888, Fourcc::Xrgb8888];
            let gbm_surface = match GbmBufferedSurface::new(
                drm_surface,
                gbm_allocator,
                &color_formats,
                dmabuf_render_formats.clone(),
            ) {
                Ok(s) => s,
                Err(e) => {
                    warn!("truss: failed to create GbmBufferedSurface: {}", e);
                    continue;
                }
            };

            let egl_context = match EGLContext::new(&egl_display) {
                Ok(c) => c,
                Err(e) => {
                    warn!("truss: failed to create EGLContext for display: {}", e);
                    continue;
                }
            };

            let renderer = match unsafe { GlesRenderer::new(egl_context) } {
                Ok(r) => r,
                Err(e) => {
                    warn!(
                        "truss: failed to create GlesRenderer for DRM display: {}",
                        e
                    );
                    continue;
                }
            };

            let connector_type_name = format!("{:?}", conn_info.interface());
            let conn_name = format!("{}-{}", connector_type_name, conn_info.interface_id());
            info!(
                "truss: configured DRM physical output '{}' on card {} (crtc {:?}, {}x{} @ {}Hz)",
                conn_name, card_id, crtc_handle, width, height, vrefresh
            );

            // Register physical output in OutputManager and create wl_output global
            let output = app.output_manager.create_output(
                &conn_name,
                (0, 0).into(),
                (width, height).into(),
                vrefresh * 1000,
            );
            let _global = output.create_global::<App>(dh);

            displays.push(DrmDisplay {
                name: conn_name,
                card_id,
                crtc: crtc_handle,
                output,
                gbm_surface,
                renderer,
                size: (width, height),
                cursor_manager: crate::backend::cursor::CursorManager::new(),
            });
        }

        // Register DRM page flip notifier in Calloop event loop
        let event_tx = vblank_tx.clone();
        let _ = loop_handle.insert_source(notifier, move |event, _, _state: &mut App| {
            if let DrmEvent::VBlank(_crtc) = event {
                // A queued GBM buffer may be released only after this event.
                let _ = event_tx.send((card_id, _crtc));
            }
        });
    }

    displays
}
