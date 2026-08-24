//! Descriptions widget — 描述列表，Ant Design 风格。
//!
//! 用于只读展示多条字段信息，支持 bordered、column 布局、label/value 键值对。

use std::cell::{Ref, RefCell};

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::platform::windowing::ControlSize;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::ColorValue;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{SnapshotFields, ThemeTokens, WidgetTree};
use crate::widget;

// 保存由 UIX 声明的默认宽度、列约束与初始外观。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DescriptionsDefaultsVisual {
    width: f32,
    min_column_width: f32,
    column: usize,
    label_width: f32,
    bordered: bool,
}

// 保存由 UIX 声明的标题、单元格与描边几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DescriptionsLayoutVisual {
    title_height: f32,
    title_horizontal_padding: f32,
    horizontal_padding: f32,
    vertical_padding: f32,
    max_label_fraction: f32,
    border_width: f32,
}

// 保存由 UIX 声明的标题、条目字号与行高比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DescriptionsTypographyVisual {
    title_font_size: f32,
    item_font_size: f32,
    line_height: f32,
}

// 保存由 UIX 声明的三档控件基础行高。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DescriptionsControlVisual {
    small_height: f32,
    medium_height: f32,
    large_height: f32,
}

// 描述列表边框使用的主题圆角尺寸角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DescriptionsRadiusRole {
    Default,
}

impl DescriptionsRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Default => tokens.border_radius(),
        }
    }
}

// 保存由 UIX 声明的描述列表主题语义色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DescriptionsPaletteVisual {
    background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    label_text: ColorValue,
    label_fill: ColorValue,
}

// 完整视觉配置由全部 Descriptions 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DescriptionsVisual {
    defaults: DescriptionsDefaultsVisual,
    layout: DescriptionsLayoutVisual,
    typography: DescriptionsTypographyVisual,
    control: DescriptionsControlVisual,
    radius: DescriptionsRadiusRole,
    palette: DescriptionsPaletteVisual,
}

// 同目录 UIX 生成全部分组视觉、根视觉记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/descriptions/descriptions.uix");

// 复用行高向量容量，并用宽度与列数锁定当前有效结果。
#[derive(Debug, Default)]
struct DescriptionsLayoutCache {
    width_bits: u32,
    columns: usize,
    valid: bool,
    row_heights: Vec<f32>,
}

// 向 UIX 提供默认圆角主题角色。
const fn descriptions_default_radius() -> DescriptionsRadiusRole {
    DescriptionsRadiusRole::Default
}

// 向 UIX 提供容器背景主题角色。
const fn descriptions_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}

// 向 UIX 提供次级边框主题角色。
const fn descriptions_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}

// 向 UIX 提供正文主题角色。
const fn descriptions_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

// 向 UIX 提供标签次级正文主题角色。
const fn descriptions_label_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}

// 向 UIX 提供标签四级填充主题角色。
const fn descriptions_label_fill() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillQuaternary)
}

#[derive(Debug, Clone, Copy)]
struct ItemPlacement {
    item_index: usize,
    row: usize,
    column: usize,
    span: usize,
}

/// 单个描述项。
#[derive(Debug, Clone, PartialEq)]
pub struct DescriptionsItem {
    /// 字段标签文本。
    pub label: String,
    /// 字段值文本。
    pub value: String,
    /// 该项占用的布局列数。
    pub span: usize,
}

// Descriptions — 描述列表。
widget! {
    /// 以标签和值组成的网格展示只读字段信息。
    pub struct Descriptions {
        title: String,
        items: Vec<DescriptionsItem>,
        bordered: bool,
        #[snapshot(skip)]
        bordered_authored: bool,
        column: usize,
        #[snapshot(skip)]
        column_authored: bool,
        label_width: f32,
        #[snapshot(skip)]
        label_width_authored: bool,
        size: ControlSize,
        #[snapshot(skip)]
        layout_cache: RefCell<DescriptionsLayoutCache>,
        #[snapshot(skip)]
        visual: &'static DescriptionsVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        let width = constraints
            .clamp(Size::new(self.visual.defaults.width, 0.0))
            .w;
        let row_heights = self.row_heights(width);
        constraints.clamp(Size::new(
            width,
            self.title_height() + row_heights.iter().sum::<f32>(),
        ))
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let bg = self.visual.palette.background.resolve(ctx.tokens());
        let border = self.visual.palette.border.resolve(ctx.tokens());
        let text = self.visual.palette.text.resolve(ctx.tokens());
        let text_sec = self.visual.palette.label_text.resolve(ctx.tokens());
        let fill = self.visual.palette.label_fill.resolve(ctx.tokens());
        let r = Radius::uniform(self.visual.radius.resolve(ctx.tokens()));
        let mut y = frame.y;
        let columns = self.effective_columns(frame.w);
        let col_w = frame.w / columns as f32;
        let row_heights = self.row_heights_for(frame.w, columns);
        let body_height = row_heights.iter().sum::<f32>();
        ctx.push_clip(frame);

        // 标题
        if !self.title.is_empty() {
            let title_rect = Rect::new(
                frame.x,
                y,
                frame.w,
                self.visual.layout.title_height.min(frame.h),
            );
            let text_rect = Rect::new(
                title_rect.x + self.visual.layout.title_horizontal_padding,
                title_rect.y,
                (title_rect.w - self.visual.layout.title_horizontal_padding * 2.0).max(0.0),
                title_rect.h,
            );
            // 复用 UI 绘制上下文拥有的保守单行省略算法。
            if let Some(title) = ctx.elide_single_line(
                &self.title,
                self.visual.typography.title_font_size,
                text_rect.w,
            ) {
                let ty =
                    ctx.visual_center_y(title_rect, self.visual.typography.title_font_size);
                ctx.push_clip(text_rect);
                ctx.draw_text(
                    &title,
                    Point::new(text_rect.x, ty),
                    text,
                    self.visual.typography.title_font_size,
                );
                ctx.pop_clip();
            }
            y += self.visual.layout.title_height;
        }

        // 主体背景
        if self.bordered {
            let body = Rect::new(
                frame.x,
                y,
                frame.w,
                body_height.min((frame.y + frame.h - y).max(0.0)),
            );
            ctx.fill_rect(body, bg, Some(r));
            ctx.stroke_rect(body, border, self.visual.layout.border_width, Some(r));
        }

        // 按 span 顺序装箱；放不下的条目从下一行开始。
        let mut current_row = 0usize;
        let mut row_y = y;
        for placement in self.item_placements(columns) {
            while current_row < placement.row {
                row_y += row_heights[current_row];
                current_row += 1;
            }
            let item = &self.items[placement.item_index];
            let item_x = frame.x + placement.column as f32 * col_w;
            let item_w = placement.span as f32 * col_w;
            let item_h = row_heights[placement.row];
            let label_width = self.effective_label_width(item_w);
            let row_rect = Rect::new(item_x, row_y, item_w, item_h);
            let label_rect = Rect::new(item_x, row_y, label_width, item_h);
            let value_rect = Rect::new(
                item_x + label_width,
                row_y,
                (item_w - label_width).max(0.0),
                item_h,
            );
            if self.bordered {
                ctx.fill_rect(label_rect, fill, None);
                ctx.stroke_rect(row_rect, border, self.visual.layout.border_width, None);
            }
            draw_wrapped_cell_text(ctx, label_rect, &item.label, text_sec, self.visual);
            draw_wrapped_cell_text(ctx, value_rect, &item.value, text, self.visual);
        }
        ctx.pop_clip();
    }
}

// 把描述项数据、网格算法与绘制内核融合为 UIX 声明的单一叶节点。
fn build_descriptions_view(
    mut kernel: Descriptions,
    visual: &'static DescriptionsVisual,
) -> ViewNode {
    if !kernel.bordered_authored {
        kernel.bordered = visual.defaults.bordered;
    }
    if !kernel.column_authored {
        kernel.column = visual.defaults.column.max(1);
    }
    if !kernel.label_width_authored {
        kernel.label_width = visual.defaults.label_width;
    }
    kernel.visual = visual;
    kernel.invalidate_layout_cache();
    ViewNode::leaf(kernel)
}

impl View for Descriptions {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 内核继续独占数据布局、换行与绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/descriptions/descriptions.uix")
    }
}

impl Descriptions {
    /// 创建三列、无边框且使用全局控件尺寸的空描述列表。
    pub fn new() -> Self {
        let visual = DESCRIPTIONS_VISUAL_REF;
        Self {
            title: String::new(),
            items: Vec::new(),
            bordered: visual.defaults.bordered,
            bordered_authored: false,
            column: visual.defaults.column,
            column_authored: false,
            label_width: visual.defaults.label_width,
            label_width_authored: false,
            size: crate::ui::widget_runtime::config::use_config().size,
            layout_cache: RefCell::new(DescriptionsLayoutCache::default()),
            visual,
        }
    }
    /// 设置描述列表标题。
    pub fn title(mut self, t: &str) -> Self {
        self.title = t.to_string();
        self.invalidate_layout_cache();
        self
    }
    /// 替换描述列表中的全部项目。
    pub fn items(mut self, items: Vec<DescriptionsItem>) -> Self {
        self.items = items;
        self.invalidate_layout_cache();
        self
    }
    #[allow(
        clippy::should_implement_trait,
        reason = "add is the established fluent builder API, not arithmetic addition"
    )]
    /// 在描述列表末尾追加一个项目。
    pub fn add(mut self, item: DescriptionsItem) -> Self {
        self.items.push(item);
        self.invalidate_layout_cache();
        self
    }
    /// 设置是否绘制单元格边框。
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self.bordered_authored = true;
        self
    }
    /// 设置每行列数；零会被规范化为一列。
    pub fn column(mut self, v: usize) -> Self {
        self.column = v.max(1);
        self.column_authored = true;
        self.invalidate_layout_cache();
        self
    }
    /// 设置标签区域宽度；非有限值归零，负值截断为零。
    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = Self::normalize_dimension(w);
        self.label_width_authored = true;
        self.invalidate_layout_cache();
        self
    }
    /// 设置描述列表的控件尺寸。
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self.invalidate_layout_cache();
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Descriptions {
            title: self.title.clone(),
            items: self.items.clone(),
            bordered: self.bordered,
            column: self.column,
            label_width: self.label_width,
            descriptions_size: self.size,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.title = next.title;
        self.items = next.items;
        self.bordered = next.bordered;
        self.bordered_authored = next.bordered_authored;
        self.column = next.column.max(1);
        self.column_authored = next.column_authored;
        self.label_width = Self::normalize_dimension(next.label_width);
        self.label_width_authored = next.label_width_authored;
        self.size = next.size;
        self.visual = next.visual;
        self.invalidate_layout_cache();
    }

    fn title_height(&self) -> f32 {
        if self.title.is_empty() {
            0.0
        } else {
            self.visual.layout.title_height
        }
    }

    fn base_item_height(&self) -> f32 {
        match self.size {
            ControlSize::Small => self.visual.control.small_height,
            ControlSize::Medium => self.visual.control.medium_height,
            ControlSize::Large => self.visual.control.large_height,
        }
    }

    fn effective_columns(&self, width: f32) -> usize {
        let available = if width.is_finite() {
            width.max(0.0)
        } else {
            0.0
        };
        let fitting = (available / self.visual.defaults.min_column_width).floor() as usize;
        self.column.min(fitting.max(1)).max(1)
    }

    fn item_placements(&self, columns: usize) -> impl Iterator<Item = ItemPlacement> + '_ {
        let columns = columns.max(1);
        let mut row = 0usize;
        let mut used_columns = 0usize;
        self.items
            .iter()
            .enumerate()
            .map(move |(item_index, item)| {
                let span = item.span.clamp(1, columns);
                if used_columns == columns || used_columns + span > columns {
                    row += 1;
                    used_columns = 0;
                }
                let placement = ItemPlacement {
                    item_index,
                    row,
                    column: used_columns,
                    span,
                };
                used_columns += span;
                placement
            })
    }

    fn row_heights(&self, width: f32) -> Ref<'_, [f32]> {
        let columns = self.effective_columns(width);
        self.row_heights_for(width, columns)
    }

    fn row_heights_for(&self, width: f32, columns: usize) -> Ref<'_, [f32]> {
        let width = width.max(0.0);
        let columns = columns.max(1);
        let needs_refresh = {
            let cache = self.layout_cache.borrow();
            !cache.valid || cache.width_bits != width.to_bits() || cache.columns != columns
        };
        if needs_refresh {
            let mut cache = self.layout_cache.borrow_mut();
            cache.row_heights.clear();
            self.fill_row_heights(width, columns, &mut cache.row_heights);
            cache.width_bits = width.to_bits();
            cache.columns = columns;
            cache.valid = true;
        }
        Ref::map(self.layout_cache.borrow(), |cache| {
            cache.row_heights.as_slice()
        })
    }

    fn fill_row_heights(&self, width: f32, columns: usize, heights: &mut Vec<f32>) {
        let col_w = width.max(0.0) / columns.max(1) as f32;
        for placement in self.item_placements(columns) {
            if placement.row == heights.len() {
                heights.push(self.base_item_height());
            }
            let item = &self.items[placement.item_index];
            let item_w = placement.span as f32 * col_w;
            let label_width = self.effective_label_width(item_w);
            let value_width = (item_w - label_width).max(0.0);
            let label_height = wrapped_text_height(&item.label, label_width, self.visual);
            let value_height = wrapped_text_height(&item.value, value_width, self.visual);
            heights[placement.row] = heights[placement.row]
                .max(label_height.max(value_height) + self.visual.layout.vertical_padding * 2.0);
        }
    }

    fn effective_label_width(&self, item_width: f32) -> f32 {
        self.label_width
            .min(item_width.max(0.0) * self.visual.layout.max_label_fraction)
            .max(0.0)
    }

    fn invalidate_layout_cache(&self) {
        self.layout_cache.borrow_mut().valid = false;
    }

    // 测试目标观察 UIX 声明的关键视觉契约，不暴露到公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, f32, f32) {
        (
            self.visual.defaults.width,
            self.visual.defaults.min_column_width,
            self.visual.layout.title_height,
            self.visual.typography.title_font_size,
            self.visual.typography.item_font_size,
            self.visual.layout.horizontal_padding,
            self.visual.layout.vertical_padding,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }

    fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }
}

impl Default for Descriptions {
    fn default() -> Self {
        Self::new()
    }
}

impl DescriptionsItem {
    /// 创建占一列的标签和值描述项。
    pub fn new(label: &str, value: &str) -> Self {
        Self {
            label: label.to_string(),
            value: value.to_string(),
            span: 1,
        }
    }
    /// 设置项目占用的列数；零会被规范化为一列。
    pub fn span(mut self, s: usize) -> Self {
        self.span = s.max(1);
        self
    }
}

// 集中验证 UIX 声明壳与描述列表 Rust 内核的单叶契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/display/descriptions_tests.rs"]
mod tests;

fn wrapped_text_height(text: &str, region_width: f32, visual: &DescriptionsVisual) -> f32 {
    let text_width = region_width - visual.layout.horizontal_padding * 2.0;
    if text_width <= 0.0 {
        return 0.0;
    }
    crate::draw::resources::font::text_backend::estimate_text_metrics(
        text,
        text_width,
        visual.typography.item_font_size,
    )
    .line_count
    .max(1) as f32
        * visual.typography.item_font_size
        * visual.typography.line_height
}

fn draw_wrapped_cell_text(
    ctx: &mut PaintContext,
    region: Rect,
    text: &str,
    color: crate::draw::Color,
    visual: &DescriptionsVisual,
) {
    let text_rect = Rect::new(
        region.x + visual.layout.horizontal_padding,
        region.y + visual.layout.vertical_padding,
        (region.w - visual.layout.horizontal_padding * 2.0).max(0.0),
        (region.h - visual.layout.vertical_padding * 2.0).max(0.0),
    );
    if text_rect.w <= 0.0 || text_rect.h <= 0.0 {
        return;
    }
    ctx.push_clip(text_rect);
    ctx.draw_text_wrapped(text, text_rect, color, visual.typography.item_font_size);
    ctx.pop_clip();
}
