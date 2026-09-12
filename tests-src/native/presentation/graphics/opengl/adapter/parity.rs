//! OpenGL ES 真实 WSI parity 的原生诊断与故障恢复 Adapter。

use std::ffi::c_void;

use crate::core::Errc;
use crate::platform::presentation::rhi::{
    GraphicsContextRhi, GraphicsSurface, SurfaceToken, WsiParityAdapter, WsiParityFramePresenter,
    WsiParityProfile,
};

use super::egl::EglContext;

pub(crate) struct OpenGlWsiParityAdapter;

impl WsiParityAdapter for OpenGlWsiParityAdapter {
    type Context = EglContext;

    fn profile() -> WsiParityProfile {
        WsiParityProfile::new("OpenGL ES", "UIX OpenGL ES WSI parity", true)
    }

    fn create_context(
        native_surface: *mut c_void,
        width: i32,
        height: i32,
    ) -> crate::core::Result<Self::Context> {
        EglContext::new(native_surface, width, height)
    }

    fn diagnostic(context: &Self::Context) -> String {
        let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "<unset>".to_owned());
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "<unset>".to_owned());
        let adapter = context
            .parity_adapter_diagnostic()
            .expect("the real EGL window context must report its GPU identity");
        format!("Wayland display={display}; session={session}; EGL window surface; {adapter}")
    }

    fn verify_surface_recovery(
        context: &mut Self::Context,
        initial: SurfaceToken,
        presenter: &mut dyn WsiParityFramePresenter,
    ) {
        let replacements_before = context.window_surface_replacements_for_test();
        let acquired = verify_surface_fault(context, presenter, OpenGlSurfaceFault::Acquire);
        let presented =
            verify_surface_fault(context, presenter, OpenGlSurfaceFault::PresentAfterSubmit);
        assert_eq!(acquired.generation, initial.generation + 1);
        assert_eq!(presented.generation, initial.generation + 2);
        assert_eq!(
            context.window_surface_replacements_for_test(),
            replacements_before + 2,
        );
        eprintln!(
            "OpenGL ES WSI Surface recovery verified: paths=2; generation={}->{}; real-egl-surface-replacements=2; recovered-ui-submits=2; recovered-eglSwapBuffers=2",
            initial.generation, presented.generation,
        );
    }
}

// 只选择显式 parity 故障安排位置，不拥有共享恢复状态。
#[derive(Clone, Copy, Debug)]
enum OpenGlSurfaceFault {
    Acquire,
    PresentAfterSubmit,
}

// 在同一真实 window surface 上证明一次拒绝只触发一次原生 replacement。
fn verify_surface_fault(
    context: &mut EglContext,
    presenter: &mut dyn WsiParityFramePresenter,
    fault: OpenGlSurfaceFault,
) -> SurfaceToken {
    let old = context.surface_ref().token();
    let replacements_before = context.window_surface_replacements_for_test();
    let injected = match fault {
        OpenGlSurfaceFault::Acquire => {
            context
                .surface()
                .inject_surface_lost_for_test()
                .expect("acquire SurfaceLost injection must arm at an idle boundary");
            assert_eq!(
                context.surface_ref().token(),
                old,
                "arming acquire SurfaceLost must not commit authoritative state",
            );
            presenter.present(context, None)
        }
        OpenGlSurfaceFault::PresentAfterSubmit => {
            let mut armed_after_submit = false;
            let mut inject_after_submit = |surface: &mut dyn GraphicsSurface| {
                surface
                    .inject_surface_lost_for_test()
                    .expect("present SurfaceLost injection must arm after submit");
                assert_eq!(
                    surface.token(),
                    old,
                    "arming present SurfaceLost must not commit authoritative state",
                );
                armed_after_submit = true;
            };
            let result = presenter.present(context, Some(&mut inject_after_submit));
            assert!(
                armed_after_submit,
                "present SurfaceLost must be armed after the shared RHI submit",
            );
            result
        }
    };

    let retry = injected.expect_err("SurfaceLost must reject the old OpenGL WSI frame");
    assert_eq!(retry.code(), Errc::GraphicsSurfaceChanged);
    let rebuilt = context.surface_ref().token();
    assert_eq!(rebuilt.extent, old.extent, "recovery must keep WSI extent");
    assert_eq!(
        rebuilt.generation,
        old.generation + 1,
        "recovery must commit exactly one shared generation",
    );
    assert_eq!(
        context.window_surface_replacements_for_test(),
        replacements_before + 1,
        "one SurfaceLost must replace the real EGLSurface exactly once",
    );

    let recovered = presenter
        .present(context, None)
        .expect("the next real UI production frame must submit and eglSwapBuffers");
    assert_eq!(recovered, rebuilt);
    assert_eq!(
        context.window_surface_replacements_for_test(),
        replacements_before + 1,
        "the recovered frame must reuse the single replacement EGLSurface",
    );
    eprintln!(
        "OpenGL ES WSI recovery path: fault={fault:?}; old={}x{}@{}; new={}x{}@{}; injection-token=unchanged; retry=RetryFrame(GraphicsSurfaceChanged); egl-surface-replacements=1; recovered-submit=ok; recovered-eglSwapBuffers=ok@{}",
        old.extent.width,
        old.extent.height,
        old.generation,
        rebuilt.extent.width,
        rebuilt.extent.height,
        rebuilt.generation,
        recovered.generation,
    );
    rebuilt
}
