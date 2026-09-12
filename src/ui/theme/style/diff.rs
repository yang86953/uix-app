//! StyleDiff — 逐字段类型化的样式差异声明。
//!
//! `Style` 的字段值本身无法区分「未声明」与「显式声明了默认值」
//! （padding=0、opacity=1 等），因此状态层与覆盖层使用 `StyleDiff`：
//! 外层 `None` 表示该字段未声明、保留低层结果；外层 `Some` 表示
//! 显式声明，内层值（包括零、false、none、auto、默认枚举）一律覆盖。

use super::*;

/// 逐字段差异样式；仅记录显式声明的字段。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleDiff {
    /// 显式外边距；零值也可作为恢复声明。
    pub margin: Option<EdgeInsets>,
    /// 显式内边距；零值也可作为恢复声明。
    pub padding: Option<EdgeInsets>,
    /// 显式边框颜色；`Some(None)` 表示显式无边框色。
    pub border_color: Option<Option<ColorValue>>,
    /// 显式四边边框宽度；零值也可作为恢复声明。
    pub border_width: Option<EdgeInsets>,
    /// 显式边框线型；`Some(None)` 表示显式回到未声明线型。
    pub border_style: Option<Option<BorderStyle>>,
    /// 显式圆角单值；零值也可作为恢复声明，并使四角声明失效。
    pub border_radius: Option<f32>,
    /// 显式四角圆角；`Some(None)` 表示显式恢复单值形式（清角）。
    ///
    /// 与单值是同一属性的不叠加输入：本层四角声明优先；本层单值显式
    /// （含 0）时以 `Some(None)` 清除低层四角。
    pub border_radius_corners: Option<Option<CornerRadii>>,
    /// 显式宽度；`Some(None)` 表示显式 auto。
    pub width: Option<Option<f32>>,
    /// 显式高度；`Some(None)` 表示显式 auto。
    pub height: Option<Option<f32>>,
    /// 显式最小宽度；`Some(Auto)` 表示显式恢复不约束。
    pub min_width: Option<StyleLength>,
    /// 显式最大宽度；`Some(Auto)` 表示显式恢复不约束。
    pub max_width: Option<StyleLength>,
    /// 显式最小高度；`Some(Auto)` 表示显式恢复不约束。
    pub min_height: Option<StyleLength>,
    /// 显式最大高度；`Some(Auto)` 表示显式恢复不约束。
    pub max_height: Option<StyleLength>,
    /// 显式显示模式；默认枚举值也可作为恢复声明。
    pub display: Option<DisplayMode>,
    /// 显式 Flex 主轴方向。
    pub flex_direction: Option<FlexDirection>,
    /// 显式是否换行。
    pub flex_wrap: Option<bool>,
    /// 显式是否允许子内容溢出容器。
    pub overflow_content: Option<bool>,
    /// 显式子树裁剪；false 也可作为恢复声明。
    pub clip_content: Option<Option<bool>>,
    /// 显式主轴对齐。
    pub justify_content: Option<JustifyContent>,
    /// 显式交叉轴对齐。
    pub align_items: Option<AlignItems>,
    /// 显式子项间距；零值也可作为恢复声明。
    pub gap: Option<f32>,
    /// 显式 Grid 列轨道模板；空模板也可作为恢复声明。
    pub grid_template_columns: Option<Vec<GridTrack>>,
    /// 显式 Grid 行轨道模板；空模板也可作为恢复声明。
    pub grid_template_rows: Option<Vec<GridTrack>>,
    /// 显式 Grid 列间距。
    pub grid_column_gap: Option<f32>,
    /// 显式 Grid 行间距。
    pub grid_row_gap: Option<f32>,
    /// 显式 Flex 扩展系数。
    pub flex_grow: Option<f32>,
    /// 显式 Flex 收缩系数；1 也可作为恢复声明。
    pub flex_shrink: Option<f32>,
    /// 显式子项交叉轴覆盖；`Some(None)` 表示显式取消覆盖。
    pub align_self: Option<Option<AlignItems>>,
    /// 显式 Grid 单元；`Some(None)` 表示显式回到自动放置。
    pub grid_cell: Option<Option<usize>>,
    /// 显式 Grid 跨列数。
    pub grid_column_span: Option<u32>,
    /// 显式 Grid 跨行数。
    pub grid_row_span: Option<u32>,
    /// 显式背景色；`Some(None)` 表示显式透明。
    pub background: Option<Option<ColorValue>>,
    /// 显式背景图来源；`Some(None)` 表示显式无背景图。
    pub background_image: Option<Option<BackgroundImage>>,
    /// 显式背景定位；`Some(None)` 表示显式回到未声明定位。
    pub background_position: Option<Option<BackgroundPosition>>,
    /// 显式背景重复；`Some(None)` 表示显式回到未声明重复。
    pub background_repeat: Option<Option<BackgroundRepeat>>,
    /// 显式背景图尺寸；`Some(Auto)` 表示显式恢复固有尺寸。
    pub background_size: Option<BackgroundSize>,
    /// 显式悬停背景色；`Some(None)` 表示显式取消悬停背景。
    pub background_hover: Option<Option<ColorValue>>,
    /// 显式焦点背景色；`Some(None)` 表示显式取消焦点背景。
    pub background_focus: Option<Option<ColorValue>>,
    /// 显式激活背景色；`Some(None)` 表示显式取消激活背景。
    pub background_active: Option<Option<ColorValue>>,
    /// 显式文字颜色；默认 token 也可作为恢复声明。
    pub color: Option<ColorValue>,
    /// 显式字号 token。
    pub font_size: Option<TypographyToken>,
    /// 显式字体族；`Some(None)` 表示显式回到系统字体。
    pub font_family: Option<Option<FontFamily>>,
    /// 显式字重；`Some(None)` 表示显式回到未声明字重。
    pub font_weight: Option<Option<FontWeight>>,
    /// 显式行高；`Some(None)` 表示显式回到 normal 行高。
    pub line_height: Option<Option<LineHeight>>,
    /// 显式文本对齐；`Some(None)` 表示显式回到未声明对齐。
    pub text_align: Option<Option<TextAlign>>,
    /// 显式文本装饰；`Some(None)` 表示显式回到未声明装饰。
    pub text_decoration: Option<Option<TextDecoration>>,
    /// 显式整体透明度；1 也可作为恢复声明。
    pub opacity: Option<f32>,
    /// 显式盒阴影；`Some(None)` 表示显式清除阴影。
    pub box_shadow: Option<Option<BoxShadowDef>>,
    /// 显式可见性；true 也可作为恢复声明。
    pub visible: Option<bool>,
}

impl StyleDiff {
    /// 创建一个没有任何声明的差异样式。
    pub fn new() -> Self {
        Self::default()
    }

    /// 是否没有任何显式声明的字段。
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// 把另一份差异的显式声明叠加到本差异上；未声明字段保留本层结果。
    ///
    /// 与 [`Self::apply_to`] 的目标差异表示相同，用于把先后声明的
    /// 两份差异合并成一份（后声明者覆盖同字段）。
    pub fn overlay(&mut self, top: &StyleDiff) {
        overlay_optional_field(&mut self.margin, &top.margin);
        overlay_optional_field(&mut self.padding, &top.padding);
        overlay_optional_field(&mut self.border_color, &top.border_color);
        overlay_optional_field(&mut self.border_width, &top.border_width);
        overlay_optional_field(&mut self.border_style, &top.border_style);
        // 圆角单值与四角是同一属性：本层四角通道显式（声明四角或显式
        // 恢复单值）时整体采纳；否则本层单值显式（含 0）以显式清角标记
        // 覆盖低层四角；两者都未声明时保留低层有效结果。
        if top.border_radius_corners.is_some() {
            self.border_radius = top.border_radius;
            self.border_radius_corners = top.border_radius_corners;
        } else if top.border_radius.is_some() {
            self.border_radius = top.border_radius;
            self.border_radius_corners = Some(None);
        }
        overlay_optional_field(&mut self.width, &top.width);
        overlay_optional_field(&mut self.height, &top.height);
        overlay_optional_field(&mut self.min_width, &top.min_width);
        overlay_optional_field(&mut self.max_width, &top.max_width);
        overlay_optional_field(&mut self.min_height, &top.min_height);
        overlay_optional_field(&mut self.max_height, &top.max_height);
        overlay_optional_field(&mut self.display, &top.display);
        overlay_optional_field(&mut self.flex_direction, &top.flex_direction);
        overlay_optional_field(&mut self.flex_wrap, &top.flex_wrap);
        overlay_optional_field(&mut self.overflow_content, &top.overflow_content);
        overlay_optional_field(&mut self.clip_content, &top.clip_content);
        overlay_optional_field(&mut self.justify_content, &top.justify_content);
        overlay_optional_field(&mut self.align_items, &top.align_items);
        overlay_optional_field(&mut self.gap, &top.gap);
        overlay_optional_field(&mut self.grid_template_columns, &top.grid_template_columns);
        overlay_optional_field(&mut self.grid_template_rows, &top.grid_template_rows);
        overlay_optional_field(&mut self.grid_column_gap, &top.grid_column_gap);
        overlay_optional_field(&mut self.grid_row_gap, &top.grid_row_gap);
        overlay_optional_field(&mut self.flex_grow, &top.flex_grow);
        overlay_optional_field(&mut self.flex_shrink, &top.flex_shrink);
        overlay_optional_field(&mut self.align_self, &top.align_self);
        overlay_optional_field(&mut self.grid_cell, &top.grid_cell);
        overlay_optional_field(&mut self.grid_column_span, &top.grid_column_span);
        overlay_optional_field(&mut self.grid_row_span, &top.grid_row_span);
        overlay_optional_field(&mut self.background, &top.background);
        overlay_optional_field(&mut self.background_image, &top.background_image);
        overlay_optional_field(&mut self.background_position, &top.background_position);
        overlay_optional_field(&mut self.background_repeat, &top.background_repeat);
        overlay_optional_field(&mut self.background_size, &top.background_size);
        overlay_optional_field(&mut self.background_hover, &top.background_hover);
        overlay_optional_field(&mut self.background_focus, &top.background_focus);
        overlay_optional_field(&mut self.background_active, &top.background_active);
        overlay_optional_field(&mut self.color, &top.color);
        overlay_optional_field(&mut self.font_size, &top.font_size);
        overlay_optional_field(&mut self.font_family, &top.font_family);
        overlay_optional_field(&mut self.font_weight, &top.font_weight);
        overlay_optional_field(&mut self.line_height, &top.line_height);
        overlay_optional_field(&mut self.text_align, &top.text_align);
        overlay_optional_field(&mut self.text_decoration, &top.text_decoration);
        overlay_optional_field(&mut self.opacity, &top.opacity);
        overlay_optional_field(&mut self.box_shadow, &top.box_shadow);
        overlay_optional_field(&mut self.visible, &top.visible);
    }

    /// 记录 `after` 相对 `before` 实际变化的字段。
    ///
    /// 用于 [`crate::ui::view::ViewNode`] 的受控样式更新闭包：闭包
    /// 直接改写完整 [`Style`]，只有被闭包真正写过的字段才进入声明，
    /// 显式写回相同值不视为新声明。
    pub(crate) fn changes_between(before: &Style, after: &Style) -> Self {
        Self {
            margin: changed_field(&before.margin, &after.margin),
            padding: changed_field(&before.padding, &after.padding),
            border_color: changed_field(&before.border_color, &after.border_color),
            border_width: changed_field(&before.border_width, &after.border_width),
            border_style: changed_field(&before.border_style, &after.border_style),
            border_radius: changed_field(&before.border_radius, &after.border_radius),
            // Some(corners)→None 的清除必须保留为显式声明 Some(None)，
            // 否则 map_style 的恢复路径会丢掉"回到单值形式"的信息。
            border_radius_corners: changed_field(
                &before.border_radius_corners,
                &after.border_radius_corners,
            ),
            width: changed_field(&before.width, &after.width),
            height: changed_field(&before.height, &after.height),
            min_width: changed_field(&before.min_width, &after.min_width),
            max_width: changed_field(&before.max_width, &after.max_width),
            min_height: changed_field(&before.min_height, &after.min_height),
            max_height: changed_field(&before.max_height, &after.max_height),
            display: changed_field(&before.display, &after.display),
            flex_direction: changed_field(&before.flex_direction, &after.flex_direction),
            flex_wrap: changed_field(&before.flex_wrap, &after.flex_wrap),
            overflow_content: changed_field(&before.overflow_content, &after.overflow_content),
            clip_content: changed_field(&before.clip_content, &after.clip_content),
            justify_content: changed_field(&before.justify_content, &after.justify_content),
            align_items: changed_field(&before.align_items, &after.align_items),
            gap: changed_field(&before.gap, &after.gap),
            grid_template_columns: changed_field(
                &before.grid_template_columns,
                &after.grid_template_columns,
            ),
            grid_template_rows: changed_field(
                &before.grid_template_rows,
                &after.grid_template_rows,
            ),
            grid_column_gap: changed_field(&before.grid_column_gap, &after.grid_column_gap),
            grid_row_gap: changed_field(&before.grid_row_gap, &after.grid_row_gap),
            flex_grow: changed_field(&before.flex_grow, &after.flex_grow),
            flex_shrink: changed_field(&before.flex_shrink, &after.flex_shrink),
            align_self: changed_field(&before.align_self, &after.align_self),
            grid_cell: changed_field(&before.grid_cell, &after.grid_cell),
            grid_column_span: changed_field(&before.grid_column_span, &after.grid_column_span),
            grid_row_span: changed_field(&before.grid_row_span, &after.grid_row_span),
            background: changed_field(&before.background, &after.background),
            background_image: changed_field(&before.background_image, &after.background_image),
            background_position: changed_field(
                &before.background_position,
                &after.background_position,
            ),
            background_repeat: changed_field(&before.background_repeat, &after.background_repeat),
            background_size: changed_field(&before.background_size, &after.background_size),
            background_hover: changed_field(&before.background_hover, &after.background_hover),
            background_focus: changed_field(&before.background_focus, &after.background_focus),
            background_active: changed_field(&before.background_active, &after.background_active),
            color: changed_field(&before.color, &after.color),
            font_size: changed_field(&before.font_size, &after.font_size),
            font_family: changed_field(&before.font_family, &after.font_family),
            font_weight: changed_field(&before.font_weight, &after.font_weight),
            line_height: changed_field(&before.line_height, &after.line_height),
            text_align: changed_field(&before.text_align, &after.text_align),
            text_decoration: changed_field(&before.text_decoration, &after.text_decoration),
            opacity: changed_field(&before.opacity, &after.opacity),
            box_shadow: changed_field(&before.box_shadow, &after.box_shadow),
            visible: changed_field(&before.visible, &after.visible),
        }
    }

    /// 把显式声明逐字段叠加到 `style` 上；未声明字段保留原值。
    pub fn apply_to(self, style: &mut Style) {
        let diff = self;
        if let Some(v) = diff.margin {
            style.margin = v;
        }
        if let Some(v) = diff.padding {
            style.padding = v;
        }
        if let Some(v) = diff.border_color {
            style.border_color = v;
        }
        if let Some(v) = diff.border_width {
            style.border_width = v;
        }
        if let Some(v) = diff.border_style {
            style.border_style = v;
        }
        // 单值声明（含显式 0）先落地并清除四角形式；随后四角通道声明
        // （Some(Some(c)) 设四角，Some(None) 显式清角）最终生效，保持
        // “同一属性、后写的输入形式生效”的声明语义。
        if let Some(v) = diff.border_radius {
            style.border_radius = v;
            style.border_radius_corners = None;
        }
        if let Some(v) = diff.border_radius_corners {
            style.border_radius_corners = v;
        }
        if let Some(v) = diff.width {
            style.width = v;
        }
        if let Some(v) = diff.height {
            style.height = v;
        }
        if let Some(v) = diff.min_width {
            style.min_width = v;
        }
        if let Some(v) = diff.max_width {
            style.max_width = v;
        }
        if let Some(v) = diff.min_height {
            style.min_height = v;
        }
        if let Some(v) = diff.max_height {
            style.max_height = v;
        }
        if let Some(v) = diff.display {
            style.display = v;
        }
        if let Some(v) = diff.flex_direction {
            style.flex_direction = v;
        }
        if let Some(v) = diff.flex_wrap {
            style.flex_wrap = v;
        }
        if let Some(v) = diff.overflow_content {
            style.overflow_content = v;
        }
        if let Some(v) = diff.clip_content {
            style.clip_content = v;
        }
        if let Some(v) = diff.justify_content {
            style.justify_content = v;
        }
        if let Some(v) = diff.align_items {
            style.align_items = v;
        }
        if let Some(v) = diff.gap {
            style.gap = v;
        }
        if let Some(v) = diff.grid_template_columns {
            style.grid_template_columns = v;
        }
        if let Some(v) = diff.grid_template_rows {
            style.grid_template_rows = v;
        }
        if let Some(v) = diff.grid_column_gap {
            style.grid_column_gap = v;
        }
        if let Some(v) = diff.grid_row_gap {
            style.grid_row_gap = v;
        }
        if let Some(v) = diff.flex_grow {
            style.flex_grow = v;
        }
        if let Some(v) = diff.flex_shrink {
            style.flex_shrink = v;
        }
        if let Some(v) = diff.align_self {
            style.align_self = v;
        }
        if let Some(v) = diff.grid_cell {
            style.grid_cell = v;
        }
        if let Some(v) = diff.grid_column_span {
            style.grid_column_span = v;
        }
        if let Some(v) = diff.grid_row_span {
            style.grid_row_span = v;
        }
        if let Some(v) = diff.background {
            style.background = v;
        }
        if let Some(v) = diff.background_image {
            style.background_image = v;
        }
        if let Some(v) = diff.background_position {
            style.background_position = v;
        }
        if let Some(v) = diff.background_repeat {
            style.background_repeat = v;
        }
        if let Some(v) = diff.background_size {
            style.background_size = v;
        }
        if let Some(v) = diff.background_hover {
            style.background_hover = v;
        }
        if let Some(v) = diff.background_focus {
            style.background_focus = v;
        }
        if let Some(v) = diff.background_active {
            style.background_active = v;
        }
        if let Some(v) = diff.color {
            style.color = v;
        }
        if let Some(v) = diff.font_size {
            style.font_size = v;
        }
        if let Some(v) = diff.font_family {
            style.font_family = v;
        }
        if let Some(v) = diff.font_weight {
            style.font_weight = v;
        }
        if let Some(v) = diff.line_height {
            style.line_height = v;
        }
        if let Some(v) = diff.text_align {
            style.text_align = v;
        }
        if let Some(v) = diff.text_decoration {
            style.text_decoration = v;
        }
        if let Some(v) = diff.opacity {
            style.opacity = v;
        }
        if let Some(v) = diff.box_shadow {
            style.box_shadow = v;
        }
        if let Some(v) = diff.visible {
            style.visible = v;
        }
    }

    // ── 盒模型 ──

    /// 声明显式外边距。
    pub fn margin(mut self, m: EdgeInsets) -> Self {
        self.margin = Some(m);
        self
    }
    /// 声明显式内边距；零表示恢复无内边距。
    pub fn padding(mut self, p: EdgeInsets) -> Self {
        self.padding = Some(p);
        self
    }
    /// 声明显式统一边框颜色与宽度。
    pub fn border(mut self, color: impl Into<ColorValue>, width: f32) -> Self {
        self.border_color = Some(Some(color.into()));
        self.border_width = Some(EdgeInsets::uniform(width));
        self
    }
    /// 声明显式边框颜色。
    pub fn border_color(mut self, c: impl Into<ColorValue>) -> Self {
        self.border_color = Some(Some(c.into()));
        self
    }
    /// 声明显式无边框色。
    pub fn border_color_none(mut self) -> Self {
        self.border_color = Some(None);
        self
    }
    /// 声明显式四边边框宽度；零表示恢复无边框宽度。
    pub fn border_width(mut self, w: impl Into<EdgeInsets>) -> Self {
        self.border_width = Some(w.into());
        self
    }
    /// 声明显式边框线型。
    pub fn border_style(mut self, s: BorderStyle) -> Self {
        self.border_style = Some(Some(s));
        self
    }
    /// 声明显式取消线型覆盖。
    pub fn border_style_none(mut self) -> Self {
        self.border_style = Some(None);
        self
    }
    /// 声明显式圆角；零表示恢复直角，并清除同一差量的四角声明。
    pub fn border_radius(mut self, r: f32) -> Self {
        self.border_radius = Some(r);
        self.border_radius_corners = Some(None);
        self
    }
    /// 声明显式四角圆角；覆盖任何低层单值或四角声明。
    pub fn border_radius_corners(mut self, corners: CornerRadii) -> Self {
        self.border_radius_corners = Some(Some(corners));
        self
    }

    // ── 尺寸 ──

    /// 声明显式宽度。
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(Some(w));
        self
    }
    /// 声明显式宽度 auto。
    pub fn width_auto(mut self) -> Self {
        self.width = Some(None);
        self
    }
    /// 声明显式高度。
    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(Some(h));
        self
    }
    /// 声明显式高度 auto。
    pub fn height_auto(mut self) -> Self {
        self.height = Some(None);
        self
    }
    /// 声明显式最小宽度；`StyleLength::Auto` 表示显式恢复不约束。
    pub fn min_width(mut self, length: impl Into<StyleLength>) -> Self {
        self.min_width = Some(length.into().expect_size_bound("min_width"));
        self
    }
    /// 声明显式最大宽度；`StyleLength::Auto` 表示显式恢复不约束。
    pub fn max_width(mut self, length: impl Into<StyleLength>) -> Self {
        self.max_width = Some(length.into().expect_size_bound("max_width"));
        self
    }
    /// 声明显式最小高度；`StyleLength::Auto` 表示显式恢复不约束。
    pub fn min_height(mut self, length: impl Into<StyleLength>) -> Self {
        self.min_height = Some(length.into().expect_size_bound("min_height"));
        self
    }
    /// 声明显式最大高度；`StyleLength::Auto` 表示显式恢复不约束。
    pub fn max_height(mut self, length: impl Into<StyleLength>) -> Self {
        self.max_height = Some(length.into().expect_size_bound("max_height"));
        self
    }

    // ── 布局 ──

    /// 声明显式显示模式。
    pub fn display(mut self, d: DisplayMode) -> Self {
        self.display = Some(d);
        self
    }
    /// 声明显式 Flex 主轴方向。
    pub fn direction(mut self, d: FlexDirection) -> Self {
        self.flex_direction = Some(d);
        self
    }
    /// 声明显式是否换行。
    pub fn wrap(mut self, w: bool) -> Self {
        self.flex_wrap = Some(w);
        self
    }
    /// 声明显式是否允许子内容溢出容器。
    pub fn overflow_content(mut self, v: bool) -> Self {
        self.overflow_content = Some(v);
        self
    }
    /// 声明显式子树裁剪。
    pub fn clip(mut self, v: bool) -> Self {
        self.clip_content = Some(Some(v));
        self
    }
    /// 声明显式取消子树裁剪覆盖。
    pub fn clip_none(mut self) -> Self {
        self.clip_content = Some(None);
        self
    }
    /// 声明显式主轴对齐。
    pub fn justify(mut self, j: JustifyContent) -> Self {
        self.justify_content = Some(j);
        self
    }
    /// 声明显式交叉轴对齐。
    pub fn align(mut self, a: AlignItems) -> Self {
        self.align_items = Some(a);
        self
    }
    /// 声明显式子项间距；零表示恢复无间距。
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = Some(g);
        self
    }
    /// 声明显式 Grid 列轨道模板。
    pub fn grid_columns(mut self, columns: Vec<GridTrack>) -> Self {
        self.grid_template_columns = Some(columns);
        self
    }
    /// 声明显式 Grid 行轨道模板。
    pub fn grid_rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.grid_template_rows = Some(rows);
        self
    }
    /// 声明显式 Grid 列/行间距。
    pub fn grid_gap(mut self, col: f32, row: f32) -> Self {
        self.grid_column_gap = Some(col);
        self.grid_row_gap = Some(row);
        self
    }
    /// 声明显式 Flex 扩展系数。
    pub fn grow(mut self, g: f32) -> Self {
        self.flex_grow = Some(g);
        self
    }
    /// 声明显式 Flex 收缩系数；1 表示恢复默认收缩。
    pub fn shrink(mut self, s: f32) -> Self {
        self.flex_shrink = Some(s);
        self
    }
    /// 声明显式子项交叉轴覆盖。
    pub fn align_self(mut self, a: AlignItems) -> Self {
        self.align_self = Some(Some(a));
        self
    }
    /// 声明显式取消交叉轴覆盖。
    pub fn align_self_none(mut self) -> Self {
        self.align_self = Some(None);
        self
    }
    /// 声明显式 Grid 单元索引。
    pub fn grid_cell(mut self, cell: usize) -> Self {
        self.grid_cell = Some(Some(cell));
        self
    }
    /// 声明显式 Grid 自动放置。
    pub fn grid_cell_auto(mut self) -> Self {
        self.grid_cell = Some(None);
        self
    }
    /// 声明显式 Grid 跨列数。
    pub fn grid_column_span(mut self, span: u32) -> Self {
        self.grid_column_span = Some(span.max(1));
        self
    }
    /// 声明显式 Grid 跨行数。
    pub fn grid_row_span(mut self, span: u32) -> Self {
        self.grid_row_span = Some(span.max(1));
        self
    }

    // ── 视觉 ──

    /// 声明显式背景色。
    pub fn background(mut self, c: impl Into<ColorValue>) -> Self {
        self.background = Some(Some(c.into()));
        self
    }
    /// 声明显式透明背景。
    pub fn background_none(mut self) -> Self {
        self.background = Some(None);
        self
    }
    /// 声明显式背景图来源。
    pub fn background_image(mut self, image: BackgroundImage) -> Self {
        self.background_image = Some(Some(image));
        self
    }
    /// 声明显式清除背景图。
    pub fn background_image_none(mut self) -> Self {
        self.background_image = Some(None);
        self
    }
    /// 声明显式背景定位。
    pub fn background_position(mut self, position: BackgroundPosition) -> Self {
        self.background_position = Some(Some(position));
        self
    }
    /// 声明显式取消背景定位覆盖。
    pub fn background_position_none(mut self) -> Self {
        self.background_position = Some(None);
        self
    }
    /// 声明显式背景重复。
    pub fn background_repeat(mut self, repeat: BackgroundRepeat) -> Self {
        self.background_repeat = Some(Some(repeat));
        self
    }
    /// 声明显式取消背景重复覆盖。
    pub fn background_repeat_none(mut self) -> Self {
        self.background_repeat = Some(None);
        self
    }
    /// 声明显式背景图尺寸；`Auto` 表示显式恢复固有尺寸。
    pub fn background_size(mut self, size: BackgroundSize) -> Self {
        self.background_size = Some(size);
        self
    }
    /// 声明显式悬停背景色。
    pub fn bg_hover(mut self, c: impl Into<ColorValue>) -> Self {
        self.background_hover = Some(Some(c.into()));
        self
    }
    /// 声明显式取消悬停背景色。
    pub fn bg_hover_none(mut self) -> Self {
        self.background_hover = Some(None);
        self
    }
    /// 声明显式焦点背景色。
    pub fn bg_focus(mut self, c: impl Into<ColorValue>) -> Self {
        self.background_focus = Some(Some(c.into()));
        self
    }
    /// 声明显式取消焦点背景色。
    pub fn bg_focus_none(mut self) -> Self {
        self.background_focus = Some(None);
        self
    }
    /// 声明显式激活背景色。
    pub fn bg_active(mut self, c: impl Into<ColorValue>) -> Self {
        self.background_active = Some(Some(c.into()));
        self
    }
    /// 声明显式取消激活背景色。
    pub fn bg_active_none(mut self) -> Self {
        self.background_active = Some(None);
        self
    }
    /// 声明显式文字颜色。
    pub fn color(mut self, c: impl Into<ColorValue>) -> Self {
        self.color = Some(c.into());
        self
    }
    /// 声明显式字号 token。
    pub fn font_size(mut self, s: impl Into<TypographyToken>) -> Self {
        self.font_size = Some(s.into());
        self
    }
    /// 声明显式字体族回退列表。
    pub fn font_family(mut self, f: FontFamily) -> Self {
        self.font_family = Some(Some(f));
        self
    }
    /// 声明显式取消字体族覆盖。
    pub fn font_family_none(mut self) -> Self {
        self.font_family = Some(None);
        self
    }
    /// 声明显式字重。
    pub fn font_weight(mut self, w: FontWeight) -> Self {
        self.font_weight = Some(Some(w));
        self
    }
    /// 声明显式取消字重覆盖。
    pub fn font_weight_none(mut self) -> Self {
        self.font_weight = Some(None);
        self
    }
    /// 声明显式行高。
    pub fn line_height(mut self, l: LineHeight) -> Self {
        self.line_height = Some(Some(l));
        self
    }
    /// 声明显式取消行高覆盖。
    pub fn line_height_none(mut self) -> Self {
        self.line_height = Some(None);
        self
    }
    /// 声明显式文本对齐。
    pub fn text_align(mut self, a: TextAlign) -> Self {
        self.text_align = Some(Some(a));
        self
    }
    /// 声明显式取消对齐覆盖。
    pub fn text_align_none(mut self) -> Self {
        self.text_align = Some(None);
        self
    }
    /// 声明显式文本装饰。
    pub fn text_decoration(mut self, d: TextDecoration) -> Self {
        self.text_decoration = Some(Some(d));
        self
    }
    /// 声明显式取消装饰覆盖。
    pub fn text_decoration_none(mut self) -> Self {
        self.text_decoration = Some(None);
        self
    }
    /// 声明显式整体透明度；1 表示恢复不透明。
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = Some(o);
        self
    }
    /// 声明显式盒阴影。
    pub fn shadow(mut self, s: BoxShadowDef) -> Self {
        self.box_shadow = Some(Some(s));
        self
    }
    /// 声明显式清除盒阴影。
    pub fn shadow_none(mut self) -> Self {
        self.box_shadow = Some(None);
        self
    }
    /// 声明显式可见性；true 表示恢复可见。
    pub fn visible(mut self, v: bool) -> Self {
        self.visible = Some(v);
        self
    }
}

impl From<Style> for StyleDiff {
    /// 把完整样式转换为差异层，字段显式性与旧 [`Style::apply`]
    /// 的值推断一致：只有旧合并路径会复制的字段才进入差异层。
    /// 由此，向状态层传入完整 [`Style`] 的既有公开调用行为不变；
    /// 需要显式恢复零、false、none、auto 的调用应直接声明
    /// [`StyleDiff`] 字段。
    fn from(style: Style) -> Self {
        Self {
            margin: (style.margin != EdgeInsets::zero()).then_some(style.margin),
            padding: (style.padding != EdgeInsets::zero()).then_some(style.padding),
            border_color: style.border_color.map(Some),
            border_width: (style.border_width != EdgeInsets::zero()).then_some(style.border_width),
            border_style: style.border_style.map(Some),
            border_radius: (style.border_radius != 0.0).then_some(style.border_radius),
            border_radius_corners: style.border_radius_corners.map(Some),
            width: style.width.map(Some),
            height: style.height.map(Some),
            min_width: (!style.min_width.is_auto()).then_some(style.min_width),
            max_width: (!style.max_width.is_auto()).then_some(style.max_width),
            min_height: (!style.min_height.is_auto()).then_some(style.min_height),
            max_height: (!style.max_height.is_auto()).then_some(style.max_height),
            display: (style.display != DisplayMode::default()).then_some(style.display),
            flex_direction: (style.flex_direction != FlexDirection::default())
                .then_some(style.flex_direction),
            flex_wrap: style.flex_wrap.then_some(true),
            overflow_content: style.overflow_content.then_some(true),
            clip_content: style.clip_content.map(Some),
            justify_content: (style.justify_content != JustifyContent::default())
                .then_some(style.justify_content),
            align_items: (style.align_items != AlignItems::default()).then_some(style.align_items),
            gap: (style.gap != 0.0).then_some(style.gap),
            grid_template_columns: (!style.grid_template_columns.is_empty())
                .then_some(style.grid_template_columns),
            grid_template_rows: (!style.grid_template_rows.is_empty())
                .then_some(style.grid_template_rows),
            grid_column_gap: (style.grid_column_gap != 0.0).then_some(style.grid_column_gap),
            grid_row_gap: (style.grid_row_gap != 0.0).then_some(style.grid_row_gap),
            flex_grow: (style.flex_grow != 0.0).then_some(style.flex_grow),
            flex_shrink: (style.flex_shrink != 1.0).then_some(style.flex_shrink),
            align_self: style.align_self.map(Some),
            grid_cell: style.grid_cell.map(Some),
            grid_column_span: (style.grid_column_span != 1).then_some(style.grid_column_span),
            grid_row_span: (style.grid_row_span != 1).then_some(style.grid_row_span),
            background: style.background.map(Some),
            background_image: style.background_image.map(Some),
            background_position: style.background_position.map(Some),
            background_repeat: style.background_repeat.map(Some),
            background_size: (style.background_size != BackgroundSize::Auto)
                .then_some(style.background_size),
            background_hover: style.background_hover.map(Some),
            background_focus: style.background_focus.map(Some),
            background_active: style.background_active.map(Some),
            color: (style.color != ColorValue::default()).then_some(style.color),
            font_size: (style.font_size != TypographyToken::default()).then_some(style.font_size),
            font_family: style.font_family.map(Some),
            font_weight: style.font_weight.map(Some),
            line_height: style.line_height.map(Some),
            text_align: style.text_align.map(Some),
            text_decoration: style.text_decoration.map(Some),
            opacity: (style.opacity != 1.0).then_some(style.opacity),
            box_shadow: style.box_shadow.map(Some),
            visible: (!style.visible).then_some(false),
        }
    }
}

// 用上层声明的字段覆盖下层；上层未声明时保留下层结果。
fn overlay_optional_field<T: Clone>(lower: &mut Option<T>, top: &Option<T>) {
    if let Some(value) = top {
        *lower = Some(value.clone());
    }
}

// 记录实际变化的字段；写回相同值不视为新声明。
fn changed_field<T: PartialEq + Clone>(before: &T, after: &T) -> Option<T> {
    (before != after).then(|| after.clone())
}
