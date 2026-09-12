//! `ui/widget_runtime/widget/tree_core/methods.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::ui::theme::Theme;

// 主题令牌实际变化必须请求一次根声明协调。
#[test]
fn changed_theme_tokens_request_root_reconcile() {
    let mut tree = WidgetTree::default();
    assert!(!tree.take_reconcile_requested());
    // 新树以默认亮色起步：切换到暗色代表令牌实际变化。
    let dark = Theme::antd_dark().tokens_arc();
    tree.set_theme_tokens(dark.clone());
    assert!(tree.take_reconcile_requested());
    // 请求只投递一次，消费后复位。
    assert!(!tree.take_reconcile_requested());
    // 相同 Arc 是逐帧同步的稳定主题，不得反复请求协调。
    tree.set_theme_tokens(dark);
    assert!(!tree.take_reconcile_requested());
    // 新令牌实例代表主题切换，必须再次请求协调。
    tree.set_theme_tokens(Theme::antd_light().tokens_arc());
    assert!(tree.take_reconcile_requested());
}

// —— 自源文件移入的扩展 impl（impl WidgetTree） ——

impl WidgetTree {
    // 测试目标保留布局帧 trace 取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_frame_trace(&self) -> Vec<(u8, WidgetId, i32, i32)> {
        std::mem::take(&mut *self.layout_frame_trace.borrow_mut())
    }

    // 测试目标保留布局写入计数取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_frame_writes(&self) -> u32 {
        self.layout_frame_writes.replace(0)
    }

    // 测试目标保留 shrink 操作计数取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_shrink_ops(&self) -> u32 {
        self.layout_shrink_ops.replace(0)
    }

    // 测试目标保留收敛 pass 计数取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_converge_passes(&self) -> u32 {
        self.layout_converge_passes.replace(0)
    }

    // 测试目标保留 expand 操作计数取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_expand_ops(&self) -> u32 {
        self.layout_expand_ops.replace(0)
    }
}
