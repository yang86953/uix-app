use super::*;
// 有效圆角解析产出的 draw System 公开圆角值。
use crate::draw::Radius;

impl Style {
    /// 创建一个空样式（所有字段使用默认值）。
    pub fn new() -> Self {
        Self::default()
    }

    // ── 便捷预设构造器 ──────────────────────────────────────

    /// Flex 行方向容器。
    pub fn row() -> Self {
        Self {
            display: DisplayMode::Flex,
            flex_direction: FlexDirection::Row,
            ..Self::default()
        }
    }

    /// Flex 列方向容器。
    pub fn column() -> Self {
        Self {
            display: DisplayMode::Flex,
            flex_direction: FlexDirection::Column,
            ..Self::default()
        }
    }

    /// 默认容器样式。
    pub fn container() -> Self {
        Self {
            display: DisplayMode::Flex,
            flex_direction: FlexDirection::Column,
            ..Self::default()
        }
    }

    // ── State helpers ──────────────────────────────────────

    /// 根据 hover/pressed 状态返回当前背景色（优先返回状态色，fallback 到 background）。
    pub fn effective_bg(&self, hovered: bool, pressed: bool) -> Option<ColorValue> {
        self.effective_bg_for_state(StyleState {
            hovered,
            pressed,
            ..StyleState::default()
        })
    }

    /// 按 active → focus → hover → normal 解析交互背景。
    ///
    /// 某一状态未声明专用颜色时继续尝试较低优先级状态，保证新增 focus
    /// 样式不会遮蔽已有 hover 反馈。
    pub fn effective_bg_for_state(&self, state: StyleState) -> Option<ColorValue> {
        if state.pressed {
            return self.background_active.or(self.background);
        }
        if state.focused {
            return self
                .background_focus
                .or(if state.hovered {
                    self.background_hover
                } else {
                    None
                })
                .or(self.background);
        }
        if state.hovered {
            return self.background_hover.or(self.background);
        }
        self.background
    }

    /// 使用主题令牌解析最终文本颜色。
    pub fn resolve_color(&self, tokens: &dyn ThemeTokens) -> Color {
        self.color.resolve(tokens)
    }

    /// 使用主题令牌解析最终字体大小。
    pub fn resolve_font_size(&self, tokens: &dyn ThemeTokens) -> f32 {
        self.font_size.resolve(tokens)
    }

    // ── 链式 Builder 方法 ──────────────────────────────────

    /// 批量设置完整的 Style。
    pub fn with_style(mut self, s: Self) -> Self {
        self.margin = s.margin;
        self.padding = s.padding;
        self.border_color = s.border_color;
        self.border_width = s.border_width;
        // 完整替换保留显式 solid 与未声明之间的差异。
        self.border_style = s.border_style;
        self.border_radius = s.border_radius;
        self.border_radius_corners = s.border_radius_corners;
        self.width = s.width;
        self.height = s.height;
        // 完整替换保留未声明与显式尺寸约束之间的差异。
        self.min_width = s.min_width;
        self.max_width = s.max_width;
        self.min_height = s.min_height;
        self.max_height = s.max_height;
        self.display = s.display;
        self.flex_direction = s.flex_direction;
        self.flex_wrap = s.flex_wrap;
        self.overflow_content = s.overflow_content;
        // 完整替换保留未声明与显式 visible 之间的差异。
        self.clip_content = s.clip_content;
        self.justify_content = s.justify_content;
        self.align_items = s.align_items;
        self.gap = s.gap;
        self.grid_template_columns = s.grid_template_columns;
        self.grid_template_rows = s.grid_template_rows;
        self.grid_column_gap = s.grid_column_gap;
        self.grid_row_gap = s.grid_row_gap;
        self.flex_grow = s.flex_grow;
        self.flex_shrink = s.flex_shrink;
        self.align_self = s.align_self;
        self.grid_cell = s.grid_cell;
        self.grid_column_span = s.grid_column_span;
        self.grid_row_span = s.grid_row_span;
        self.background = s.background;
        // 完整替换保留未声明与显式 none 之间的差异。
        self.background_image = s.background_image;
        // 完整替换保留未声明与显式左上角之间的差异。
        self.background_position = s.background_position;
        // 完整替换保留未声明与显式 repeat 之间的差异。
        self.background_repeat = s.background_repeat;
        // 完整替换保留未声明 Auto 与显式尺寸策略之间的差异。
        self.background_size = s.background_size;
        self.background_hover = s.background_hover;
        self.background_focus = s.background_focus;
        self.background_active = s.background_active;
        self.color = s.color;
        self.font_size = s.font_size;
        // 完整替换保留未声明字体族与显式列表的差异。
        self.font_family = s.font_family;
        // 完整替换保留未声明字重与显式 normal 的差异。
        self.font_weight = s.font_weight;
        // 完整替换保留未声明行高与显式行高的差异。
        self.line_height = s.line_height;
        // 完整替换保留未声明对齐与显式 left 的差异。
        self.text_align = s.text_align;
        // 完整替换保留未声明装饰与显式 none 的差异。
        self.text_decoration = s.text_decoration;
        self.opacity = s.opacity;
        self.box_shadow = s.box_shadow;
        self.visible = s.visible;
        self
    }

    /// 应用另一个 Style 到自身（非 None 字段覆盖，None 字段保留原值）。
    pub fn apply(mut self, other: Self) -> Self {
        if other.margin != EdgeInsets::zero() {
            self.margin = other.margin;
        }
        if other.padding != EdgeInsets::zero() {
            self.padding = other.padding;
        }
        if other.border_color.is_some() {
            self.border_color = other.border_color;
        }
        if other.border_width != EdgeInsets::zero() {
            self.border_width = other.border_width;
        }
        // 仅显式线型覆盖既有值，None 表示没有声明而不是 CSS none。
        if other.border_style.is_some() {
            // Option 内的 BorderStyle::None 仍会作为显式值复制。
            self.border_style = other.border_style;
        }
        // 圆角按一个属性整体覆盖：任一形式声明都替换两种表示；
        // 显式 0 的恢复语义请使用 StyleDiff（本入口无法区分 0 与未声明）。
        if other.border_radius != 0.0 || other.border_radius_corners.is_some() {
            self.border_radius = other.border_radius;
            self.border_radius_corners = other.border_radius_corners;
        }
        if other.width.is_some() {
            self.width = other.width;
        }
        if other.height.is_some() {
            self.height = other.height;
        }
        // 只有非 Auto 的尺寸约束覆盖既有值；显式恢复 auto 请使用 StyleDiff。
        if !other.min_width.is_auto() {
            self.min_width = other.min_width;
        }
        if !other.max_width.is_auto() {
            self.max_width = other.max_width;
        }
        if !other.min_height.is_auto() {
            self.min_height = other.min_height;
        }
        if !other.max_height.is_auto() {
            self.max_height = other.max_height;
        }
        if other.display != DisplayMode::default() {
            self.display = other.display;
        }
        if other.flex_direction != FlexDirection::default() {
            self.flex_direction = other.flex_direction;
        }
        if other.flex_wrap {
            self.flex_wrap = other.flex_wrap;
        }
        if other.overflow_content {
            self.overflow_content = other.overflow_content;
        }
        // 只有显式 overflow 声明覆盖既有裁剪策略。
        if other.clip_content.is_some() {
            // 复制显式 visible 或 hidden 结果。
            self.clip_content = other.clip_content;
        }
        if other.justify_content != JustifyContent::default() {
            self.justify_content = other.justify_content;
        }
        if other.align_items != AlignItems::default() {
            self.align_items = other.align_items;
        }
        if other.gap != 0.0 {
            self.gap = other.gap;
        }
        if !other.grid_template_columns.is_empty() {
            self.grid_template_columns = other.grid_template_columns;
        }
        if !other.grid_template_rows.is_empty() {
            self.grid_template_rows = other.grid_template_rows;
        }
        if other.grid_column_gap != 0.0 {
            self.grid_column_gap = other.grid_column_gap;
        }
        if other.grid_row_gap != 0.0 {
            self.grid_row_gap = other.grid_row_gap;
        }
        if other.flex_grow != 0.0 {
            self.flex_grow = other.flex_grow;
        }
        if other.flex_shrink != 1.0 {
            self.flex_shrink = other.flex_shrink;
        }
        if other.align_self.is_some() {
            self.align_self = other.align_self;
        }
        if other.grid_cell.is_some() {
            self.grid_cell = other.grid_cell;
        }
        if other.grid_column_span != 1 {
            self.grid_column_span = other.grid_column_span;
        }
        if other.grid_row_span != 1 {
            self.grid_row_span = other.grid_row_span;
        }
        if other.background.is_some() {
            self.background = other.background;
        }
        // 只有显式背景图声明覆盖继承值，包括显式 none。
        if other.background_image.is_some() {
            // 保存已经类型化的单层背景图来源。
            self.background_image = other.background_image;
        }
        // 只有显式背景定位覆盖继承值。
        if other.background_position.is_some() {
            // 保存两个轴的确定定位值。
            self.background_position = other.background_position;
        }
        // 只有显式重复方式覆盖继承值，包括显式 repeat。
        if other.background_repeat.is_some() {
            // 保存闭合重复枚举。
            self.background_repeat = other.background_repeat;
        }
        // 只有显式非 Auto 的尺寸策略覆盖；显式恢复 auto 请使用 StyleDiff。
        if other.background_size != BackgroundSize::Auto {
            self.background_size = other.background_size;
        }
        if other.background_hover.is_some() {
            self.background_hover = other.background_hover;
        }
        if other.background_focus.is_some() {
            self.background_focus = other.background_focus;
        }
        if other.background_active.is_some() {
            self.background_active = other.background_active;
        }
        if other.color != ColorValue::default() {
            self.color = other.color;
        }
        if other.font_size != TypographyToken::default() {
            self.font_size = other.font_size;
        }
        // 只有显式字体族列表覆盖继承值。
        if other.font_family.is_some() {
            // 完整替换列表以保留声明顺序和 CSS 回退语义。
            self.font_family = other.font_family;
        }
        // 只有显式字重覆盖继承值，包括显式 normal。
        if other.font_weight.is_some() {
            // 保存已经验证的精确数值。
            self.font_weight = other.font_weight;
        }
        // 只有显式行高覆盖继承值。
        if other.line_height.is_some() {
            // 行高值已由构造器保证为正有限数值。
            self.line_height = other.line_height;
        }
        // 只有显式对齐覆盖继承值，包括显式 left。
        if other.text_align.is_some() {
            // 保存闭合枚举值而不改变盒模型测量。
            self.text_align = other.text_align;
        }
        // 只有显式装饰覆盖继承值，包括显式 none。
        if other.text_decoration.is_some() {
            // 保存闭合枚举值而不影响布局属性。
            self.text_decoration = other.text_decoration;
        }
        if other.opacity != 1.0 {
            self.opacity = other.opacity;
        }
        if other.box_shadow.is_some() {
            self.box_shadow = other.box_shadow;
        }
        if !other.visible {
            self.visible = other.visible;
        }
        self
    }

    // ── 盒模型链式方法 ──

    /// 设置四边外边距。
    pub fn with_margin(mut self, m: EdgeInsets) -> Self {
        self.margin = m;
        self
    }
    /// 设置四边内边距。
    pub fn with_padding(mut self, p: EdgeInsets) -> Self {
        self.padding = p;
        self
    }
    /// 设置统一宽度和颜色的边框。
    pub fn with_border(mut self, color: impl Into<ColorValue>, width: f32) -> Self {
        self.border_color = Some(color.into());
        self.border_width = EdgeInsets::uniform(width);
        self
    }
    /// 分别设置四边边框宽度。
    pub fn with_border_width(mut self, width: impl Into<EdgeInsets>) -> Self {
        self.border_width = width.into();
        self
    }
    /// 设置显式边框线型。
    pub fn with_border_style(mut self, border_style: BorderStyle) -> Self {
        // Some 保留显式 solid 覆盖继承值的语义。
        self.border_style = Some(border_style);
        // 返回可继续链式设置的样式。
        self
    }
    /// 设置单值圆角半径；显式覆盖任何既有四角声明。
    pub fn with_rounded(mut self, r: f32) -> Self {
        self.border_radius = r;
        self.border_radius_corners = None;
        self
    }
    /// 设置四角圆角半径；覆盖任何既有单值或四角声明。
    pub fn with_border_radius_corners(mut self, corners: CornerRadii) -> Self {
        self.border_radius_corners = Some(corners);
        self
    }
    /// 解析最终有效圆角；直角（含收敛后的非法值）返回 `None`。
    ///
    /// 这是单值与四角两种输入的唯一消费入口；归一化（相邻半径之和超出
    /// 边长时按比例缩小）由 draw System 在绘制前统一执行，不在本层重复。
    pub fn effective_border_radius(&self) -> Option<Radius> {
        if let Some(corners) = self.border_radius_corners {
            let radius = corners.to_radius();
            return (radius.tl > 0.0 || radius.tr > 0.0 || radius.br > 0.0 || radius.bl > 0.0)
                .then_some(radius);
        }
        (self.border_radius > 0.0 && self.border_radius.is_finite())
            .then(|| Radius::uniform(self.border_radius))
    }

    // ── 尺寸链式方法 ──

    /// 设置显式宽度。
    pub fn with_width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    /// 设置显式高度。
    pub fn with_height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }
    /// 同时设置显式宽度和高度。
    pub fn with_size(mut self, w: f32, h: f32) -> Self {
        self.width = Some(w);
        self.height = Some(h);
        self
    }
    /// 设置最小宽度；接受 px 数值或 [`StyleLength`]。
    pub fn with_min_width(mut self, length: impl Into<StyleLength>) -> Self {
        self.min_width = length.into().expect_size_bound("min_width");
        self
    }
    /// 设置最大宽度；接受 px 数值或 [`StyleLength`]。
    pub fn with_max_width(mut self, length: impl Into<StyleLength>) -> Self {
        self.max_width = length.into().expect_size_bound("max_width");
        self
    }
    /// 设置最小高度；接受 px 数值或 [`StyleLength`]。
    pub fn with_min_height(mut self, length: impl Into<StyleLength>) -> Self {
        self.min_height = length.into().expect_size_bound("min_height");
        self
    }
    /// 设置最大高度；接受 px 数值或 [`StyleLength`]。
    pub fn with_max_height(mut self, length: impl Into<StyleLength>) -> Self {
        self.max_height = length.into().expect_size_bound("max_height");
        self
    }
    /// 返回四个方向的尺寸约束声明；直接写入字段的非法值（负单值 px/%、非有限
    /// 分量）在此与便捷入口同样被拒绝。
    pub fn size_constraints(&self) -> SizeConstraints {
        SizeConstraints {
            min_width: self.min_width,
            max_width: self.max_width,
            min_height: self.min_height,
            max_height: self.max_height,
        }
        .expect_legal()
    }

    // ── 布局链式方法 ──

    /// 设置显示与布局模式。
    pub fn with_display(mut self, d: DisplayMode) -> Self {
        self.display = d;
        self
    }
    /// 设置 Flex 主轴方向。
    pub fn with_direction(mut self, d: FlexDirection) -> Self {
        self.flex_direction = d;
        self
    }
    /// 设置 Flex 子项是否换行。
    pub fn with_wrap(mut self, w: bool) -> Self {
        self.flex_wrap = w;
        self
    }
    /// 设置 Flex 主轴对齐方式。
    pub fn with_justify(mut self, j: JustifyContent) -> Self {
        self.justify_content = j;
        self
    }
    /// 设置 Flex 交叉轴对齐方式。
    pub fn with_align(mut self, a: AlignItems) -> Self {
        self.align_items = a;
        self
    }
    /// 设置子项统一间距。
    pub fn with_gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    /// 设置 Grid 列轨道模板。
    pub fn with_grid_columns(mut self, columns: Vec<GridTrack>) -> Self {
        self.grid_template_columns = columns;
        self
    }
    /// 设置 Grid 行轨道模板。
    pub fn with_grid_rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.grid_template_rows = rows;
        self
    }
    /// 分别设置 Grid 列间距和行间距。
    pub fn with_grid_gap(mut self, col_gap: f32, row_gap: f32) -> Self {
        self.grid_column_gap = col_gap;
        self.grid_row_gap = row_gap;
        self
    }
    /// 设置 Flex 扩展系数。
    pub fn with_grow(mut self, g: f32) -> Self {
        self.flex_grow = g;
        self
    }
    /// 设置 Flex 收缩系数。
    pub fn with_shrink(mut self, s: f32) -> Self {
        self.flex_shrink = s;
        self
    }
    /// 设置当前子项的交叉轴覆盖对齐方式。
    pub fn with_align_self(mut self, a: AlignItems) -> Self {
        self.align_self = Some(a);
        self
    }
    /// 设置兼容的一维 Grid 单元索引。
    pub fn with_grid_cell(mut self, cell: usize) -> Self {
        self.grid_cell = Some(cell);
        self
    }
    /// 设置至少为一的 Grid 跨列数。
    pub fn with_grid_column_span(mut self, span: u32) -> Self {
        self.grid_column_span = span.max(1);
        self
    }
    /// 设置至少为一的 Grid 跨行数。
    pub fn with_grid_row_span(mut self, span: u32) -> Self {
        self.grid_row_span = span.max(1);
        self
    }
    /// 同时设置至少为一的 Grid 跨列数和跨行数。
    pub fn with_grid_span(mut self, columns: u32, rows: u32) -> Self {
        self.grid_column_span = columns.max(1);
        self.grid_row_span = rows.max(1);
        self
    }

    // ── 视觉链式方法 ──

    /// 设置常态背景色。
    pub fn with_bg(mut self, c: impl Into<ColorValue>) -> Self {
        self.background = Some(c.into());
        self
    }
    /// 设置显式背景图来源。
    pub fn with_background_image(mut self, image: BackgroundImage) -> Self {
        // Some 保留显式 none 与未声明之间的差异。
        self.background_image = Some(image);
        // 返回可继续链式设置的样式。
        self
    }
    /// 设置显式背景定位。
    pub fn with_background_position(mut self, position: BackgroundPosition) -> Self {
        // 保存两个轴的类型化定位值。
        self.background_position = Some(position);
        // 返回可继续链式设置的样式。
        self
    }
    /// 返回最终背景定位；未声明时使用左上角。
    pub fn effective_background_position(&self) -> BackgroundPosition {
        // 公共 Style 层只提供稳定默认值。
        self.background_position.unwrap_or_default()
    }
    /// 设置显式背景重复方式。
    pub fn with_background_repeat(mut self, repeat: BackgroundRepeat) -> Self {
        // Some 保留显式 repeat 与未声明之间的差异。
        self.background_repeat = Some(repeat);
        // 返回可继续链式设置的样式。
        self
    }
    /// 返回最终背景重复方式；未声明时沿两个轴重复。
    pub fn effective_background_repeat(&self) -> BackgroundRepeat {
        // 公共 Style 层只提供稳定默认值。
        self.background_repeat.unwrap_or_default()
    }
    /// 设置显式背景图尺寸策略。
    pub fn with_background_size(mut self, size: BackgroundSize) -> Self {
        // Auto 是显式恢复值，与非声明共用同一默认。
        self.background_size = size;
        self
    }
    /// 返回最终背景图尺寸策略；未声明时为固有尺寸。
    pub fn effective_background_size(&self) -> BackgroundSize {
        // 公共 Style 层只提供稳定默认值。
        self.background_size
    }
    /// 设置悬停态背景色。
    pub fn with_bg_hover(mut self, c: impl Into<ColorValue>) -> Self {
        self.background_hover = Some(c.into());
        self
    }
    /// 设置聚焦态背景色。
    pub fn with_bg_focus(mut self, c: impl Into<ColorValue>) -> Self {
        self.background_focus = Some(c.into());
        self
    }
    /// 设置激活态背景色。
    pub fn with_bg_active(mut self, c: impl Into<ColorValue>) -> Self {
        self.background_active = Some(c.into());
        self
    }
    /// 设置文本颜色。
    pub fn with_color(mut self, c: impl Into<ColorValue>) -> Self {
        self.color = c.into();
        self
    }
    /// 设置字体大小令牌。
    pub fn with_font_size(mut self, s: impl Into<TypographyToken>) -> Self {
        self.font_size = s.into();
        self
    }
    /// 设置显式字体族回退列表。
    pub fn with_font_family(mut self, font_family: FontFamily) -> Self {
        // Some 区分显式列表与未声明时的当前系统字体。
        self.font_family = Some(font_family);
        // 返回可继续链式设置的样式。
        self
    }
    /// 设置显式字体粗细。
    pub fn with_font_weight(mut self, font_weight: FontWeight) -> Self {
        // Some 保留显式 normal 与未声明之间的差异。
        self.font_weight = Some(font_weight);
        // 返回可继续链式设置的样式。
        self
    }
    /// 返回最终字体粗细；未声明时使用文档默认 normal。
    pub fn effective_font_weight(&self) -> FontWeight {
        // 公共 Style 层只提供稳定默认值，不替代组件兼容语义。
        self.font_weight.unwrap_or_default()
    }
    /// 设置显式行高。
    pub fn with_line_height(mut self, line_height: LineHeight) -> Self {
        // 保存已经验证的倍率或像素值。
        self.line_height = Some(line_height);
        // 返回可继续链式设置的样式。
        self
    }
    /// 按当前字体尺寸解析显式行高；未声明时返回空值。
    pub fn resolve_line_height(&self, font_size: f32) -> Option<f32> {
        // 只解析显式值，让文本组件保留各自既有 normal 策略。
        self.line_height.map(|value| value.resolve(font_size))
    }
    /// 设置显式文本水平对齐。
    pub fn with_text_align(mut self, alignment: TextAlign) -> Self {
        // Some 保留显式 left 与未声明之间的差异。
        self.text_align = Some(alignment);
        // 返回可继续链式设置的样式。
        self
    }
    /// 返回最终文本水平对齐；未声明时使用默认 left。
    pub fn effective_text_align(&self) -> TextAlign {
        // 公共 Style 层只负责稳定默认值。
        self.text_align.unwrap_or_default()
    }
    /// 设置显式文本装饰。
    pub fn with_text_decoration(mut self, decoration: TextDecoration) -> Self {
        // Some 保留显式 none 与未声明之间的差异。
        self.text_decoration = Some(decoration);
        // 返回可继续链式设置的样式。
        self
    }
    /// 返回最终文本装饰；未声明时使用 CSS 默认 none。
    pub fn effective_text_decoration(&self) -> TextDecoration {
        // 只在公共 Style 层提供默认值，不替代组件兼容策略。
        self.text_decoration.unwrap_or_default()
    }
    /// 设置全局透明度。
    pub fn with_opacity(mut self, o: f32) -> Self {
        self.opacity = o;
        self
    }
    /// 设置盒阴影。
    pub fn with_shadow(mut self, shadow: BoxShadowDef) -> Self {
        self.box_shadow = Some(shadow);
        self
    }
    /// 设置样式可见性。
    pub fn with_visible(mut self, v: bool) -> Self {
        self.visible = v;
        self
    }

    /// 当前样式是否需要绘制边框。
    pub fn has_border(&self) -> bool {
        self.border_color.is_some()
            // CSS none 只关闭绘制，不改变边框宽度。
            && self.effective_border_style() != BorderStyle::None
            && (self.border_width.left > 0.0
                || self.border_width.top > 0.0
                || self.border_width.right > 0.0
                || self.border_width.bottom > 0.0)
    }

    /// 返回未显式声明时采用 CSS 默认 solid 的有效线型。
    pub fn effective_border_style(&self) -> BorderStyle {
        // Option 只表达声明存在性，公开有效值始终确定。
        self.border_style.unwrap_or(BorderStyle::Solid)
    }

    /// 返回当前 Canvas 描边使用的单一宽度，取四边最大值。
    pub fn stroke_width(&self) -> f32 {
        self.border_width
            .left
            .max(self.border_width.top)
            .max(self.border_width.right)
            .max(self.border_width.bottom)
    }
}

// ════════════════════════════════════════════════════════════════════════════
