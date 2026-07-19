use super::{fade_color, Select, VisibleRow, DROPDOWN_ROW_HEIGHT};
use crate::core::{Point, Rect};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};

const TEXT_SIZE: f32 = 13.0;
const CONTROL_LEFT_PADDING: f32 = 10.0;
const ARROW_SLOT_WIDTH: f32 = 28.0;

impl Select {
    pub(super) fn render_select(&self, frame: Rect, ctx: &mut PaintContext<'_>) {
        self.capture_bound_value_dependency();
        self.multi_remove_rects.borrow_mut().clear();

        let control_height = frame.h.max(0.0).min(self.control_height());
        let control_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), control_height);
        self.control_rect
            .set(Rect::new(0.0, 0.0, control_rect.w, control_rect.h));

        let text = if self.disabled {
            ctx.tokens().color_text_quaternary()
        } else {
            ctx.tokens().color_text()
        };
        let text_secondary = if self.disabled {
            ctx.tokens().color_text_quaternary()
        } else {
            ctx.tokens().color_text_secondary()
        };
        let primary = ctx.tokens().color_primary();
        let radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let background = if self.disabled {
            ctx.tokens().color_fill_tertiary()
        } else if self.hovered || self.open {
            ctx.tokens().color_bg_container()
        } else {
            ctx.tokens().color_bg_elevated()
        };
        let border = if self.focused || self.open {
            primary
        } else if self.disabled {
            ctx.tokens().color_border_secondary()
        } else {
            ctx.tokens().color_border()
        };

        ctx.fill_rect(control_rect, background, radius);

        let text_area = Rect::new(
            control_rect.x + CONTROL_LEFT_PADDING,
            control_rect.y,
            (control_rect.w - CONTROL_LEFT_PADDING - ARROW_SLOT_WIDTH).max(0.0),
            control_rect.h,
        );
        self.render_control_value(control_rect, text_area, ctx, text, text_secondary, primary);

        let arrow_rect = Rect::new(
            control_rect.x + (control_rect.w - ARROW_SLOT_WIDTH).max(0.0),
            control_rect.y,
            ARROW_SLOT_WIDTH,
            control_rect.h,
        );
        crate::ui::widgets::icon::paint_icon_in_frame(
            ctx,
            if self.is_present() {
                "chevron-up"
            } else {
                "chevron-down"
            },
            arrow_rect,
            text_secondary,
            12.0,
        );
        ctx.stroke_rect(
            control_rect,
            border,
            if self.focused || self.open { 2.0 } else { 1.0 },
            radius,
        );

        if self.is_present() {
            self.render_dropdown(frame, control_rect, ctx);
        }
    }

    fn render_control_value(
        &self,
        control_rect: Rect,
        text_area: Rect,
        ctx: &mut PaintContext<'_>,
        text: Color,
        text_secondary: Color,
        primary: Color,
    ) {
        if text_area.w <= 0.0 || text_area.h <= 0.0 {
            self.search_cursor_rect.set(Rect::new(
                text_area.x,
                control_rect.y + 4.0,
                1.5,
                (control_rect.h - 8.0).max(0.0),
            ));
            return;
        }

        if self.multiple && !self.selected_multi.is_empty() && self.search_query.is_empty() {
            self.render_multi_value(control_rect, text_area, ctx, text, text_secondary);
            self.search_cursor_rect.set(Rect::new(
                text_area.x,
                control_rect.y + 4.0,
                1.5,
                (control_rect.h - 8.0).max(0.0),
            ));
            return;
        }

        let showing_query = self.search && self.open && !self.search_query.is_empty();
        let selected_value = self.current_value();
        let display_text = if showing_query {
            self.search_query.as_str()
        } else {
            selected_value.as_deref().unwrap_or("")
        };
        let showing_placeholder = display_text.is_empty() && !self.placeholder.is_empty();
        let display_text = if showing_placeholder {
            self.placeholder.as_str()
        } else {
            display_text
        };
        let display_color = if showing_placeholder {
            text_secondary
        } else {
            text
        };
        let display_width = if display_text.is_empty() {
            0.0
        } else {
            ctx.measure_text(display_text, TEXT_SIZE).w
        };
        let draw_x = if showing_query && display_width > text_area.w {
            text_area.x + text_area.w - display_width
        } else {
            text_area.x
        };
        let draw_y = ctx.visual_center_y(text_area, TEXT_SIZE);
        ctx.push_clip(text_area);
        if !display_text.is_empty() {
            ctx.draw_text(
                display_text,
                Point::new(draw_x, draw_y),
                display_color,
                TEXT_SIZE,
            );
        }
        ctx.pop_clip();

        let query_width = if self.search_query.is_empty() {
            0.0
        } else {
            ctx.measure_text(&self.search_query, TEXT_SIZE).w
        };
        let query_draw_x = if query_width > text_area.w {
            text_area.x + text_area.w - query_width
        } else {
            text_area.x
        };
        let cursor_x = (query_draw_x + query_width).clamp(text_area.x, text_area.x + text_area.w);
        let cursor = Rect::new(
            cursor_x,
            control_rect.y + 4.0,
            1.5,
            (control_rect.h - 8.0).max(0.0),
        );
        self.search_cursor_rect.set(cursor);
        if self.search && self.focused && self.open {
            ctx.fill_rect(cursor, primary, None);
        }
    }

    fn render_multi_value(
        &self,
        control_rect: Rect,
        text_area: Rect,
        ctx: &mut PaintContext<'_>,
        text: Color,
        text_secondary: Color,
    ) {
        let tag_height = (control_rect.h - 8.0).clamp(0.0, 24.0);
        if tag_height <= 0.0 {
            return;
        }
        let tag_y = control_rect.y + (control_rect.h - tag_height) * 0.5;
        let mut x = text_area.x;
        let right = text_area.x + text_area.w;
        let all_options = self.all_options();

        ctx.push_clip(text_area);
        for &option_index in &self.selected_multi {
            let Some(label) = all_options.get(option_index).copied() else {
                continue;
            };
            let remaining = (right - x).max(0.0);
            if remaining < 20.0 {
                break;
            }
            let close_width = if self.disabled { 0.0 } else { 18.0 };
            let desired_width = ctx.measure_text(label, 12.0).w + 12.0 + close_width;
            let tag_width = desired_width.min(remaining);
            let tag_rect = Rect::new(x, tag_y, tag_width, tag_height);
            ctx.fill_rect(
                tag_rect,
                ctx.tokens().color_fill_tertiary(),
                Some(Radius::uniform(4.0)),
            );

            let close_rect = Rect::new(
                tag_rect.x + (tag_rect.w - close_width).max(0.0),
                tag_rect.y,
                close_width,
                tag_rect.h,
            );
            let label_rect = Rect::new(
                tag_rect.x + 6.0,
                tag_rect.y,
                (tag_rect.w - 12.0 - close_width).max(0.0),
                tag_rect.h,
            );
            if label_rect.w > 0.0 {
                let y = ctx.visual_center_y(label_rect, 12.0);
                ctx.push_clip(label_rect);
                ctx.draw_text(label, Point::new(label_rect.x, y), text, 12.0);
                ctx.pop_clip();
            }
            if close_width > 0.0 && close_rect.w > 0.0 {
                crate::ui::widgets::icon::paint_icon_in_frame(
                    ctx,
                    "x",
                    close_rect,
                    text_secondary,
                    10.0,
                );
                self.multi_remove_rects.borrow_mut().push((
                    option_index,
                    Rect::new(
                        close_rect.x - control_rect.x,
                        close_rect.y - control_rect.y,
                        close_rect.w,
                        close_rect.h,
                    ),
                ));
            }
            x += tag_width + 4.0;
            if tag_width < desired_width {
                break;
            }
        }
        ctx.pop_clip();
    }

    fn render_dropdown(&self, frame: Rect, control_rect: Rect, ctx: &mut PaintContext<'_>) {
        let visible_rows = self.visible_rows();
        let no_data = !self.loading && visible_rows.is_empty();
        let row_count = self.dropdown_row_count();
        let list_height = self.dropdown_viewport_height(row_count);
        let surface_height = ctx.logical_surface_size().h;
        let below_y = control_rect.y + control_rect.h;
        let above_y = control_rect.y - list_height;
        let list_y = if below_y + list_height <= surface_height || above_y < 0.0 {
            below_y
        } else {
            above_y
        };
        let list_rect = Rect::new(frame.x, list_y, frame.w.max(0.0), list_height);
        self.dropdown_rect.set(Rect::new(
            list_rect.x - frame.x,
            list_rect.y - frame.y,
            list_rect.w,
            list_rect.h,
        ));

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let background = fade_color(ctx.tokens().color_bg_elevated(), opacity);
        let border = fade_color(ctx.tokens().color_border(), opacity);
        let text = fade_color(ctx.tokens().color_text(), opacity);
        let primary = fade_color(ctx.tokens().color_primary(), opacity);
        let primary_bg = fade_color(ctx.tokens().color_primary_bg(), opacity);
        let hover = fade_color(ctx.tokens().color_fill_tertiary(), opacity);
        let text_secondary = fade_color(ctx.tokens().color_text_secondary(), opacity);
        let group_background = fade_color(ctx.tokens().color_fill_quaternary(), opacity);
        let shadow = ctx.tokens().box_shadow_secondary();
        let radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        ctx.draw_box_shadow(
            list_rect,
            shadow.layer_1.2,
            shadow.layer_1.0,
            shadow.layer_1.1,
            shadow.layer_1.3,
            radius,
        );
        ctx.fill_rect(list_rect, background, radius);
        ctx.stroke_rect(list_rect, border, 1.0, radius);

        let scroll_offset = self.dropdown_scroll.scroll_offset();
        let (start, end) =
            self.dropdown_scroll
                .scroll_range(row_count, DROPDOWN_ROW_HEIGHT, list_height);
        ctx.push_clip(list_rect);
        for flat_index in start..end {
            let item_y = list_y + flat_index as f32 * DROPDOWN_ROW_HEIGHT - scroll_offset;
            if item_y + DROPDOWN_ROW_HEIGHT < list_y || item_y > list_y + list_height {
                continue;
            }
            if self.loading {
                let row = Rect::new(frame.x, item_y, frame.w, DROPDOWN_ROW_HEIGHT);
                let radius = 5.0_f32.min(row.w.min(row.h) * 0.25);
                if radius > 0.0 {
                    ctx.stroke_arc(
                        row.x + row.w * 0.5,
                        row.y + row.h * 0.5,
                        radius,
                        self.loading_phase,
                        self.loading_phase + std::f32::consts::PI * 1.45,
                        primary,
                        1.8,
                    );
                }
            } else if no_data {
                let row = Rect::new(frame.x, item_y, frame.w, DROPDOWN_ROW_HEIGHT);
                let content = Rect::new(row.x + 10.0, row.y, (row.w - 20.0).max(0.0), row.h);
                let y = ctx.visual_center_y(content, TEXT_SIZE);
                ctx.push_clip(content);
                ctx.draw_text(
                    crate::ui::locale::use_locale().no_data,
                    Point::new(content.x, y),
                    text_secondary,
                    TEXT_SIZE,
                );
                ctx.pop_clip();
            } else if let Some(row) = visible_rows.get(flat_index).copied() {
                self.render_dropdown_row(
                    frame,
                    item_y,
                    flat_index,
                    row,
                    ctx,
                    text,
                    primary,
                    primary_bg,
                    hover,
                    text_secondary,
                    group_background,
                );
            }
        }
        ctx.pop_clip();
    }

    #[allow(clippy::too_many_arguments)]
    fn render_dropdown_row(
        &self,
        frame: Rect,
        item_y: f32,
        flat_index: usize,
        row: VisibleRow,
        ctx: &mut PaintContext<'_>,
        text: Color,
        primary: Color,
        primary_bg: Color,
        hover: Color,
        text_secondary: Color,
        group_background: Color,
    ) {
        let row_rect = Rect::new(frame.x, item_y, frame.w, DROPDOWN_ROW_HEIGHT);
        match row {
            VisibleRow::Group(group_index) => {
                ctx.fill_rect(row_rect, group_background, None);
                if let Some(label) = self.group_label(group_index) {
                    let content = Rect::new(
                        row_rect.x + 10.0,
                        row_rect.y,
                        (row_rect.w - 20.0).max(0.0),
                        row_rect.h,
                    );
                    let y = ctx.visual_center_y(content, 12.0);
                    ctx.push_clip(content);
                    ctx.draw_text(label, Point::new(content.x, y), text_secondary, 12.0);
                    ctx.pop_clip();
                }
            }
            VisibleRow::Option(option_index) => {
                let Some(label) = self.option_label(option_index) else {
                    return;
                };
                let selected = if self.multiple {
                    self.selected_multi.contains(&option_index)
                } else {
                    self.selected == option_index
                };
                let keyboard_active =
                    (self.search || self.multiple) && self.highlighted_option == Some(option_index);
                if self.hovered_option == Some(flat_index) || keyboard_active {
                    ctx.fill_rect(row_rect, hover, None);
                } else if selected {
                    ctx.fill_rect(row_rect, primary_bg, None);
                }

                let text_color = if selected { primary } else { text };
                if self.multiple {
                    let check_rect = Rect::new(
                        row_rect.x + 10.0,
                        row_rect.y + (row_rect.h - 14.0) * 0.5,
                        14.0,
                        14.0,
                    );
                    if selected {
                        ctx.fill_rect(check_rect, primary, Some(Radius::uniform(3.0)));
                        crate::ui::widgets::icon::paint_icon_in_frame(
                            ctx,
                            "check",
                            check_rect,
                            Color::white(),
                            10.0,
                        );
                    } else {
                        ctx.stroke_rect(
                            check_rect,
                            text_secondary,
                            1.0,
                            Some(Radius::uniform(3.0)),
                        );
                    }
                    let content = Rect::new(
                        row_rect.x + 32.0,
                        row_rect.y,
                        (row_rect.w - 42.0).max(0.0),
                        row_rect.h,
                    );
                    if !self.custom_option_views {
                        self.draw_clipped_row_text(label, content, ctx, text_color);
                    }
                } else {
                    let content = Rect::new(
                        row_rect.x + 10.0,
                        row_rect.y,
                        (row_rect.w - 42.0).max(0.0),
                        row_rect.h,
                    );
                    if !self.custom_option_views {
                        self.draw_clipped_row_text(label, content, ctx, text_color);
                    }
                    if selected {
                        crate::ui::widgets::icon::paint_icon_in_frame(
                            ctx,
                            "check",
                            Rect::new(row_rect.x + row_rect.w - 28.0, row_rect.y, 20.0, row_rect.h),
                            primary,
                            12.0,
                        );
                    }
                }
            }
        }
    }

    fn draw_clipped_row_text(
        &self,
        label: &str,
        content: Rect,
        ctx: &mut PaintContext<'_>,
        color: Color,
    ) {
        if content.w <= 0.0 {
            return;
        }
        let y = ctx.visual_center_y(content, TEXT_SIZE);
        ctx.push_clip(content);
        ctx.draw_text(label, Point::new(content.x, y), color, TEXT_SIZE);
        ctx.pop_clip();
    }
}
