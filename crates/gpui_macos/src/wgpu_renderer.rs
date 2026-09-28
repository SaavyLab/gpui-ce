//! The WGPU renderer behind the Metal renderer's interface, for the `wgpu`
//! feature. Applications can then share the window's device through
//! `gpui_wgpu::WgpuContextHandle` and composite their own textures.

use std::{ffi::c_void, sync::Arc};

use foreign_types::ForeignType as _;
use gpui::{DevicePixels, GpuSpecs, Scene, Size};
use gpui_apple::metal_renderer::new_window_layer;
use gpui_wgpu::{GpuContext, WgpuAtlas, WgpuContextHandle, WgpuRenderer, WgpuSurfaceConfig};
use metal::{CAMetalLayer, MetalLayer, MetalLayerRef};

pub type Context = GpuContext;
pub type Renderer = MacWgpuRenderer;

pub unsafe fn new_renderer(
    context: Context,
    _native_window: *mut c_void,
    _native_view: *mut c_void,
    bounds: gpui::Size<f32>,
    transparent: bool,
) -> Renderer {
    MacWgpuRenderer::new(context, bounds, transparent)
}

pub struct MacWgpuRenderer {
    // Declared before `layer` so the surface drops first.
    renderer: WgpuRenderer,
    layer: MetalLayer,
}

impl MacWgpuRenderer {
    fn new(context: Context, bounds: gpui::Size<f32>, transparent: bool) -> Self {
        let layer = new_window_layer(transparent);
        let config = WgpuSurfaceConfig {
            // The view resizes this to device pixels once it knows its scale factor.
            size: Size {
                width: DevicePixels(bounds.width.max(1.0) as i32),
                height: DevicePixels(bounds.height.max(1.0) as i32),
            },
            transparent,
            preferred_present_mode: None,
        };
        // SAFETY: the renderer drops before the layer it presents to.
        let renderer = unsafe {
            WgpuRenderer::new_for_metal_layer(context, layer.as_ptr().cast(), config, None)
        }
        .expect("failed to create the WGPU renderer for a macOS window");
        Self { renderer, layer }
    }

    pub fn layer(&self) -> Option<&MetalLayerRef> {
        Some(&self.layer)
    }

    pub fn layer_ptr(&self) -> *mut CAMetalLayer {
        self.layer.as_ptr()
    }

    pub fn sprite_atlas(&self) -> &Arc<WgpuAtlas> {
        self.renderer.sprite_atlas()
    }

    pub fn set_presents_with_transaction(&mut self, presents_with_transaction: bool) {
        self.layer
            .set_presents_with_transaction(presents_with_transaction);
    }

    pub fn update_drawable_size(&mut self, size: Size<DevicePixels>) {
        self.renderer.update_drawable_size(size);
    }

    /// The surface's alpha mode sets the layer's opacity whenever WGPU
    /// configures it, so it must not be set on the layer directly.
    pub fn update_transparency(&mut self, transparent: bool) {
        self.renderer.update_transparency(transparent);
    }

    pub fn destroy(&mut self) {
        self.renderer.destroy();
    }

    /// Draws `scene`, recovering the device first if it was lost. Returns
    /// whether the next frame must render the scene again, uncached.
    pub fn draw(&mut self, scene: &Scene) -> bool {
        if self.renderer.device_lost() {
            // SAFETY: the renderer drops before the layer it presents to.
            if let Err(error) = unsafe {
                self.renderer
                    .recover_metal_layer(self.layer.as_ptr().cast())
            } {
                log::warn!("GPU recovery failed, will retry on next frame: {error}");
            }
            return true;
        }
        self.renderer.draw(scene);
        self.renderer.needs_redraw()
    }

    pub fn gpu_specs(&self) -> GpuSpecs {
        self.renderer.gpu_specs()
    }

    pub fn gpu_context_info(&self) -> Option<WgpuContextHandle> {
        self.renderer.gpu_context_info()
    }

    #[cfg(feature = "test-support")]
    pub fn render_to_image(&mut self, scene: &Scene) -> anyhow::Result<image::RgbaImage> {
        self.renderer.render_to_image(scene)
    }

    // WGPU's offscreen rendering needs gpui_wgpu's test-support, which the
    // `test-support` feature enables.
    #[cfg(all(test, not(feature = "test-support")))]
    pub fn render_to_image(&mut self, _scene: &Scene) -> anyhow::Result<image::RgbaImage> {
        anyhow::bail!("rendering to an image needs the test-support feature")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparency_survives_resizing() {
        let mut renderer = MacWgpuRenderer::new(Context::default(), gpui::size(64.0, 64.0), false);
        assert!(renderer.layer.is_opaque());

        renderer.update_transparency(true);
        assert!(!renderer.layer.is_opaque());
        renderer.update_drawable_size(gpui::size(DevicePixels(128), DevicePixels(96)));
        assert!(!renderer.layer.is_opaque());

        renderer.update_transparency(false);
        renderer.update_drawable_size(gpui::size(DevicePixels(64), DevicePixels(64)));
        assert!(renderer.layer.is_opaque());
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn renders_scenes_to_images() {
        let mut renderer = MacWgpuRenderer::new(Context::default(), gpui::size(32.0, 16.0), false);
        let mut scene = Scene::default();
        scene.finish();
        let image = renderer.render_to_image(&scene).unwrap();
        assert_eq!(image.dimensions(), (32, 16));
    }
}
