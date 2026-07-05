//! Input 组件渲染实现（单行 / 多行）

use super::*;

use crate::draw::painting::RenderContext;
use crate::draw::Radius;
use crate::native::{Point, Rect};

// ════════════════════════════════════════════════════════════════════════════
// 多行渲染
// ════════════════════════════════════════════════════════════════════════════

impl Input {
    pub(super) fn render_textarea(&self, frame: Rect, ctx: &mut RenderContext) {
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let border_color = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let text_color = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let border_radius_sm = ctx.tokens().border_radius_sm();

        let inner_frame = Rect::new(frame.x, frame.y, frame.w, frame.h);

        let (bg, border) = if self.disabled {
            (fill_tertiary, border_color)
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
        ctx.canvas_2d().push_clip(text_area);

        let display_text = if self.value.is_empty() && !self.focused {
            &self.placeholder
        } else {
            &self.value
        };
        let disp_color = if self.value.is_empty() && !self.focused {
            text_tertiary
        } else {
            text_color
        };

        let lines: Vec<&str> = if display_text == &self.placeholder {
            vec![self.placeholder.as_str()]
        } else {
            display_text.lines().collect()
        };

        // 计算光标所在行
        let cursor_line = self.cursor_line_col().0;

        // 垂直滚动：确保光标行可见
        let vis_lines = (text_area.h / LINE_HEIGHT) as usize;
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
        for (li, line) in lines.iter().enumerate() {
            if li < adj_scroll {
                continue;
            }
            if y + line_h > text_area.y + text_area.h {
                break;
            }
            // 计算该行的字符范围
            let line_start: usize = lines[..li].iter().map(|s| s.chars().count()).sum();
            // 加上换行符的数量
            let line_start = line_start + li; // each '\n' adds 1 char
            let line_end = line_start + line.chars().count();

            // 选中高亮
            if let Some((sel_s, sel_e)) = self.selection.get() {
                if sel_s < sel_e && sel_s < line_end && sel_e > line_start {
                    let sel_in_line_start = sel_s.saturating_sub(line_start);
                    let sel_in_line_end = if sel_e < line_end {
                        sel_e - line_start
                    } else {
                        line.chars().count()
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

            ctx.draw_text(
                line,
                Point::new(text_area.x, y + 2.0),
                disp_color,
                FONT_SIZE,
            );

            // 光标（在当前行且 focused）
            if self.focused && self.selection.get().is_none() && li == cursor_line {
                let col = self.cursor_line_col().1;
                let before: String = line.chars().take(col).collect();
                let cx = text_area.x + ctx.measure_text(&before, FONT_SIZE).w;
                ctx.fill_rect(Rect::new(cx, y + 2.0, 1.5, line_h - 4.0), primary, None);
            }

            y += line_h;
        }

        // 如果没有任何行且 focused，在顶部画光标
        if self.focused && lines.is_empty() {
            ctx.fill_rect(
                Rect::new(text_area.x, text_area.y + 2.0, 1.5, line_h - 4.0),
                primary,
                None,
            );
        }

        ctx.canvas_2d().pop_clip();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 单行渲染（原逻辑精简）
// ════════════════════════════════════════════════════════════════════════════

impl Input {
    pub(super) fn render_singleline(&self, frame: Rect, ctx: &mut RenderContext) {
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

        let addon_left_w = if self.addon_before.is_empty() {
            0.0
        } else {
            self.addon_before.len() as f32 * 8.0 + 16.0
        };
        let addon_right_w = if self.addon_after.is_empty() {
            0.0
        } else {
            self.addon_after.len() as f32 * 8.0 + 16.0
        };

        if !self.addon_before.is_empty() {
            let addon_rect = Rect::new(input_frame.x, input_frame.y, addon_left_w, input_frame.h);
            ctx.fill_rect(
                addon_rect,
                fill_tertiary,
                Some(Radius::uniform(border_radius_sm)),
            );
            let ay = ctx.visual_center_y(addon_rect, 13.0);
            ctx.draw_text(
                &self.addon_before,
                Point::new(addon_rect.x + 8.0, ay),
                text_sec,
                13.0,
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
            let ay = ctx.visual_center_y(addon_rect, 13.0);
            ctx.draw_text(
                &self.addon_after,
                Point::new(addon_rect.x + 8.0, ay),
                text_sec,
                13.0,
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
        let clear_w = if self.clearable && !self.value.is_empty() {
            20.0
        } else {
            0.0
        };
        let pwd_w = if self.password { 24.0 } else { 0.0 };
        let search_w = if self.search { 24.0 } else { 0.0 };
        let right_extra = suffix_w + clear_w + pwd_w + search_w;

        let (bg, border, text_color) = if self.disabled {
            (fill_tertiary, border_color, text_quaternary)
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
            let px = inner_frame.x + 6.0;
            let py = ctx.visual_center_y(inner_frame, 12.0);
            let icon_str = crate::ui::widgets::icon::icon_char(&self.prefix);
            let saved = *ctx.font();
            if let Some(fh) = crate::ui::widgets::icon::lucide_handle() {
                ctx.set_font(fh);
            }
            ctx.draw_text(icon_str, Point::new(px, py), text_sec, 12.0);
            ctx.set_font(saved);
        }

        if !self.suffix.is_empty() {
            let sx = inner_frame.x + inner_frame.w - suffix_w - right_extra + 4.0 + suffix_w;
            let sy = ctx.visual_center_y(inner_frame, 12.0);
            let icon_str = crate::ui::widgets::icon::icon_char(&self.suffix);
            let saved = *ctx.font();
            if let Some(fh) = crate::ui::widgets::icon::lucide_handle() {
                ctx.set_font(fh);
            }
            ctx.draw_text(icon_str, Point::new(sx, sy), text_sec, 12.0);
            ctx.set_font(saved);
        }

        let display_text = if self.value.is_empty() {
            &self.placeholder
        } else {
            &self.value
        };
        let disp_color = if self.value.is_empty() && !self.focused {
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
        ctx.canvas_2d().push_clip(text_area);

        let mut scroll_off = self.scroll_offset_x.get();
        let total_text_w = if !self.value.is_empty() {
            ctx.measure_text(&self.value, FONT_SIZE).w
        } else {
            0.0
        };

        let cursor_byte_pos = self
            .value
            .char_indices()
            .nth(self.cursor_char)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        let text_before = &self.value[..cursor_byte_pos];
        let text_before_w = if !text_before.is_empty() {
            ctx.measure_text(text_before, FONT_SIZE).w
        } else {
            0.0
        };

        let right_margin = 10.0;
        if text_before_w - scroll_off > text_area_w - right_margin {
            scroll_off = text_before_w - text_area_w + right_margin;
        }
        if text_before_w - scroll_off < 0.0 {
            scroll_off = text_before_w;
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
            let backend_opts = crate::draw::font::text_backend::TextLayoutOptions::from(opts);
            let fh = *ctx.font();
            let layout = ctx
                .font_service()
                .layout_text(&fh, display_text, &backend_opts);

            let abs_pos = Point::new(draw_x, draw_y);
            {
                let mut xs = self.glyph_xs.borrow_mut();
                xs.clear();
                for g in &layout.glyphs {
                    xs.push(g.x);
                }
            }
            if !self.value.is_empty() {
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
            ctx.blit_glyph_layout(&layout, abs_pos, disp_color, FONT_SIZE);
        }

        ctx.canvas_2d().pop_clip();

        if self.focused && self.selection.get().is_none() {
            let cursor_x = text_area_x + text_before_w - scroll_off;
            ctx.fill_rect(
                Rect::new(cursor_x, inner_frame.y + 4.0, 1.5, inner_frame.h - 8.0),
                primary,
                None,
            );
        }

        if self.clearable && !self.value.is_empty() && self.focused {
            let cx = inner_frame.x + inner_frame.w - 20.0;
            let cy = ctx.visual_center_y(inner_frame, 12.0);
            ctx.draw_text("✕", Point::new(cx, cy), text_sec, 12.0);
        }

        if self.password {
            let px = inner_frame.x + inner_frame.w - pwd_w;
            let py = ctx.visual_center_y(inner_frame, 12.0);
            ctx.draw_text(
                if self.password_visible { "◎" } else { "◉" },
                Point::new(px, py),
                text_sec,
                14.0,
            );
        }

        if self.search {
            let sx = inner_frame.x + inner_frame.w - search_w;
            let sy = ctx.visual_center_y(inner_frame, 12.0);
            ctx.draw_text("🔍", Point::new(sx, sy), text_sec, 12.0);
        }
    }
}
