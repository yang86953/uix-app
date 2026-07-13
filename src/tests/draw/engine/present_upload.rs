use crate::tests::common::*;
use crate::draw::backend::{ BackendKind, CpuBackend };
use crate::draw::engine::{ GraphicsFailure };
use crate::draw::pipeline::RenderSession;
use crate::draw::pipeline::{ EncodedFrameExecution, EncodedPictureExecution };
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::draw::engine::present_upload::*;
use crate::core::Result;
use std::sync::{ atomic::{AtomicUsize, Ordering} };

#[derive(Clone, Debug, PartialEq)]
struct PresentedFrame {
    width: i32,
    height: i32,
    len: usize,
    damage: PresentDamage,
}

#[derive(Default)]
struct RecordingPixelContext {
    frame: Arc<Mutex<Option<PresentedFrame>>>,
    width: i32,
    height: i32,
    initialize_calls: Arc<AtomicUsize>,
    fail_present: bool,
    fail_resize: bool,
    partial_present: bool,
    shutdowns: Arc<AtomicUsize>,
    checked_shutdowns: Arc<AtomicUsize>,
}

impl RecordingPixelContext {
    fn new(frame: Arc<Mutex<Option<PresentedFrame>>>) -> Self {
        Self::with_extent(frame, 4, 3)
    }

    fn with_extent(frame: Arc<Mutex<Option<PresentedFrame>>>, width: i32, height: i32) -> Self {
        Self {
            frame,
            width,
            height,
            initialize_calls: Arc::new(AtomicUsize::new(0)),
            fail_present: false,
            fail_resize: false,
            partial_present: false,
            shutdowns: Arc::new(AtomicUsize::new(0)),
            checked_shutdowns: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl IGraphicsContext for RecordingPixelContext {
    fn caps(&self) -> GraphicsContextCaps {
        let mut caps = GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::D3d11, 1.0);
        caps.partial_present = self.partial_present;
        caps
    }

    fn graphics_backend(&self) -> GraphicsBackend {
        GraphicsBackend::D3d11
    }

    fn initialize(
        &mut self,
        _native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Result<()> {
        self.initialize_calls.fetch_add(1, Ordering::SeqCst);
        self.width = width;
        self.height = height;
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<()> {
        if self.fail_resize {
            return Err(Error::new(
                crate::core::error::Errc::GraphicsSurfaceLost,
                "injected resize failure",
            ));
        }
        self.width = width;
        self.height = height;

        Ok(())
    }

    fn make_current(&mut self) -> crate::core::Result<()> {
        Ok(())
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.checked_shutdowns.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn read_pixels(
        &mut self,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<()> {
        if self.fail_present {
            return Err(Error::new(
                crate::core::error::Errc::PlatformError,
                "injected pixel-present failure",
            ));
        }
        *self.frame.lock().unwrap() = Some(PresentedFrame {
            width,
            height,
            len: pixels.len(),
            damage,
        });
        Ok(())
    }
}

#[test]
fn present_upload_engine_uses_factory_initialized_context_without_second_initialize() {
    let frame = Arc::new(Mutex::new(None));
    let context = RecordingPixelContext::with_extent(frame, 7, 5);
    let initialize_calls = Arc::clone(&context.initialize_calls);
    let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

    engine.initialize(64, 48).unwrap();

    assert_eq!(initialize_calls.load(Ordering::SeqCst), 0);
    assert_eq!((engine.session.width(), engine.session.height()), (7, 5));
}

#[test]
fn present_upload_engine_does_not_report_present_after_context_failure() {
    let frame = Arc::new(Mutex::new(None));
    let mut context = RecordingPixelContext::new(frame.clone());
    context.fail_present = true;
    let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

    engine.initialize(4, 3).unwrap();
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);

    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        RenderOutcome::Failed(GraphicsFailure::Other(error))
            if error.code() == crate::core::error::Errc::PlatformError
    ));
    assert_eq!(*frame.lock().unwrap(), None);
}

#[test]
fn present_upload_engine_propagates_resize_failure_without_committing_new_extent() {
    let frame = Arc::new(Mutex::new(None));
    let mut context = RecordingPixelContext::new(frame);
    context.fail_resize = true;
    let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();
    engine.initialize(4, 3).unwrap();

    let error = engine.resize(8, 6).expect_err("resize must be observable");

    assert_eq!(error.code(), crate::core::error::Errc::GraphicsSurfaceLost);
    assert_eq!((engine.session.width(), engine.session.height()), (4, 3));
}

#[test]
fn present_upload_engine_submits_cpu_pixels_to_context() {
    let frame = Arc::new(Mutex::new(None));
    let context = RecordingPixelContext::new(frame.clone());
    let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

    engine.initialize(4, 3).unwrap();
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    let _ = engine.end_frame(&DamageRegion::full());

    assert_eq!(
        *frame.lock().unwrap(),
        Some(PresentedFrame {
            width: 4,
            height: 3,
            len: 12,
            damage: PresentDamage::Full,
        })
    );
    assert_eq!(
        engine.capabilities(),
        GraphicsCapabilities {
            presentation_mode: crate::draw::traits::PresentationMode::EngineManaged,
            partial_redraw: true,
            offscreen: true,
        }
    );
}

#[test]
fn present_upload_upgrades_partial_damage_when_context_does_not_prove_preservation() {
    let frame = Arc::new(Mutex::new(None));
    let context = RecordingPixelContext::new(frame.clone());
    let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

    engine.initialize(4, 3).unwrap();
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    let _ = engine.end_frame(&DamageRegion::partial(vec![Rect::new(1.0, 1.0, 2.0, 1.0)]));

    assert_eq!(
        frame
            .lock()
            .unwrap()
            .as_ref()
            .map(|presented| &presented.damage),
        Some(&PresentDamage::Full)
    );
}

#[test]
fn present_upload_forwards_partial_damage_only_when_context_proves_preservation() {
    let frame = Arc::new(Mutex::new(None));
    let mut context = RecordingPixelContext::new(frame.clone());
    context.partial_present = true;
    let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

    engine.initialize(4, 3).unwrap();
    let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
    let _ = engine.end_frame(&DamageRegion::partial(vec![Rect::new(1.0, 1.0, 2.0, 1.0)]));

    assert_eq!(
        frame
            .lock()
            .unwrap()
            .as_ref()
            .map(|presented| &presented.damage),
        Some(&PresentDamage::Partial(vec![(1, 1, 2, 1)]))
    );
}

#[test]
fn present_upload_shutdown_and_drop_use_checked_context_release_once() {
    let frame = Arc::new(Mutex::new(None));
    let context = RecordingPixelContext::new(frame);
    let shutdowns = context.shutdowns.clone();
    let checked_shutdowns = context.checked_shutdowns.clone();
    let mut engine = PresentUploadEngine::new(Box::new(context)).unwrap();

    engine.try_shutdown().expect("checked shutdown");
    drop(engine);

    assert_eq!(checked_shutdowns.load(Ordering::SeqCst), 1);
    assert_eq!(shutdowns.load(Ordering::SeqCst), 0);
}
