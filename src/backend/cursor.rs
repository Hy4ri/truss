use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::utils::{Point, Transform};
use xcursor::parser::parse_xcursor;
use xcursor::CursorTheme;

/// Manages loading and rendering default/named xcursor themes with fallback software cursor
pub struct CursorManager {
    theme: CursorTheme,
    cached_buffer: Option<(String, u32, MemoryRenderBuffer, (i32, i32))>,
    fallback_buffer: MemoryRenderBuffer,
}

impl Default for CursorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorManager {
    pub fn new() -> Self {
        let theme_name = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "default".into());
        let theme = CursorTheme::load(&theme_name);

        // Generate a 16x16 crisp arrow fallback cursor in ARGB8888
        let fallback_buffer = Self::create_fallback_cursor();

        Self {
            theme,
            cached_buffer: None,
            fallback_buffer,
        }
    }

    fn create_fallback_cursor() -> MemoryRenderBuffer {
        const W: i32 = 16;
        const H: i32 = 16;
        let mut pixels = vec![0u8; (W * H * 4) as usize];

        // Standard classic cursor arrow shape
        // ARGB8888 layout: [B, G, R, A] in little-endian byte slice
        for y in 0..H {
            for x in 0..W {
                let idx = ((y * W + x) * 4) as usize;
                let is_outline = (x == 0 && y < 15)
                    || (y == 0 && x < 15)
                    || (x == y && x < 11)
                    || (y == 10 && (4..=6).contains(&x))
                    || (x == 10 && (4..=6).contains(&y))
                    || (x + y == 14 && x >= 5 && y >= 5);

                let is_fill = x > 0 && y > 0 && ((x < y && x + y < 14) || (x == y && x < 10));

                if is_outline {
                    // Black border: A=255, R=0, G=0, B=0
                    pixels[idx] = 0; // B
                    pixels[idx + 1] = 0; // G
                    pixels[idx + 2] = 0; // R
                    pixels[idx + 3] = 255; // A
                } else if is_fill {
                    // White fill: A=255, R=255, G=255, B=255
                    pixels[idx] = 255; // B
                    pixels[idx + 1] = 255; // G
                    pixels[idx + 2] = 255; // R
                    pixels[idx + 3] = 255; // A
                }
            }
        }

        MemoryRenderBuffer::from_slice(
            &pixels,
            Fourcc::Argb8888,
            (W, H),
            1,
            Transform::Normal,
            None,
        )
    }

    /// Load or retrieve cached xcursor buffer for a named icon (e.g. "default", "left_ptr")
    pub fn get_or_load_cursor(
        &mut self,
        name: &str,
        size: u32,
    ) -> (&MemoryRenderBuffer, (i32, i32)) {
        let is_cached = if let Some((ref cached_name, cached_size, _, _)) = self.cached_buffer {
            cached_name == name && cached_size == size
        } else {
            false
        };

        if is_cached {
            let (_, _, ref buf, hotspot) = self.cached_buffer.as_ref().unwrap();
            return (buf, *hotspot);
        }

        let icon_names = match name {
            "default" => vec!["default", "left_ptr", "arrow"],
            "pointer" => vec!["pointer", "hand", "hand2", "pointing_hand"],
            "text" => vec!["text", "xterm", "ibeam"],
            "crosshair" => vec!["crosshair", "cross"],
            "grab" => vec!["grab", "openhand", "hand1"],
            "grabbing" => vec!["grabbing", "closedhand"],
            _ => vec![name],
        };

        for candidate in icon_names {
            if let Some(path) = self.theme.load_icon(candidate) {
                if let Ok(data) = std::fs::read(&path) {
                    if let Some(images) = parse_xcursor(&data) {
                        // Pick closest image to target size
                        if let Some(best) = images
                            .into_iter()
                            .min_by_key(|img| (img.size as i32 - size as i32).abs())
                        {
                            let buf = MemoryRenderBuffer::from_slice(
                                &best.pixels_rgba,
                                Fourcc::Argb8888,
                                (best.width as i32, best.height as i32),
                                1,
                                Transform::Normal,
                                None,
                            );
                            let hotspot = (best.xhot as i32, best.yhot as i32);
                            self.cached_buffer = Some((name.to_string(), size, buf, hotspot));
                            let (_, _, ref b, h) = self.cached_buffer.as_ref().unwrap();
                            return (b, *h);
                        }
                    }
                }
            }
        }

        // Fallback arrow
        (&self.fallback_buffer, (0, 0))
    }

    /// Render named cursor at position
    pub fn render_named_cursor(
        &mut self,
        renderer: &mut GlesRenderer,
        pos: (i32, i32),
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        let (buf, (xhot, yhot)) = self.get_or_load_cursor("default", 24);
        let render_pos: Point<f64, smithay::utils::Physical> =
            Point::from(((pos.0 - xhot) as f64, (pos.1 - yhot) as f64));

        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            render_pos,
            buf,
            None,
            None,
            None,
            Kind::Cursor,
        )
        .ok()
    }
}
