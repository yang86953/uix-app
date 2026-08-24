//! Ant Design 风格的水平或垂直分隔线，支持可选文本。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
// 引入 UIX 声明壳物化原叶节点所需的 View 契约。
use crate::ui::view::{View, ViewNode};
// 引入 UIX 静态颜色角色。
use crate::ui::theme::style::ColorValue;
// 引入分隔线与次级文字使用的中性色角色。
use crate::ui::SnapshotFields;
use crate::ui::theme::NeutralRole;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::widget;

/// 水平分隔线的文本位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerOrientation {
    /// 文本靠左放置。
    Left,
    /// 文本居中放置。
    Center,
    /// 文本靠右放置。
    Right,
}

/// 分隔线方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerDirection {
    /// 水平分隔线。
    Horizontal,
    /// 垂直分隔线。
    Vertical,
}

// 保存由 UIX 声明、由 Rust 分隔线绘制内核消费的紧凑静态视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DividerVisual {
    // 标签文字字号。
    text_size: f32,
    // 无字形引擎精确测量时的兼容字符宽度。
    text_glyph_width: f32,
    // 文字宽度中两侧额外留白的合计值。
    text_padding: f32,
    // 左/右对齐时距离边缘的偏移。
    edge_offset: f32,
    // 文字与左侧线段之间的间距。
    text_gap: f32,
    // 带文字水平分隔线的线宽。
    line_width: f32,
    // 虚线单段长度。
    dash_segment: f32,
    // 虚线段间距。
    dash_gap: f32,
    // 带文字水平分隔线的固有高度。
    labelled_extent: f32,
    // 无文字水平线或垂直线的固有厚度。
    plain_extent: f32,
    // 默认线条主题色角色。
    line_color: ColorValue,
    // 标签文字主题色角色。
    text_color: ColorValue,
}

// 同目录 UIX 生成唯一视觉值及静态借用。
crate::uix_items!("src/ui/widgets/general/divider/divider.uix");

widget! {
    /// 支持可选标签的分隔线组件。
    pub struct Divider {
        text: Option<String>,
        orientation: DividerOrientation,
        direction: DividerDirection,
        /// 可选的分隔线颜色覆写；`None` 使用主题边框色。
        pub color: Option<Color>,
        dashed: bool,
        #[snapshot(skip)]
        /// UIX 声明的标签、线段、固有尺寸与主题色角色。
        visual: &'static DividerVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let line_color = self
            .color
            .unwrap_or_else(|| self.visual.line_color.resolve(ctx.tokens()));
        let text_secondary = self.visual.text_color.resolve(ctx.tokens());

        let draw_line = |ctx: &mut PaintContext, x: f32, y: f32, w: f32, h: f32, color: Color| {
            if !self.dashed {
                ctx.fill_rect(Rect::new(x, y, w, h), color, None);
            } else {
                let seg_len = self.visual.dash_segment;
                let gap_len = self.visual.dash_gap;
                let mut dx = 0.0;
                let is_h = h <= w;
                while dx < (if is_h { w } else { h }) {
                    let seg = seg_len.min(if is_h { w - dx } else { h - dx });
                    if is_h {
                        ctx.fill_rect(Rect::new(x + dx, y, seg, h), color, None);
                    } else {
                        ctx.fill_rect(Rect::new(x, y + dx, w, seg), color, None);
                    }
                    dx += seg + gap_len;
                }
            }
        };

        match self.direction {
            DividerDirection::Horizontal => {
                let center_y = frame.y + frame.h * 0.5;

                if let Some(ref text) = self.text {
                    let text_w = text.len() as f32 * self.visual.text_glyph_width
                        + self.visual.text_padding;

                    let text_x = match self.orientation {
                        DividerOrientation::Left => frame.x + self.visual.edge_offset,
                        DividerOrientation::Center => frame.x + (frame.w - text_w) * 0.5,
                        DividerOrientation::Right => {
                            frame.x + frame.w - text_w - self.visual.edge_offset
                        }
                    };
                    let text_y = ctx.visual_center_y(frame, self.visual.text_size);

                    let left_end = text_x - self.visual.text_gap;
                    if left_end > frame.x {
                        draw_line(
                            ctx,
                            frame.x,
                            center_y,
                            left_end - frame.x,
                            self.visual.line_width,
                            line_color,
                        );
                    }

                    ctx.draw_text(
                        text,
                        Point::new(text_x, text_y),
                        text_secondary,
                        self.visual.text_size,
                    );

                    let right_start = text_x + text_w;
                    if right_start < frame.x + frame.w {
                        draw_line(
                            ctx,
                            right_start,
                            center_y,
                            frame.x + frame.w - right_start,
                            self.visual.line_width,
                            line_color,
                        );
                    }
                } else {
                    draw_line(ctx, frame.x, frame.y, frame.w, frame.h, line_color);
                }
            }
            DividerDirection::Vertical => {
                draw_line(ctx, frame.x, frame.y, frame.w, frame.h, line_color);
            }
        }
    }
}

impl Divider {
    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.orientation = next.orientation;
        self.direction = next.direction;
        self.color = next.color;
        self.dashed = next.dashed;
        // 同步 UIX 声明的静态视觉，不改变文字、方向或虚线运行配置。
        self.visual = next.visual;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Divider {
            text: self.text.clone(),
            orientation: self.orientation,
            direction: self.direction,
            color: self.color,
            text_size: self.visual.text_size,
            dashed: self.dashed,
        }
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}

impl Divider {
    /// 创建居中的无文本水平实线分隔线。
    pub fn new() -> Self {
        Self {
            text: None,
            orientation: DividerOrientation::Center,
            direction: DividerDirection::Horizontal,
            color: None,
            dashed: false,
            visual: DIVIDER_VISUAL_REF,
        }
    }

    /// 设置水平分隔线显示的文本。
    pub fn with_text(mut self, t: &str) -> Self {
        self.text = Some(t.to_string());
        self
    }
    /// 设置水平分隔线的文本位置。
    pub fn orientation(mut self, o: DividerOrientation) -> Self {
        self.orientation = o;
        self
    }
    /// 将分隔线方向设为垂直。
    pub fn vertical(mut self) -> Self {
        self.direction = DividerDirection::Vertical;
        self
    }
    /// 设置分隔线颜色。
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// 将分隔线设为虚线。
    pub fn dashed(mut self) -> Self {
        self.dashed = true;
        self
    }

    fn intrinsic_size(&self) -> Size {
        match self.direction {
            DividerDirection::Horizontal => {
                if self.text.is_some() {
                    Size::new(0.0, self.visual.labelled_extent)
                } else {
                    Size::new(0.0, self.visual.plain_extent)
                }
            }
            DividerDirection::Vertical => Size::new(self.visual.plain_extent, 0.0),
        }
    }
}

// 向 UIX 静态模板提供零分配次级边框色。
const fn divider_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}

// 向 UIX 静态模板提供零分配次级文本色。
const fn divider_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}

// 把 UIX 声明的静态配置融合进原有 Divider 叶内核。
fn build_divider_view(mut kernel: Divider, visual: &'static DividerVisual) -> ViewNode {
    kernel.visual = visual;
    // 保持原有单 Widget 树形与分配数量。
    ViewNode::leaf(kernel)
}

impl View for Divider {
    fn build(self) -> ViewNode {
        // 使用局部名称交接拥有型 Rust 分隔线内核。
        let kernel = self;
        // 静态展示契约从 UIX 文件物化。
        crate::uix!("src/ui/widgets/general/divider/divider.uix")
    }
}

// 集中验证 Divider 声明融合与快照契约。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/general/divider__tests.rs"]
// 保留原模块私有契约访问能力。
mod tests;
