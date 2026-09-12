//! 视图构建期的窗口视口作用域与 `@media` 断点记录。
//!
//! `.uix` 的 `@media (min-width: …)/(max-width: …)` 在构建期按所属窗口树的
//! 根 frame 逻辑宽度评估；协调入口在捕获边界内安装该宽度快照（与
//! [`super::build_theme`] 的窗口主题作用域并列）。每次评估同时登记阈值，
//! 树在根 frame 宽度变化时只在某个阈值被跨越时请求一次根协调，不轮询。
//! screen token 阈值由生成代码经构建期有效主题读取，主题变化沿既有
//! `set_theme_tokens` 协调路径重新评估。

use std::cell::{Cell, RefCell};

thread_local! {
    // 当前构建所属窗口的逻辑客户区宽度；脱离窗口的构建为 None（按 0 宽评估）。
    static BUILD_VIEWPORT_WIDTH: Cell<Option<f32>> = const { Cell::new(None) };
    // 捕获栈：每层记录本次根构建评估过的断点阈值。
    static MEDIA_CAPTURE: RefCell<Vec<Vec<MediaBreakpoint>>> = const { RefCell::new(Vec::new()) };
}

/// 一次 `@media` 评估使用的阈值对；`None` 表示该方向无条件。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaBreakpoint {
    /// `(min-width: …)` 阈值，宽度 ≥ 阈值成立。
    pub min_width: Option<f32>,
    /// `(max-width: …)` 阈值，宽度 ≤ 阈值成立。
    pub max_width: Option<f32>,
}

impl MediaBreakpoint {
    /// 按逻辑客户区宽度评估条件；边界含等号。
    pub fn matches(&self, width: f32) -> bool {
        self.min_width.is_none_or(|min| width >= min)
            && self.max_width.is_none_or(|max| width <= max)
    }

    /// 宽度从 `before` 变为 `after` 时条件真值是否翻转。
    pub fn flips_between(&self, before: f32, after: f32) -> bool {
        self.matches(before) != self.matches(after)
    }
}

// 构建可以嵌套（组件根内作用域重建）；退出时恢复外层视口，不跨树污染。
pub(crate) struct BuildViewportScope(Option<f32>);

impl BuildViewportScope {
    /// 安装当前构建所属窗口的逻辑客户区宽度。
    pub(crate) fn enter(width: f32) -> Self {
        Self(BUILD_VIEWPORT_WIDTH.with(|current| current.replace(Some(width))))
    }
}

impl Drop for BuildViewportScope {
    fn drop(&mut self) {
        BUILD_VIEWPORT_WIDTH.with(|current| current.set(self.0));
    }
}

/// 返回当前构建期视口宽度；脱离窗口时为 None。
pub(crate) fn current_build_viewport_width() -> Option<f32> {
    BUILD_VIEWPORT_WIDTH.with(Cell::get)
}

/// 生成代码入口：按构建期窗口逻辑宽度评估 `@media` 条件并登记阈值。
///
/// 脱离窗口的构建按 0 宽评估：`min-width` 条件不成立，`max-width` 条件成立。
pub fn uix_media_matches(min_width: Option<f32>, max_width: Option<f32>) -> bool {
    let breakpoint = MediaBreakpoint {
        min_width: min_width.filter(|value| value.is_finite()),
        max_width: max_width.filter(|value| value.is_finite()),
    };
    MEDIA_CAPTURE.with(|stack| {
        if let Some(frame) = stack.borrow_mut().last_mut()
            && !frame.contains(&breakpoint)
        {
            frame.push(breakpoint);
        }
    });
    breakpoint.matches(current_build_viewport_width().unwrap_or(0.0))
}

/// 开始记录一次根构建评估过的断点。
pub(crate) fn begin_media_capture() {
    MEDIA_CAPTURE.with(|stack| stack.borrow_mut().push(Vec::new()));
}

/// 结束记录并返回本层评估过的断点。
pub(crate) fn end_media_capture() -> Vec<MediaBreakpoint> {
    MEDIA_CAPTURE.with(|stack| stack.borrow_mut().pop().unwrap_or_default())
}

/// 窗口树持有的断点集合与上次构建使用的宽度。
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct MediaBreakpoints {
    // 去重后的阈值对。
    breakpoints: Vec<MediaBreakpoint>,
    // 上次根构建评估条件时使用的宽度。
    built_width: Option<f32>,
}

impl MediaBreakpoints {
    /// 用一次完整根构建的结果替换集合。
    pub(crate) fn replace(&mut self, breakpoints: Vec<MediaBreakpoint>, built_width: Option<f32>) {
        self.breakpoints.clear();
        self.merge(breakpoints, built_width);
    }

    /// 合并局部（作用域或动态）构建评估过的断点；宽度取最新已知值。
    pub(crate) fn merge(&mut self, breakpoints: Vec<MediaBreakpoint>, built_width: Option<f32>) {
        for breakpoint in breakpoints {
            if !self.breakpoints.contains(&breakpoint) {
                self.breakpoints.push(breakpoint);
            }
        }
        if built_width.is_some() {
            self.built_width = built_width;
        }
    }

    /// 根宽度变为 `width` 时是否有任一已评估条件翻转（需要重新构建）。
    pub(crate) fn crosses(&self, width: f32) -> bool {
        let Some(built) = self.built_width else {
            return !self.breakpoints.is_empty();
        };
        self.breakpoints
            .iter()
            .any(|breakpoint| breakpoint.flips_between(built, width))
    }
}

#[cfg(test)]
#[path = "../../../tests-src/ui/widget_runtime/build_viewport_tests.rs"]
mod tests;

