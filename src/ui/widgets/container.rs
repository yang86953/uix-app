//! Container widget — flexbox layout container with background/border.

use crate::define_widget;
use crate::graphics::{
    compute_flex_layout, AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent, Radius,
};
use crate::graphics::{Color};
use crate::base::{EdgeInsets, Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{WidgetCore, WidgetId, WidgetTree};

define_widget! {
    pub struct Container {
        pub bg_color: Option<Color>,
        pub border_color: Option<Color>,
        pub border_width: f32,
        pub border_radius: f32,
        pub padding: EdgeInsets,
        pub gap: f32,
        pub direction: FlexDirection,
        pub justify: JustifyContent,
        pub align: AlignItems,
        pub fixed_width: Option<f32>,
        pub fixed_height: Option<f32>,
        pub flex_grow: f32,
        pub flex_shrink: f32,
    }

    // NOTE(布局): preferred_size 签名仅为 (&self, _engine: Option<&dyn GraphicsEngine>) -> Size，
    // 无法访问 WidgetTree，因此无法遍历子节点估算内容尺寸。
    // 当无 fixed_width/fixed_height 时返回 (0,0)，由父容器 flex 布局分配实际空间。
    // 如需精确的 preferred_size 内容估算，需修改 define_widget! 宏以传入 tree 引用。
    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(0.0),
            self.fixed_height.unwrap_or(0.0),
        )
    }

    flex_grow => (&self) -> f32 { self.flex_grow }

    flex_shrink => (&self) -> f32 { self.flex_shrink }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Background
        if let Some(c) = self.bg_color {
            let r = if self.border_radius > 0.0 {
                Some(Radius::uniform(self.border_radius))
            } else {
                None
            };
            ctx.fill_rect(frame, c, r);
        }
        // Border
        if let Some(c) = self.border_color {
            let r = if self.border_radius > 0.0 {
                Some(Radius::uniform(self.border_radius))
            } else {
                None
            };
            ctx.stroke_rect(frame, c, self.border_width, r);
        }
    }
    layout_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }

        let child_sizes: Vec<Size> = children
            .iter()
            .map(|&cid| {
                let pref = tree.get(cid)
                    .map(|c| c.preferred_size(None))
                    .unwrap_or_default();
                let actual_h = tree.get(cid)
                    .map(|c| c.frame().h)
                    .unwrap_or(0.0);
                // 子节点 frame 可能已被 Phase 2 扩展，取较大值
                Size::new(pref.w, pref.h.max(actual_h))
            })
            .collect();

        let flex_children: Vec<FlexChild> = children
            .iter()
            .map(|&cid| {
                let w = tree.get(cid);
                FlexChild {
                    flex_grow: w.map(|c| c.inner().flex_grow()).unwrap_or(0.0),
                    flex_shrink: w.map(|c| c.inner().flex_shrink()).unwrap_or(1.0),
                    ..FlexChild::default()
                }
            })
            .collect();

        let input = FlexInput {
            direction: self.direction,
            gap: self.gap,
            padding: self.padding,
            container: frame,
            children: flex_children,
            child_sizes,
            justify_content: self.justify,
            align_items: self.align,
            ..FlexInput::default()
        };

        let output = compute_flex_layout(&input);
        children
            .iter()
            .zip(output.child_rects)
            .map(|(&cid, rect)| (cid, rect))
            .collect()
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl Container {
    pub fn new() -> Self {
        Self {
            bg_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            padding: EdgeInsets::zero(),
            gap: 0.0,
            direction: FlexDirection::Row,
            justify: JustifyContent::Start,
            align: AlignItems::Stretch,
            fixed_width: None,
            fixed_height: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }
    }

    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    pub fn border(mut self, c: Color, w: f32) -> Self {
        self.border_color = Some(c);
        self.border_width = w;
        self
    }
    pub fn rounded(mut self, r: f32) -> Self {
        self.border_radius = r;
        self
    }
    pub fn pad(mut self, p: EdgeInsets) -> Self {
        self.padding = p;
        self
    }
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    pub fn dir(mut self, d: FlexDirection) -> Self {
        self.direction = d;
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }
    pub fn flex_grow(mut self, v: f32) -> Self {
        self.flex_grow = v;
        self
    }
    pub fn flex_shrink(mut self, v: f32) -> Self {
        self.flex_shrink = v;
        self
    }
}
