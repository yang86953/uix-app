//! 逐窗可见性、surface 可用性与 Agent 可呈现状态。

use super::WindowDriver;
use crate::app::window_semantics::{AgentWindowState, WindowSemanticState};
use crate::draw::target::RenderTarget;
use crate::platform::windowing::window::{PlatformWindow, WindowOcclusionState};

impl WindowDriver {
    pub(super) fn cancel_outstanding_native_frame(
        &mut self,
        platform_window: &mut dyn PlatformWindow,
    ) {
        let Some(token) = self.frame_scheduler.outstanding_native_token() else {
            return;
        };
        if let Err(error) = platform_window.cancel_native_frame(token) {
            // 取消失败是瞬态平台噪声：经 observe_transient 冷却去重观察。
            self.diagnostics.observe_transient_error(
                "window_driver",
                "native frame cancellation failed", &error);
        }
    }

    pub(super) fn agent_surface_presentable(&self, platform_window: &dyn PlatformWindow) -> bool {
        let properties = platform_window.properties();
        let visibility_allows_work = self.deferred_show || platform_window.is_visible();
        visibility_allows_work
            && properties.width() > 0
            && properties.height() > 0
            && platform_window.occlusion_state() != WindowOcclusionState::Occluded
            && self.frame_scheduler.is_renderable()
    }

    pub(super) fn publish_agent_window_availability(
        &self,
        semantic_state: &mut WindowSemanticState,
        platform_window: &dyn PlatformWindow,
        engine: &dyn RenderTarget,
    ) {
        let properties = platform_window.properties();
        semantic_state.publish_agent_window_state(AgentWindowState {
            visible: platform_window.is_visible(),
            presentable: self.agent_surface_presentable(platform_window),
            focused: self.window_focused,
            logical_width: properties.width(),
            logical_height: properties.height(),
            // 语义 bounds（logical）与截屏像素（physical）之间的换算事实。
            device_pixel_ratio: engine.device_pixel_ratio(),
            maximized: properties.is_maximized(),
            minimized: properties.is_minimized(),
            fullscreen: properties.is_fullscreen(),
        });
    }
}
