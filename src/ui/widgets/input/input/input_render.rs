//! Input 组件渲染实现（单行 / 多行）

use super::*;

use crate::core::{Point, Rect};
use crate::draw::{Color, Radius};
use crate::ui::widget_runtime::paint_context::PaintContext;
// ════════════════════════════════════════════════════════════════════════════
// 多行渲染
// ════════════════════════════════════════════════════════════════════════════

impl Input {
    fn status_color(&self, visual: ResolvedInputVisual) -> Option<Color> {
        self.status.map(|status| match status {
            InputStatus::Success => visual.success,
            InputStatus::Warning => visual.warning,
            InputStatus::Error => visual.error,
        })
    }

    pub(super) fn render_status_message(
        &self,
        frame: Rect,
        control_height: f32,
        ctx: &mut PaintContext,
        visual: ResolvedInputVisual,
    ) {
        if self.status_message.is_empty() || frame.h <= control_height {
            return;
        }
        let message_frame = Rect::new(
            frame.x,
            frame.y + control_height,
            frame.w,
            (frame.h - control_height).min(self.visual.chrome.status_message_height),
        );
        ctx.push_clip(message_frame);
        let color = self.status_color(visual).unwrap_or(visual.text_secondary);
        let font_size = self.visual.typography.status_font_size;
        let y = ctx.visual_center_y(message_frame, font_size);
        ctx.draw_text(
            &self.status_message,
            Point::new(message_frame.x, y),
            color,
            font_size,
        );
        ctx.pop_clip();
    }

    pub(super) fn render_textarea(
        &self,
        frame: Rect,
        ctx: &mut PaintContext,
        visual: ResolvedInputVisual,
    ) {
        let layout = self.visual.layout;
        let typography = self.visual.typography;

        let inner_frame = Rect::new(frame.x, frame.y, frame.w, frame.h);

        let status_border = self.status_color(visual);
        let (bg, border) = if self.disabled {
            (visual.fill_tertiary, visual.border)
        } else if let Some(status_border) = status_border {
            (visual.background_elevated, status_border)
        } else if self.focused {
            (visual.background_elevated, visual.primary)
        } else if self.hovered {
            (visual.background_elevated, visual.primary_hover)
        } else {
            (visual.background, visual.border)
        };

        let radius = Some(Radius::uniform(visual.radius));
        ctx.fill_rect(inner_frame, bg, radius);
        ctx.stroke_rect(
            inner_frame,
            border,
            if self.focused {
                self.visual.chrome.focus_border_width
            } else {
                self.visual.chrome.border_width
            },
            radius,
        );

        let text_area = Rect::new(
            inner_frame.x + layout.horizontal_padding,
            inner_frame.y + layout.textarea_content_vertical_inset,
            (inner_frame.w - layout.horizontal_padding * 2.0)
                .max(layout.textarea_content_min_width),
            (inner_frame.h - layout.textarea_content_vertical_inset * 2.0)
                .max(layout.textarea_content_min_height),
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
            visual.text_tertiary
        } else {
            visual.text
        };

        // 仅统计一次显示行数供字形槽位初始化；逐行内容直接借用 split 迭代器。
        let line_count = display_text.split('\n').count();

        // 计算光标所在行
        let cursor_line = self.cursor_line_col().0;

        // 垂直滚动：确保光标行可见
        let vis_lines = ((text_area.h / typography.line_height) as usize).max(1);
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
        let line_h = typography.line_height;
        let mut y = text_area.y;
        // 刷新多行方向感知命中使用的真实 shaping 字形。
        let mut line_glyphs = self.line_glyphs.borrow_mut();
        // 清除上一帧逐行字形缓存。
        line_glyphs.clear();
        // 为每个逻辑行准备独立视觉字形数组。
        line_glyphs.resize_with(line_count, Vec::new);
        // 值行游标先线性越过滚动区域，后续每个显示行只推进一次。
        let mut value_cursor = LogicalLineCursor::new(&self.value);
        value_cursor.skip_lines(adj_scroll);
        for (li, line) in display_text.split('\n').enumerate().skip(adj_scroll) {
            if y >= text_area.y + text_area.h {
                break;
            }
            // 一次推进即可得到该行在原始值中的 Unicode 字符范围。
            let (value_line, line_start, line_end) = value_cursor.next_line();

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
                        // 使用共享 shaping、双向与字素簇选择几何生成视觉片段。
                        let selection_rects = ctx.selection_rects(
                            // 选择几何使用未注入 composition 的真实值行。
                            value_line,
                            // 使用输入控件字体大小。
                            typography.font_size,
                            // 把行内几何平移到文本区域。
                            Point::new(text_area.x, y),
                            // 传入行内合法选择起点。
                            sel_in_line_start,
                            // 传入行内合法选择终点。
                            sel_in_line_end,
                        );
                        // 双向文本可能产生多个不连续视觉选择片段。
                        for selection_rect in selection_rects {
                            // 绘制当前视觉选择片段。
                            ctx.fill_rect(
                                // 保留共享排版计算出的几何。
                                selection_rect,
                                // 使用主题选择背景色。
                                visual.primary.with_alpha(
                                    self.visual.chrome.selection_alpha.min(u8::MAX as u32) as u8,
                                ),
                                // 选择背景不使用圆角。
                                None,
                            );
                        }
                    }
                }
            }

            // 行内光学居中：用 visual_center_y，去掉魔法 +2.0
            let text_y = ctx.visual_center_y(
                Rect::new(text_area.x, y, text_area.w, line_h),
                typography.font_size,
            );
            ctx.draw_text(
                line,
                Point::new(text_area.x, text_y),
                disp_color,
                typography.font_size,
            );

            // 收集该行每个字符的 x 坐标（用于 char_at_xy 命中）
            let hit_text = if showing_placeholder { "" } else { line };
            // 构造与绘制一致的单行文本布局选项。
            let hit_options = crate::draw::TextLayoutOptions {
                // 多行控件按逻辑换行拆分后不再限制单行宽度。
                max_width: f32::MAX,
                // 命中布局不限制高度。
                max_height: 0.0,
                // 保持输入控件现有行高。
                line_height: typography.line_height,
                // 当前逻辑行禁止再次自动换行。
                word_wrap: false,
                // 使用左侧行盒对齐并由 UAX #9 决定 run 视觉顺序。
                h_align: crate::draw::HAlign::Left,
                // 使用顶部行盒对齐。
                v_align: crate::draw::VAlign::Top,
                // 使用输入控件字体大小。
                font_size: typography.font_size,
            };
            // 转换为字体后端布局选项。
            let backend_options =
                crate::draw::resources::font::text_backend::TextLayoutOptions::from(hit_options);
            // 读取当前绘制字体句柄。
            let font = *ctx.font();
            // 执行真实 shaping 与双向视觉重排。
            let hit_layout = ctx
                // 借用字体服务。
                .font_service()
                // 布局当前显示逻辑行。
                .layout_text(&font, hit_text, &backend_options);
            // 保存视觉顺序字形及其逻辑 cluster 范围。
            line_glyphs[li] = hit_layout.glyphs;

            if li == cursor_line {
                let col = self.cursor_line_col().1;
                let composition_chars = if has_composition {
                    self.composition.chars().count()
                } else {
                    0
                };
                // 使用方向感知光标几何定位当前合法字符边界。
                let cx = text_area.x
                    + ctx.text_cursor_x(line, typography.font_size, col + composition_chars);
                let caret_h = (line_h - layout.caret_vertical_inset)
                    .max(typography.font_size * layout.caret_min_font_ratio);
                let caret_y = y + (line_h - caret_h) * 0.5;
                self.caret_rect
                    .set(Rect::new(cx, caret_y, layout.caret_width, caret_h));

                if has_composition {
                    // composition 起点同样使用方向感知字符边界几何。
                    let composition_x =
                        text_area.x + ctx.text_cursor_x(line, typography.font_size, col);
                    let composition_w = ctx.measure_text(&self.composition, typography.font_size).w;
                    ctx.fill_rect(
                        Rect::new(
                            composition_x,
                            y + line_h - layout.composition_bottom_inset_textarea,
                            composition_w.max(layout.composition_min_width),
                            layout.composition_underline_height,
                        ),
                        visual.primary,
                        None,
                    );
                }

                if self.focused && self.selection.get().is_none() {
                    ctx.fill_rect(
                        Rect::new(cx, caret_y, layout.caret_width, caret_h),
                        visual.primary,
                        None,
                    );
                }
            }

            y += line_h;
        }

        // 如果没有任何行且 focused，在顶部画光标
        if self.focused && line_count == 0 {
            let caret_h = (line_h - layout.caret_vertical_inset)
                .max(typography.font_size * layout.caret_min_font_ratio);
            let caret_y = text_area.y + (line_h - caret_h) * 0.5;
            let caret = Rect::new(text_area.x, caret_y, layout.caret_width, caret_h);
            self.caret_rect.set(caret);
            ctx.fill_rect(caret, visual.primary, None);
        }

        ctx.pop_clip();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 单行渲染（原逻辑精简）
// ════════════════════════════════════════════════════════════════════════════

impl Input {
    pub(super) fn render_singleline(
        &self,
        frame: Rect,
        ctx: &mut PaintContext,
        visual: ResolvedInputVisual,
    ) {
        let layout = self.visual.layout;
        let typography = self.visual.typography;
        let h = input_height(self.input_size).min(frame.h);
        let input_frame = Rect::new(frame.x, frame.y, frame.w, h);

        let addon_left_w = addon_width(&self.addon_before, self.visual);
        let addon_right_w = addon_width(&self.addon_after, self.visual);

        if !self.addon_before.is_empty() {
            let addon_rect = Rect::new(input_frame.x, input_frame.y, addon_left_w, input_frame.h);
            ctx.fill_rect(
                addon_rect,
                visual.fill_tertiary,
                Some(Radius::uniform(visual.radius)),
            );
            let ay = ctx.visual_center_y(addon_rect, typography.addon_font_size);
            ctx.draw_text(
                &self.addon_before,
                Point::new(addon_rect.x + layout.addon_text_inset, ay),
                visual.text_secondary,
                typography.addon_font_size,
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
                visual.fill_tertiary,
                Some(Radius::uniform(visual.radius)),
            );
            let ay = ctx.visual_center_y(addon_rect, typography.addon_font_size);
            ctx.draw_text(
                &self.addon_after,
                Point::new(addon_rect.x + layout.addon_text_inset, ay),
                visual.text_secondary,
                typography.addon_font_size,
            );
        }

        let inner_frame = Rect::new(
            input_frame.x + addon_left_w,
            input_frame.y,
            input_frame.w - addon_left_w - addon_right_w,
            input_frame.h,
        );

        let prefix_w = if self.prefix.is_empty() {
            0.0
        } else {
            layout.prefix_width
        };
        let suffix_w = if self.suffix.is_empty() {
            0.0
        } else {
            layout.suffix_width
        };
        let clear_w = if self.clearable {
            layout.clear_width
        } else {
            0.0
        };
        let pwd_w = if self.password {
            layout.password_width
        } else {
            0.0
        };
        let search_w = if self.search {
            layout.search_width
        } else {
            0.0
        };
        let right_extra = suffix_w + clear_w + pwd_w + search_w;

        let status_border = self.status_color(visual);
        let (bg, border, text_color) = if self.disabled {
            (visual.fill_tertiary, visual.border, visual.text_quaternary)
        } else if let Some(status_border) = status_border {
            (visual.background_elevated, status_border, visual.text)
        } else if self.focused {
            (visual.background_elevated, visual.primary, visual.text)
        } else if self.hovered {
            (
                visual.background_elevated,
                visual.primary_hover,
                visual.text,
            )
        } else {
            (visual.background, visual.border, visual.text)
        };

        let radius = Some(Radius::uniform(visual.radius));
        ctx.fill_rect(inner_frame, bg, radius);
        ctx.stroke_rect(
            inner_frame,
            border,
            if self.focused {
                self.visual.chrome.focus_border_width
            } else {
                self.visual.chrome.border_width
            },
            radius,
        );

        if !self.prefix.is_empty() {
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                &self.prefix,
                Rect::new(
                    inner_frame.x + layout.prefix_icon_inset,
                    inner_frame.y,
                    prefix_w,
                    inner_frame.h,
                ),
                visual.text_secondary,
                typography.prefix_icon_size,
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
            // 输入框内图标字号：统一使用主题 font_size_sm token。
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                &self.suffix,
                rect,
                visual.text_secondary,
                visual.accessory_icon_size,
            );
        }

        let has_composition = !self.composition.is_empty();
        let showing_placeholder = self.value.is_empty() && !has_composition;
        let display_text = if showing_placeholder {
            Cow::Borrowed(self.placeholder.as_str())
        } else {
            self.display_value_with_composition()
        };
        let disp_color = if showing_placeholder {
            visual.text_tertiary
        } else {
            text_color
        };

        let text_area_x = inner_frame.x + layout.horizontal_padding + prefix_w;
        let text_area_w =
            (inner_frame.w - layout.horizontal_padding * 2.0 - prefix_w - right_extra)
                .max(layout.singleline_text_min_width);
        if text_area_w <= 0.0 {
            return;
        }
        let text_area = Rect::new(text_area_x, inner_frame.y, text_area_w, inner_frame.h);
        ctx.push_clip(text_area);

        let mut scroll_off = self.scroll_offset_x.get();
        let total_text_w = if !display_text.is_empty() {
            ctx.measure_text(&display_text, typography.font_size).w
        } else {
            0.0
        };

        let text_before = self.visual_text_before_cursor();
        let text_before_w = if !text_before.is_empty() {
            ctx.measure_text(&text_before, typography.font_size).w
        } else {
            0.0
        };
        let composition_w = if has_composition {
            ctx.measure_text(&self.composition, typography.font_size).w
        } else {
            0.0
        };
        // composition 会在显示文本中占据额外字符位置。
        let display_cursor = self.cursor_char + self.composition.chars().count();
        // 使用共享 UAX #9 与字素簇光标几何替代逻辑前缀宽度。
        let caret_text_w = if display_text.is_empty() {
            // 空文本光标停在行起点。
            0.0
        } else {
            // 查询方向感知主光标水平坐标。
            ctx.text_cursor_x(&display_text, typography.font_size, display_cursor)
        };

        let right_margin = layout.scroll_right_margin;
        if caret_text_w - scroll_off > text_area_w - right_margin {
            scroll_off = caret_text_w - text_area_w + right_margin;
        }
        if caret_text_w - scroll_off < 0.0 {
            scroll_off = caret_text_w;
        }
        scroll_off = scroll_off
            .min(total_text_w - layout.scroll_end_guard)
            .max(0.0);
        self.scroll_offset_x.set(scroll_off);

        let draw_x = text_area_x - scroll_off;
        let draw_y = ctx.visual_center_y(text_area, typography.font_size);

        if !display_text.is_empty() {
            let opts = crate::draw::TextLayoutOptions {
                max_width: f32::MAX,
                max_height: 0.0,
                line_height: typography.font_size * typography.layout_line_height_ratio,
                word_wrap: false,
                h_align: crate::draw::HAlign::Left,
                v_align: crate::draw::VAlign::Top,
                font_size: typography.font_size,
            };
            let backend_opts =
                crate::draw::resources::font::text_backend::TextLayoutOptions::from(opts);
            let fh = *ctx.font();
            let layout = ctx
                .font_service()
                .layout_text(&fh, &display_text, &backend_opts);

            let abs_pos = Point::new(draw_x, draw_y);
            {
                // 刷新单行方向感知命中使用的真实 shaping 字形。
                let mut glyphs = self.glyphs.borrow_mut();
                // 清除上一帧字形缓存。
                glyphs.clear();
                if !showing_placeholder {
                    // 保存布局中的视觉顺序字形和逻辑 cluster 范围。
                    for g in &layout.glyphs {
                        // 字形为可复制的小型布局记录。
                        glyphs.push(*g);
                    }
                }
            }
            // 单行模式不保留多行字形缓存。
            self.line_glyphs.borrow_mut().clear();
            if !self.value.is_empty() && !has_composition {
                if let Some((sel_s, sel_e)) = self.selection.get() {
                    if sel_s < sel_e {
                        let visual_h = ctx
                            .font_service()
                            .horizontal_line_metrics(&fh, typography.font_size)
                            .map(|m| m.ascent + m.descent)
                            .unwrap_or(
                                typography.font_size * typography.fallback_line_height_ratio,
                            );
                        for line in &layout.lines {
                            let gs = line.glyph_start;
                            let gc = line.glyph_count;
                            let ge = gs + gc;
                            // 取得当前视觉行的完整字形范围。
                            let line_glyphs = &layout.glyphs[gs..ge.min(layout.glyphs.len())];
                            // 双向选择可能形成多个不连续视觉片段。
                            let ranges = crate::draw::resources::font::text_backend::glyph_selection_x_ranges(
                                // 传入视觉行字形。
                                line_glyphs,
                                // 传入已归一选择起点。
                                sel_s,
                                // 传入已归一选择终点。
                                sel_e,
                            );
                            // 分别绘制每个连续视觉选择片段。
                            for (line_x0, line_x1) in ranges {
                                // 把行内片段平移到控件绘制坐标。
                                ctx.fill_rect(
                                    // 构造当前视觉片段矩形。
                                    Rect::new(
                                        // 平移片段左边界。
                                        abs_pos.x + line_x0,
                                        // 平移当前行垂直坐标。
                                        abs_pos.y + line.y,
                                        // 使用非负片段宽度。
                                        (line_x1 - line_x0).max(0.0),
                                        // 使用字体视觉行高。
                                        visual_h,
                                    ),
                                    // 使用主题选择背景色。
                                    visual.primary.with_alpha(
                                        self.visual.chrome.selection_alpha.min(u8::MAX as u32)
                                            as u8,
                                    ),
                                    // 选择背景不使用圆角。
                                    None,
                                );
                            }
                        }
                    }
                }
            }
            ctx.blit_owned_glyph_layout(layout, abs_pos, disp_color, typography.font_size);
        }

        if has_composition {
            let composition_x = text_area_x + text_before_w - scroll_off;
            ctx.fill_rect(
                Rect::new(
                    composition_x,
                    inner_frame.y + inner_frame.h - layout.composition_bottom_inset_singleline,
                    composition_w.max(layout.composition_min_width),
                    layout.composition_underline_height,
                ),
                visual.primary,
                None,
            );
        }

        ctx.pop_clip();

        let cursor_x = text_area_x + caret_text_w - scroll_off;
        let caret = Rect::new(
            cursor_x,
            inner_frame.y + layout.caret_vertical_inset,
            layout.caret_width,
            inner_frame.h - layout.caret_vertical_inset * 2.0,
        );
        self.caret_rect.set(caret);
        if self.focused && self.selection.get().is_none() {
            ctx.fill_rect(caret, visual.primary, None);
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
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.icons.clear,
                rect,
                visual.text_secondary,
                visual.accessory_icon_size,
            );
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
                    self.visual.icons.password_visible
                } else {
                    self.visual.icons.password_hidden
                },
                rect,
                visual.text_secondary,
                typography.password_icon_size,
            );
        } else {
            self.pwd_icon_rect.set(Rect::zero());
        }

        if let Some(rect) = search_rect {
            // 输入框内图标字号：统一使用主题 font_size_sm token。
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.icons.search,
                rect,
                visual.text_secondary,
                visual.accessory_icon_size,
            );
        }
    }
}
