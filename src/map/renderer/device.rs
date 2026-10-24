// Native resource integration for the ported TinyTorrent2 MapRenderer.cs.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::HashMap;
use windows::{
    Win32::{
        Foundation::{HMODULE, HWND},
        Graphics::{
            Direct2D::{
                Common::{
                    D2D1_ALPHA_MODE_IGNORE, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
                    D2D1_FIGURE_END_OPEN, D2D1_FILL_MODE_ALTERNATE, D2D1_PIXEL_FORMAT,
                },
                D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
                D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
                D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_LINE_JOIN_ROUND,
                D2D1_STROKE_STYLE_PROPERTIES1, D2D1_STROKE_TRANSFORM_TYPE_FIXED, D2D1CreateFactory,
                ID2D1Bitmap1, ID2D1DeviceContext3, ID2D1Factory1, ID2D1GeometryRealization,
                ID2D1Image, ID2D1PathGeometry1, ID2D1SolidColorBrush, ID2D1StrokeStyle1,
            },
            Direct3D::{
                D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0,
                D3D_FEATURE_LEVEL_11_1,
            },
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device, ID3D11DeviceContext,
            },
            Dxgi::{
                Common::{DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
                DXGI_MWA_NO_ALT_ENTER, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
                DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_DISCARD,
                DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGIOutput,
                IDXGISurface, IDXGISwapChain1,
            },
        },
    },
    core::{Error, Interface, Result},
};
use windows_numerics::{Matrix3x2, Vector2};

pub struct Device {
    pub contrast: bool,
    pub context: ID2D1DeviceContext3,
    pub factory: ID2D1Factory1,
    pub mode: &'static str,
    pub land: ID2D1SolidColorBrush,
    pub edge: ID2D1SolidColorBrush,
    pub fixed_stroke: ID2D1StrokeStyle1,
    rounded: ID2D1StrokeStyle1,
    d3d: ID3D11Device,
    swap_factory: IDXGIFactory2,
    immediate: ID3D11DeviceContext,
    targets: HashMap<isize, Target>,
    target: isize,
    size: (u32, u32, u32),
    #[cfg(test)]
    pub(super) gpu: Option<super::resource_checks::Gpu>,
    #[cfg(test)]
    pub(super) observations: Option<std::rc::Rc<crate::map::Presentations>>,
}

struct Target {
    bitmap: Option<ID2D1Bitmap1>,
    swap_chain: IDXGISwapChain1,
    size: (u32, u32, u32),
}

impl Device {
    pub fn new() -> Result<Self> {
        // SAFETY: All objects remain on this UI thread. COM retains each
        // device dependency for the lifetime of the resources created from it.
        unsafe {
            let mut device = None;
            let mut immediate = None;
            let mut mode = "Hardware D3D11";
            let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
            if D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut immediate),
            )
            .is_err()
            {
                mode = "Software D3D11 (WARP)";
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_WARP,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut immediate),
                )?;
            }
            let device: ID3D11Device = device.unwrap();
            let dxgi: IDXGIDevice = device.cast()?;
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let context: ID2D1DeviceContext3 = factory
                .CreateDevice(&dxgi)?
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?
                .cast()?;
            let swap_factory: IDXGIFactory2 = dxgi.GetAdapter()?.GetParent()?;
            let palette = crate::map::Palette::default();
            let land = context.CreateSolidColorBrush(&palette.land, None)?;
            let edge = context.CreateSolidColorBrush(&palette.edge, None)?;
            let style = D2D1_STROKE_STYLE_PROPERTIES1 {
                lineJoin: D2D1_LINE_JOIN_ROUND,
                ..Default::default()
            };
            let rounded = factory.CreateStrokeStyle(&style, None)?;
            let fixed_stroke = factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES1 {
                    transformType: D2D1_STROKE_TRANSFORM_TYPE_FIXED,
                    ..style
                },
                None,
            )?;
            Ok(Self {
                contrast: false,
                context,
                factory,
                mode,
                land,
                edge,
                rounded,
                fixed_stroke,
                d3d: device,
                swap_factory,
                immediate: immediate.unwrap(),
                targets: HashMap::new(),
                target: 0,
                size: (0, 0, 0),
                #[cfg(test)]
                gpu: None,
                #[cfg(test)]
                observations: None,
            })
        }
    }

    pub fn attach(&mut self, window: HWND) -> Result<()> {
        let key = window.0 as isize;
        if self.targets.contains_key(&key) {
            return Ok(());
        }
        // SAFETY: All targets use this UI thread's device and live native windows.
        let swap_chain = unsafe {
            self.swap_factory
                .MakeWindowAssociation(window, DXGI_MWA_NO_ALT_ENTER)?;
            self.swap_factory.CreateSwapChainForHwnd(
                &self.d3d,
                window,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: 1,
                    Height: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    Scaling: DXGI_SCALING_STRETCH,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                    AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                    ..Default::default()
                },
                None,
                None::<&IDXGIOutput>,
            )?
        };
        self.targets.insert(
            key,
            Target {
                swap_chain,
                bitmap: None,
                size: (1, 1, 0),
            },
        );
        Ok(())
    }

    pub fn retain_targets(&mut self, mut keep: impl FnMut(HWND) -> bool) {
        // Detach before dropping any bitmap or flip chain. Flush native deferred
        // destruction before a later attachment can reuse the same HWND.
        // SAFETY: Resource retirement remains on the owning UI thread.
        unsafe {
            self.context.SetTarget(None::<&ID2D1Image>);
        }
        let count = self.targets.len();
        self.targets.retain(|&key, _| keep(HWND(key as _)));
        if self.targets.len() != count {
            // SAFETY: Flush submits destruction without waiting for GPU completion.
            unsafe {
                self.immediate.ClearState();
                self.immediate.Flush();
            }
        }
    }

    pub fn select(&mut self, window: HWND, width: u32, height: u32, dpi: u32) -> Result<bool> {
        let key = window.0 as isize;
        let target = self.targets.get_mut(&key).unwrap();
        let dpi_changed = target.size.2 != dpi;
        // SAFETY: Release the context and retained bitmap references before
        // ResizeBuffers. All other HWND targets retain their own buffers.
        unsafe {
            if target.size != (width, height, dpi) {
                self.context.SetTarget(None::<&ID2D1Image>);
                target.bitmap = None;
                target.swap_chain.ResizeBuffers(
                    2,
                    width,
                    height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )?;
            }
            if target.bitmap.is_none() {
                let surface: IDXGISurface = target.swap_chain.GetBuffer(0)?;
                target.bitmap = Some(self.context.CreateBitmapFromDxgiSurface(
                    &surface,
                    Some(&D2D1_BITMAP_PROPERTIES1 {
                        pixelFormat: D2D1_PIXEL_FORMAT {
                            format: DXGI_FORMAT_B8G8R8A8_UNORM,
                            alphaMode: D2D1_ALPHA_MODE_IGNORE,
                        },
                        dpiX: dpi as f32,
                        dpiY: dpi as f32,
                        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                        ..Default::default()
                    }),
                )?);
            }
            self.context.SetTarget(target.bitmap.as_ref().unwrap());
            self.context.SetDpi(dpi as f32, dpi as f32);
        }
        target.size = (width, height, dpi);
        self.target = key;
        self.size = (width, height, dpi);
        Ok(dpi_changed)
    }

    pub fn apply_appearance(&mut self, appearance: crate::map::Appearance) {
        self.contrast = appearance.contrast.is_some();
        let colors = appearance.colors();
        // SAFETY: Brush updates stay on their owning render thread.
        unsafe {
            self.land.SetColor(&colors.land);
            self.edge.SetColor(&colors.edge);
        }
    }

    pub fn begin(&self, appearance: crate::map::Appearance) {
        #[cfg(test)]
        if let Some(gpu) = &self.gpu {
            gpu.begin();
        }
        // SAFETY: Renderer brackets drawing on this thread with one live target.
        unsafe {
            self.context.BeginDraw();
            self.context.Clear(Some(&appearance.colors().background));
        }
    }

    pub fn end(&self) -> Result<()> {
        // SAFETY: Renderer pairs this with BeginDraw, including drawing failures.
        unsafe {
            let (mut stage, mut detail) = (0, 0);
            self.context
                .EndDraw(Some(&mut stage), Some(&mut detail))
                .map_err(|error| {
                    Error::new(
                        error.code(),
                        format!("Map draw stage {stage}/{detail}: {error}"),
                    )
                })?;
            #[cfg(test)]
            if let Some(gpu) = &self.gpu {
                gpu.end();
            }
            #[cfg(test)]
            if let Some(observations) = &self.observations
                && let Some(path) = observations.capture.borrow_mut().take()
            {
                // A requested test image must copy this completed drawing
                // before flip-model Present advances to another back buffer.
                super::resource_checks::readback(&self.context, &path)?;
            }
            #[cfg(test)]
            if let Some(observations) = &self.observations {
                observations.attempts.set(observations.attempts.get() + 1);
            }
            self.targets[&self.target]
                .swap_chain
                .Present(1, Default::default())
                .ok()?;
            #[cfg(test)]
            if let Some(observations) = &self.observations {
                observations.returned.set(Some(std::time::Instant::now()));
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn immediate(&self) -> &ID3D11DeviceContext {
        &self.immediate
    }

    pub fn border_width(&self) -> f32 {
        0.5f32.max(96.0 / self.size.2 as f32)
    }

    pub fn tolerance(&self, scale: f32) -> f32 {
        0.25 * 96.0 / self.size.2 as f32 / scale
    }

    pub fn path<'a>(
        &self,
        figures: impl IntoIterator<Item = &'a [Vector2]>,
        closed: bool,
    ) -> Result<ID2D1PathGeometry1> {
        // SAFETY: The sink is local, its input slices live through each native copy,
        // and Close completes the path before exposing it to drawing or realization.
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            sink.SetFillMode(D2D1_FILL_MODE_ALTERNATE);
            for points in figures {
                let Some(first) = points.first() else {
                    continue;
                };
                sink.BeginFigure(*first, D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLines(&points[1..]);
                sink.EndFigure(if closed {
                    D2D1_FIGURE_END_CLOSED
                } else {
                    D2D1_FIGURE_END_OPEN
                });
            }
            sink.Close()?;
            Ok(path)
        }
    }

    pub fn stroke(&self, path: &ID2D1PathGeometry1, scale: Vector2, width: f32) -> Result<Stroke> {
        // SAFETY: Widen after both axis scales so the pen stays round. The cached
        // realization owns its tessellation; temporary transformed geometry can drop.
        unsafe {
            let widened = self
                .factory
                .CreateTransformedGeometry(path, &Matrix3x2::scale(scale.X, scale.Y))?;
            let geometry = self.context.CreateStrokedGeometryRealization(
                &widened,
                self.tolerance(1.0),
                width,
                &self.rounded,
            )?;
            Ok(Stroke { geometry, scale })
        }
    }
}

pub struct Stroke {
    pub geometry: ID2D1GeometryRealization,
    pub scale: Vector2,
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: Retire the bitmap and D3D bindings on their owning thread.
        unsafe {
            self.context.SetTarget(None::<&ID2D1Image>);
            self.immediate.ClearState();
        }
        // A flip chain's final release must precede the destruction flush before
        // Renderer can create another chain for the same HWND during recovery.
        self.targets.clear();
        // SAFETY: The retained immediate context submits deferred destruction;
        // this is not a wait for GPU completion.
        unsafe {
            self.immediate.Flush();
        }
    }
}

impl Stroke {
    pub fn current(&self, scale: Vector2, tolerance: f32) -> bool {
        let change = scale / self.scale - Vector2::new(1.0, 1.0);
        change.X.abs() <= tolerance && change.Y.abs() <= tolerance
    }
}
