//! 逐窗可见性、surface 可用性与 Agent 可呈现状态。

use super::WindowDriver;
use crate::app::window_semantics::WindowSemanticState;
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
            tracing::warn!(
                "[WindowDriver] native frame cancellation failed: {}",
                error.short_what()
            );
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
        semantic_state: &WindowSemanticState,
        platform_window: &dyn PlatformWindow,
    ) {
        semantic_state.publish_agent_availability(
            platform_window.is_visible(),
            self.agent_surface_presentable(platform_window),
        );
    }
}
