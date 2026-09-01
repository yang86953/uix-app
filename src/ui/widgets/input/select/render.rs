// 复用选择弹层的共享表面解析与绝对坐标转换。
use super::{ResolvedSelectVisual, Select, VisibleRow, normalize_select_rect, select_popup_rect};
// 复用 widgets 层共享的颜色衰减辅助，不依赖 feedback capability。
use crate::core::{Point, Rect};
use crate::draw::{Color, Radius};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widgets::fade_token_color;

impl Select {
    pub(super) fn render_select(&self, frame: Rect, ctx: &mut PaintContext) {
        self.capture_bound_value_dependency();
        self.multi_remove_rects.borrow_mut().clear();
        // 控件与弹层共享同一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());

        let control_height = frame.h.max(0.0).min(self.control_height());
        let control_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), control_height);
        self.control_rect
            .set(Rect::new(0.0, 0.0, control_rect.w, control_rect.h));

        let text_secondary = if self.disabled {
            visual.text_quaternary
        } else {
            visual.text_secondary
        };
        let primary = visual.primary;
        let radius = Some(Radius::uniform(visual.radius));
        let background = if self.disabled {
            visual.fill_tertiary
        } else if self.hovered || self.open {
            visual.background
        } else {
            visual.background_elevated
        };
        let border = if self.focused || self.open {
            primary
        } else if self.disabled {
            visual.border_secondary
        } else {
            visual.border
        };

        ctx.fill_rect(control_rect, background, radius);

        let text_area = Rect::new(
            control_rect.x + self.visual.layout.control_left_padding,
            control_rect.y,
            (control_rect.w
                - self.visual.layout.control_left_padding
                - self.visual.layout.arrow_slot_width)
                .max(0.0),
            control_rect.h,
        );
        self.render_control_value(control_rect, text_area, ctx, &visual);

        let arrow_rect = Rect::new(
            control_rect.x + (control_rect.w - self.visual.layout.arrow_slot_width).max(0.0),
            control_rect.y,
            self.visual.layout.arrow_slot_width,
            control_rect.h,
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            if self.is_present() {
                self.visual.icons.arrow_up
            } else {
                self.visual.icons.arrow_down
            },
            arrow_rect,
            text_secondary,
            visual.arrow_icon_size,
        );
        ctx.stroke_rect(
            control_rect,
            border,
            if self.focused || self.open {
                self.visual.chrome.focus_border_width
            } else {
                self.visual.chrome.border_width
            },
            radius,
        );

        if self.is_present() {
            // 弹层几何由共享解析器从 frame 与当前表面派生。
            self.render_dropdown(frame, ctx, &visual);
        }
    }

    fn render_control_value(
        &self,
        control_rect: Rect,
        text_area: Rect,
        ctx: &mut PaintContext,
        visual: &ResolvedSelectVisual,
    ) {
        let text = if self.disabled {
            visual.text_quaternary
        } else {
            visual.text
        };
        let text_secondary = if self.disabled {
            visual.text_quaternary
        } else {
            visual.text_secondary
        };
        if text_area.w <= 0.0 || text_area.h <= 0.0 {
            self.search_cursor_rect.set(Rect::new(
                text_area.x,
                control_rect.y + self.visual.layout.caret_vertical_inset,
                self.visual.layout.caret_width,
                (control_rect.h - self.visual.layout.caret_vertical_inset * 2.0).max(0.0),
            ));
            return;
        }

        if self.multiple && !self.selected_multi.is_empty() && self.search_query.is_empty() {
            self.render_multi_value(control_rect, text_area, ctx, visual, text, text_secondary);
            self.search_cursor_rect.set(Rect::new(
                text_area.x,
                control_rect.y + self.visual.layout.caret_vertical_inset,
                self.visual.layout.caret_width,
                (control_rect.h - self.visual.layout.caret_vertical_inset * 2.0).max(0.0),
            ));
            return;
        }

        let showing_query = self.search && self.open && !self.search_query.is_empty();
        // 受控状态保存稳定 value，闭合控件始终绘制对应的用户可读 label。
        let selected_label = self.current_label();
        let display_text = if showing_query {
            self.search_query.as_str()
        } else {
            selected_label.unwrap_or("")
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
            ctx.measure_text(display_text, visual.text_size).w
        };
        let draw_x = if showing_query && display_width > text_area.w {
            text_area.x + text_area.w - display_width
        } else {
            text_area.x
        };
        let draw_y = ctx.visual_center_y(text_area, visual.text_size);
        ctx.push_clip(text_area);
        if !display_text.is_empty() {
            ctx.draw_text(
                display_text,
                Point::new(draw_x, draw_y),
                display_color,
                visual.text_size,
            );
        }
        ctx.pop_clip();

        let query_width = if self.search_query.is_empty() {
            0.0
        } else {
            ctx.measure_text(&self.search_query, visual.text_size).w
        };
        let query_draw_x = if query_width > text_area.w {
            text_area.x + text_area.w - query_width
        } else {
            text_area.x
        };
        let cursor_x = (query_draw_x + query_width).clamp(text_area.x, text_area.x + text_area.w);
        let cursor = Rect::new(
            cursor_x,
            control_rect.y + self.visual.layout.caret_vertical_inset,
            self.visual.layout.caret_width,
            (control_rect.h - self.visual.layout.caret_vertical_inset * 2.0).max(0.0),
        );
        self.search_cursor_rect.set(cursor);
        if self.search && self.focused && self.open {
            ctx.fill_rect(cursor, visual.primary, None);
        }
    }

    fn render_multi_value(
        &self,
        control_rect: Rect,
        text_area: Rect,
        ctx: &mut PaintContext,
        visual: &ResolvedSelectVisual,
        text: Color,
        text_secondary: Color,
    ) {
        let tag_height = (control_rect.h - self.visual.layout.tag_vertical_inset)
            .clamp(0.0, self.visual.layout.tag_max_height);
        if tag_height <= 0.0 {
            return;
        }
        let tag_y = control_rect.y + (control_rect.h - tag_height) * 0.5;
        let mut x = text_area.x;
        let right = text_area.x + text_area.w;
        ctx.push_clip(text_area);
        for &option_index in &self.selected_multi {
            // 多选标签绘制显示文案，状态提交仍由稳定值负责。
            let Some(label) = self.option_label(option_index) else {
                continue;
            };
            let remaining = (right - x).max(0.0);
            if remaining < self.visual.layout.tag_min_remaining {
                break;
            }
            let close_width = if self.disabled {
                0.0
            } else {
                self.visual.layout.tag_close_width
            };
            let desired_width = ctx.measure_text(label, visual.tag_text_size).w
                + self.visual.layout.tag_horizontal_padding * 2.0
                + close_width;
            let tag_width = desired_width.min(remaining);
            let tag_rect = Rect::new(x, tag_y, tag_width, tag_height);
            ctx.fill_rect(
                tag_rect,
                visual.fill_tertiary,
                Some(Radius::uniform(self.visual.chrome.tag_radius)),
            );

            let close_rect = Rect::new(
                tag_rect.x + (tag_rect.w - close_width).max(0.0),
                tag_rect.y,
                close_width,
                tag_rect.h,
            );
            let label_rect = Rect::new(
                tag_rect.x + self.visual.layout.tag_horizontal_padding,
                tag_rect.y,
                (tag_rect.w - self.visual.layout.tag_horizontal_padding * 2.0 - close_width)
                    .max(0.0),
                tag_rect.h,
            );
            if label_rect.w > 0.0 {
                let y = ctx.visual_center_y(label_rect, visual.tag_text_size);
                ctx.push_clip(label_rect);
                ctx.draw_text(
                    label,
                    Point::new(label_rect.x, y),
                    text,
                    visual.tag_text_size,
                );
                ctx.pop_clip();
            }
            if close_width > 0.0 && close_rect.w > 0.0 {
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    self.visual.icons.close,
                    close_rect,
                    text_secondary,
                    visual.tag_close_icon_size,
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
            x += tag_width + self.visual.layout.tag_gap;
            if tag_width < desired_width {
                break;
            }
        }
        ctx.pop_clip();
    }

    // 绘制受当前逻辑表面约束的选择弹层。
    fn render_dropdown(&self, frame: Rect, ctx: &mut PaintContext, visual: &ResolvedSelectVisual) {
        // 读取当前可见行集合。
        let visible_rows = self.visible_rows();
        // 记录是否需要绘制空状态。
        let no_data = !self.loading && visible_rows.is_empty();
        // 计算当前实际弹层行数。
        let row_count = self.dropdown_row_count();
        // 将逻辑表面映射到当前组件坐标，兼容被提升的滚动浮层。
        let surface = normalize_select_rect(ctx.logical_surface_rect());
        // 解析并缓存与布局、命中和登记相同的相对弹层矩形。
        let popup = self.remember_dropdown_rect(frame, surface, row_count);
        // 将最终弹层转换为窗口绝对坐标。
        let list_rect = select_popup_rect(frame, popup);
        // 空表面或无可用空间时跳过弹层绘制。
        if list_rect.w <= 0.0 || list_rect.h <= 0.0 {
            // 保留已更新的空几何供其他管线复用。
            return;
        }
        // 读取最终列表纵坐标供行布局复用。
        let list_y = list_rect.y;
        // 将阴影、背景、行与文字统一裁到当前逻辑表面。
        ctx.push_clip(surface);

        // 读取当前过渡透明度。
        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let background = fade_token_color(visual.background_elevated, opacity);
        let border = fade_token_color(visual.border, opacity);
        let text = fade_token_color(visual.text, opacity);
        let primary = fade_token_color(visual.primary, opacity);
        let primary_bg = fade_token_color(visual.primary_background, opacity);
        let hover = fade_token_color(visual.fill_tertiary, opacity);
        let text_secondary = fade_token_color(visual.text_secondary, opacity);
        let group_background = fade_token_color(visual.fill_quaternary, opacity);
        let shadow = visual.shadow;
        let radius = Some(Radius::uniform(visual.radius));
        ctx.draw_box_shadow(
            list_rect,
            shadow.layer_1.2,
            shadow.layer_1.0,
            shadow.layer_1.1,
            shadow.layer_1.3,
            radius,
        );
        ctx.fill_rect(list_rect, background, radius);
        ctx.stroke_rect(list_rect, border, self.visual.chrome.border_width, radius);

        // 读取当前列表滚动偏移。
        let scroll_offset = self.dropdown_scroll.scroll_offset();
        // 使用受表面缩高后的实际列表高度计算物化范围。
        let (start, end) = self.dropdown_scroll.scroll_range(
            row_count,
            self.visual.layout.row_height,
            list_rect.h,
        );
        // 行内容继续裁在弹层本体内。
        ctx.push_clip(list_rect);
        for flat_index in start..end {
            let item_y = list_y + flat_index as f32 * self.visual.layout.row_height - scroll_offset;
            if item_y + self.visual.layout.row_height < list_y || item_y > list_y + list_rect.h {
                continue;
            }
            if self.loading {
                // 加载行使用最终弹层横向几何。
                let row = Rect::new(
                    list_rect.x,
                    item_y,
                    list_rect.w,
                    self.visual.layout.row_height,
                );
                let radius = self
                    .visual
                    .layout
                    .loading_radius
                    .min(row.w.min(row.h) * self.visual.layout.loading_radius_ratio);
                if radius > 0.0 {
                    ctx.stroke_arc(
                        row.x + row.w * 0.5,
                        row.y + row.h * 0.5,
                        radius,
                        self.loading_phase,
                        self.loading_phase
                            + std::f32::consts::PI * self.visual.chrome.loading_sweep_pi,
                        primary,
                        self.visual.chrome.loading_stroke,
                    );
                }
            } else if no_data {
                // 空状态行使用最终弹层横向几何。
                let row = Rect::new(
                    list_rect.x,
                    item_y,
                    list_rect.w,
                    self.visual.layout.row_height,
                );
                let content = Rect::new(
                    row.x + self.visual.layout.row_horizontal_padding,
                    row.y,
                    (row.w - self.visual.layout.row_horizontal_padding * 2.0).max(0.0),
                    row.h,
                );
                let y = ctx.visual_center_y(content, visual.text_size);
                ctx.push_clip(content);
                ctx.draw_text(
                    crate::ui::widget_runtime::locale::use_locale().no_data,
                    Point::new(content.x, y),
                    text_secondary,
                    visual.text_size,
                );
                ctx.pop_clip();
            } else if let Some(row) = visible_rows.get(flat_index).copied() {
                self.render_dropdown_row(
                    list_rect,
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
                    visual,
                );
            }
        }
        // 恢复列表本体裁剪。
        ctx.pop_clip();
        // 恢复逻辑表面裁剪。
        ctx.pop_clip();
    }

    #[allow(clippy::too_many_arguments)]
    fn render_dropdown_row(
        &self,
        popup_rect: Rect,
        item_y: f32,
        flat_index: usize,
        row: VisibleRow,
        ctx: &mut PaintContext,
        text: Color,
        primary: Color,
        primary_bg: Color,
        hover: Color,
        text_secondary: Color,
        group_background: Color,
        visual: &ResolvedSelectVisual,
    ) {
        // 每行使用受表面约束后的实际弹层横向几何。
        let row_rect = Rect::new(
            // 行起点跟随弹层横坐标。
            popup_rect.x,
            // 行纵坐标由滚动窗口决定。
            item_y,
            // 行宽度跟随弹层受限宽度。
            popup_rect.w,
            // 保持既有固定行高。
            self.visual.layout.row_height,
        );
        match row {
            VisibleRow::Group(group_index) => {
                ctx.fill_rect(row_rect, group_background, None);
                if let Some(label) = self.group_label(group_index) {
                    let content = Rect::new(
                        row_rect.x + self.visual.layout.row_horizontal_padding,
                        row_rect.y,
                        (row_rect.w - self.visual.layout.row_horizontal_padding * 2.0).max(0.0),
                        row_rect.h,
                    );
                    let y = ctx.visual_center_y(content, visual.group_text_size);
                    ctx.push_clip(content);
                    ctx.draw_text(
                        label,
                        Point::new(content.x, y),
                        text_secondary,
                        visual.group_text_size,
                    );
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
                        row_rect.x + self.visual.layout.row_horizontal_padding,
                        row_rect.y + (row_rect.h - self.visual.layout.check_size) * 0.5,
                        self.visual.layout.check_size,
                        self.visual.layout.check_size,
                    );
                    if selected {
                        ctx.fill_rect(
                            check_rect,
                            primary,
                            Some(Radius::uniform(self.visual.chrome.check_radius)),
                        );
                        crate::ui::widgets::icon::Icon::paint_in_frame(
                            ctx,
                            self.visual.icons.check,
                            check_rect,
                            visual.white,
                            visual.option_check_icon_size,
                        );
                    } else {
                        ctx.stroke_rect(
                            check_rect,
                            text_secondary,
                            self.visual.chrome.border_width,
                            Some(Radius::uniform(self.visual.chrome.check_radius)),
                        );
                    }
                    let content = Rect::new(
                        row_rect.x + self.visual.layout.custom_multi_left,
                        row_rect.y,
                        (row_rect.w - self.visual.layout.option_right_inset).max(0.0),
                        row_rect.h,
                    );
                    if !self.custom_option_views {
                        self.draw_clipped_row_text(
                            label,
                            content,
                            ctx,
                            text_color,
                            visual.text_size,
                        );
                    }
                } else {
                    let content = Rect::new(
                        row_rect.x + self.visual.layout.row_horizontal_padding,
                        row_rect.y,
                        (row_rect.w - self.visual.layout.option_right_inset).max(0.0),
                        row_rect.h,
                    );
                    if !self.custom_option_views {
                        self.draw_clipped_row_text(
                            label,
                            content,
                            ctx,
                            text_color,
                            visual.text_size,
                        );
                    }
                    if selected {
                        crate::ui::widgets::icon::Icon::paint_in_frame(
                            ctx,
                            self.visual.icons.check,
                            Rect::new(
                                row_rect.x + row_rect.w - self.visual.layout.selected_icon_slot,
                                row_rect.y,
                                self.visual.layout.selected_icon_width,
                                row_rect.h,
                            ),
                            primary,
                            visual.selected_check_icon_size,
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
        ctx: &mut PaintContext,
        color: Color,
        text_size: f32,
    ) {
        if content.w <= 0.0 {
            return;
        }
        let y = ctx.visual_center_y(content, text_size);
        ctx.push_clip(content);
        ctx.draw_text(label, Point::new(content.x, y), color, text_size);
        ctx.pop_clip();
    }
}
