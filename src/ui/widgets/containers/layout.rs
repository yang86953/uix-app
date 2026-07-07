//! Layout 页面布局组件 — Header / Sider / Content / Footer 骨架。
//!
//! 组合使用构建标准页面布局。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::SnapshotFields;
use crate::ui::WidgetTree;

component! {
    /// Layout — 页面布局容器（flex 列）。
    pub struct Layout {
        bg_color: Option<Color>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
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


    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
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


    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::WidgetId], _tree: &WidgetTree)
        -> Vec<(crate::ui::WidgetId, Rect)>
    {
        // Sider 作为布局容器，将其 frame 直接传递给所有子节点
        children.iter().map(|&cid| (cid, frame)).collect()
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


    flex_grow => (&self) -> f32 { 1.0 }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::WidgetId], _tree: &WidgetTree)
        -> Vec<(crate::ui::WidgetId, Rect)>
    {
        // Content 作为布局容器，将其 frame 直接传递给所有子节点
        children.iter().map(|&cid| (cid, frame)).collect()
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


    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
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

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Footer {
            height: self.height,
            bg_color: self.bg_color,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_layout_shell_parts() {
        assert_eq!(
            Layout::new().measure(Constraints::loose(Size::new(120.0, 80.0))),
            Size::new(120.0, 80.0)
        );
        assert_eq!(
            Header::new(64.0).measure(Constraints::loose(Size::new(120.0, 48.0))),
            Size::new(0.0, 48.0)
        );
        assert_eq!(
            Sider::new(200.0)
                .collapsed(true)
                .collapsed_width(64.0)
                .measure(Constraints::loose(Size::new(120.0, 80.0))),
            Size::new(64.0, 0.0)
        );
        assert_eq!(
            Content::new().measure(Constraints::new(
                Size::new(10.0, 12.0),
                Size::new(120.0, 80.0),
                None,
            )),
            Size::new(10.0, 12.0)
        );
        assert_eq!(
            Footer::new(40.0).measure(Constraints::loose(Size::new(120.0, 24.0))),
            Size::new(0.0, 24.0)
        );
    }
}
