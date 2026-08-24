//! Badge — 徽章组件。
//!
//! 支持物理单位：`offset()` 接受 mm/cm/pt，自动适配 DPI。

// 引入组合子树一次性交接所需的共享单元。
use std::cell::{Cell, RefCell};
// 引入组合子树声明期所有权所需的共享句柄。
use std::rc::Rc;

use crate::widget;
// 引入组件与子树布局使用的身份、约束和几何类型。
use crate::core::{Constraints, Rect, Size, WidgetId};
use crate::draw::geometry::spatial::PhysicalUnit;
// 引入组合装饰器所需的子节点后绘制阶段。
use crate::draw::painting::PaintPass;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
// 引入从组件树测量真实子节点的 System 私有边界。
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints;
use crate::ui::widget_runtime::widget::WidgetTree;
// 引入组合布局快照类型。
use crate::ui::{LayoutChild, PrimaryHue, SnapshotFields, ThemeTokens};

/// 徽章状态。
///
/// 与 ValidateStatus/InputStatus/StepStatus/UploadStatus 共享「组件状态」命名模式，
/// 但各自语义与变体独立（本枚举含 Default/Processing 徽章专用态），勿强行合并。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BadgeStatus {
    /// 表示操作成功或状态正常。
    Success,
    /// 表示操作正在进行中。
    Processing,
    /// 表示不带功能色倾向的默认状态。
    Default,
    /// 表示操作失败或发生错误。
    Error,
    /// 表示需要用户注意的警告状态。
    Warning,
}

/// 预设徽章颜色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BadgeColor {
    /// 蓝色预设；在主题上下文中跟随品牌主色。
    Blue,
    /// 绿色预设；在主题上下文中跟随成功色。
    Green,
    /// 橙色预设。
    Orange,
    /// 红色预设；在主题上下文中跟随错误色。
    Red,
    /// 紫色预设。
    Purple,
}

impl BadgeColor {
    /// 映射为对应的 `Color`。
    /// 无主题上下文时返回 theme 层 Ant Design 色阶的兼容主色。
    pub fn to_color(self) -> Color {
        // 兼容纯值 API 只读取 theme 层拥有的预设色阶。
        match self {
            // 蓝色返回预设蓝色色阶主色。
            BadgeColor::Blue => PrimaryHue::Blue.primary(),
            // 绿色返回预设绿色色阶主色。
            BadgeColor::Green => PrimaryHue::Green.primary(),
            // 橙色返回预设橙色色阶主色。
            BadgeColor::Orange => PrimaryHue::Orange.primary(),
            // 红色返回预设红色色阶主色。
            BadgeColor::Red => PrimaryHue::Red.primary(),
            // 紫色返回预设紫色色阶主色。
            BadgeColor::Purple => PrimaryHue::Purple.primary(),
        }
    }

    // 在拥有主题上下文时解析预设色。
    fn resolve(self, tokens: &dyn ThemeTokens) -> Color {
        // 可定制功能角色服从当前主题 token，其余色相读取 theme 色阶。
        match self {
            // 蓝色徽章跟随当前主题品牌主色。
            BadgeColor::Blue => tokens.color_primary(),
            // 绿色徽章跟随当前主题成功色。
            BadgeColor::Green => tokens.color_success(),
            // 橙色徽章保留明确的预设色相。
            BadgeColor::Orange => PrimaryHue::Orange.primary(),
            // 红色徽章跟随当前主题错误色。
            BadgeColor::Red => tokens.color_error(),
            // 紫色徽章保留明确的预设色相。
            BadgeColor::Purple => PrimaryHue::Purple.primary(),
        }
    }

    // 从兼容纯色值恢复框架拥有的预设身份。
    fn from_compat_color(color: Color) -> Option<Self> {
        // 依次匹配五个稳定预设主色。
        [
            // 蓝色预设。
            Self::Blue,
            // 绿色预设。
            Self::Green,
            // 橙色预设。
            Self::Orange,
            // 红色预设。
            Self::Red,
            // 紫色预设。
            Self::Purple,
        ]
        // 转为按值遍历。
        .into_iter()
        // 查找产生相同兼容颜色的预设。
        .find(|preset| preset.to_color() == color)
    }
}

// 解析 Badge 最终背景色并保持自定义颜色优先级。
fn resolve_badge_background(
    // 保存构建器登记的可选颜色。
    color: Option<Color>,
    // 标记该颜色是否来自框架预设。
    adaptive_foreground: bool,
    // 提供当前主题 token。
    tokens: &dyn ThemeTokens,
) -> Color {
    // 没有显式颜色时继续使用错误色默认值。
    let Some(color) = color else {
        // 默认徽章服从当前主题错误色。
        return tokens.color_error();
    };
    // 只有框架预设才允许重新解析主题角色。
    if adaptive_foreground {
        // 从稳定兼容色恢复预设身份。
        if let Some(preset) = BadgeColor::from_compat_color(color) {
            // 返回当前主题下的预设结果。
            return preset.resolve(tokens);
        }
    }
    // 任意调用方颜色保持原值。
    color
}

/// 可用于 [`Badge::color`] 的颜色输入。
pub trait IntoBadgeColor {
    /// 转换为兼容颜色及是否应按当前主题重新解析预设色的标志。
    fn into_badge_color(self) -> (Color, bool);
}

impl IntoBadgeColor for Color {
    fn into_badge_color(self) -> (Color, bool) {
        (self, false)
    }
}

impl IntoBadgeColor for BadgeColor {
    fn into_badge_color(self) -> (Color, bool) {
        (self.to_color(), true)
    }
}

fn finite_badge_offset(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

widget! {
    /// 在可选子内容上叠加数字、圆点或文字角标的装饰组件。
    pub struct Badge {
        count: i32,
        max: i32,
        dot: bool,
        color: Option<Color>,
        adaptive_foreground: bool,
        ribbon: bool,
        status: Option<BadgeStatus>,
        show_zero: bool,
        text: String,

        /// 标记当前声明是否拥有唯一真实子 View。
        composite: bool,
        // Badge 在声明期拥有完整子 ViewNode，并只向组件树交接一次。
        #[snapshot(skip)]
        child_view: Option<Rc<RefCell<Option<crate::ui::view::ViewNode>>>>,
        /// 保存组件树当前登记的直接子节点数量。
        child_count: usize,
        /// 缓存唯一子节点包含 margin 的自然外尺寸，供布局收敛使用。
        child_outer_size: Cell<Size>,
        /// 保存最终安排出的真实子节点 border-box。
        child_frame: Cell<Rect>,
        /// 保存上一帧装饰实际绘制边界，供旧位置清理使用。
        decoration_bounds: Cell<Rect>,
        /// 保存最近一次绘制使用的 DPI，供物理偏移脏区换算使用。
        last_dpi: Cell<f32>,

        // ── 2D 偏移（f32 像素）──
        offset_x: f32,
        offset_y: f32,

        // ── 物理单位偏移（优先级高于 offset_x/y）──
        offset_unit: Option<(PhysicalUnit, PhysicalUnit)>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        // 组合模式由真实子节点的外尺寸参与正常布局流。
        let desired = if self.composite {
            // 使用上一轮子测量写回的有限外尺寸推动同次布局收敛。
            self.child_outer_size.get()
        } else {
            // 零子节点继续使用旧徽章固有尺寸。
            self.intrinsic_size()
        };
        // 始终尊重父级约束。
        constraints.clamp(desired)
    }

    // 记录真实直接子树的挂载、卸载或替换事实。
    on_children_changed => (&mut self, child_count: usize) {
        // 保存精确基数，避免额外子节点被静默消费。
        self.child_count = child_count;
        // 结构变化后清除旧子尺寸，禁止跨身份沿用布局事实。
        self.child_outer_size.set(Size::zero());
        // 结构变化后清除旧子 frame，装饰将在下一次布局后重新锚定。
        self.child_frame.set(Rect::zero());
    }

    // 将声明期拥有的唯一子 ViewNode 一次性交给组件树物化。
    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        // 借用可选的一次性交接槽位。
        self.child_view
            // 组合模式才持有声明期子树。
            .as_ref()
            // 取走完整 ViewNode，后续 reconcile 由组件树保持身份。
            .and_then(|view| view.borrow_mut().take())
            // 把零或一个子树物化为直接子节点集合。
            .into_iter()
            // 返回组件树可消费的有序集合。
            .collect()
    }

    // 使用唯一真实子节点自己的布局契约取得自然尺寸与 margin。
    measure_children => (&self, _frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // 只有精确一个直接子节点才满足组合契约。
        if !self.composite || children.len() != 1 {
            // 多子节点不得退化为静默选择第一项。
            return Vec::new();
        }
        // 使用无约束测量取得真实子节点自然 border-box。
        vec![child_from_tree_with_constraints(
            // 唯一索引已由上方基数门禁保证。
            children[0],
            // 读取子节点组件契约的当前树。
            tree,
            // 装饰器不把自身装饰尺寸施加给子节点。
            Constraints::unconstrained(),
        )]
    }

    // 安排唯一子节点并把其 margin 外尺寸写回父级测量缓存。
    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        // 精确基数是运行时组合的必要条件。
        if !self.composite || children.len() != 1 || self.child_count != 1 {
            // 无有效子树时清除当前布局事实。
            self.child_outer_size.set(Size::zero());
            // 无有效子树时也清除装饰锚点。
            self.child_frame.set(Rect::zero());
            // 不为非法基数生成任何布局结果。
            return Vec::new();
        }
        // 借用本轮唯一子节点测量快照。
        let child = &children[0];
        // 把非有限或负自然宽度收敛为零。
        let natural_w = Self::finite_dimension(child.measured_size.w);
        // 把非有限或负自然高度收敛为零。
        let natural_h = Self::finite_dimension(child.measured_size.h);
        // 保存包含左右 margin 的正常流外宽度。
        let outer_w = Self::finite_dimension(natural_w + child.margin.left + child.margin.right);
        // 保存包含上下 margin 的正常流外高度。
        let outer_h = Self::finite_dimension(natural_h + child.margin.top + child.margin.bottom);
        // 写回下一测量阶段使用的真实外尺寸。
        self.child_outer_size.set(Size::new(outer_w, outer_h));
        // 已分配正宽度时让子节点填满扣除 margin 后的内容宽度。
        let child_w = if frame.w > 0.0 {
            // margin 仍参与可用内容宽度计算。
            Self::finite_dimension(frame.w - child.margin.left - child.margin.right)
        } else {
            // 首轮零 frame 使用自然宽度推动布局收敛。
            natural_w
        };
        // 已分配正高度时让子节点填满扣除 margin 后的内容高度。
        let child_h = if frame.h > 0.0 {
            // margin 仍参与可用内容高度计算。
            Self::finite_dimension(frame.h - child.margin.top - child.margin.bottom)
        } else {
            // 首轮零 frame 使用自然高度推动布局收敛。
            natural_h
        };
        // 子节点 border-box 从 Badge 内容原点加自身 margin 开始。
        let child_frame = Rect::new(
            // 左 margin 只移动真实子节点，不移动 Badge 正常流原点。
            frame.x + child.margin.left,
            // 上 margin 只移动真实子节点，不移动 Badge 正常流原点。
            frame.y + child.margin.top,
            // 使用最终内容宽度。
            child_w,
            // 使用最终内容高度。
            child_h,
        );
        // 保存装饰定位与快照共同消费的最终 border-box。
        self.child_frame.set(child_frame);
        // 返回唯一真实子节点的确定布局结果。
        vec![(child.id, child_frame)]
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    // 单 child 装饰器必须在真实子树完成后绘制角标。
    paint_after_children => (&self) -> bool {
        // 叶模式继续只使用 Content 阶段。
        self.composite && self.child_count == 1
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        // 组合模式只在真实子树绘制完成后叠加装饰。
        let expected_pass = if self.composite && self.child_count == 1 {
            // 保证装饰覆盖在子节点视觉之上。
            PaintPass::AfterChildren
        } else {
            // 零子节点保持旧叶组件绘制阶段。
            PaintPass::Content
        };
        // 忽略当前模式不使用的绘制阶段。
        if ctx.paint_pass() != expected_pass {
            // 防止同一装饰在内容与子节点后阶段重复绘制。
            return;
        }
        // 计算实际偏移：物理单位优先
        let (off_x, off_y) = if let Some((ux, uy)) = self.offset_unit {
            let dpi = ctx.dpi();
            (ux.to_dip(dpi), uy.to_dip(dpi))
        } else {
            (self.offset_x, self.offset_y)
        };
        let (off_x, off_y) = (finite_badge_offset(off_x), finite_badge_offset(off_y));
        // 记录当前 DPI，供下一次失效计算物理单位的新位置。
        self.last_dpi.set(ctx.dpi());
        // 组合模式按真实子 border-box 计算独立装饰矩形。
        let actual_frame = if self.composite && self.child_count == 1 {
            // 装饰只改变视觉位置，不扩大正常布局尺寸。
            self.composite_decoration_frame(frame, off_x, off_y)
        } else {
            // 零子节点完全保留旧 frame 与偏移行为。
            Rect::new(frame.x + off_x, frame.y + off_y, frame.w, frame.h)
        };
        // 保存最终装饰边界供快照与下一次脏区清理使用。
        self.decoration_bounds.set(actual_frame);

        // 隐藏状态不绘制空装饰，但真实子树已经正常完成布局和绘制。
        if actual_frame.w <= 0.0 || actual_frame.h <= 0.0 {
            // count=0 且未 showZero 只隐藏装饰。
            return;
        }

        if let Some(status) = self.status {
            let marker_color = match status {
                BadgeStatus::Success => ctx.tokens().color_success(),
                BadgeStatus::Processing => ctx.tokens().color_primary(),
                BadgeStatus::Default => ctx.tokens().color_text_quaternary(),
                BadgeStatus::Error => ctx.tokens().color_error(),
                BadgeStatus::Warning => ctx.tokens().color_warning(),
            };
            self.render_marker_label(ctx, actual_frame, marker_color);
            return;
        }

        // 预设色在绘制时解析当前主题，自定义色保持原值。
        let bg = resolve_badge_background(self.color, self.adaptive_foreground, ctx.tokens());
        // 自适应前景：按亮度取黑白 token 对比色。
        let foreground = if self.adaptive_foreground && bg.is_light() {
            ctx.tokens().color_black()
        } else {
            ctx.tokens().color_white()
        };
        if self.ribbon {
            self.render_ribbon(ctx, actual_frame, bg, foreground);
            return;
        }
        if self.dot {
            self.render_marker_label(ctx, actual_frame, bg);
            return;
        }
        if self.count == 0 && !self.show_zero && self.text.is_empty() { return; }
        if !self.text.is_empty() {
            let r = Some(Radius::uniform(actual_frame.h * 0.5));
            ctx.fill_rect(actual_frame, bg, r);
            let fs = Self::PILL_FONT_SIZE;
            let tw = ctx.measure_text(&self.text, fs).w;
            let th = ctx.line_box_height(fs);
            ctx.draw_text(
                &self.text,
                crate::core::Point::new(
                    actual_frame.x + (actual_frame.w - tw) * 0.5,
                    actual_frame.y + (actual_frame.h - th) * 0.5,
                ),
                foreground,
                fs,
            );
        } else {
            let r = Some(Radius::uniform(actual_frame.h * 0.5));
            ctx.fill_rect(actual_frame, bg, r);
            let label = self.count_label();
            let fs = Self::PILL_FONT_SIZE;
            let tw = ctx.measure_text(&label, fs).w;
            let th = ctx.line_box_height(fs);
            ctx.draw_text(
                &label,
                crate::core::Point::new(
                    actual_frame.x + (actual_frame.w - tw) * 0.5,
                    actual_frame.y + (actual_frame.h - th) * 0.5,
                ),
                foreground,
                fs,
            );
        }
    }

    // 脏区覆盖父 frame、真实子 frame、旧装饰位置与按最近 DPI 解析的新位置。
    dirty_rect => (&self, frame: Rect) -> Rect {
        // 先保留 Badge 正常流 frame。
        let mut bounds = frame;
        // 组合模式还必须覆盖真实子节点 border-box。
        if self.composite && self.child_count == 1 && Self::has_area(self.child_frame.get()) {
            // 合并布局阶段保存的真实子 frame。
            bounds = bounds.union(&self.child_frame.get());
        }
        // 非空旧装饰位置必须被清理。
        if Self::has_area(self.decoration_bounds.get()) {
            // 合并上一帧实际绘制位置以保证旧像素可被清理。
            bounds = bounds.union(&self.decoration_bounds.get());
        }
        // 使用最近一次有效 DPI 解析当前声明的新装饰位置。
        let (off_x, off_y) = self.resolved_offset(self.last_dpi.get());
        // 组合与叶模式分别复用各自几何契约。
        let current = if self.composite && self.child_count == 1 {
            // 新装饰继续锚定真实子 border-box。
            self.composite_decoration_frame(frame, off_x, off_y)
        } else {
            // 叶模式新位置继续平移完整 frame。
            Rect::new(frame.x + off_x, frame.y + off_y, frame.w, frame.h)
        };
        // 非空新装饰位置必须进入本轮重绘区域。
        if Self::has_area(current) {
            // 返回旧位置与新位置的确定并集。
            bounds.union(&current)
        } else {
            // 隐藏装饰只需清理旧位置并保留子树区域。
            bounds
        }
    }
}

impl Default for Badge {
    fn default() -> Self {
        Self::new()
    }
}

// 把徽章组合状态、唯一子树交接槽与绘制内核融合为 UIX 声明的单一叶根。
fn build_badge_view(kernel: Badge) -> ViewNode {
    ViewNode::leaf(kernel)
}

impl View for Badge {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根；真实子树仍由 Badge 生命周期端口一次性交给组件树。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/badge/badge.uix")
    }
}

impl Badge {
    const MARKER_DIAMETER: f32 = 10.0;
    const MARKER_TEXT_GAP: f32 = 8.0;
    const PILL_HEIGHT: f32 = 20.0;
    const PILL_FONT_SIZE: f32 = 11.0;
    const MARKER_LABEL_FONT_SIZE: f32 = 13.0;
    const TEXT_HORIZONTAL_PADDING: f32 = 12.0;
    const RIBBON_HEIGHT: f32 = 24.0;
    const RIBBON_HORIZONTAL_PADDING: f32 = 24.0;

    fn intrinsic_size(&self) -> Size {
        if self.dot || self.status.is_some() {
            if self.text.is_empty() {
                Size::new(Self::MARKER_DIAMETER, Self::MARKER_DIAMETER)
            } else {
                Size::new(
                    Self::MARKER_DIAMETER
                        + Self::MARKER_TEXT_GAP
                        + Self::estimated_text_width(&self.text, Self::MARKER_LABEL_FONT_SIZE),
                    Self::PILL_HEIGHT,
                )
            }
        } else if self.ribbon {
            let w = Self::estimated_text_width(&self.text, Self::PILL_FONT_SIZE)
                + Self::RIBBON_HORIZONTAL_PADDING;
            Size::new(w.max(Self::RIBBON_HEIGHT), Self::RIBBON_HEIGHT)
        } else if !self.text.is_empty() {
            let w = Self::estimated_text_width(&self.text, Self::PILL_FONT_SIZE)
                + Self::TEXT_HORIZONTAL_PADDING;
            Size::new(w.max(Self::PILL_HEIGHT), Self::PILL_HEIGHT)
        } else if self.count > 0 || (self.count == 0 && self.show_zero) {
            let w = Self::estimated_text_width(&self.count_label(), Self::PILL_FONT_SIZE)
                + Self::TEXT_HORIZONTAL_PADDING;
            Size::new(w.max(Self::PILL_HEIGHT), Self::PILL_HEIGHT)
        } else {
            Size::zero()
        }
    }

    // 把任意布局维度收敛为有限非负值。
    fn finite_dimension(value: f32) -> f32 {
        // 有限值保留并拒绝负尺寸。
        if value.is_finite() {
            // 布局尺寸下界固定为零。
            value.max(0.0)
        } else {
            // 非有限输入不得进入组件树 frame。
            0.0
        }
    }

    // 判断矩形是否具有可绘制面积。
    fn has_area(rect: Rect) -> bool {
        // 两个维度都为正才允许参与并集，避免零矩形把原点带入脏区。
        rect.w > 0.0 && rect.h > 0.0
    }

    // 使用给定 DPI 解析当前像素或物理单位偏移。
    fn resolved_offset(&self, dpi: f32) -> (f32, f32) {
        // 物理单位声明优先于像素偏移。
        let (x, y) = if let Some((unit_x, unit_y)) = self.offset_unit {
            // 非法 DPI 回退到标准桌面 DPI，避免生成非有限脏区。
            let dpi = if dpi.is_finite() && dpi > 0.0 {
                dpi
            } else {
                96.0
            };
            // 每个轴只在此处执行一次 DIP 换算。
            (unit_x.to_dip(dpi), unit_y.to_dip(dpi))
        } else {
            // 未登记物理单位时使用作者像素偏移。
            (self.offset_x, self.offset_y)
        };
        // 返回有限偏移以保护绘制和损伤几何。
        (finite_badge_offset(x), finite_badge_offset(y))
    }

    // 返回组合模式中装饰相对真实子 border-box 的最终矩形。
    fn composite_decoration_frame(&self, fallback: Rect, off_x: f32, off_y: f32) -> Rect {
        // 隐藏装饰不应生成占位或损伤面积。
        let size = self.intrinsic_size();
        // 零尺寸直接返回空矩形。
        if size.w <= 0.0 || size.h <= 0.0 {
            // 使用零矩形表达隐藏状态。
            return Rect::zero();
        }
        // 优先使用布局阶段保存的真实子 border-box。
        let child = if self.child_frame.get().w > 0.0 || self.child_frame.get().h > 0.0 {
            // 已布局时消费精确子 frame。
            self.child_frame.get()
        } else {
            // 首帧绘制保护路径使用当前 Badge frame。
            fallback
        };
        // 丝带覆盖子节点右上边缘，而不是把中心放到角点。
        if self.ribbon {
            // 返回丝带右边缘与子节点右边缘对齐的矩形。
            return Rect::new(
                // 偏移只作用于装饰。
                child.x + child.w - size.w + off_x,
                // 丝带顶部与子节点顶部对齐。
                child.y + off_y,
                // 使用徽章自身固有宽度。
                size.w,
                // 使用徽章自身固有高度。
                size.h,
            );
        }
        // 带文字的 marker 让圆点中心精确锚定子节点右上角。
        let marker_label = (self.dot || self.status.is_some()) && !self.text.is_empty();
        // marker 文本从锚点向右展开，普通胶囊则整体以锚点为中心。
        let x = if marker_label {
            // 圆点半径决定 marker 左边缘。
            child.x + child.w - Self::MARKER_DIAMETER * 0.5 + off_x
        } else {
            // 数字、纯圆点和文本胶囊中心落在右上角。
            child.x + child.w - size.w * 0.5 + off_x
        };
        // 非丝带装饰的垂直中心落在子节点顶部。
        Rect::new(
            // 使用按形态解析的横坐标。
            x,
            // 垂直偏移只移动装饰。
            child.y - size.h * 0.5 + off_y,
            // 使用独立装饰宽度。
            size.w,
            // 使用独立装饰高度。
            size.h,
        )
    }

    /// 创建计数为零、上限为 99 且默认隐藏零值的徽标。
    pub fn new() -> Self {
        Self {
            count: 0,
            max: 99,
            dot: false,
            color: None,
            adaptive_foreground: false,
            ribbon: false,
            status: None,
            show_zero: false,
            text: String::new(),
            // 默认保持零子节点叶组件兼容模式。
            composite: false,
            // 默认没有待交接的子 View。
            child_view: None,
            // 组件树尚未登记任何直接子节点。
            child_count: 0,
            // 首次子测量前没有自然外尺寸缓存。
            child_outer_size: Cell::new(Size::zero()),
            // 首次布局前没有真实子 frame。
            child_frame: Cell::new(Rect::zero()),
            // 首次绘制前没有旧装饰边界。
            decoration_bounds: Cell::new(Rect::zero()),
            // 标准桌面 DPI 作为首次脏区换算的保守基线。
            last_dpi: Cell::new(96.0),
            offset_x: 0.0,
            offset_y: 0.0,
            offset_unit: None,
        }
    }
    /// 设置非负计数；负数会被归零。
    pub fn count(mut self, n: i32) -> Self {
        self.count = n.max(0);
        self
    }
    /// 设置计数显示上限；小于一的值会被归一化为一。
    pub fn max(mut self, n: i32) -> Self {
        self.max = n.max(1);
        self
    }
    /// 启用圆点模式并把计数设置为一。
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self.count = 1;
        self
    }
    /// 按布尔值配置圆点模式，供声明式 UIX 表达式保持同类型生成。
    pub fn dot_when(mut self, enabled: bool) -> Self {
        // 动态切换只改变圆点装饰，不伪造数字计数。
        self.dot = enabled;
        // 返回配置后的组件。
        self
    }
    /// 设置自定义颜色或可随主题解析的预设颜色。
    pub fn color(mut self, c: impl IntoBadgeColor) -> Self {
        let (color, adaptive_foreground) = c.into_badge_color();
        self.color = Some(color);
        self.adaptive_foreground = adaptive_foreground;
        self
    }

    /// 使用预设颜色变体设置徽章颜色。
    pub fn preset_color(self, c: BadgeColor) -> Self {
        self.color(c)
    }

    /// 创建角标丝带（绝对定位，不参与父级正常布局流）。
    pub fn ribbon(text: impl Into<String>, color: BadgeColor) -> Self {
        let text = text.into();
        let mut badge = Self::new().color(color).text(&text);
        badge.ribbon = true;
        badge
    }
    /// 设置徽标的语义状态样式。
    pub fn status(mut self, s: BadgeStatus) -> Self {
        self.status = Some(s);
        self
    }
    /// 设置计数为零时是否仍呈现徽标。
    pub fn show_zero(mut self, v: bool) -> Self {
        self.show_zero = v;
        self
    }
    /// 设置徽标中显示的自定义文本。
    pub fn text(mut self, t: &str) -> Self {
        self.text = t.to_string();
        self
    }

    /// 让 Badge 成为唯一真实子 View 的透明装饰器。
    pub fn child<V: crate::ui::view::View>(mut self, child: V) -> Self {
        // 启用组合布局、绘制和语义契约。
        self.composite = true;
        // 保存包含身份、样式、处理器和状态捕获的完整 ViewNode。
        self.child_view = Some(Rc::new(RefCell::new(Some(
            // 通过公开 View 契约构建调用方子树。
            crate::ui::view::View::build(child),
        ))));
        // 返回拥有待物化子树的 Badge。
        self
    }

    /// 像素偏移。
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset_x = finite_badge_offset(x);
        self.offset_y = finite_badge_offset(y);
        self
    }

    /// 物理单位偏移（优先级高于 `offset()`）。
    /// 自动适配 DPI：`10.mm()` 在不同屏幕上物理尺寸一致。
    pub fn offset_unit(mut self, x: PhysicalUnit, y: PhysicalUnit) -> Self {
        self.offset_unit = Some((x, y));
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.count = next.count.max(0);
        self.max = next.max.max(1);
        self.dot = next.dot;
        self.color = next.color;
        self.adaptive_foreground = next.adaptive_foreground;
        self.ribbon = next.ribbon;
        self.status = next.status;
        self.show_zero = next.show_zero;
        self.text = next.text;
        // 同步声明期组合模式，装饰配置变化不会重建现有子实例。
        self.composite = next.composite;
        // 交给组件树 reconcile 下一声明提供的完整子 ViewNode。
        self.child_view = next.child_view;
        self.offset_x = finite_badge_offset(next.offset_x);
        self.offset_y = finite_badge_offset(next.offset_y);
        self.offset_unit = next.offset_unit;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Badge {
            count: self.count,
            max: self.max,
            dot: self.dot,
            color: self.color,
            adaptive_foreground: self.adaptive_foreground,
            ribbon: self.ribbon,
            size: self.intrinsic_size().h,
            status: self.status,
            show_zero: self.show_zero,
            text: self.text.clone(),
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            offset_unit: self.offset_unit,
            // 标记声明是否采用组合装饰器模式。
            composite: self.composite,
            // 只有精确一个已登记子节点才发布 child-present。
            child_present: self.composite && self.child_count == 1,
            // 发布最终装饰边界，不复制真实子节点自身快照。
            decoration_bounds: self.decoration_bounds.get(),
        }
    }

    fn render_marker_label(&self, ctx: &mut PaintContext, frame: Rect, marker_color: Color) {
        let radius = Self::MARKER_DIAMETER * 0.5;
        let center_y = frame.y + frame.h * 0.5;
        ctx.fill_circle(frame.x + radius, center_y, radius, marker_color);
        if !self.text.is_empty() {
            let font_size = Self::MARKER_LABEL_FONT_SIZE;
            let text_y = ctx.visual_center_y(frame, font_size);
            ctx.draw_text(
                &self.text,
                crate::core::Point::new(
                    frame.x + Self::MARKER_DIAMETER + Self::MARKER_TEXT_GAP,
                    text_y,
                ),
                ctx.tokens().color_text(),
                font_size,
            );
        }
    }

    fn render_ribbon(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        background: Color,
        foreground: Color,
    ) {
        let slant = (frame.h * 0.22).min(frame.w * 0.2);
        let mut path = PathBuilder::new();
        path.move_to(frame.x + slant, frame.y)
            .line_to(frame.x + frame.w, frame.y)
            .line_to(frame.x + frame.w - slant, frame.y + frame.h)
            .line_to(frame.x, frame.y + frame.h)
            .close();
        ctx.fill_path(&path.build(), background, FillRule::NonZero);

        let font_size = Self::PILL_FONT_SIZE;
        let text_width = ctx.measure_text(&self.text, font_size).w;
        let text_height = ctx.line_box_height(font_size);
        ctx.draw_text(
            &self.text,
            crate::core::Point::new(
                frame.x + (frame.w - text_width) * 0.5,
                frame.y + (frame.h - text_height) * 0.5,
            ),
            foreground,
            font_size,
        );
    }

    fn count_label(&self) -> String {
        format!(
            "{}{}",
            self.count.min(self.max),
            if self.count > self.max { "+" } else { "" }
        )
    }

    fn estimated_text_width(text: &str, font_size: f32) -> f32 {
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            text,
            f32::INFINITY,
            font_size,
        )
        .max_line_width
    }
}

// 把组合布局、交互、协调和几何回归限制在 Badge 模块内部。
#[cfg(test)]
// 测试子模块可以验证私有运行时缓存而不扩大公开 API。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/display/badge/tests.rs"]
mod tests;
