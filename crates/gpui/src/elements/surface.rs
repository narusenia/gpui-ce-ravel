use crate::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    ObjectFit, Pixels, Style, StyleRefinement, Styled, Window,
};
#[cfg(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "windows"
))]
use crate::{DevicePixels, Size};
#[cfg(target_os = "macos")]
use core::ffi::c_void;
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
use refineable::Refineable;
use std::sync::Arc;

/// A notification delivered after the renderer has finished consuming a
/// texture-backed surface.
///
/// The callback runs on the renderer's completion path and is retained by the
/// submitted command until that command finishes. Resource providers can use
/// it to release a borrowed GPU resource without making the renderer know the
/// resource's concrete type or ownership rules.
pub type SurfaceCompletion = Arc<dyn Fn() + Send + Sync + 'static>;

/// A source of a surface's content.
pub enum SurfaceSource {
    /// A macOS image buffer from CoreVideo
    #[cfg(target_os = "macos")]
    Surface(CVPixelBuffer),
    /// A straight RGBA Metal texture, type-erased to avoid depending on the
    /// Metal bindings. The pointer is borrowed from the owner for the life of
    /// the scene that consumes it; it is never retained or released here.
    #[cfg(target_os = "macos")]
    Texture {
        /// An `id<MTLTexture>` represented opaquely so callers need not agree
        /// on an `objc2` / `metal` binding version.
        texture: *mut c_void,
        /// Dimensions of the texture in device pixels.
        size: Size<DevicePixels>,
        /// Called after the renderer's command buffer has finished sampling
        /// the texture, if the provider needs a completion signal.
        completion: Option<SurfaceCompletion>,
    },
    /// A GPU texture handle (type-erased to avoid depending on wgpu)
    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
    Texture {
        /// The GPU texture, type-erased (expected to be `Arc<wgpu::Texture>`)
        texture: Arc<dyn std::any::Any + Send + Sync>,
        /// Dimensions of the texture in device pixels
        size: Size<DevicePixels>,
    },
}

impl Clone for SurfaceSource {
    fn clone(&self) -> Self {
        match *self {
            #[cfg(target_os = "macos")]
            SurfaceSource::Surface(ref buf) => SurfaceSource::Surface(buf.clone()),
            #[cfg(target_os = "macos")]
            SurfaceSource::Texture {
                texture,
                size,
                ref completion,
            } => SurfaceSource::Texture {
                texture,
                size,
                completion: completion.clone(),
            },
            #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
            SurfaceSource::Texture { ref texture, size } => SurfaceSource::Texture {
                texture: Arc::clone(texture),
                size,
            },
        }
    }
}

impl std::fmt::Debug for SurfaceSource {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            #[cfg(target_os = "macos")]
            SurfaceSource::Surface(ref buf) => _f.debug_tuple("Surface").field(buf).finish(),
            #[cfg(target_os = "macos")]
            SurfaceSource::Texture { size, .. } => _f
                .debug_struct("Texture")
                .field("size", &size)
                .finish_non_exhaustive(),
            #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
            SurfaceSource::Texture { size, .. } => _f
                .debug_struct("Texture")
                .field("size", &size)
                .finish_non_exhaustive(),
        }
    }
}

#[cfg(target_os = "macos")]
impl From<CVPixelBuffer> for SurfaceSource {
    fn from(value: CVPixelBuffer) -> Self {
        SurfaceSource::Surface(value)
    }
}

/// A surface element.
pub struct Surface {
    source: SurfaceSource,
    object_fit: ObjectFit,
    style: StyleRefinement,
}

/// Create a new surface element.
pub fn surface(source: impl Into<SurfaceSource>) -> Surface {
    Surface {
        source: source.into(),
        object_fit: ObjectFit::Contain,
        style: Default::default(),
    }
}

impl Surface {
    /// Set the object fit for the image.
    pub fn object_fit(mut self, object_fit: ObjectFit) -> Self {
        self.object_fit = object_fit;
        self
    }
}

impl Element for Surface {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        _window: &mut Window,
        _: &mut App,
    ) {
        match self.source {
            #[cfg(target_os = "macos")]
            SurfaceSource::Surface(ref surface) => {
                let size = crate::size(surface.get_width().into(), surface.get_height().into());
                let new_bounds = self.object_fit.get_bounds(_bounds, size);
                // TODO: Add support for corner_radii
                _window.paint_surface(new_bounds, surface.clone());
            }
            #[cfg(target_os = "macos")]
            SurfaceSource::Texture { ref size, .. } => {
                let new_bounds = self.object_fit.get_bounds(_bounds, *size);
                // TODO: Add support for corner_radii
                _window.paint_surface(new_bounds, self.source.clone());
            }
            #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
            SurfaceSource::Texture {
                ref texture,
                ref size,
            } => {
                let new_bounds = self.object_fit.get_bounds(_bounds, *size);
                _window.paint_surface(new_bounds, Arc::clone(texture), *size, None);
            }
        }
    }
}

impl IntoElement for Surface {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for Surface {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn texture_completion_survives_source_clone() {
        let calls = Arc::new(AtomicUsize::new(0));
        let callback_calls = Arc::clone(&calls);
        let source = SurfaceSource::Texture {
            texture: std::ptr::null_mut(),
            size: crate::size(DevicePixels(1), DevicePixels(1)),
            completion: Some(Arc::new(move || {
                callback_calls.fetch_add(1, Ordering::Relaxed);
            })),
        };

        let cloned = source.clone();
        let SurfaceSource::Texture {
            completion: Some(completion),
            ..
        } = cloned
        else {
            panic!("texture completion was not cloned");
        };
        completion();

        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}
