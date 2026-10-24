use super::*;
use crate::{
    connections::{
        Bytes, Endpoint, Life, Observation, ObservationId, ProcessId, Rates, TcpState, Tuple,
    },
    geography::{Point, World},
    geolocation::Location,
    map::{
        Appearance, Inspection,
        model::{Located, Model},
    },
};
use std::{path::Path, sync::Arc};
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct2D::{Common::*, *},
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        },
        UI::WindowsAndMessaging::*,
    },
    core::{Interface, w},
};

struct Window(HWND);

impl Window {
    fn new(width: i32, height: i32) -> Self {
        // SAFETY: STATIC is a system class. These hidden, unparented test HWNDs
        // have no application callback, borrowed state or system changes.
        Self(unsafe {
            CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("NetMap renderer proof"),
                WS_POPUP,
                0,
                0,
                width,
                height,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        })
    }
}

fn settle(
    renderer: &mut Renderer,
    window: HWND,
    prepared: &Prepared,
    width: u32,
    height: u32,
    dpi: u32,
) -> Frame {
    for _ in 0..256 {
        let frame = renderer
            .paint_target(window, prepared, width, height, dpi)
            .unwrap();
        if !matches!(frame, Frame::Again) {
            return frame;
        }
    }
    panic!("Renderer did not finish its bounded rebuilds");
}

fn pixel(renderer: &Renderer, x: u32, y: u32) -> [u8; 3] {
    let device = renderer.device.as_ref().unwrap();
    // SAFETY: Drawing has ended. The staging bitmap and mapped bytes remain
    // owned here through Unmap; the fixture alone uses this context.
    unsafe {
        let target: ID2D1Bitmap1 = device.context.GetTarget().unwrap().cast().unwrap();
        let staging = device
            .context
            .CreateBitmap(
                target.GetPixelSize(),
                None,
                0,
                &D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_IGNORE,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                    ..Default::default()
                },
            )
            .unwrap();
        staging.CopyFromBitmap(None, &target, None).unwrap();
        let mapped = staging.Map(D2D1_MAP_OPTIONS_READ).unwrap();
        let offset = y as usize * mapped.pitch as usize + x as usize * 4;
        let rgb = std::slice::from_raw_parts(mapped.bits.add(offset), 3)
            .try_into()
            .unwrap();
        staging.Unmap().unwrap();
        rgb
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: This fixture alone owns the HWND on its creating test thread;
        // Renderer was declared after it and releases its target first.
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

#[test]
#[ignore = "Creates an owned hidden D3D/D2D window; checks native sprite/effect/static rendering"]
fn native_active_and_idle_marks_draw_and_static_modes_stop_animation() {
    let window = Window::new(640, 360);
    let second = Window::new(960, 540);
    let mut renderer = Renderer::new(window.0).unwrap();
    let world = Arc::new(
        World::decode(
            &std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/world.bin")).unwrap(),
        )
        .unwrap(),
    );
    let locations = [
        Location {
            code: "US".into(),
            country: "United States".into(),
            city: None,
            point: Some(Point {
                latitude: 40.0,
                longitude: -100.0,
            }),
        },
        Location {
            code: "JP".into(),
            country: "Japan".into(),
            city: None,
            point: Some(Point {
                latitude: 35.0,
                longitude: 139.0,
            }),
        },
    ];
    let observations: Vec<_> = ["8.8.8.8:443", "1.1.1.1:443"]
        .iter()
        .enumerate()
        .map(|(index, address)| Observation {
            id: ObservationId(index as u64 + 1),
            tuple: Tuple {
                process: ProcessId {
                    pid: std::process::id(),
                    generation: 1,
                },
                local: Endpoint::new("127.0.0.1:40000".parse().unwrap()),
                remote: Endpoint::new(address.parse().unwrap()),
            },
            life: Life::Tcp(TcpState::Established),
            introduced: Duration::from_secs(10),
            bytes: Bytes::default(),
            rates: Some(Rates {
                received: if index == 0 { 500_000.0 } else { 0.0 },
                sent: if index == 0 { 200_000.0 } else { 0.0 },
            }),
        })
        .collect();
    let mut viewport = Viewport::default();
    viewport.resize(640.0, 360.0);
    let model = Arc::new(Model::arrange(
        &world,
        observations
            .iter()
            .zip(&locations)
            .map(|(observation, location)| Located {
                observation,
                location: Some(location),
            }),
        true,
        0.0,
        viewport.scale,
    ));
    let mut prepared = Prepared {
        viewport,
        world: Some(world),
        model,
        origin: Some(Point {
            latitude: 0.0,
            longitude: 0.0,
        }),
        now: Duration::from_secs(10),
        highlight: Some(Highlight {
            bright: Duration::from_secs(1),
            fade: Duration::from_secs(2),
        }),
        inspection: Inspection::default(),
        appearance: Appearance::default(),
        speeds: crate::map::Speeds::default(),
        navigation_bounds: None,
        revision: 0,
    };
    let settled = |renderer: &mut Renderer, prepared: &Prepared| {
        settle(renderer, window.0, prepared, 640, 360, 96)
    };
    assert!(
        matches!(settled(&mut renderer, &prepared), Frame::After(delay) if delay == Duration::from_millis(33)),
        "Measured bidirectional traffic should animate"
    );
    prepared.appearance.animations = false;
    assert!(
        matches!(settled(&mut renderer, &prepared), Frame::After(delay) if delay == Duration::from_secs(3)),
        "Reduced motion should use the removal deadline, without pulse/fade frames"
    );
    // Each flip buffer must contain the current appearance before readback.
    prepared.appearance.palette =
        crate::map::Palette::new(true, crate::map::palette::argb(0xff99ebff));
    settled(&mut renderer, &prepared);
    settled(&mut renderer, &prepared);
    assert_eq!(
        pixel(&renderer, 0, 0),
        [44, 44, 44],
        "Source Dark background"
    );
    prepared.appearance.palette = crate::map::Palette::default();
    settled(&mut renderer, &prepared);
    settled(&mut renderer, &prepared);
    assert_eq!(
        pixel(&renderer, 0, 0),
        [255, 255, 255],
        "Source Light background"
    );

    // Three observations at one place retain their distinct directional dots.
    let original = Arc::clone(&prepared.model);
    let original_palette = prepared.appearance.palette;
    let original_origin = prepared.origin.take();
    let mixed: Vec<_> = [
        Some(Rates {
            received: 4.0,
            sent: 1.0,
        }),
        Some(Rates {
            received: 1.0,
            sent: 4.0,
        }),
        None,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, rates)| {
        let mut observation = observations[0].clone();
        observation.id = ObservationId(index as u64 + 10);
        observation.rates = rates;
        observation
    })
    .collect();
    prepared.model = Arc::new(Model::arrange(
        prepared.world.as_ref().unwrap(),
        mixed.iter().map(|observation| Located {
            observation,
            location: Some(&locations[0]),
        }),
        true,
        0.0,
        viewport.scale,
    ));
    let color = |r, g, b| D2D1_COLOR_F { r, g, b, a: 1.0 };
    prepared.appearance.palette.download = color(1.0, 0.0, 0.0);
    prepared.appearance.palette.upload = color(0.0, 1.0, 0.0);
    prepared.appearance.palette.ink = color(0.0, 0.0, 1.0);
    settled(&mut renderer, &prepared);
    settled(&mut renderer, &prepared);
    let position = viewport.screen(locations[0].point.unwrap());
    for (offset, expected) in [(-5.0, [0, 0, 255]), (0.0, [0, 255, 0]), (5.0, [255, 0, 0])] {
        assert_eq!(
            pixel(&renderer, (position.X + offset) as u32, position.Y as u32),
            expected
        );
    }
    prepared.appearance.contrast = Some((color(0.0, 0.0, 0.0), color(1.0, 1.0, 1.0)));
    settled(&mut renderer, &prepared);
    settled(&mut renderer, &prepared);
    for offset in [-5.0, 0.0, 5.0] {
        assert_eq!(
            pixel(&renderer, (position.X + offset) as u32, position.Y as u32),
            [255, 255, 255]
        );
    }
    prepared.appearance.contrast = None;
    prepared.appearance.palette = original_palette;
    prepared.origin = original_origin;
    prepared.model = original;

    let city_model = Arc::clone(&prepared.model);
    let origin = prepared.origin.take();
    let country_model = |observations: &[Observation]| {
        Arc::new(Model::arrange(
            prepared.world.as_ref().unwrap(),
            observations
                .iter()
                .zip(&locations)
                .map(|(observation, location)| Located {
                    observation,
                    location: Some(location),
                }),
            false,
            0.0,
            viewport.scale,
        ))
    };
    prepared.model = country_model(&observations);
    settled(&mut renderer, &prepared);
    settled(&mut renderer, &prepared);
    let interior = viewport.screen(Point {
        latitude: 38.0,
        longitude: -116.0,
    });
    let active = pixel(&renderer, interior.X as u32, interior.Y as u32);
    let idle: Vec<_> = observations
        .iter()
        .cloned()
        .map(|mut row| {
            row.rates = Some(Rates {
                received: 0.0,
                sent: 0.0,
            });
            row
        })
        .collect();
    prepared.model = country_model(&idle);
    settled(&mut renderer, &prepared);
    settled(&mut renderer, &prepared);
    let bright = pixel(&renderer, interior.X as u32, interior.Y as u32);
    assert_ne!(
        active, bright,
        "Measured activity must change estimated-country shading"
    );
    prepared.now = Duration::from_secs(14);
    assert!(matches!(settled(&mut renderer, &prepared), Frame::Idle));
    settled(&mut renderer, &prepared);
    assert_ne!(
        bright,
        pixel(&renderer, interior.X as u32, interior.Y as u32),
        "An idle country highlight must disappear at its deadline"
    );
    prepared.model = city_model;
    prepared.origin = origin;
    prepared.now = Duration::from_secs(10);
    prepared.speeds.rates = Some(Rates {
        received: 1024.0 * 1024.0,
        sent: 512.0 * 1024.0,
    });
    for corner in [
        crate::map::SpeedCorner::TopLeft,
        crate::map::SpeedCorner::BottomLeft,
        crate::map::SpeedCorner::BottomRight,
    ] {
        prepared.speeds.corner = corner;
        settled(&mut renderer, &prepared);
    }
    prepared.appearance.contrast = Some((
        D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    ));
    assert!(
        matches!(settled(&mut renderer, &prepared), Frame::After(delay) if delay == Duration::from_secs(3))
    );
    // Present rotates the two-buffer flip chain. Warm both buffers before
    // reading the current target, which may already be the next back buffer.
    settled(&mut renderer, &prepared);
    assert_eq!(
        pixel(&renderer, 0, 0),
        [0, 0, 0],
        "High Contrast must use the supplied system background"
    );
    let expired = prepared
        .model
        .destinations
        .iter()
        .find(|destination| prepared.model.groups[destination.group].place.code.as_ref() == "JP")
        .unwrap();
    let position = prepared.viewport.screen(expired.point);
    prepared.inspection = Inspection {
        pointed: Some(expired.key.clone()),
        selected: Some(prepared.model.groups[expired.group].place.clone()),
    };
    prepared.now = Duration::from_secs(14);
    assert!(
        matches!(settled(&mut renderer, &prepared), Frame::Idle),
        "A selected, expired idle mark cannot keep a static animation timer alive"
    );
    assert!(
        renderer
            .hit(
                &prepared.viewport,
                position,
                prepared.now,
                prepared.highlight.unwrap(),
                true
            )
            .is_none(),
        "Expired marks must leave hit testing even while selected"
    );

    // A second target shares the device/context while retaining its own extent
    // and physical-pixel DPI. First use equal DIP extents for shared map detail.
    let context = renderer.device.as_ref().unwrap().context.clone();
    let factory = renderer.device.as_ref().unwrap().factory.clone();
    renderer.attach(second.0).unwrap();
    settle(&mut renderer, second.0, &prepared, 960, 540, 144);
    settle(&mut renderer, second.0, &prepared, 960, 540, 144);
    assert_eq!(renderer.targets.len(), 2);
    assert_eq!(renderer.device.as_ref().unwrap().context, context);
    assert_eq!(renderer.device.as_ref().unwrap().factory, factory);
    assert_eq!(pixel(&renderer, 0, 0), [0, 0, 0]);
    let active = prepared.viewport.screen(locations[0].point.unwrap());
    assert_eq!(
        pixel(&renderer, (active.X * 1.5) as u32, (active.Y * 1.5) as u32),
        [255, 255, 255]
    );

    // Resizing one target cannot replace the other's chain or render caches.
    prepared
        .viewport
        .resize(1280.0 * 96.0 / 144.0, 720.0 * 96.0 / 144.0);
    settle(&mut renderer, second.0, &prepared, 1280, 720, 144);
    settle(&mut renderer, second.0, &prepared, 1280, 720, 144);
    let active = prepared.viewport.screen(locations[0].point.unwrap());
    assert_eq!(
        pixel(&renderer, (active.X * 1.5) as u32, (active.Y * 1.5) as u32),
        [255, 255, 255]
    );
    prepared.viewport.resize(640.0, 360.0);
    settled(&mut renderer, &prepared);
    let active = prepared.viewport.screen(locations[0].point.unwrap());
    assert_eq!(
        pixel(&renderer, active.X as u32, active.Y as u32),
        [255, 255, 255]
    );
    assert_eq!(renderer.device.as_ref().unwrap().context, context);

    // Retirement flushes the old flip chain before this same HWND reattaches.
    renderer.retain_targets(|target| target == window.0);
    assert_eq!(renderer.targets.len(), 1);
    renderer.attach(second.0).unwrap();
    settle(&mut renderer, second.0, &prepared, 960, 540, 144);
    settle(&mut renderer, second.0, &prepared, 960, 540, 144);
    assert_eq!(renderer.device.as_ref().unwrap().context, context);
    assert_eq!(pixel(&renderer, 0, 0), [0, 0, 0]);
    drop(context);
    drop(factory);
    renderer.retain_targets(|_| false);
    assert!(renderer.targets.is_empty());
    assert!(renderer.outlines.is_none());
    assert!(renderer.device.is_none());
    settled(&mut renderer, &prepared);
    settled(&mut renderer, &prepared);
    assert_eq!(pixel(&renderer, 0, 0), [0, 0, 0]);

    prepared.viewport.resize(1.0, 1.0);
    assert!(
        matches!(renderer.paint(&prepared, 1, 1, 96).unwrap(), Frame::Idle),
        "A temporarily undrawable view must remain valid"
    );
}
