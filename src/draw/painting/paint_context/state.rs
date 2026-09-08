//! 状态管理与文本绘制。

use super::*;

impl<'a> PaintContext<'a> {
    /// 平移后续绘制的逻辑坐标，并把状态变化写入 DisplayList。
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.record_op(PaintOp::Translate { dx, dy });
        self.spatial.canvas_2d().translate(dx, dy);
    }

    /// 当前仿射变换。
    pub fn current_transform(&mut self) -> Transform {
        self.spatial.canvas_2d().current_transform()
    }

    /// 设置后续绘制使用的仿射变换。
    pub fn set_transform(&mut self, transform: Transform) {
        self.record_op(PaintOp::SetTransform { transform });
        self.spatial.canvas_2d().set_transform(transform);
    }

    /// 将变换连接到当前仿射变换之后。
    pub fn concat_transform(&mut self, transform: Transform) {
        let combined = self.current_transform().concat(transform);
        self.set_transform(combined);
    }

    /// 设置后续绘制的绝对透明度。
    pub fn set_opacity(&mut self, opacity: f32) {
        self.record_op(PaintOp::SetOpacity { opacity });
        self.spatial.canvas_2d().set_opacity(opacity);
    }

    /// 在闭包内把当前透明度乘以 `opacity`，结束或 panic 展开时恢复原状态。
    pub fn with_opacity<R>(&mut self, opacity: f32, draw: impl FnOnce(&mut Self) -> R) -> R {
        let factor = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.save();
        let inherited = self.spatial.canvas_2d().opacity();
        self.set_opacity(inherited * factor);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| draw(self)));
        self.restore();
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// 设置后续绘制的混合模式。
    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        self.record_op(PaintOp::SetBlendMode { mode });
        self.spatial.canvas_2d().set_blend_mode(mode);
    }

    /// 保存渲染状态。
    #[inline(always)]
    pub fn save(&mut self) {
        self.record_op(PaintOp::Save);
        self.spatial.canvas_2d().save();
    }

    /// 恢复渲染状态。
    #[inline(always)]
    pub fn restore(&mut self) {
        self.record_op(PaintOp::Restore);
        self.spatial.canvas_2d().restore();
    }

    /// 推入局部裁剪，并保持 DisplayList 的状态栈顺序。
    pub fn push_clip(&mut self, rect: Rect) {
        self.record_op(PaintOp::PushClip { rect });
        self.spatial.canvas_2d().push_clip(rect);
    }

    /// 推入任意路径裁剪。
    pub fn push_clip_path(&mut self, path: &Path) {
        self.record_op(PaintOp::PushClipPath { path: path.clone() });
        self.spatial.canvas_2d().push_clip_path(path);
    }

    /// 弹出最近一次局部裁剪。
    pub fn pop_clip(&mut self) {
        self.record_op(PaintOp::PopClip);
        self.spatial.canvas_2d().pop_clip();
    }

    // ════════════════════════════════════════════════════════════════════
    // 文本绘制（委托给 TextRenderService）
    // ════════════════════════════════════════════════════════════════════

    /// 在 3D 空间中绘制文本。
    pub fn draw_text_spatial(
        &mut self,
        text: &str,
        pos: Vec3,
        color: Color,
        font_size: PhysicalUnit,
    ) {
        if text.is_empty() {
            return;
        }
        let (sx, sy) = self.spatial.project(&pos);
        let fs = font_size.to_dip(self.spatial.dpi());
        self.draw_text(text, Point::new(sx, sy), color, fs);
    }

    /// 在 3D 空间中的矩形区域内居中绘制文本。
    pub fn text_center_spatial(
        &mut self,
        text: &str,
        box_3d: AABB3D,
        color: Color,
        font_size: PhysicalUnit,
    ) {
        if text.is_empty() {
            return;
        }
        let fs = font_size.to_dip(self.spatial.dpi());
        let quad = self.spatial.project_aabb(&box_3d);
        let bounds = quad.bounds();
        let sz = self.text.measure_text(text, fs);
        let x = bounds.x + (bounds.w - sz.w) * 0.5;
        let y = bounds.y + (bounds.h - fs * 1.5) * 0.5;
        self.draw_text(text, Point::new(x, y), color, fs);
    }

    /// 绘制文本（左对齐，顶部对齐）。
    pub fn draw_text(&mut self, text: &str, pos: Point, color: Color, font_size: f32) {
        self.record_op_reusing(
            |op| match op {
                PaintOp::DrawText {
                    text: recorded_text,
                    pos: recorded_pos,
                    color: recorded_color,
                    font_size: recorded_size,
                } if recorded_text.as_ref() == text => {
                    *recorded_pos = pos;
                    *recorded_color = color;
                    *recorded_size = font_size;
                    true
                }
                _ => false,
            },
            || PaintOp::DrawText {
                text: Arc::from(text),
                pos,
                color,
                font_size,
            },
        );
        self.text
            .draw_text(self.spatial.canvas_2d(), text, pos, color, font_size);
    }

    /// 基于基线绘制文本。
    pub fn draw_text_baseline(
        &mut self,
        text: &str,
        x: f32,
        baseline_y: f32,
        color: Color,
        font_size: f32,
    ) {
        self.record_op_reusing(
            |op| match op {
                PaintOp::DrawTextBaseline {
                    text: recorded_text,
                    x: recorded_x,
                    baseline_y: recorded_baseline,
                    color: recorded_color,
                    font_size: recorded_size,
                } if recorded_text.as_ref() == text => {
                    *recorded_x = x;
                    *recorded_baseline = baseline_y;
                    *recorded_color = color;
                    *recorded_size = font_size;
                    true
                }
                _ => false,
            },
            || PaintOp::DrawTextBaseline {
                text: Arc::from(text),
                x,
                baseline_y,
                color,
                font_size,
            },
        );
        self.text.draw_text_baseline(
            self.spatial.canvas_2d(),
            text,
            x,
            baseline_y,
            color,
            font_size,
        );
    }

    /// 在矩形内居中绘制文本。
    pub fn text_center(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        self.record_op_reusing(
            |op| match op {
                PaintOp::TextCenter {
                    text: recorded_text,
                    rect: recorded_rect,
                    color: recorded_color,
                    font_size: recorded_size,
                } if recorded_text.as_ref() == text => {
                    *recorded_rect = rect;
                    *recorded_color = color;
                    *recorded_size = font_size;
                    true
                }
                _ => false,
            },
            || PaintOp::TextCenter {
                text: Arc::from(text),
                rect,
                color,
                font_size,
            },
        );
        self.text
            .text_center(self.spatial.canvas_2d(), text, rect, color, font_size);
    }

    /// 左对齐、垂直居中的文本绘制。
    pub fn draw_text_in_frame(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        if !text.is_empty() {
            self.record_op_reusing(
                |op| match op {
                    PaintOp::DrawTextInFrame {
                        text: recorded_text,
                        rect: recorded_rect,
                        color: recorded_color,
                        font_size: recorded_size,
                    } if recorded_text.as_ref() == text => {
                        *recorded_rect = rect;
                        *recorded_color = color;
                        *recorded_size = font_size;
                        true
                    }
                    _ => false,
                },
                || PaintOp::DrawTextInFrame {
                    text: Arc::from(text),
                    rect,
                    color,
                    font_size,
                },
            );
        }
        self.text
            .draw_text_in_frame(self.spatial.canvas_2d(), text, rect, color, font_size);
    }

    /// 在矩形内绘制自动换行文本。
    pub fn draw_text_wrapped(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        self.record_op_reusing(
            |op| match op {
                PaintOp::DrawTextWrapped {
                    text: recorded_text,
                    rect: recorded_rect,
                    color: recorded_color,
                    font_size: recorded_size,
                } if recorded_text.as_ref() == text => {
                    *recorded_rect = rect;
                    *recorded_color = color;
                    *recorded_size = font_size;
                    true
                }
                _ => false,
            },
            || PaintOp::DrawTextWrapped {
                text: Arc::from(text),
                rect,
                color,
                font_size,
            },
        );
        self.text
            .draw_text_wrapped(self.spatial.canvas_2d(), text, rect, color, font_size);
    }

    /// 仅填充文本选择背景，并把可重放的逻辑选择写入显示列表。
    #[allow(
        clippy::too_many_arguments,
        reason = "selection color and bounds are part of the text rendering contract"
    )]
    pub fn fill_text_selection(
        &mut self,
        text: &str,
        font_size: f32,
        pos: Point,
        start: usize,
        end: usize,
        color: Color,
    ) {
        // 稳态重录优先复用同一文字载荷，只更新选择与视觉参数。
        self.record_op_reusing(
            |op| match op {
                PaintOp::FillTextSelection {
                    text: recorded_text,
                    font_size: recorded_size,
                    pos: recorded_pos,
                    start: recorded_start,
                    end: recorded_end,
                    color: recorded_color,
                } if recorded_text.as_ref() == text => {
                    *recorded_size = font_size;
                    *recorded_pos = pos;
                    *recorded_start = start;
                    *recorded_end = end;
                    *recorded_color = color;
                    true
                }
                _ => false,
            },
            || PaintOp::FillTextSelection {
                text: Arc::from(text),
                font_size,
                pos,
                start,
                end,
                color,
            },
        );
        // 直接绘制时流式消费选区几何，不建立临时矩形数组。
        self.text.fill_text_selection(
            self.spatial.canvas_2d(),
            text,
            font_size,
            pos,
            start,
            end,
            color,
        );
    }

    /// 绘制文本选中背景 + 文本。
    pub fn draw_text_with_selection(
        &mut self,
        text: &str,
        pos: Point,
        color: Color,
        font_size: f32,
        selection: Option<(usize, usize)>,
        selection_bg: Color,
    ) {
        self.record_op_reusing(
            |op| match op {
                PaintOp::DrawTextWithSelection {
                    text: recorded_text,
                    pos: recorded_pos,
                    color: recorded_color,
                    font_size: recorded_size,
                    selection: recorded_selection,
                    selection_bg: recorded_selection_bg,
                } if recorded_text.as_ref() == text => {
                    *recorded_pos = pos;
                    *recorded_color = color;
                    *recorded_size = font_size;
                    *recorded_selection = selection;
                    *recorded_selection_bg = selection_bg;
                    true
                }
                _ => false,
            },
            || PaintOp::DrawTextWithSelection {
                text: Arc::from(text),
                pos,
                color,
                font_size,
                selection,
                selection_bg,
            },
        );
        self.text.draw_text_with_selection(
            self.spatial.canvas_2d(),
            text,
            pos,
            color,
            font_size,
            selection,
            selection_bg,
        );
    }

    /// 获取选中文本的矩形区域列表。
    pub fn selection_rects(
        &mut self,
        text: &str,
        font_size: f32,
        pos: Point,
        start: usize,
        end: usize,
    ) -> Vec<Rect> {
        self.text
            .selection_rects(self.spatial.canvas_2d(), text, font_size, pos, start, end)
    }

    /// 测量文本尺寸（不换行）。
    pub fn measure_text(&mut self, text: &str, font_size: f32) -> Size {
        self.text.measure_text(text, font_size)
    }

    /// Default line box height, shared by layout and `draw_text`.
    pub fn line_box_height(&mut self, font_size: f32) -> f32 {
        self.text.line_box_height(font_size)
    }

    /// 测量文本尺寸（换行模式）。
    pub fn measure_text_wrapped(&mut self, text: &str, font_size: f32, max_width: f32) -> Size {
        self.text.measure_text_wrapped(text, font_size, max_width)
    }

    /// 文本命中测试。
    pub fn text_hit_test(&mut self, text: &str, font_size: f32, point: Point) -> Option<usize> {
        self.text.text_hit_test(text, font_size, point)
    }

    /// 获取指定字符的光标 x 位置。
    pub fn text_cursor_x(&mut self, text: &str, font_size: f32, char_index: usize) -> f32 {
        self.text.text_cursor_x(text, font_size, char_index)
    }

    /// 行盒在 rect 内几何居中时的 layout 原点 y（过渡；优先布局算盒再 draw_text）。
    pub fn visual_center_y(&mut self, rect: Rect, font_size: f32) -> f32 {
        self.text.visual_center_y(rect, font_size)
    }

    /// 设置文本绘制最大宽度。
    pub fn set_max_text_width(&mut self, width: f32) {
        self.text.set_max_text_width(width);
    }

    /// 将 glyph layout 绘制到引擎上。
    pub fn blit_glyph_layout(
        &mut self,
        layout: &crate::draw::resources::font::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        self.text
            .blit_to(self.spatial.canvas_2d(), layout, pos, color, font_size);
        self.record_op_lazy(|| PaintOp::BlitGlyphLayout {
            layout: Arc::new(layout.clone()),
            pos,
            color,
            font_size,
        });
    }

    /// 绘制并录制共享 glyph layout；缓存命中与 DisplayList 只增加引用计数。
    pub(crate) fn blit_shared_glyph_layout(
        &mut self,
        layout: Arc<crate::draw::resources::font::text_backend::TextLayout>,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        self.text
            .blit_to(self.spatial.canvas_2d(), &layout, pos, color, font_size);
        self.record_op_lazy(|| PaintOp::BlitGlyphLayout {
            layout,
            pos,
            color,
            font_size,
        });
    }

    // ── 访问器 ──

    /// 获取字体服务。
    #[inline(always)]
    pub fn font_service(&mut self) -> &FontService {
        self.text.font_service
    }

    /// 获取图片服务。
    #[inline(always)]
    pub fn image_service(&self) -> &ImageService {
        self.image_service
    }

    /// 绘制解码位图（按目标区域 fit 居中）。
    pub fn draw_image(&mut self, handle: BitmapHandle, bounds: Rect) {
        self.blit_bitmap(handle, bounds, true);
    }

    /// 绘制解码位图（拉伸填满 bounds）。
    pub fn draw_image_fill(&mut self, handle: BitmapHandle, bounds: Rect) {
        self.blit_bitmap(handle, bounds, false);
    }

    fn blit_bitmap(&mut self, handle: BitmapHandle, bounds: Rect, fit: bool) {
        self.record_op(PaintOp::DrawImage {
            handle,
            bounds,
            fit,
        });
        blit_handle(
            self.image_service,
            self.spatial.canvas_2d(),
            handle,
            bounds,
            fit,
        );
    }

    /// 获取当前字体句柄。
    #[inline(always)]
    pub fn font(&self) -> &FontHandle {
        self.text.font()
    }

    /// 设置字体句柄。
    #[inline(always)]
    pub fn set_font(&mut self, font: FontHandle) {
        self.record_op(PaintOp::SetFont { font });
        self.text.set_font(font);
    }

    /// 设置调试模式。
    #[inline(always)]
    pub fn set_debug_mode(&mut self, mode: bool) {
        self.debug.set_debug_mode(mode);
    }

    /// 是否处于调试模式。
    #[inline(always)]
    pub fn debug_mode(&self) -> bool {
        self.debug.debug_mode
    }

    // ════════════════════════════════════════════════════════════════════
    // 调试绘制（委托给 DebugRenderService）
    // ════════════════════════════════════════════════════════════════════

    /// 绘制节点调试边框。
    pub fn draw_debug_border(&mut self, rect: Rect, depth: usize, hovered: bool) {
        if !self.debug.debug_mode || !hovered {
            return;
        }
        let color =
            DebugRenderService::DEBUG_COLORS[depth % DebugRenderService::DEBUG_COLORS.len()];
        self.stroke_rect(rect, color, 1.5, None);
    }

    /// 在节点左上角显示调试标签。
    pub fn draw_debug_label(&mut self, node_slot: usize, depth: usize, rect: Rect) {
        if !self.debug.debug_mode {
            return;
        }
        let color =
            DebugRenderService::DEBUG_COLORS[depth % DebugRenderService::DEBUG_COLORS.len()];
        let label = format!("#{} d{}", node_slot, depth);
        let font_size = 12.0;
        let label_w = label.len() as f32 * 7.0 + 6.0;
        let label_h = 16.0;
        self.fill_rect(
            Rect::new(rect.x, rect.y, label_w, label_h),
            Color::from_rgba(0, 0, 0, 180),
            None,
        );
        self.draw_text(
            &label,
            Point::new(rect.x + 2.0, rect.y + font_size * 0.75),
            color,
            font_size,
        );
    }

    /// 在 widget 下方显示 frame 坐标和尺寸。
    pub fn draw_debug_frame_info(&mut self, node_slot: usize, rect: Rect) {
        if !self.debug.debug_mode {
            return;
        }
        let info = format!(
            "#{} ({:.0},{:.0}) {:.0}×{:.0}",
            node_slot, rect.x, rect.y, rect.w, rect.h
        );
        let font_size = 11.0;
        let info_w = info.len() as f32 * 6.5 + 6.0;
        let info_h = 15.0;
        let info_y = rect.y + rect.h;
        self.fill_rect(
            Rect::new(rect.x, info_y, info_w, info_h),
            Color::from_rgba(0, 0, 0, 160),
            None,
        );
        self.draw_text(
            &info,
            Point::new(rect.x + 2.0, info_y + font_size * 0.75),
            Color::from_rgba(200, 200, 200, 220),
            font_size,
        );
    }

    /// 在悬停节点附近绘制受表面边界约束的多行检查器面板。
    pub fn draw_debug_inspector_lines(
        &mut self,
        anchor: Rect,
        lines: &[String],
        surface_w: i32,
        surface_h: i32,
    ) {
        if !self.debug.debug_mode || lines.is_empty() || surface_w <= 24 || surface_h <= 24 {
            return;
        }
        const MARGIN: f32 = 6.0;
        const GAP: f32 = 8.0;
        const PAD_X: f32 = 8.0;
        const PAD_Y: f32 = 6.0;
        const LINE_H: f32 = 14.0;
        const FONT_SIZE: f32 = 11.0;
        let available_w = (surface_w as f32 - MARGIN * 2.0).max(1.0);
        let available_h = (surface_h as f32 - MARGIN * 2.0).max(1.0);
        let longest = lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(1);
        let panel_w = (longest as f32 * 6.2 + PAD_X * 2.0)
            .max(220.0)
            .min(available_w);
        let max_lines = ((available_h - PAD_Y * 2.0) / LINE_H).floor().max(1.0) as usize;
        let visible_lines = lines.len().min(max_lines);
        let panel_h = visible_lines as f32 * LINE_H + PAD_Y * 2.0;
        let max_x = (surface_w as f32 - panel_w - MARGIN).max(MARGIN);
        let preferred_x = anchor.x + anchor.w + GAP;
        let x = if preferred_x + panel_w <= surface_w as f32 - MARGIN {
            preferred_x
        } else {
            anchor.x - panel_w - GAP
        }
        .clamp(MARGIN, max_x);
        let max_y = (surface_h as f32 - panel_h - MARGIN).max(MARGIN);
        let y = anchor.y.clamp(MARGIN, max_y);
        self.fill_rect(
            Rect::new(x, y, panel_w, panel_h),
            Color::from_rgba(16, 18, 24, 232),
            None,
        );
        self.stroke_rect(
            Rect::new(x, y, panel_w, panel_h),
            Color::from_rgba(80, 180, 255, 220),
            1.0,
            None,
        );
        let max_chars = ((panel_w - PAD_X * 2.0) / 6.2).floor().max(1.0) as usize;
        for (index, line) in lines.iter().take(visible_lines).enumerate() {
            let line = truncate_debug_line(line, max_chars);
            let color = if index == 0 {
                Color::from_rgba(110, 205, 255, 255)
            } else {
                Color::from_rgba(226, 230, 238, 245)
            };
            self.draw_text(
                &line,
                Point::new(
                    x + PAD_X,
                    y + PAD_Y + index as f32 * LINE_H + FONT_SIZE * 0.8,
                ),
                color,
                FONT_SIZE,
            );
        }
    }
}

// 按 Unicode 字符而非 UTF-8 字节截断面板文本，避免产生非法边界。
fn truncate_debug_line(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        return line.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let mut truncated = line.chars().take(max_chars - 1).collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::FontHandle;
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::geometry::spatial::Orientation;
    use crate::draw::painting::{DisplayList, PaintSurfaceConfig};
    use crate::draw::resources::{FontService, ImageService};

    fn record_text_variants(
        ctx: &mut PaintContext<'_>,
        list: &mut DisplayList,
        text: &str,
        x: f32,
    ) {
        list.begin_rewrite();
        ctx.with_recorder(list, |ctx| {
            ctx.draw_text(text, Point::new(x, 1.0), Color::black(), 12.0);
            ctx.draw_text_baseline(text, x, 14.0, Color::red(), 13.0);
            ctx.text_center(text, Rect::new(x, 2.0, 30.0, 12.0), Color::green(), 14.0);
            ctx.draw_text_in_frame(text, Rect::new(x, 3.0, 30.0, 12.0), Color::blue(), 15.0);
            ctx.draw_text_wrapped(text, Rect::new(x, 4.0, 30.0, 24.0), Color::white(), 16.0);
            ctx.fill_text_selection(text, 16.5, Point::new(x, 4.5), 1, 4, Color::green());
            ctx.draw_text_with_selection(
                text,
                Point::new(x, 5.0),
                Color::black(),
                17.0,
                None,
                Color::blue(),
            );
        });
        list.finish_rewrite();
    }

    fn recorded_texts(list: &DisplayList) -> Vec<Arc<str>> {
        list.ops()
            .iter()
            .map(|op| match op {
                PaintOp::DrawText { text, .. }
                | PaintOp::DrawTextBaseline { text, .. }
                | PaintOp::TextCenter { text, .. }
                | PaintOp::DrawTextInFrame { text, .. }
                | PaintOp::DrawTextWrapped { text, .. }
                | PaintOp::FillTextSelection { text, .. }
                | PaintOp::DrawTextWithSelection { text, .. } => Arc::clone(text),
                other => panic!("只应录制文字操作，实际为 {other:?}"),
            })
            .collect()
    }

    // 七类文字操作都应在稳定内容下复用 Arc，并在内容变化时正确替换。
    #[test]
    fn text_variants_reuse_stable_content_and_replace_changed_content() {
        let mut canvas = NoopCanvas2D;
        let font_service = FontService::new();
        let image_service = ImageService::new();
        let mut ctx = PaintContext::new(
            &mut canvas,
            FontHandle::new(0),
            &font_service,
            &image_service,
            PaintSurfaceConfig {
                dpi: 96.0,
                device_pixel_ratio: 1.0,
                orientation: Orientation::YDown,
                surface_w: 64,
                surface_h: 64,
            },
        );
        let mut list = DisplayList::new();

        record_text_variants(&mut ctx, &mut list, "steady", 1.0);
        let first_texts = recorded_texts(&list);
        record_text_variants(&mut ctx, &mut list, "steady", 2.0);
        let second_texts = recorded_texts(&list);
        assert_eq!(first_texts.len(), 7);
        assert!(
            first_texts
                .iter()
                .zip(&second_texts)
                .all(|(first, second)| Arc::ptr_eq(first, second))
        );
        let PaintOp::DrawText { pos, .. } = &list.ops()[0] else {
            panic!("首条操作应为普通文字");
        };
        assert_eq!(*pos, Point::new(2.0, 1.0));

        record_text_variants(&mut ctx, &mut list, "changed", 3.0);
        let changed_texts = recorded_texts(&list);
        assert!(changed_texts.iter().all(|text| text.as_ref() == "changed"));
        assert!(
            second_texts
                .iter()
                .zip(&changed_texts)
                .all(|(steady, changed)| !Arc::ptr_eq(steady, changed))
        );
    }
}
