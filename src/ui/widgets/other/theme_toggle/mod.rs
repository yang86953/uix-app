//! ThemeToggle — 主题切换按钮（暗色 ↔ 亮色）。
//!
//! 点击切换暗色/亮色主题，通过 `Cell<bool>` 通知外部代码。

use crate::core::{Constraints, Rect, Size};
// 引入 UIX 声明壳物化原叶节点的 View 契约。
use crate::ui::view::{View, ViewNode};
// 引入 UIX 声明的主题色角色。
use crate::ui::theme::style::{ColorValue, PaletteColor};
// 引入默认文本中性色角色。
use crate::ui::theme::NeutralRole;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, WidgetId,
    WidgetTree,
};
use crate::widget;
use std::cell::Cell;

// 保存由 UIX 声明、由 Rust 主题切换内核消费的紧凑静态视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ThemeToggleVisual {
    // 当前处于暗色主题时展示的目标图标。
    dark_icon: &'static str,
    // 当前处于亮色主题时展示的目标图标。
    light_icon: &'static str,
    // 图标绘制尺寸。
    icon_size: f32,
    // 组件方形固有边长。
    extent: f32,
    // 键盘焦点环宽度。
    focus_width: f32,
    // 焦点环半径相对当前最短边的比例。
    focus_radius_ratio: f32,
    // 图标主题色角色。
    icon_color: ColorValue,
    // 焦点环主题色角色。
    focus_color: ColorValue,
}

// 同目录 UIX 生成唯一视觉值及静态借用。
crate::uix_items!("src/ui/widgets/other/theme_toggle/theme_toggle.uix");

widget! {
    /// ThemeToggle — 主题切换按钮。
    pub struct ThemeToggle {
        #[snapshot(skip)]
        /// 当前是否选择暗色主题的内部可变状态。
        pub dark: Cell<bool>,
        initial_dark: bool,
        focused: bool,
        pending_change: Cell<Option<bool>>,
        #[snapshot(skip)]
        /// UIX 声明的双状态图标、几何与主题色角色。
        visual: &'static ThemeToggleVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                button: MouseButton::Left,
                ..
            }
            | SystemEvent::KeyDown {
                key: KeyCode::Enter | KeyCode::Space,
                ..
            } => {
                self.toggle();
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change.take().map(|dark| {
            SemanticEvent::change(id, if dark { "dark" } else { "light" })
        })
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 运行内核只根据当前状态选择 UIX 声明的两个图标。
        let icon = if self.dark.get() {
            self.visual.dark_icon
        } else {
            self.visual.light_icon
        };
        // 在绘制边界解析 UIX 选择的文本色角色。
        let text_color = self.visual.icon_color.resolve(ctx.tokens());
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            icon,
            frame,
            text_color,
            self.visual.icon_size,
        );
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                frame,
                self.visual.focus_color.resolve(ctx.tokens()),
                self.visual.focus_width,
                Some(crate::draw::Radius::uniform(
                    frame.w.min(frame.h).max(0.0)
                        * self.visual.focus_radius_ratio.clamp(0.0, 0.5),
                )),
            );
        }
    }
}

impl ThemeToggle {
    /// 创建初始处于亮色模式的主题切换按钮。
    pub fn new() -> Self {
        Self {
            dark: Cell::new(false),
            initial_dark: false,
            focused: false,
            pending_change: Cell::new(None),
            visual: THEME_TOGGLE_VISUAL_REF,
        }
    }

    /// 设置组件的初始暗色模式状态。
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark.set(dark);
        self.initial_dark = dark;
        self
    }

    /// 返回组件当前是否处于暗色模式。
    pub fn is_dark(&self) -> bool {
        self.dark.get()
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.initial_dark = next.initial_dark;
        // 协调时同步 UIX 声明配置，不覆盖当前切换与焦点运行态。
        self.visual = next.visual;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ThemeToggle {
            dark: self.dark.get(),
        }
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.visual.extent, self.visual.extent)
    }

    fn toggle(&self) {
        let dark = !self.dark.get();
        self.dark.set(dark);
        self.pending_change.set(Some(dark));
    }
}

// 向 UIX 静态模板提供零分配默认文本色。
const fn theme_toggle_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

// 向 UIX 静态模板提供零分配主色。
const fn theme_toggle_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

// 把 UIX 声明的静态配置融合进原有 ThemeToggle 叶内核。
fn build_theme_toggle_view(
    mut kernel: ThemeToggle,
    visual: &'static ThemeToggleVisual,
) -> ViewNode {
    kernel.visual = visual;
    // 保持原有单 Widget 树形和分配数量。
    ViewNode::leaf(kernel)
}

impl View for ThemeToggle {
    fn build(self) -> ViewNode {
        // 使用局部名称交接拥有型 Rust 主题状态内核。
        let kernel = self;
        // 静态展示契约从 UIX 文件物化。
        crate::uix!("src/ui/widgets/other/theme_toggle/theme_toggle.uix")
    }
}

impl Default for ThemeToggle {
    fn default() -> Self {
        Self::new()
    }
}

// 集中验证 ThemeToggle 声明融合与交互契约。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/general/theme_toggle__tests.rs"]
// 保留原模块私有契约访问能力。
mod tests;
