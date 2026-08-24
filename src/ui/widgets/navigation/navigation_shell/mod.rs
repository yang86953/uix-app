//! Navigation 侧栏外壳，只拥有标题、版本与整栏折叠呈现。

// 引入内部可变布局请求与最近宽度缓存。
use std::cell::Cell;

// 引入组件声明宏。
use crate::widget;
// 引入组件身份、约束与矩形类型。
use crate::core::{Constraints, Rect, Size, WidgetId};
// 引入绘制上下文。
use crate::ui::widget_runtime::paint_context::PaintContext;
// 引入子节点布局快照。
use crate::ui::layout::LayoutChild;
// 引入调用方拥有的折叠状态。
use crate::ui::State;
// 引入事件、快照与组件树契约。
use crate::ui::{
    EventResult, KeyCode, MouseButton, SnapshotFields, SystemEvent, View, ViewNode, WidgetTree,
};

mod presentation;
use presentation::*;

widget! {
    /// 组合 Navigation 的侧栏外壳，唯一直接子节点必须是受控 Menu。
    pub struct NavigationShell {
        title: String,
        version: Option<String>,
        #[snapshot(skip)]
        collapsed_binding: State<bool>,
        collapsed: bool,
        expanded_width: f32,
        collapsed_width: f32,
        fixed_height: f32,
        focused: bool,
        last_width: Cell<f32>,
        layout_requested: Cell<bool>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static NavigationShellVisual,
    }

    // 外壳只为折叠按钮提供焦点；Menu 子节点保留自己的导航焦点。
    tab_index => (&self) -> i32 { 1 }

    // 宽度由调用方折叠状态决定，标题和版本不拥有额外状态。
    measure => (&self, constraints: Constraints) -> Size {
        // 折叠时使用紧凑宽度，展开时使用普通侧栏宽度。
        let width = if self.collapsed { self.collapsed_width } else { self.expanded_width };
        // 返回受父布局约束的侧栏尺寸。
        constraints.clamp(Size::new(width, self.fixed_height))
    }

    // Menu 子节点只占用标题/折叠按钮与版本之间的内容区域。
    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        // 缓存实际宽度供指针命中折叠按钮。
        self.last_width.set(frame.w.max(0.0));
        // 没有受控 Menu 时不产生伪布局。
        let Some(menu) = children.first() else { return Vec::new(); };
        // 折叠态只保留顶部按钮行，展开态保留标题行。
        let header_height = if self.collapsed {
            self.visual.layout.collapsed_header_height
        } else {
            self.visual.layout.expanded_header_height
        };
        // 只有展开且存在调用方版本时才预留版本行。
        let version_height = if !self.collapsed && self.version.is_some() {
            self.visual.layout.version_height
        } else {
            0.0
        };
        // Menu 获得剩余的完整侧栏内容区域。
        vec![(menu.id, Rect::new(
            frame.x,
            frame.y + header_height,
            frame.w.max(0.0),
            (frame.h - header_height - version_height).max(0.0),
        ))]
    }

    // 外壳处理自己的折叠按钮，不截获 Menu 内容区事件。
    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            // 左键命中右上折叠按钮时切换调用方状态。
            SystemEvent::PointerDown { pos, button: MouseButton::Left, .. }
                if pos.y >= 0.0
                    && pos.y <= self.visual.layout.toggle_size
                    && pos.x >= (self.last_width.get() - self.visual.layout.toggle_size).max(0.0) =>
            {
                // 建立新的整栏折叠事实。
                self.toggle_collapsed();
                // 折叠按钮消费本次点击。
                EventResult::Handled
            }
            // 外壳获得焦点时显示键盘焦点环。
            SystemEvent::FocusIn => {
                // 保存焦点状态。
                self.focused = true;
                // 消费焦点进入。
                EventResult::Handled
            }
            // 外壳失焦时移除焦点环。
            SystemEvent::FocusOut => {
                // 清除焦点状态。
                self.focused = false;
                // 消费焦点离开。
                EventResult::Handled
            }
            // Enter/Space 激活外壳拥有的折叠按钮。
            SystemEvent::KeyDown { key: KeyCode::Enter | KeyCode::Space, .. } => {
                // 建立新的整栏折叠事实。
                self.toggle_collapsed();
                // 消费键盘激活。
                EventResult::Handled
            }
            // 其他事件继续交付 Menu 或祖先。
            _ => EventResult::NotHandled,
        }
    }

    // 绘制侧栏背景、标题、版本和折叠图标；Menu 由唯一子组件绘制。
    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 缓存实际宽度以保持绘制与命中同源。
        self.last_width.set(frame.w.max(0.0));
        // 标题、版本与折叠按钮同帧共享一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let layout = &self.visual.layout;
        let typography = &self.visual.typography;
        // 侧栏外壳使用主题容器背景。
        ctx.fill_rect(frame, visual.container_background, None);
        // 展开态精确显示调用方标题。
        if !self.collapsed {
            // 标题占据顶部完整行并为折叠按钮留出空间。
            ctx.draw_text_in_frame(
                &self.title,
                Rect::new(
                    frame.x + layout.title_start,
                    frame.y,
                    (frame.w - layout.title_end_reserve).max(0.0),
                    layout.expanded_header_height,
                ),
                visual.text,
                typography.title,
            );
        }
        // 折叠按钮始终位于右上角。
        let toggle = Rect::new(
            frame.x + (frame.w - layout.toggle_size).max(0.0),
            frame.y,
            layout.toggle_size,
            layout.toggle_size,
        );
        // 图标方向表达切换后的目标状态。
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            if self.collapsed {
                self.visual.icons.expand
            } else {
                self.visual.icons.collapse
            },
            toggle,
            visual.text_secondary,
            typography.toggle_icon,
        );
        // 展开态底部精确显示调用方版本元数据。
        if !self.collapsed {
            // 只有显式版本才绘制且占用版本行。
            if let Some(version) = self.version.as_ref() {
                // 版本行贴近侧栏底部。
                ctx.draw_text_in_frame(
                    version,
                    Rect::new(
                        frame.x + layout.version_start,
                        frame.y + (frame.h - layout.version_height).max(0.0),
                        (frame.w - layout.version_end_reserve).max(0.0),
                        layout.version_height,
                    ),
                    visual.text_quaternary,
                    typography.version,
                );
            }
        }
        // 键盘焦点可见时只圈出折叠按钮。
        if self.focused && tree.keyboard_focus_visible() {
            // 使用主题主色绘制明确焦点边界。
            ctx.stroke_rect(
                toggle,
                visual.primary,
                self.visual.chrome.focus_width,
                None,
            );
        }
    }

    // 折叠改变会影响自身宽度和 Menu 子节点布局。
    take_layout_request => (&mut self) -> bool {
        // 原子取走一次待布局请求。
        self.layout_requested.replace(false)
    }
}

// 定义侧栏外壳的构造、刷新与快照边界。
impl NavigationShell {
    /// 构造调用方状态驱动的侧栏外壳。
    pub fn new(title: impl Into<String>, version: Option<String>, collapsed: &State<bool>) -> Self {
        let visual = NAVIGATION_SHELL_VISUAL_REF;
        // 在声明构建期读取一次 State，以登记响应式依赖。
        let collapsed_value = collapsed.get();
        // 返回只拥有外壳状态的组件。
        Self {
            // 保存调用方标题元数据。
            title: title.into(),
            // 保存可选调用方版本元数据。
            version,
            // 借用调用方折叠事实句柄。
            collapsed_binding: collapsed.clone(),
            // 保存本次声明快照中的折叠值。
            collapsed: collapsed_value,
            // 使用兼容普通侧栏宽度。
            expanded_width: visual.layout.expanded_width,
            // 使用只容纳图标的紧凑宽度。
            collapsed_width: visual.layout.collapsed_width,
            // 使用兼容侧栏高度。
            fixed_height: visual.layout.height,
            // 初始没有键盘焦点。
            focused: false,
            // 初始命中宽度与展开宽度一致。
            last_width: Cell::new(visual.layout.expanded_width),
            // 初始没有待处理布局请求。
            layout_requested: Cell::new(false),
            visual,
        }
    }

    /// 覆盖展开侧栏自然宽度。
    pub fn width(mut self, width: f32) -> Self {
        // 负值不进入布局契约。
        self.expanded_width = width.max(0.0);
        // 返回完成配置的外壳。
        self
    }

    /// 覆盖侧栏自然高度。
    pub fn height(mut self, height: f32) -> Self {
        // 负值不进入布局契约。
        self.fixed_height = height.max(0.0);
        // 返回完成配置的外壳。
        self
    }

    /// 返回本次组件声明观察到的折叠值。
    pub fn is_collapsed(&self) -> bool {
        // 暴露只读呈现事实供测试和快照使用。
        self.collapsed
    }

    /// 原位刷新声明元数据并保持运行焦点状态。
    pub(crate) fn sync_from(&mut self, next: Self) {
        // 记录折叠几何是否发生变化。
        let layout_changed = self.collapsed != next.collapsed
            || self.expanded_width != next.expanded_width
            || self.collapsed_width != next.collapsed_width
            || self.fixed_height != next.fixed_height
            || self.version.is_some() != next.version.is_some();
        // 替换调用方标题。
        self.title = next.title;
        // 替换调用方版本元数据。
        self.version = next.version;
        // 替换折叠状态句柄。
        self.collapsed_binding = next.collapsed_binding;
        // 同步最新声明折叠值。
        self.collapsed = next.collapsed;
        // 同步展开宽度。
        self.expanded_width = next.expanded_width;
        // 同步紧凑宽度。
        self.collapsed_width = next.collapsed_width;
        // 同步自然高度。
        self.fixed_height = next.fixed_height;
        // 同步 UIX 生成的视觉表引用，不保留 Rust 视觉副本。
        self.visual = next.visual;
        // 几何变化时请求重新布局唯一 Menu 子树。
        if layout_changed {
            // 合并尚未消费的布局请求。
            self.layout_requested.set(true);
        }
    }

    /// 返回侧栏元数据与折叠事实快照。
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        // 使用专属快照变体保留调用方精确版本文本。
        SnapshotFields::Navigation {
            // 快照标题元数据。
            title: self.title.clone(),
            // 快照可选版本元数据。
            version: self.version.clone(),
            // 快照整栏折叠事实。
            collapsed: self.collapsed,
        }
    }

    /// 切换整栏折叠事实并请求布局。
    fn toggle_collapsed(&mut self) {
        // 计算唯一下一状态。
        let next = !self.collapsed;
        // 先更新调用方拥有的状态事实。
        self.collapsed_binding.set(next);
        // 同步本组件当前帧呈现。
        self.collapsed = next;
        // 请求组件树重新计算外壳宽度和 Menu 内容区域。
        self.layout_requested.set(true);
    }
}

// UIX 只注入静态视觉表，Rust 内核继续拥有折叠状态、子树布局和输入。
fn build_navigation_shell_view(
    mut kernel: NavigationShell,
    visual: &'static NavigationShellVisual,
) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for NavigationShell {
    fn build(self) -> ViewNode {
        build_navigation_shell_view(self, NAVIGATION_SHELL_VISUAL_REF)
    }
}
