//! SVG image widget — renders SVG data as an egui texture.
//!
//! Uses `resvg` + `tiny-skia` to rasterize SVG content into an RGBA pixel
//! buffer, then uploads it as an `egui::TextureHandle`.  Gated behind the
//! `svg` feature flag since `resvg` adds ~2 MB to the binary.
//!
//! Primary use: Mermaid diagram rendering in Plan visualization, SVG icon
//! display in the Claw WebBridge panel, and file preview overlays.

#[cfg(feature = "svg")]
mod inner {
    use egui::{ColorImage, TextureHandle, TextureOptions};
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};
    use std::sync::{Mutex, OnceLock};

    /// Global content-addressed cache for rasterized SVG textures. The key
    /// covers the raw SVG bytes and requested pixel size so identical inputs
    /// reuse the same GPU texture across frames.
    static SVG_TEXTURE_CACHE: OnceLock<Mutex<HashMap<u64, TextureHandle>>> = OnceLock::new();
    /// Rough cap on the number of distinct SVG textures kept alive. When the
    /// cache exceeds this it is cleared; in practice diagrams are few and large.
    const SVG_CACHE_CAP: usize = 64;

    fn svg_cache_key(svg_data: &[u8], width: u32, height: u32) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hasher.write(svg_data);
        width.hash(&mut hasher);
        height.hash(&mut hasher);
        hasher.finish()
    }

    /// Rasterize raw SVG data to an `egui::ColorImage` at the given pixel size.
    ///
    /// Returns `None` if the SVG is malformed or empty.
    pub fn svg_to_color_image(svg_data: &[u8], width: u32, height: u32) -> Option<ColorImage> {
        let tree = resvg::usvg::Tree::from_data(svg_data, &resvg::usvg::Options::default()).ok()?;
        let size = tree.size();
        if size.width() <= 0.0 || size.height() <= 0.0 {
            return None;
        }

        // Scale to fit while preserving aspect ratio.
        let scale_x = width as f32 / size.width();
        let scale_y = height as f32 / size.height();
        let scale = scale_x.min(scale_y);

        let pixmap_size = tiny_skia::IntSize::from_wh(
            (size.width() * scale).ceil() as u32,
            (size.height() * scale).ceil() as u32,
        )?;

        let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height())?;
        let transform = tiny_skia::Transform::from_scale(scale, scale);
        let mut pixmap_mut = pixmap.as_mut();
        resvg::render(&tree, transform, &mut pixmap_mut);

        // Convert tiny-skia RGBA → egui ColorImage RGBA.
        let pixels: Vec<egui::Color32> = pixmap
            .data()
            .chunks_exact(4)
            .map(|c| egui::Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]))
            .collect();

        Some(ColorImage {
            size: [pixmap.width() as usize, pixmap.height() as usize],
            pixels,
            source_size: egui::Vec2::new(pixmap.width() as f32, pixmap.height() as f32),
        })
    }

    /// Rasterize an SVG and upload it as a texture, returning the handle.
    ///
    /// Identical `(svg_data, width, height)` inputs reuse the existing texture,
    /// avoiding repeated rasterization and GPU upload.
    pub fn svg_texture(
        ctx: &egui::Context,
        svg_data: &[u8],
        width: u32,
        height: u32,
    ) -> Option<TextureHandle> {
        let key = svg_cache_key(svg_data, width, height);
        let cache = SVG_TEXTURE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        {
            let locked = cache.lock().unwrap();
            if let Some(handle) = locked.get(&key) {
                return Some(handle.clone());
            }
        }

        let image = svg_to_color_image(svg_data, width, height)?;
        let handle = ctx.load_texture(
            format!("svg_{key}"),
            image,
            TextureOptions {
                magnification: egui::TextureFilter::Linear,
                minification: egui::TextureFilter::Linear,
                ..Default::default()
            },
        );

        let mut locked = cache.lock().unwrap();
        if locked.len() >= SVG_CACHE_CAP {
            locked.clear();
        }
        locked.insert(key, handle.clone());
        Some(handle)
    }
}

#[cfg(feature = "svg")]
pub use inner::*;

/// Stub that always returns `None` when the `svg` feature is disabled.
#[cfg(not(feature = "svg"))]
#[allow(dead_code)]
pub fn svg_to_color_image(_svg_data: &[u8], _width: u32, _height: u32) -> Option<egui::ColorImage> {
    None
}

/// Stub that always returns `None` when the `svg` feature is disabled.
#[cfg(not(feature = "svg"))]
#[allow(dead_code)]
pub fn svg_texture(
    _ctx: &egui::Context,
    _svg_data: &[u8],
    _width: u32,
    _height: u32,
) -> Option<egui::TextureHandle> {
    None
}

/// Render an SVG as an egui image, falling back to a placeholder text label
/// when the `svg` feature is not enabled or rasterization fails.
#[cfg(any(feature = "svg", test))]
#[allow(dead_code)]
pub fn render_svg_or_fallback(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    svg_data: &[u8],
    max_width: f32,
    max_height: f32,
    fallback_label: &str,
) {
    let handle = svg_texture(ctx, svg_data, max_width as u32, max_height as u32);

    if let Some(tex) = handle {
        let tex_size = tex.size_vec2();
        let scale = (max_width / tex_size.x)
            .min(max_height / tex_size.y)
            .min(1.0);
        let display_size = tex_size * scale;
        let img = egui::Image::from_texture(egui::load::SizedTexture::new(tex.id(), tex_size))
            .fit_to_exact_size(display_size)
            .sense(egui::Sense::hover());
        ui.add(img);
    } else {
        // Fallback: centered text label in a subtle frame.
        crate::design_system::surface_panel(ui, |ui| {
            ui.set_min_size(egui::vec2(max_width.min(200.0), max_height.min(100.0)));
            ui.vertical_centered(|ui| {
                crate::design_system::gap(ui, crate::design_system::Space::S4);
                crate::design_system::text(
                    ui,
                    fallback_label,
                    crate::design_system::TextStyle::Body,
                );
                crate::design_system::gap(ui, crate::design_system::Space::S4);
            });
        });
    }
}
