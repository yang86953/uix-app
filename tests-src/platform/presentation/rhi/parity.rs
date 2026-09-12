//! 显式图形 parity 使用的 API 中立测试端口。

use std::ffi::c_void;

use crate::core::Result;
use crate::platform::presentation::GraphicsContextLifecycle;

use super::{GraphicsContextRhi, GraphicsSurface, RhiExtent, SurfaceToken, TextureHandle};

// 冻结一次真实 WSI 验收所需的展示信息与平台事件观测选择。
#[derive(Clone, Copy)]
pub(crate) struct WsiParityProfile {
    backend_label: &'static str,
    window_title: &'static str,
    observe_platform_pacing: bool,
}

impl WsiParityProfile {
    // 具体 Adapter 只声明自身事实，不取得共享帧数、超时或场景权威。
    pub(crate) const fn new(
        backend_label: &'static str,
        window_title: &'static str,
        observe_platform_pacing: bool,
    ) -> Self {
        Self {
            backend_label,
            window_title,
            observe_platform_pacing,
        }
    }

    pub(crate) const fn backend_label(self) -> &'static str {
        self.backend_label
    }

    pub(crate) const fn window_title(self) -> &'static str {
        self.window_title
    }

    pub(crate) const fn observe_platform_pacing(self) -> bool {
        self.observe_platform_pacing
    }
}

// Adapter 只调用该窄端口请求共享 UI 帧，不得取得或复制 Drawing 场景。
pub(crate) trait WsiParityFramePresenter {
    fn present(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        before_present: Option<&mut dyn FnMut(&mut dyn GraphicsSurface)>,
    ) -> Result<SurfaceToken>;
}

// 具体 WSI Adapter 实现创建、诊断和故障恢复，组合根只消费本合同。
pub(crate) trait WsiParityAdapter {
    type Context: GraphicsContextRhi + GraphicsContextLifecycle;

    fn profile() -> WsiParityProfile;

    fn create_context(
        native_surface: *mut c_void,
        width: i32,
        height: i32,
    ) -> Result<Self::Context>;

    fn diagnostic(context: &Self::Context) -> String;

    fn verify_surface_recovery(
        context: &mut Self::Context,
        initial: SurfaceToken,
        presenter: &mut dyn WsiParityFramePresenter,
    );
}

// 离屏 UI parity 同样只向组合根暴露中立 Device 与机械回读端口。
pub(crate) trait HeadlessUiParityAdapter {
    type Context: GraphicsContextRhi + GraphicsContextLifecycle;

    fn create_context(extent: RhiExtent) -> Result<Self::Context>;

    fn diagnostic(context: &Self::Context) -> String;

    fn readback(context: &mut Self::Context, texture: TextureHandle) -> Vec<u8>;
}
