//! List widget — 列表组件，Ant Design 风格。
//!
//! 支持列表项渲染、header/footer、bordered、size 等选项。

use crate::core::{Constraints, Rect, Size};
use crate::draw::{Color, Radius};
use crate::platform::windowing::ControlSize;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入真实子 View 测量入口。
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints;
// 引入子树布局、身份与快照契约。
use crate::ui::{LayoutChild, SnapshotFields, WidgetId, WidgetTree};
// 引入一次性交接声明子树所需的内部可变单元。
use std::cell::{Cell, RefCell};

// 保存由 UIX 声明的 List 默认尺寸与边框开关。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ListDefaultsVisual {
    width: f32,
    min_height: f32,
    bordered: bool,
}

// 保存由 UIX 声明的三档行高、留白与兼容文本排版。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ListRowsVisual {
    small_height: f32,
    medium_height: f32,
    large_height: f32,
    horizontal_padding: f32,
    horizontal_padding_frame_ratio: f32,
    header_footer_font_size: f32,
    load_more_height: f32,
    load_more_font_size: f32,
}

impl ListRowsVisual {
    fn height(self, size: ControlSize) -> f32 {
        match size {
            ControlSize::Small => self.small_height,
            ControlSize::Medium => self.medium_height,
            ControlSize::Large => self.large_height,
        }
    }
}

// List 正文使用的主题字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListFontRole {
    Body,
}

impl ListFontRole {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> f32 {
        match self {
            Self::Body => tokens.font_size(),
        }
    }
}

// List 外框使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListRadiusRole {
    Body,
}

impl ListRadiusRole {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> f32 {
        match self {
            Self::Body => tokens.border_radius(),
        }
    }
}

// 保存由 UIX 声明的外框、分隔线与圆角几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ListFrameVisual {
    radius: ListRadiusRole,
    radius_limit_ratio: f32,
    border_inset: f32,
    border_width: f32,
    divider_width: f32,
}

// 保存由 UIX 声明的主题语义色与正文字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ListPaletteVisual {
    background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    primary: ColorValue,
    item_font: ListFontRole,
}

// 完整视觉配置由全部 List 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ListVisual {
    defaults: ListDefaultsVisual,
    rows: ListRowsVisual,
    frame: ListFrameVisual,
    palette: ListPaletteVisual,
}

// 同目录 UIX 生成四组视觉记录、根视觉记录及稳定静态借用。
crate::uix_items!("src/ui/widgets/display/list/list.uix");

// 保存 List 每帧复用的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedListVisual {
    background: Color,
    border: Color,
    text: Color,
    text_secondary: Color,
    item_font_size: f32,
    radius: f32,
}

impl ListVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedListVisual {
        ResolvedListVisual {
            background: self.palette.background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            item_font_size: self.palette.item_font.resolve(tokens),
            radius: self.frame.radius.resolve(tokens),
        }
    }
}

// 向 UIX 提供受限表达式不能直接写入的默认值与主题角色。
const fn list_body_font() -> ListFontRole {
    ListFontRole::Body
}
const fn list_body_radius() -> ListRadiusRole {
    ListRadiusRole::Body
}
const fn list_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
const fn list_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
const fn list_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
const fn list_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
const fn list_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

/// List 尺寸对应的行高。
pub fn list_item_height(size: ControlSize) -> f32 {
    LIST_VISUAL.rows.height(size)
}

// List — 列表组件。
widget! {
    /// 按顺序展示文本项及可选头部、尾部和加载入口的列表组件。
    pub struct List {
        header: String,
        footer: String,
        bordered: bool,
        #[snapshot(skip)]
        bordered_authored: bool,
        list_size: ControlSize,
        items: Vec<String>,
        load_more_text: String,
        /// 标记页首当前由真实 View 而非兼容文本负责。
        header_view_enabled: bool,
        /// 标记页尾当前由真实 View 而非兼容文本负责。
        footer_view_enabled: bool,
        /// 标记加载入口当前由真实 View 而非兼容文本负责。
        load_more_view_enabled: bool,
        // 保存尚未向运行时树交接的页首声明子树。
        #[snapshot(skip)]
        header_view: RefCell<Option<crate::ui::view::ViewNode>>,
        // 保存尚未向运行时树交接的页尾声明子树。
        #[snapshot(skip)]
        footer_view: RefCell<Option<crate::ui::view::ViewNode>>,
        // 保存尚未向运行时树交接的加载入口声明子树。
        #[snapshot(skip)]
        load_more_view: RefCell<Option<crate::ui::view::ViewNode>>,
        // 保存组件树当前登记的直接插槽子节点数量。
        #[snapshot(skip)]
        child_count: usize,
        // 缓存页首插槽包含 margin 的最终正常流高度。
        #[snapshot(skip)]
        header_view_height: Cell<f32>,
        // 缓存页尾插槽包含 margin 的最终正常流高度。
        #[snapshot(skip)]
        footer_view_height: Cell<f32>,
        // 缓存加载入口插槽包含 margin 的最终正常流高度。
        #[snapshot(skip)]
        load_more_view_height: Cell<f32>,
        #[snapshot(skip)]
        visual: &'static ListVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    // 节点型插槽在同一测量周期用真实子树自然尺寸撑开 List。
    measure_from_children => (&self, constraints: Constraints, children: &[WidgetId], tree: &WidgetTree)
        -> Option<Size>
    {
        // 没有节点型插槽时继续复用纯文本固有尺寸路径。
        if self.slot_view_count() == 0 {
            // 空值让统一布局入口回退到 measure。
            return None;
        }
        // 运行时直接子节点基数必须与声明的三个稳定插槽一致。
        if children.len() != self.slot_view_count() {
            // 异常结构不猜测角色，只保留可诊断的兼容尺寸。
            return Some(constraints.clamp(self.intrinsic_size()));
        }
        // 按固定 header/footer/load-more 顺序测量全部真实插槽。
        let measured = self.measure_slot_children(children, tree);
        // 从文本条目和仍使用文本兼容入口的槽位开始累计高度。
        let mut height = self.text_content_height();
        // 默认宽度保持旧 List 的四百逻辑像素契约。
        let mut width = self.visual.defaults.width;
        // 将每个真实插槽的 margin 外尺寸纳入组件自然尺寸。
        for child in &measured {
            // 读取经过有限值归一化的子树 margin 外尺寸。
            let outer = Self::child_outer_size(child);
            // List 宽度至少容纳最宽的真实插槽。
            width = width.max(outer.w);
            // 三个插槽沿垂直正常流依次累加。
            height += outer.h;
        }
        // 保留旧 List 最小高度并尊重父级约束。
        Some(constraints.clamp(Size::new(
            width,
            height.max(self.visual.defaults.min_height),
        )))
    }

    // 将三个声明期插槽以固定 key 一次性交给运行时树。
    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        // 最多预留三个稳定插槽，避免交接时重复分配。
        let mut children = Vec::with_capacity(3);
        // 页首存在时以角色 key 保持 reconcile 身份。
        if let Some(view) = self.header_view.borrow_mut().take() {
            // 页首始终处于直接子节点序列第一位。
            children.push(view.key(Self::HEADER_VIEW_KEY));
        }
        // 页尾存在时以独立角色 key 保持 reconcile 身份。
        if let Some(view) = self.footer_view.borrow_mut().take() {
            // 页尾位于页首之后、加载入口之前。
            children.push(view.key(Self::FOOTER_VIEW_KEY));
        }
        // 加载入口存在时以独立角色 key 保持 reconcile 身份。
        if let Some(view) = self.load_more_view.borrow_mut().take() {
            // 加载入口始终是最后一个真实插槽。
            children.push(view.key(Self::LOAD_MORE_VIEW_KEY));
        }
        // 组件树取得完整子树所有权后负责后续协调与释放。
        children
    }

    // 直接子树变化后清除跨身份的旧布局缓存。
    on_children_changed => (&mut self, child_count: usize) {
        // 保存精确基数供异常结构门禁使用。
        self.child_count = child_count;
        // 新声明子树必须在下一轮布局重新测量页首。
        self.header_view_height.set(0.0);
        // 新声明子树必须在下一轮布局重新测量页尾。
        self.footer_view_height.set(0.0);
        // 新声明子树必须在下一轮布局重新测量加载入口。
        self.load_more_view_height.set(0.0);
    }

    // 以自然高度测量三个插槽，最终宽度由 List 内容框统一拉伸。
    measure_children => (&self, _frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // 异常基数不能按位置猜测插槽角色。
        if children.len() != self.slot_view_count() || children.len() != self.child_count {
            // 返回空布局结果让错误结构保持不可交互。
            return Vec::new();
        }
        // 复用与父级测量完全一致的真实子树入口。
        self.measure_slot_children(children, tree)
    }

    // 按 header、文本条目、footer、load-more 的正常流顺序排列真实插槽。
    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        // 只有声明角色、实际子节点与测量结果一一对应时才允许布局。
        if children.len() != self.slot_view_count() || children.len() != self.child_count {
            // 清除全部旧高度，避免绘制继续为失效子树留位。
            self.clear_slot_heights();
            // 不发布任何猜测位置。
            return Vec::new();
        }
        // 归一化父级分配的最终内容框。
        let frame = Self::normalized_frame(frame);
        // 从 List 顶部开始排列页首。
        let mut y = frame.y;
        // 按真实插槽存在顺序消费测量结果。
        let mut index = 0_usize;
        // 保存最终子节点位置供组件树提交。
        let mut positions = Vec::with_capacity(children.len());
        // 页首节点优先于全部文本条目。
        if self.header_view_enabled {
            // 借用固定角色对应的测量快照。
            let child = &children[index];
            // 计算并保存真实页首位置与正常流外高度。
            let (rect, outer_height) = Self::arrange_slot(frame, y, child);
            // 发布页首 border-box。
            positions.push((child.id, rect));
            // 缓存高度供父组件绘制分隔线时使用。
            self.header_view_height.set(outer_height);
            // 后续文本条目从页首 margin 外框之后开始。
            y += outer_height;
            // 消费页首测量结果。
            index += 1;
        } else {
            // 文本页首仍使用一个标准行高。
            y += self.text_header_height();
            // 未启用节点页首时清除旧缓存。
            self.header_view_height.set(0.0);
        }
        // 全部文本数据项继续由 List 自绘并占据标准行高。
        y += self.items.len() as f32 * self.row_height();
        // 页尾节点位于全部文本条目之后。
        if self.footer_view_enabled {
            // 借用固定角色对应的测量快照。
            let child = &children[index];
            // 计算并保存真实页尾位置与正常流外高度。
            let (rect, outer_height) = Self::arrange_slot(frame, y, child);
            // 发布页尾 border-box。
            positions.push((child.id, rect));
            // 缓存高度供绘制顺序使用。
            self.footer_view_height.set(outer_height);
            // 加载入口从页尾 margin 外框之后开始。
            y += outer_height;
            // 消费页尾测量结果。
            index += 1;
        } else {
            // 文本页尾仍使用一个标准行高。
            y += self.text_footer_height();
            // 未启用节点页尾时清除旧缓存。
            self.footer_view_height.set(0.0);
        }
        // 加载入口节点始终位于 List 最后。
        if self.load_more_view_enabled {
            // 借用最后一个角色对应的测量快照。
            let child = &children[index];
            // 计算并保存真实加载入口位置与正常流外高度。
            let (rect, outer_height) = Self::arrange_slot(frame, y, child);
            // 发布加载入口 border-box。
            positions.push((child.id, rect));
            // 缓存高度供固有尺寸与绘制阶段共享。
            self.load_more_view_height.set(outer_height);
        } else {
            // 未启用节点加载入口时清除旧缓存。
            self.load_more_view_height.set(0.0);
        }
        // 三个真实子树保留自身事件、焦点与语义生命周期。
        positions
    }

    // 子树只能在 List 自身的圆角内容区域内绘制。
    children_clip => (&self, frame: Rect) -> Option<Rect> {
        // 使用与父级自绘一致的有限非负 frame。
        Some(Self::normalized_frame(frame))
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let resolved = self.visual.resolve(ctx.tokens());
        let item_h = self.row_height();
        let radius = resolved
            .radius
            .min(frame.w.min(frame.h) * self.visual.frame.radius_limit_ratio);
        let r = Radius::uniform(radius);
        let mut y = frame.y;
        let horizontal_padding = self
            .visual
            .rows
            .horizontal_padding
            .min(frame.w * self.visual.rows.horizontal_padding_frame_ratio);

        ctx.push_clip(frame);
        ctx.fill_rect(frame, resolved.background, Some(r));
        if self.bordered {
            let border_frame = Self::inset(frame, self.visual.frame.border_inset);
            ctx.stroke_rect(
                border_frame,
                resolved.border,
                self.visual.frame.border_width,
                Some(Radius::uniform(
                    radius.min(
                        border_frame.w.min(border_frame.h)
                            * self.visual.frame.radius_limit_ratio,
                    ),
                )),
            );
        }

        if self.header_view_enabled {
            // 节点型页首由真实子树绘制，List 只保留其正常流占位与底部分隔线。
            let header_height = self.header_view_height.get();
            // 非零真实页首之后绘制与文本兼容路径一致的整宽分隔线。
            if header_height > 0.0 {
                // 分隔线位于页首 margin 外框的底边。
                ctx.fill_rect(
                    Rect::new(
                        frame.x,
                        y + header_height,
                        frame.w,
                        self.visual.frame.divider_width,
                    ),
                    resolved.border,
                    None,
                );
                // 文本条目从真实页首之后开始。
                y += header_height;
            }
        } else if !self.header.is_empty() {
            let header_rect = Self::row_content_rect(frame, y, item_h, horizontal_padding);
            Self::paint_single_line(
                ctx,
                &self.header,
                header_rect,
                resolved.text_secondary,
                self.visual.rows.header_footer_font_size,
            );
            ctx.fill_rect(
                Rect::new(
                    frame.x,
                    y + item_h,
                    frame.w,
                    self.visual.frame.divider_width,
                ),
                resolved.border,
                None,
            );
            y += item_h;
        }

        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Self::row_content_rect(frame, y, item_h, horizontal_padding);
            Self::paint_single_line(ctx, item, item_rect, resolved.text, resolved.item_font_size);
            if i < self.items.len() - 1 {
                ctx.fill_rect(
                    Rect::new(
                        frame.x + horizontal_padding,
                        y + item_h - self.visual.frame.divider_width,
                        (frame.w - horizontal_padding * 2.0).max(0.0),
                        self.visual.frame.divider_width,
                    ),
                    resolved.border,
                    None,
                );
            }
            y += item_h;
        }

        if self.footer_view_enabled {
            // 真实页尾前继续绘制与文本页尾一致的内容分隔线。
            if !self.items.is_empty() {
                // 分隔线位于最后一个文本条目之后。
                ctx.fill_rect(
                    Rect::new(frame.x, y, frame.w, self.visual.frame.divider_width),
                    resolved.border,
                    None,
                );
            }
            // 页尾真实子树自行绘制，只推进其 margin 外高度。
            y += self.footer_view_height.get();
        } else if !self.footer.is_empty() {
            if !self.items.is_empty() {
                ctx.fill_rect(
                    Rect::new(frame.x, y, frame.w, self.visual.frame.divider_width),
                    resolved.border,
                    None,
                );
            }
            let footer_rect = Self::row_content_rect(frame, y, item_h, horizontal_padding);
            Self::paint_single_line(
                ctx,
                &self.footer,
                footer_rect,
                resolved.text_secondary,
                self.visual.rows.header_footer_font_size,
            );
            y += item_h;
        }

        // 节点型加载入口由真实子树绘制，只有兼容文本路径需要父组件自绘。
        if !self.load_more_view_enabled && !self.load_more_text.is_empty() {
            let load_rect = Rect::new(frame.x, y, frame.w, self.visual.rows.load_more_height);
            ctx.fill_rect(load_rect, resolved.background, None);
            ctx.stroke_rect(
                load_rect,
                resolved.border,
                self.visual.frame.border_width,
                Some(r),
            );
            let load_content = Self::row_content_rect(
                frame,
                y,
                self.visual.rows.load_more_height,
                horizontal_padding,
            );
            Self::paint_single_line(
                ctx,
                &self.load_more_text,
                load_content,
                self.visual.palette.primary.resolve(ctx.tokens()),
                self.visual.rows.load_more_font_size,
            );
        }
        ctx.pop_clip();
    }
}

impl List {
    // 页首角色使用固定运行时 key，避免其他插槽增删时误复用身份。
    const HEADER_VIEW_KEY: &'static str = "__uix_list_header";
    // 页尾角色使用独立固定运行时 key。
    const FOOTER_VIEW_KEY: &'static str = "__uix_list_footer";
    // 加载入口角色使用独立固定运行时 key。
    const LOAD_MORE_VIEW_KEY: &'static str = "__uix_list_load_more";

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    fn inset(frame: Rect, amount: f32) -> Rect {
        let amount = amount.min(frame.w * 0.5).min(frame.h * 0.5).max(0.0);
        Rect::new(
            frame.x + amount,
            frame.y + amount,
            (frame.w - amount * 2.0).max(0.0),
            (frame.h - amount * 2.0).max(0.0),
        )
    }

    fn row_content_rect(frame: Rect, y: f32, height: f32, horizontal_padding: f32) -> Rect {
        Rect::new(
            frame.x + horizontal_padding,
            y,
            (frame.w - horizontal_padding * 2.0).max(0.0),
            height.max(0.0),
        )
    }

    // 将任意非有限或负尺寸收敛为安全的零值。
    fn finite_dimension(value: f32) -> f32 {
        // 只有有限正值可以进入布局几何。
        if value.is_finite() {
            // 负值不能反向推进正常流。
            value.max(0.0)
        } else {
            // 非有限值不得泄漏到组件树 frame。
            0.0
        }
    }

    // 计算一个真实插槽包含四侧 margin 的自然外尺寸。
    fn child_outer_size(child: &LayoutChild) -> Size {
        // 水平方向包含 border-box 与左右 margin。
        let width = Self::finite_dimension(
            // 使用测量尺寸与声明 margin 共同形成正常流宽度。
            child.measured_size.w + child.margin.left + child.margin.right,
        );
        // 垂直方向包含 border-box 与上下 margin。
        let height = Self::finite_dimension(
            // 使用测量尺寸与声明 margin 共同形成正常流高度。
            child.measured_size.h + child.margin.top + child.margin.bottom,
        );
        // 返回 List 自有的尺寸语义。
        Size::new(width, height)
    }

    // 在指定正常流纵坐标安排一个真实插槽。
    fn arrange_slot(frame: Rect, y: f32, child: &LayoutChild) -> (Rect, f32) {
        // 先取得包含 margin 的自然外高度。
        let outer = Self::child_outer_size(child);
        // 子 border-box 水平拉伸到 List 内容宽度并扣除声明 margin。
        let width = Self::finite_dimension(frame.w - child.margin.left - child.margin.right);
        // 子 border-box 保留自己的自然高度。
        let height = Self::finite_dimension(child.measured_size.h);
        // margin 只移动真实子树，不改变 List 的内容原点。
        let rect = Rect::new(
            // 从 List 左边缘加左 margin 开始。
            frame.x + child.margin.left,
            // 从当前槽位顶部加上 margin 开始。
            y + child.margin.top,
            // 使用最终拉伸宽度。
            width,
            // 使用真实自然高度。
            height,
        );
        // 同时返回 border-box 与下一槽位需要推进的 margin 外高度。
        (rect, outer.h)
    }

    // 按固定角色顺序测量当前全部真实插槽。
    fn measure_slot_children(&self, children: &[WidgetId], tree: &WidgetTree) -> Vec<LayoutChild> {
        // 每个插槽用无约束自然测量建立自身高度和最小宽度。
        children
            // 按运行时直接子节点顺序遍历。
            .iter()
            // 将身份投影为完整布局快照。
            .map(|child| {
                // 复用 UI System 私有的统一子树测量入口。
                child_from_tree_with_constraints(*child, tree, Constraints::unconstrained())
            })
            // 收集供 measure 与 arrange 共同使用。
            .collect()
    }

    // 返回当前声明中启用的真实节点插槽数量。
    fn slot_view_count(&self) -> usize {
        // 三个布尔角色各贡献零或一个真实直接子节点。
        usize::from(self.header_view_enabled)
            // 累加可选页尾。
            + usize::from(self.footer_view_enabled)
            // 累加可选加载入口。
            + usize::from(self.load_more_view_enabled)
    }

    // 返回当前 UIX 视觉表中与控件尺寸匹配的标准行高。
    fn row_height(&self) -> f32 {
        self.visual.rows.height(self.list_size)
    }

    // 计算仍由兼容文本路径负责的页首高度。
    fn text_header_height(&self) -> f32 {
        // 节点型页首不再保留文本行占位。
        if self.header_view_enabled || self.header.is_empty() {
            // 缺失文本页首不占空间。
            0.0
        } else {
            // 兼容文本页首沿用标准 List 行高。
            self.row_height()
        }
    }

    // 计算仍由兼容文本路径负责的页尾高度。
    fn text_footer_height(&self) -> f32 {
        // 节点型页尾不再保留文本行占位。
        if self.footer_view_enabled || self.footer.is_empty() {
            // 缺失文本页尾不占空间。
            0.0
        } else {
            // 兼容文本页尾沿用标准 List 行高。
            self.row_height()
        }
    }

    // 计算仍由兼容文本路径负责的加载入口高度。
    fn text_load_more_height(&self) -> f32 {
        // 节点型加载入口不再保留旧文字按钮占位。
        if self.load_more_view_enabled || self.load_more_text.is_empty() {
            // 缺失文字入口不占空间。
            0.0
        } else {
            // 兼容加载文字沿用既有四十逻辑像素高度。
            self.visual.rows.load_more_height
        }
    }

    // 计算文本条目与仍使用字符串入口的全部内容高度。
    fn text_content_height(&self) -> f32 {
        // 文本数据项始终按当前控件尺寸累计。
        self.items.len() as f32 * self.row_height()
            // 兼容文本页首只在没有节点页首时参与。
            + self.text_header_height()
            // 兼容文本页尾只在没有节点页尾时参与。
            + self.text_footer_height()
            // 兼容加载文字只在没有节点入口时参与。
            + self.text_load_more_height()
    }

    // 清除全部真实插槽的旧布局高度。
    fn clear_slot_heights(&self) {
        // 清除页首旧高度。
        self.header_view_height.set(0.0);
        // 清除页尾旧高度。
        self.footer_view_height.set(0.0);
        // 清除加载入口旧高度。
        self.load_more_view_height.set(0.0);
    }

    fn paint_single_line(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: crate::draw::Color,
        font_size: f32,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line_cow(value, font_size, frame.w) else {
            return;
        };
        ctx.push_clip(frame);
        ctx.draw_text_in_frame(&value, frame, color, font_size);
        ctx.pop_clip();
    }

    fn intrinsic_size(&self) -> Size {
        // 从全部文本路径内容高度开始。
        let h = self.text_content_height()
            // 加上最近一次真实页首布局的 margin 外高度。
            + self.header_view_height.get()
            // 加上最近一次真实页尾布局的 margin 外高度。
            + self.footer_view_height.get()
            // 加上最近一次真实加载入口布局的 margin 外高度。
            + self.load_more_view_height.get();
        // 保留旧默认宽度与最小高度。
        Size::new(
            self.visual.defaults.width,
            h.max(self.visual.defaults.min_height),
        )
    }

    /// 创建使用当前配置尺寸、带边框且无内容的列表。
    pub fn new() -> Self {
        Self {
            header: String::new(),
            footer: String::new(),
            bordered: LIST_VISUAL.defaults.bordered,
            bordered_authored: false,
            list_size: crate::ui::widget_runtime::config::use_config().size,
            items: Vec::new(),
            load_more_text: String::new(),
            // 缺省页首继续使用兼容文本路径。
            header_view_enabled: false,
            // 缺省页尾继续使用兼容文本路径。
            footer_view_enabled: false,
            // 缺省加载入口继续使用兼容文本路径。
            load_more_view_enabled: false,
            // 初始没有等待交接的页首子树。
            header_view: RefCell::new(None),
            // 初始没有等待交接的页尾子树。
            footer_view: RefCell::new(None),
            // 初始没有等待交接的加载入口子树。
            load_more_view: RefCell::new(None),
            // 组件树尚未登记任何直接子节点。
            child_count: 0,
            // 页首尚未产生真实布局高度。
            header_view_height: Cell::new(0.0),
            // 页尾尚未产生真实布局高度。
            footer_view_height: Cell::new(0.0),
            // 加载入口尚未产生真实布局高度。
            load_more_view_height: Cell::new(0.0),
            visual: LIST_VISUAL_REF,
        }
    }
    /// 替换列表按声明顺序展示的文本项。
    pub fn items(mut self, items: Vec<impl Into<String>>) -> Self {
        self.items = items.into_iter().map(|s| s.into()).collect();
        self
    }
    /// 设置列表头文本；空文本不占用列表头行。
    pub fn header(mut self, h: &str) -> Self {
        // 兼容文本入口最后调用时取得页首角色所有权。
        self.header_view_enabled = false;
        // 丢弃尚未交接的节点页首，让其捕获资源正常回滚。
        self.header_view.replace(None);
        // 保存拥有型兼容文本。
        self.header = h.to_string();
        // 返回可继续配置的 List。
        self
    }
    /// 设置列表头真实 View；最后调用的文本或 View 构建器拥有该插槽。
    pub fn header_view<V: crate::ui::view::View>(mut self, view: V) -> Self {
        // 节点入口取得页首角色所有权。
        self.header_view_enabled = true;
        // 清除兼容文本，避免快照与绘制重复发布页首内容。
        self.header.clear();
        // 保存包含样式、事件、焦点和状态捕获的完整声明子树。
        self.header_view
            // 在运行时树接管前只保留一份拥有型 ViewNode。
            .replace(Some(crate::ui::view::View::build(view)));
        // 返回可继续配置的 List。
        self
    }
    /// 设置列表尾文本；空文本不占用列表尾行。
    pub fn footer(mut self, f: &str) -> Self {
        // 兼容文本入口最后调用时取得页尾角色所有权。
        self.footer_view_enabled = false;
        // 丢弃尚未交接的节点页尾。
        self.footer_view.replace(None);
        // 保存拥有型兼容文本。
        self.footer = f.to_string();
        // 返回可继续配置的 List。
        self
    }
    /// 设置列表尾真实 View；最后调用的文本或 View 构建器拥有该插槽。
    pub fn footer_view<V: crate::ui::view::View>(mut self, view: V) -> Self {
        // 节点入口取得页尾角色所有权。
        self.footer_view_enabled = true;
        // 清除兼容文本，避免快照与绘制重复发布页尾内容。
        self.footer.clear();
        // 保存包含完整声明身份的页尾子树。
        self.footer_view
            // 在运行时树接管前只保留一份拥有型 ViewNode。
            .replace(Some(crate::ui::view::View::build(view)));
        // 返回可继续配置的 List。
        self
    }
    /// 设置是否绘制列表外框与行分隔线。
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self.bordered_authored = true;
        self
    }
    /// 设置列表行采用的控件尺寸规格。
    pub fn size(mut self, s: ControlSize) -> Self {
        self.list_size = s;
        self
    }
    /// 设置列表末尾的加载更多文本；空文本不创建该区域。
    pub fn load_more(mut self, text: impl Into<String>) -> Self {
        // 兼容文本入口最后调用时取得加载角色所有权。
        self.load_more_view_enabled = false;
        // 丢弃尚未交接的节点加载入口。
        self.load_more_view.replace(None);
        // 保存拥有型兼容文本。
        self.load_more_text = text.into();
        // 返回可继续配置的 List。
        self
    }
    /// 设置列表末尾真实加载入口 View，可包含按钮及其完整交互状态。
    pub fn load_more_view<V: crate::ui::view::View>(mut self, view: V) -> Self {
        // 节点入口取得加载角色所有权。
        self.load_more_view_enabled = true;
        // 清除兼容文字入口，避免父组件继续自绘伪按钮。
        self.load_more_text.clear();
        // 保存包含完整交互与状态捕获的加载入口子树。
        self.load_more_view
            // 在运行时树接管前只保留一份拥有型 ViewNode。
            .replace(Some(crate::ui::view::View::build(view)));
        // 返回可继续配置的 List。
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::List {
            header: self.header.clone(),
            footer: self.footer.clone(),
            bordered: self.bordered,
            list_size: self.list_size,
            items: self.items.clone(),
            load_more_text: self.load_more_text.clone(),
            // 发布节点型页首存在事实，不复制子树快照。
            header_view: self.header_view_enabled,
            // 发布节点型页尾存在事实，不复制子树快照。
            footer_view: self.footer_view_enabled,
            // 发布节点型加载入口存在事实，不复制子树快照。
            load_more_view: self.load_more_view_enabled,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.header = next.header;
        self.footer = next.footer;
        self.bordered = next.bordered;
        self.bordered_authored = next.bordered_authored;
        self.list_size = next.list_size;
        self.items = next.items;
        self.load_more_text = next.load_more_text;
        // 同步三个节点角色的声明事实。
        self.header_view_enabled = next.header_view_enabled;
        // 同步页尾节点角色事实。
        self.footer_view_enabled = next.footer_view_enabled;
        // 同步加载入口节点角色事实。
        self.load_more_view_enabled = next.load_more_view_enabled;
        // 交给适配器协调本轮声明的页首 ViewNode。
        self.header_view.replace(next.header_view.into_inner());
        // 交给适配器协调本轮声明的页尾 ViewNode。
        self.footer_view.replace(next.footer_view.into_inner());
        // 交给适配器协调本轮声明的加载入口 ViewNode。
        self.load_more_view
            // 取出下一声明的一次性交接槽位。
            .replace(next.load_more_view.into_inner());
        self.visual = next.visual;
    }

    // 测试目标观察 UIX 声明的关键视觉契约，不暴露到公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, f32) {
        (
            self.visual.defaults.width,
            self.visual.defaults.min_height,
            self.visual.rows.small_height,
            self.visual.rows.medium_height,
            self.visual.rows.large_height,
            self.visual.rows.horizontal_padding,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

// 把列表数据、真实插槽与 UIX 视觉表融合为单一根节点。
fn build_list_view(mut kernel: List, visual: &'static ListVisual) -> ViewNode {
    if !kernel.bordered_authored {
        kernel.bordered = visual.defaults.bordered;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for List {
    fn build(self) -> ViewNode {
        if self.items.is_empty() {
            if let Some(empty) = crate::ui::widget_runtime::config::render_empty_for::<Self>() {
                return empty;
            }
            return View::build(crate::ui::widgets::display::Empty::new());
        }
        // UIX 拥有非空列表公开根与静态视觉；Rust 保留 Empty 策略、插槽与布局。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/list/list.uix")
    }
}

// 把节点插槽布局、交互、协调与 Empty 回归限制在 List 模块内部。
#[cfg(test)]
// 测试子模块可以读取私有角色缓存而不扩大公开 API。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/display/list/tests.rs"]
mod tests;
