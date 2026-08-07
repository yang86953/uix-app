//! 日期输入组件共用的月视图面板。

use crate::core::{Point, Rect};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::widgets::input::date_picker::{days_in_month, first_weekday, Date, DisabledDate};

pub(crate) const CALENDAR_PANEL_HEIGHT: f32 = 250.0;
// 允许日期选择器的表面解析器复用月历自然最小宽度。
pub(crate) const CALENDAR_PANEL_MIN_WIDTH: f32 = 160.0;
const HEADER_HEIGHT: f32 = 32.0;
const WEEKDAY_HEIGHT: f32 = 24.0;
const CELL_HEIGHT: f32 = 30.0;
const HORIZONTAL_INSET: f32 = 8.0;
const NAVIGATION_WIDTH: f32 = 32.0;

// 描述任意实际日期面板上的缩放布局指标。
#[derive(Debug, Clone, Copy)]
struct CalendarPanelGeometry {
    // 保存归一化后的实际面板矩形。
    popup: Rect,
    // 保存缩放后的标题栏高度。
    header_height: f32,
    // 保存缩放后的星期栏高度。
    weekday_height: f32,
    // 保存缩放后的日期单元格高度。
    cell_height: f32,
    // 保存缩放后的水平内边距。
    horizontal_inset: f32,
    // 保存缩放后的月份导航宽度。
    navigation_width: f32,
    // 保存字体与图标共同使用的缩放比例。
    font_scale: f32,
}

// 为任意实际面板计算绘制与命中共享的布局指标。
impl CalendarPanelGeometry {
    // 从最终面板矩形构造布局指标。
    fn new(popup: Rect) -> Self {
        // 收敛负宽度并保留最终横坐标。
        let popup = Rect::new(popup.x, popup.y, popup.w.max(0.0), popup.h.max(0.0));
        // 计算相对自然最小宽度的横向比例。
        let x_scale = (popup.w / CALENDAR_PANEL_MIN_WIDTH).clamp(0.0, 1.0);
        // 计算相对自然高度的纵向比例。
        let y_scale = (popup.h / CALENDAR_PANEL_HEIGHT).clamp(0.0, 1.0);
        // 返回所有消费者共享的缩放指标。
        Self {
            // 保存最终面板矩形。
            popup,
            // 按实际高度缩放标题栏。
            header_height: HEADER_HEIGHT * y_scale,
            // 按实际高度缩放星期栏。
            weekday_height: WEEKDAY_HEIGHT * y_scale,
            // 按实际高度缩放日期单元格。
            cell_height: CELL_HEIGHT * y_scale,
            // 按实际宽度缩放水平内边距。
            horizontal_inset: HORIZONTAL_INSET * x_scale,
            // 按实际宽度缩放月份导航区域。
            navigation_width: NAVIGATION_WIDTH * x_scale,
            // 字体与图标使用较小轴比例避免裁切。
            font_scale: x_scale.min(y_scale),
        }
    }

    // 返回日期网格的缩放后纵向起点。
    fn grid_top(self) -> f32 {
        // 合并标题栏与星期栏高度。
        self.header_height + self.weekday_height
    }

    // 返回七列日期网格的单元格宽度。
    fn cell_width(self) -> f32 {
        // 将有效内容宽度平均分成七列。
        (self.popup.w - self.horizontal_inset * 2.0) / 7.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MonthNavigation {
    Previous,
    Next,
}

pub(crate) struct CalendarPanelState<'a> {
    pub year: i32,
    pub month: usize,
    pub active: Option<Date>,
    pub range: Option<(Date, Date)>,
    pub hover: Option<Date>,
    pub disabled_date: Option<&'a DisabledDate>,
}

impl CalendarPanelState<'_> {
    fn is_disabled(&self, date: Date) -> bool {
        self.disabled_date.is_some_and(|predicate| predicate(date))
    }

    fn is_in_range(&self, date: Date) -> bool {
        self.range
            .is_some_and(|(start, end)| start <= date && date <= end)
    }
}

pub(crate) fn calendar_popup_rect(frame: Rect) -> Rect {
    Rect::new(
        frame.x,
        frame.y + frame.h + 2.0,
        frame.w.max(CALENDAR_PANEL_MIN_WIDTH),
        CALENDAR_PANEL_HEIGHT,
    )
}

pub(crate) fn hit_month_navigation(frame: Rect, position: Point) -> Option<MonthNavigation> {
    // 保留旧调用者的自然尺寸包装接口。
    hit_month_navigation_in_rect(calendar_popup_rect(frame), position)
}

// 在已解析的实际面板矩形内命中月份导航按钮。
pub(crate) fn hit_month_navigation_in_rect(
    // 接收绘制与登记共享的实际面板矩形。
    popup: Rect,
    // 接收与面板同一坐标系的指针位置。
    position: Point,
) -> Option<MonthNavigation> {
    // 构造绘制与命中共享的缩放布局指标。
    let geometry = CalendarPanelGeometry::new(popup);
    // 将指针转换到面板内部坐标。
    let relative = Point::new(position.x - popup.x, position.y - popup.y);
    // 面板外或标题栏外不命中月份导航。
    if !(0.0..popup.w).contains(&relative.x)
        // 同时检查缩放后的标题栏范围。
        || !(0.0..geometry.header_height).contains(&relative.y)
    {
        // 返回未命中。
        return None;
    }
    // 检查左侧导航区域。
    if relative.x < geometry.navigation_width {
        // 命中上一个月。
        Some(MonthNavigation::Previous)
    // 检查右侧导航区域。
    } else if relative.x >= popup.w - geometry.navigation_width {
        // 命中下一个月。
        Some(MonthNavigation::Next)
    // 处理标题中部。
    } else {
        // 标题中部不属于导航按钮。
        None
    }
}

pub(crate) fn hit_calendar_date(
    frame: Rect,
    position: Point,
    year: i32,
    month: usize,
) -> Option<Date> {
    // 保留旧调用者的自然尺寸包装接口。
    hit_calendar_date_in_rect(calendar_popup_rect(frame), position, year, month)
}

// 在已解析的实际面板矩形内命中日期单元格。
pub(crate) fn hit_calendar_date_in_rect(
    // 接收绘制与登记共享的实际面板矩形。
    popup: Rect,
    // 接收与面板同一坐标系的指针位置。
    position: Point,
    // 接收当前视图年份。
    year: i32,
    // 接收当前视图月份。
    month: usize,
) -> Option<Date> {
    // 构造绘制与命中共享的缩放布局指标。
    let geometry = CalendarPanelGeometry::new(popup);
    // 缩为零的网格不可参与命中。
    if geometry.cell_height <= 0.0 {
        // 返回未命中。
        return None;
    }
    // 将指针转换到面板内部坐标。
    let relative = Point::new(position.x - popup.x, position.y - popup.y);
    // 检查缩放后的日期网格边界。
    if relative.y < geometry.grid_top()
        // 检查六行日期的底边。
        || relative.y >= geometry.grid_top() + geometry.cell_height * 6.0
        // 检查左侧缩放内边距。
        || relative.x < geometry.horizontal_inset
        // 检查右侧缩放内边距。
        || relative.x >= popup.w - geometry.horizontal_inset
    {
        // 网格外返回未命中。
        return None;
    }

    // 读取共享指标计算的单元格宽度。
    let cell_width = geometry.cell_width();
    // 无有效列宽时停止命中。
    if cell_width <= 0.0 {
        // 返回未命中。
        return None;
    }
    // 按缩放后的网格指标解析行号。
    let row = ((relative.y - geometry.grid_top()) / geometry.cell_height) as usize;
    // 按缩放后的水平内边距解析列号。
    let column = ((relative.x - geometry.horizontal_inset) / cell_width) as usize;
    if row >= 6 || column >= 7 {
        return None;
    }

    let slot = row * 7 + column;
    let first = first_weekday(year, month);
    if slot < first {
        return None;
    }
    let day = slot - first + 1;
    (day <= days_in_month(year, month)).then(|| Date::new(year, month, day))
}

pub(crate) fn draw_calendar_panel(
    frame: Rect,
    ctx: &mut PaintContext,
    state: CalendarPanelState<'_>,
) {
    // 保留旧调用者的自然尺寸包装接口。
    draw_calendar_panel_in_rect(calendar_popup_rect(frame), ctx, state);
}

// 在已解析的实际矩形内绘制可缩放月历面板。
pub(crate) fn draw_calendar_panel_in_rect(
    // 接收登记、命中和脏区共享的实际面板矩形。
    popup: Rect,
    // 接收绘制上下文。
    ctx: &mut PaintContext,
    // 接收当前月份和日期状态。
    state: CalendarPanelState<'_>,
) {
    // 构造绘制与命中共享的缩放布局指标。
    let geometry = CalendarPanelGeometry::new(popup);
    // 空面板不产生绘制命令。
    if popup.w <= 0.0 || popup.h <= 0.0 {
        // 提前结束空面板绘制。
        return;
    }
    let primary = ctx.tokens().color_primary();
    let primary_bg = ctx.tokens().color_primary_bg();
    let border_color = ctx.tokens().color_border();
    let text_color = ctx.tokens().color_text();
    let text_secondary = ctx.tokens().color_text_secondary();
    let text_tertiary = ctx.tokens().color_text_tertiary();
    let bg_elevated = ctx.tokens().color_bg_elevated();
    let fill_tertiary = ctx.tokens().color_fill_tertiary();
    let radius = Some(crate::draw::Radius::uniform(
        ctx.tokens().border_radius_sm(),
    ));

    ctx.push_clip(popup);
    ctx.fill_rect(popup, bg_elevated, radius);
    ctx.stroke_rect(popup, border_color, 1.0, radius);

    let title = format!("{}年{:02}月", state.year, state.month);
    // 构造缩放后的标题栏矩形。
    let header_rect = Rect::new(popup.x, popup.y, popup.w, geometry.header_height);
    // 计算缩放后的标题字号。
    let title_font_size = 14.0 * geometry.font_scale;
    // 计算标题基线。
    let title_y = ctx.visual_center_y(header_rect, title_font_size);
    // 测量缩放后的标题宽度。
    let title_width = ctx.measure_text(&title, title_font_size).w;
    ctx.draw_text(
        &title,
        Point::new(popup.x + (popup.w - title_width) * 0.5, title_y),
        text_color,
        title_font_size,
    );
    crate::ui::widgets::icon::Icon::paint_in_frame(
        ctx,
        "chevron-left",
        Rect::new(
            popup.x,
            popup.y,
            geometry.navigation_width,
            geometry.header_height,
        ),
        text_secondary,
        12.0 * geometry.font_scale,
    );
    crate::ui::widgets::icon::Icon::paint_in_frame(
        ctx,
        "chevron-right",
        Rect::new(
            popup.x + popup.w - geometry.navigation_width,
            popup.y,
            geometry.navigation_width,
            geometry.header_height,
        ),
        text_secondary,
        12.0 * geometry.font_scale,
    );

    // 读取共享指标计算的日期单元格宽度。
    let cell_width = geometry.cell_width();
    // 构造缩放后的星期栏矩形。
    let weekday_row = Rect::new(
        popup.x + geometry.horizontal_inset,
        popup.y + geometry.header_height,
        popup.w - geometry.horizontal_inset * 2.0,
        geometry.weekday_height,
    );
    // 计算缩放后的星期字号。
    let weekday_font_size = 10.0 * geometry.font_scale;
    // 计算星期文本基线。
    let weekday_text_y = ctx.visual_center_y(weekday_row, weekday_font_size);
    for (index, weekday) in crate::ui::component::locale::use_locale()
        .weekdays_short
        .iter()
        .enumerate()
    {
        // 按共享内边距和列宽计算星期横坐标。
        let x = popup.x + geometry.horizontal_inset + index as f32 * cell_width;
        // 测量缩放后的星期文本宽度。
        let text_width = ctx.measure_text(weekday, weekday_font_size).w;
        ctx.draw_text(
            weekday,
            Point::new(x + (cell_width - text_width) * 0.5, weekday_text_y),
            text_tertiary,
            weekday_font_size,
        );
    }

    let first = first_weekday(state.year, state.month);
    for day in 1..=days_in_month(state.year, state.month) {
        let date = Date::new(state.year, state.month, day);
        let slot = first + day - 1;
        let row = slot / 7;
        let column = slot % 7;
        // 按共享内边距和列宽计算日期横坐标。
        let x = popup.x + geometry.horizontal_inset + column as f32 * cell_width;
        // 构造缩放后的日期单元格矩形。
        let cell_rect = Rect::new(
            x,
            popup.y + geometry.grid_top() + row as f32 * geometry.cell_height,
            cell_width,
            geometry.cell_height,
        );
        let is_active = state.active == Some(date);
        let is_in_range = state.is_in_range(date);
        let is_hovered = state.hover == Some(date);
        let is_disabled = state.is_disabled(date);

        if is_active || is_in_range {
            ctx.fill_rect(cell_rect, primary_bg, None);
        } else if is_hovered {
            ctx.fill_rect(cell_rect, fill_tertiary, None);
        }

        let color = if is_disabled {
            text_tertiary
        } else if is_active {
            primary
        } else {
            text_color
        };
        let day_text = day.to_string();
        // 计算缩放后的日期字号。
        let day_font_size = 12.0 * geometry.font_scale;
        // 测量缩放后的日期文本宽度。
        let text_width = ctx.measure_text(&day_text, day_font_size).w;
        // 计算日期文本基线。
        let text_y = ctx.visual_center_y(cell_rect, day_font_size);
        ctx.draw_text(
            &day_text,
            Point::new(x + (cell_width - text_width) * 0.5, text_y),
            color,
            day_font_size,
        );
    }
    ctx.pop_clip();
}
