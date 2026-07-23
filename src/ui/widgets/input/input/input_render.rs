//! Input 组件渲染实现（单行 / 多行）

use super::*;

use crate::core::{Point, Rect};
use crate::draw::api::PaintContext;
use crate::draw::{Color, Radius};
// ════════════════════════════════════════════════════════════════════════════
// 多行渲染
// ════════════════════════════════════════════════════════════════════════════

impl Input {
    fn status_color(&self, ctx: &PaintContext<'_>) -> Option<Color> {
        self.status.map(|status| match status {
            InputStatus::Success => ctx.tokens().color_success(),
            InputStatus::Warning => ctx.tokens().color_warning(),
            InputStatus::Error => ctx.tokens().color_error(),
        })
    }

    pub(super) fn render_status_message(
        &self,
        frame: Rect,
        control_height: f32,
        ctx: &mut PaintContext<'_>,
    ) {
        if self.status_message.is_empty() || frame.h <= control_height {
            return;
        }
        let message_frame = Rect::new(
            frame.x,
            frame.y + control_height,
            frame.w,
            (frame.h - control_height).min(STATUS_MESSAGE_HEIGHT),
        );
        ctx.push_clip(message_frame);
        let color = self
            .status_color(ctx)
            .unwrap_or_else(|| ctx.tokens().color_text_secondary());
        let font_size = 12.0;
        let y = ctx.visual_center_y(message_frame, font_size);
        ctx.draw_text(
            &self.status_message,
            Point::new(message_frame.x, y),
            color,
            font_size,
        );
        ctx.pop_clip();
    }

    pub(super) fn render_textarea(&self, frame: Rect, ctx: &mut PaintContext) {
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let border_color = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let text_color = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let border_radius_sm = ctx.tokens().border_radius_sm();

        let inner_frame = Rect::new(frame.x, frame.y, frame.w, frame.h);

        let status_border = self.status_color(ctx);
        let (bg, border) = if self.disabled {
            (fill_tertiary, border_color)
        } else if let Some(status_border) = status_border {
            (ctx.tokens().color_bg_elevated(), status_border)
        } else if self.focused {
            (ctx.tokens().color_bg_elevated(), primary)
        } else if self.hovered {
            (ctx.tokens().color_bg_elevated(), primary_hover)
        } else {
            (ctx.tokens().color_bg_container(), border_color)
        };

        let radius = Some(Radius::uniform(border_radius_sm));
        ctx.fill_rect(inner_frame, bg, radius);
        ctx.stroke_rect(
            inner_frame,
            border,
            if self.focused { 2.0 } else { 1.0 },
            radius,
        );

        let text_area = Rect::new(
            inner_frame.x + PAD,
            inner_frame.y + 6.0,
            (inner_frame.w - PAD * 2.0).max(20.0),
            (inner_frame.h - 12.0).max(20.0),
        );
        ctx.push_clip(text_area);

        let has_composition = !self.composition.is_empty();
        let showing_placeholder = self.value.is_empty() && !has_composition;
        let display_text = if showing_placeholder {
            Cow::Borrowed(self.placeholder.as_str())
        } else {
            self.display_value_with_composition()
        };
        let disp_color = if showing_placeholder {
            text_tertiary
        } else {
            text_color
        };

        let lines = logical_lines(&display_text);
        let value_lines = logical_lines(&self.value);

        // 计算光标所在行
        let cursor_line = self.cursor_line_col().0;

        // 垂直滚动：确保光标行可见
        let vis_lines = ((text_area.h / LINE_HEIGHT) as usize).max(1);
        let scroll_line = self.scroll_line.get();
        let adj_scroll = if cursor_line >= scroll_line + vis_lines {
            cursor_line.saturating_sub(vis_lines).saturating_add(1)
        } else if cursor_line < scroll_line {
            cursor_line
        } else {
            scroll_line
        };
        self.scroll_line.set(adj_scroll);

        // 逐行绘制
        let line_h = LINE_HEIGHT;
        let mut y = text_area.y;
        let mut line_glyph_xs = self.line_glyph_xs.borrow_mut();
        line_glyph_xs.clear();
        line_glyph_xs.resize_with(lines.len(), Vec::new);
        for (li, line) in lines.iter().enumerate() {
            if li < adj_scroll {
                continue;
            }
            if y >= text_area.y + text_area.h {
                break;
            }
            // 计算该行的字符范围
            let line_start: usize = value_lines[..li.min(value_lines.len())]
                .iter()
                .map(|s| s.chars().count())
                .sum();
            // 加上换行符的数量
            let line_start = line_start + li; // each '\n' adds 1 char
            let value_line = value_lines.get(li).copied().unwrap_or("");
            let line_end = line_start + value_line.chars().count();

            // 选中高亮
            if !has_composition {
                if let Some((sel_s, sel_e)) = self.selection.get() {
                    if sel_s < sel_e && sel_s < line_end && sel_e > line_start {
                        let sel_in_line_start = sel_s.saturating_sub(line_start);
                        let sel_in_line_end = if sel_e < line_end {
                            sel_e - line_start
                        } else {
                            value_line.chars().count()
                        };
                        // 估算选中区域的 x 位置
                        let before_sel: String = line.chars().take(sel_in_line_start).collect();
                        let sel_text: String = line
                            .chars()
                            .skip(sel_in_line_start)
                            .take(sel_in_line_end - sel_in_line_start)
                            .collect();
                        let x0 = text_area.x + ctx.measure_text(&before_sel, FONT_SIZE).w;
                        let sel_w = ctx.measure_text(&sel_text, FONT_SIZE).w;
                        ctx.fill_rect(
                            Rect::new(x0, y, sel_w, line_h),
                            primary.with_alpha(64),
                            None,
                        );
                    }
                }
            }

            // 行内光学居中：用 visual_center_y，去掉魔法 +2.0
            let text_y =
                ctx.visual_center_y(Rect::new(text_area.x, y, text_area.w, line_h), FONT_SIZE);
            ctx.draw_text(line, Point::new(text_area.x, text_y), disp_color, FONT_SIZE);

            // 收集该行每个字符的 x 坐标（用于 char_at_xy 命中）
            let hit_text = if showing_placeholder { "" } else { *line };
            let mut xs = Vec::with_capacity(hit_text.chars().count() + 1);
            let mut prefix = String::new();
            xs.push(0.0); // 相对 text area 的行首光标
            for ch in hit_text.chars() {
                prefix.push(ch);
                xs.push(ctx.measure_text(&prefix, FONT_SIZE).w);
            }
            line_glyph_xs[li] = xs;

            if li == cursor_line {
                let col = self.cursor_line_col().1;
                let composition_chars = if has_composition {
                    self.composition.chars().count()
                } else {
                    0
                };
                let before: String = line.chars().take(col + composition_chars).collect();
                let cx = text_area.x + ctx.measure_text(&before, FONT_SIZE).w;
                let caret_h = (line_h - 4.0).max(FONT_SIZE * 0.8);
                let caret_y = y + (line_h - caret_h) * 0.5;
                self.caret_rect.set(Rect::new(cx, caret_y, 1.5, caret_h));

                if has_composition {
                    let before_composition: String = line.chars().take(col).collect();
                    let composition_x =
                        text_area.x + ctx.measure_text(&before_composition, FONT_SIZE).w;
                    let composition_w = ctx.measure_text(&self.composition, FONT_SIZE).w;
                    ctx.fill_rect(
                        Rect::new(composition_x, y + line_h - 2.0, composition_w.max(1.5), 1.5),
                        primary,
                        None,
                    );
                }

                if self.focused && self.selection.get().is_none() {
                    ctx.fill_rect(Rect::new(cx, caret_y, 1.5, caret_h), primary, None);
                }
            }

            y += line_h;
        }

        // 如果没有任何行且 focused，在顶部画光标
        if self.focused && lines.is_empty() {
            let caret_h = (line_h - 4.0).max(FONT_SIZE * 0.8);
            let caret_y = text_area.y + (line_h - caret_h) * 0.5;
            let caret = Rect::new(text_area.x, caret_y, 1.5, caret_h);
            self.caret_rect.set(caret);
            ctx.fill_rect(caret, primary, None);
        }

        ctx.pop_clip();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 单行渲染（原逻辑精简）
// ════════════════════════════════════════════════════════════════════════════

impl Input {
    pub(super) fn render_singleline(&self, frame: Rect, ctx: &mut PaintContext) {
        let h = input_height(self.input_size).min(frame.h);
        let input_frame = Rect::new(frame.x, frame.y, frame.w, h);

        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let border_color = ctx.tokens().color_border();
        let text_quaternary = ctx.tokens().color_text_quaternary();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let text_color_token = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let text_sec = ctx.tokens().color_text_secondary();

        let addon_left_w = addon_width(&self.addon_before);
        let addon_right_w = addon_width(&self.addon_after);

        if !self.addon_before.is_empty() {
            let addon_rect = Rect::new(input_frame.x, input_frame.y, addon_left_w, input_frame.h);
            ctx.fill_rect(
                addon_rect,
                fill_tertiary,
                Some(Radius::uniform(border_radius_sm)),
            );
            let ay = ctx.visual_center_y(addon_rect, ADDON_FONT_SIZE);
            ctx.draw_text(
                &self.addon_before,
                Point::new(addon_rect.x + 8.0, ay),
                text_sec,
                ADDON_FONT_SIZE,
            );
        }
        if !self.addon_after.is_empty() {
            let addon_rect = Rect::new(
                input_frame.x + input_frame.w - addon_right_w,
                input_frame.y,
                addon_right_w,
                input_frame.h,
            );
            ctx.fill_rect(
                addon_rect,
                fill_tertiary,
                Some(Radius::uniform(border_radius_sm)),
            );
            let ay = ctx.visual_center_y(addon_rect, ADDON_FONT_SIZE);
            ctx.draw_text(
                &self.addon_after,
                Point::new(addon_rect.x + 8.0, ay),
                text_sec,
                ADDON_FONT_SIZE,
            );
        }

        let inner_frame = Rect::new(
            input_frame.x + addon_left_w,
            input_frame.y,
            input_frame.w - addon_left_w - addon_right_w,
            input_frame.h,
        );

        let prefix_w = if self.prefix.is_empty() { 0.0 } else { 20.0 };
        let suffix_w = if self.suffix.is_empty() { 0.0 } else { 20.0 };
        let clear_w = if self.clearable { 20.0 } else { 0.0 };
        let pwd_w = if self.password { 24.0 } else { 0.0 };
        let search_w = if self.search { 24.0 } else { 0.0 };
        let right_extra = suffix_w + clear_w + pwd_w + search_w;

        let status_border = self.status_color(ctx);
        let (bg, border, text_color) = if self.disabled {
            (fill_tertiary, border_color, text_quaternary)
        } else if let Some(status_border) = status_border {
            (
                ctx.tokens().color_bg_elevated(),
                status_border,
                text_color_token,
            )
        } else if self.focused {
            (ctx.tokens().color_bg_elevated(), primary, text_color_token)
        } else if self.hovered {
            (
                ctx.tokens().color_bg_elevated(),
                primary_hover,
                text_color_token,
            )
        } else {
            (
                ctx.tokens().color_bg_container(),
                border_color,
                text_color_token,
            )
        };

        let radius = Some(Radius::uniform(border_radius_sm));
        ctx.fill_rect(inner_frame, bg, radius);
        ctx.stroke_rect(
            inner_frame,
            border,
            if self.focused { 2.0 } else { 1.0 },
            radius,
        );

        if !self.prefix.is_empty() {
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                &self.prefix,
                Rect::new(inner_frame.x + 2.0, inner_frame.y, prefix_w, inner_frame.h),
                text_sec,
                12.0,
            );
        }

        let mut accessory_right = inner_frame.x + inner_frame.w;
        let search_rect = if self.search {
            accessory_right -= search_w;
            Some(Rect::new(
                accessory_right,
                inner_frame.y,
                search_w,
                inner_frame.h,
            ))
        } else {
            None
        };
        let password_rect = if self.password {
            accessory_right -= pwd_w;
            Some(Rect::new(
                accessory_right,
                inner_frame.y,
                pwd_w,
                inner_frame.h,
            ))
        } else {
            None
        };
        let clear_rect = if clear_w > 0.0 {
            accessory_right -= clear_w;
            Some(Rect::new(
                accessory_right,
                inner_frame.y,
                clear_w,
                inner_frame.h,
            ))
        } else {
            None
        };
        let suffix_rect = if !self.suffix.is_empty() {
            accessory_right -= suffix_w;
            Some(Rect::new(
                accessory_right,
                inner_frame.y,
                suffix_w,
                inner_frame.h,
            ))
        } else {
            None
        };

        if let Some(rect) = suffix_rect {
            crate::ui::widgets::icon::Icon::paint_in_frame(ctx, &self.suffix, rect, text_sec, 12.0);
        }

        let has_composition = !self.composition.is_empty();
        let showing_placeholder = self.value.is_empty() && !has_composition;
        let display_text = if showing_placeholder {
            Cow::Borrowed(self.placeholder.as_str())
        } else {
            self.display_value_with_composition()
        };
        let disp_color = if showing_placeholder {
            text_tertiary
        } else {
            text_color
        };

        let text_area_x = inner_frame.x + PAD + prefix_w;
        let text_area_w = (inner_frame.w - PAD * 2.0 - prefix_w - right_extra).max(20.0);
        if text_area_w <= 0.0 {
            return;
        }
        let text_area = Rect::new(text_area_x, inner_frame.y, text_area_w, inner_frame.h);
        ctx.push_clip(text_area);

        let mut scroll_off = self.scroll_offset_x.get();
        let total_text_w = if !display_text.is_empty() {
            ctx.measure_text(&display_text, FONT_SIZE).w
        } else {
            0.0
        };

        let text_before = self.visual_text_before_cursor();
        let text_before_w = if !text_before.is_empty() {
            ctx.measure_text(&text_before, FONT_SIZE).w
        } else {
            0.0
        };
        let composition_w = if has_composition {
            ctx.measure_text(&self.composition, FONT_SIZE).w
        } else {
            0.0
        };
        let caret_text_w = text_before_w + composition_w;

        let right_margin = 10.0;
        if caret_text_w - scroll_off > text_area_w - right_margin {
            scroll_off = caret_text_w - text_area_w + right_margin;
        }
        if caret_text_w - scroll_off < 0.0 {
            scroll_off = caret_text_w;
        }
        scroll_off = scroll_off.min(total_text_w - 1.0).max(0.0);
        self.scroll_offset_x.set(scroll_off);

        let draw_x = text_area_x - scroll_off;
        let draw_y = ctx.visual_center_y(text_area, FONT_SIZE);

        if !display_text.is_empty() {
            let opts = crate::draw::TextLayoutOptions {
                max_width: f32::MAX,
                max_height: 0.0,
                line_height: FONT_SIZE * 1.5,
                word_wrap: false,
                h_align: crate::draw::HAlign::Left,
                v_align: crate::draw::VAlign::Top,
                font_size: FONT_SIZE,
            };
            let backend_opts =
                crate::draw::resources::font::text_backend::TextLayoutOptions::from(opts);
            let fh = *ctx.font();
            let layout = ctx
                .font_service()
                .layout_text(&fh, &display_text, &backend_opts);

            let abs_pos = Point::new(draw_x, draw_y);
            {
                let mut xs = self.glyph_xs.borrow_mut();
                xs.clear();
                if !showing_placeholder {
                    for g in &layout.glyphs {
                        xs.push(g.x);
                    }
                }
            }
            self.line_glyph_xs.borrow_mut().clear();
            if !self.value.is_empty() && !has_composition {
                if let Some((sel_s, sel_e)) = self.selection.get() {
                    if sel_s < sel_e {
                        let visual_h = ctx
                            .font_service()
                            .horizontal_line_metrics(&fh, FONT_SIZE)
                            .map(|m| m.ascent + m.descent)
                            .unwrap_or(FONT_SIZE * 1.2);
                        let end = sel_e.min(layout.glyphs.len());
                        let start = sel_s.min(end);
                        for line in &layout.lines {
                            let gs = line.glyph_start;
                            let gc = line.glyph_count;
                            let ge = gs + gc;
                            let ls = start.max(gs);
                            let le = end.min(ge);
                            if ls >= le {
                                continue;
                            }
                            let glyphs = &layout.glyphs[ls..le];
                            let x0 = abs_pos.x + glyphs[0].x;
                            let last = glyphs[glyphs.len() - 1];
                            let x1 = abs_pos.x + last.x + last.width.max(0.0);
                            ctx.fill_rect(
                                Rect::new(x0, abs_pos.y + line.y, (x1 - x0).max(0.0), visual_h),
                                primary.with_alpha(64),
                                None,
                            );
                        }
                    }
                }
            }
            ctx.blit_owned_glyph_layout(layout, abs_pos, disp_color, FONT_SIZE);
        }

        if has_composition {
            let composition_x = text_area_x + text_before_w - scroll_off;
            ctx.fill_rect(
                Rect::new(
                    composition_x,
                    inner_frame.y + inner_frame.h - 3.0,
                    composition_w.max(1.5),
                    1.5,
                ),
                primary,
                None,
            );
        }

        ctx.pop_clip();

        let cursor_x = text_area_x + caret_text_w - scroll_off;
        let caret = Rect::new(cursor_x, inner_frame.y + 4.0, 1.5, inner_frame.h - 8.0);
        self.caret_rect.set(caret);
        if self.focused && self.selection.get().is_none() {
            ctx.fill_rect(caret, primary, None);
        }

        let clear_visible =
            self.clearable && !self.value.is_empty() && (self.focused || self.hovered);
        if clear_visible {
            let rect = clear_rect.unwrap_or(Rect::zero());
            self.clear_icon_rect.set(Rect::new(
                rect.x - input_frame.x,
                rect.y - input_frame.y,
                rect.w,
                rect.h,
            ));
            crate::ui::widgets::icon::Icon::paint_in_frame(ctx, "x", rect, text_sec, 12.0);
        } else {
            self.clear_icon_rect.set(Rect::zero());
        }

        if let Some(rect) = password_rect {
            self.pwd_icon_rect.set(Rect::new(
                rect.x - input_frame.x,
                rect.y - input_frame.y,
                rect.w,
                rect.h,
            ));
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if self.password_visible {
                    "eye"
                } else {
                    "eye-off"
                },
                rect,
                text_sec,
                14.0,
            );
        } else {
            self.pwd_icon_rect.set(Rect::zero());
        }

        if let Some(rect) = search_rect {
            crate::ui::widgets::icon::Icon::paint_in_frame(ctx, "search", rect, text_sec, 12.0);
        }
    }
}
