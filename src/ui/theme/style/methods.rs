use super::*;

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

    /// 默认按钮样式（白色背景 + 灰色边框）。
    pub fn button_default() -> Self {
        Self {
            background: None,
            border_color: Some(ColorValue::Neutral(NeutralRole::Border)),
            border_width: EdgeInsets::uniform(1.0),
            border_radius: 6.0,
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: ColorValue::Neutral(NeutralRole::Text),
            font_size: TypographyToken::Body,
            ..Self::default()
        }
    }

    /// 主按钮样式（主题色背景 + 白色文字）。
    pub fn button_primary() -> Self {
        Self {
            background: Some(ColorValue::Palette(PaletteColor::Primary)),
            border_color: Some(ColorValue::Palette(PaletteColor::Primary)),
            border_width: EdgeInsets::uniform(1.0),
            border_radius: 6.0,
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: ColorValue::Palette(PaletteColor::White),
            font_size: TypographyToken::Body,
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

    pub fn resolve_color(&self, tokens: &dyn ThemeTokens) -> Color {
        self.color.resolve(tokens)
    }

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
        self.border_radius = s.border_radius;
        self.width = s.width;
        self.height = s.height;
        self.display = s.display;
        self.flex_direction = s.flex_direction;
        self.flex_wrap = s.flex_wrap;
        self.overflow_content = s.overflow_content;
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
        self.background_hover = s.background_hover;
        self.background_focus = s.background_focus;
        self.background_active = s.background_active;
        self.color = s.color;
        self.font_size = s.font_size;
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
        if other.border_radius != 0.0 {
            self.border_radius = other.border_radius;
        }
        if other.width.is_some() {
            self.width = other.width;
        }
        if other.height.is_some() {
            self.height = other.height;
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
        if other.background_hover.is_some() {
            self.background_hover = other.background_hover;
        }
        if other.background_focus.is_some() {
            self.background_focus = other.background_focus;
        }
        if other.background_active.is_some() {
            self.background_active = other.background_active;
        }
        if other.color != ColorValue::Neutral(NeutralRole::Text) {
            self.color = other.color;
        }
        if other.font_size != TypographyToken::Body {
            self.font_size = other.font_size;
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

    pub fn with_margin(mut self, m: EdgeInsets) -> Self {
        self.margin = m;
        self
    }
    pub fn with_padding(mut self, p: EdgeInsets) -> Self {
        self.padding = p;
        self
    }
    pub fn with_border(mut self, color: impl Into<ColorValue>, width: f32) -> Self {
        self.border_color = Some(color.into());
        self.border_width = EdgeInsets::uniform(width);
        self
    }
    pub fn with_border_width(mut self, width: impl Into<EdgeInsets>) -> Self {
        self.border_width = width.into();
        self
    }
    pub fn with_rounded(mut self, r: f32) -> Self {
        self.border_radius = r;
        self
    }

    // ── 尺寸链式方法 ──

    pub fn with_width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn with_height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }
    pub fn with_size(mut self, w: f32, h: f32) -> Self {
        self.width = Some(w);
        self.height = Some(h);
        self
    }

    // ── 布局链式方法 ──

    pub fn with_display(mut self, d: DisplayMode) -> Self {
        self.display = d;
        self
    }
    pub fn with_direction(mut self, d: FlexDirection) -> Self {
        self.flex_direction = d;
        self
    }
    pub fn with_wrap(mut self, w: bool) -> Self {
        self.flex_wrap = w;
        self
    }
    pub fn with_justify(mut self, j: JustifyContent) -> Self {
        self.justify_content = j;
        self
    }
    pub fn with_align(mut self, a: AlignItems) -> Self {
        self.align_items = a;
        self
    }
    pub fn with_gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    pub fn with_grid_columns(mut self, columns: Vec<GridTrack>) -> Self {
        self.grid_template_columns = columns;
        self
    }
    pub fn with_grid_rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.grid_template_rows = rows;
        self
    }
    pub fn with_grid_gap(mut self, col_gap: f32, row_gap: f32) -> Self {
        self.grid_column_gap = col_gap;
        self.grid_row_gap = row_gap;
        self
    }
    pub fn with_grow(mut self, g: f32) -> Self {
        self.flex_grow = g;
        self
    }
    pub fn with_shrink(mut self, s: f32) -> Self {
        self.flex_shrink = s;
        self
    }
    pub fn with_align_self(mut self, a: AlignItems) -> Self {
        self.align_self = Some(a);
        self
    }
    pub fn with_grid_cell(mut self, cell: usize) -> Self {
        self.grid_cell = Some(cell);
        self
    }
    pub fn with_grid_column_span(mut self, span: u32) -> Self {
        self.grid_column_span = span.max(1);
        self
    }
    pub fn with_grid_row_span(mut self, span: u32) -> Self {
        self.grid_row_span = span.max(1);
        self
    }
    pub fn with_grid_span(mut self, columns: u32, rows: u32) -> Self {
        self.grid_column_span = columns.max(1);
        self.grid_row_span = rows.max(1);
        self
    }

    // ── 视觉链式方法 ──

    pub fn with_bg(mut self, c: impl Into<ColorValue>) -> Self {
        self.background = Some(c.into());
        self
    }
    pub fn with_bg_hover(mut self, c: impl Into<ColorValue>) -> Self {
        self.background_hover = Some(c.into());
        self
    }
    pub fn with_bg_focus(mut self, c: impl Into<ColorValue>) -> Self {
        self.background_focus = Some(c.into());
        self
    }
    pub fn with_bg_active(mut self, c: impl Into<ColorValue>) -> Self {
        self.background_active = Some(c.into());
        self
    }
    pub fn with_color(mut self, c: impl Into<ColorValue>) -> Self {
        self.color = c.into();
        self
    }
    pub fn with_font_size(mut self, s: impl Into<TypographyToken>) -> Self {
        self.font_size = s.into();
        self
    }
    pub fn with_opacity(mut self, o: f32) -> Self {
        self.opacity = o;
        self
    }
    pub fn with_shadow(mut self, shadow: BoxShadowDef) -> Self {
        self.box_shadow = Some(shadow);
        self
    }
    pub fn with_visible(mut self, v: bool) -> Self {
        self.visible = v;
        self
    }

    /// 当前样式是否需要绘制边框。
    pub fn has_border(&self) -> bool {
        self.border_color.is_some()
            && (self.border_width.left > 0.0
                || self.border_width.top > 0.0
                || self.border_width.right > 0.0
                || self.border_width.bottom > 0.0)
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
