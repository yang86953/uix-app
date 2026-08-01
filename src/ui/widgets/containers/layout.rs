//! Layout 页面布局组件 — Header / Sider / Content / Footer 骨架。
//!
//! 组合使用构建标准页面布局。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::component::widget::WidgetTree;
use crate::ui::SnapshotFields;

component! {
    /// Layout — 页面布局容器（flex 列）。
    pub struct Layout {
        bg_color: Option<Color>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    flex_grow => (&self) -> f32 { 1.0 }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }
}

component! {
    /// Header — 页面顶部栏。
    pub struct Header {
        height: f32,
        bg_color: Option<Color>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        children.iter().map(|child| (child.id, frame)).collect()
    }
}

component! {
    /// Sider — 侧边栏。
    pub struct Sider {
        width: f32,
        bg_color: Option<Color>,
        collapsible: bool,
        collapsed: bool,
        collapsed_width: f32,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        // Sider 作为布局容器，将其 frame 直接传递给所有子节点
        children.iter().map(|child| (child.id, frame)).collect()
    }
}

component! {
    /// Content — 内容区。
    pub struct Content {
        bg_color: Option<Color>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    flex_grow => (&self) -> f32 { 1.0 }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        // Content 作为布局容器，将其 frame 直接传递给所有子节点
        children.iter().map(|child| (child.id, frame)).collect()
    }
}

component! {
    /// Footer — 页面底部栏。
    pub struct Footer {
        height: f32,
        bg_color: Option<Color>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        children.iter().map(|child| (child.id, frame)).collect()
    }
}

// 构造方法
impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout {
    pub fn new() -> Self {
        Self { bg_color: None }
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(300.0, 200.0)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.bg_color = next.bg_color;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Layout {
            bg_color: self.bg_color,
        }
    }
}

impl Header {
    pub fn new(height: f32) -> Self {
        Self {
            height,
            bg_color: None,
        }
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(0.0, self.height)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.height = next.height;
        self.bg_color = next.bg_color;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Header {
            height: self.height,
            bg_color: self.bg_color,
        }
    }
}

impl Sider {
    pub fn new(width: f32) -> Self {
        Self {
            width,
            bg_color: None,
            collapsible: false,
            collapsed: false,
            collapsed_width: 80.0,
        }
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    pub fn collapsible(mut self, v: bool) -> Self {
        self.collapsible = v;
        self
    }
    pub fn collapsed(mut self, v: bool) -> Self {
        self.collapsed = v;
        self
    }
    pub fn collapsed_width(mut self, w: f32) -> Self {
        self.collapsed_width = w;
        self
    }

    fn intrinsic_size(&self) -> Size {
        if self.collapsed {
            Size::new(self.collapsed_width, 0.0)
        } else {
            Size::new(self.width, 0.0)
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.width = next.width;
        self.bg_color = next.bg_color;
        self.collapsible = next.collapsible;
        self.collapsed = next.collapsed;
        self.collapsed_width = next.collapsed_width;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Sider {
            width: self.width,
            bg_color: self.bg_color,
            collapsible: self.collapsible,
            collapsed: self.collapsed,
            collapsed_width: self.collapsed_width,
        }
    }
}

impl Default for Content {
    fn default() -> Self {
        Self::new()
    }
}

impl Content {
    pub fn new() -> Self {
        Self { bg_color: None }
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(0.0, 0.0)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.bg_color = next.bg_color;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Content {
            bg_color: self.bg_color,
        }
    }
}

impl Footer {
    pub fn new(height: f32) -> Self {
        Self {
            height,
            bg_color: None,
        }
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(0.0, self.height)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.height = next.height;
        self.bg_color = next.bg_color;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Footer {
            height: self.height,
            bg_color: self.bg_color,
        }
    }
}
