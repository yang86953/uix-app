//! DebugRenderService — 最终顶层调试绘制服务。

use std::time::Duration;

use crate::core::{Point, Rect};
use crate::draw::renderer::{InvalidationSource, RenderMetrics};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::font::text::TextRenderService;
use crate::draw::scene::viewport_transform::{node_viewport_frame, visible_viewport_rect};
use crate::draw::scene::{HoverInspectorSnapshot, PicturePolicy, ScenePaint};
use crate::draw::{Canvas2D, Color, FontHandle, Transform};

/// 上一已完成帧的无文本诊断快照，供下一次最终调试 Pass 展示。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DebugFrameSnapshot {
    /// 已成功呈现的逐窗口帧序号。
    pub frame_sequence: u64,
    /// 关联本帧输入与运行时工作的身份。
    pub correlation_id: Option<u64>,
    /// 当前场景树版本。
    pub tree_version: u64,
    /// 本帧协调造成的树版本增量。
    pub tree_version_delta: u64,
    /// 本帧完整耗时。
    pub frame_time: Duration,
    /// 布局阶段耗时。
    pub layout_time: Duration,
    /// 场景记录与渲染阶段耗时。
    pub render_time: Duration,
    /// GPU 提交阶段耗时。
    pub submit_time: Duration,
    /// 外部呈现阶段耗时。
    pub present_time: Duration,
    /// 本帧是否执行全幅重绘。
    pub dirty_full: bool,
    /// 本帧实际脏矩形数量。
    pub dirty_rect_count: usize,
    /// 脏区占窗口面积比例，范围为 0~1。
    pub dirty_area_ratio: f64,
    /// 本帧失效条目数量。
    pub invalidation_count: usize,
    /// 最大 Paint 失效节点槽位；没有 Paint 失效时为空。
    pub largest_invalidation_slot: Option<u64>,
    /// 最大 Paint 失效矩形占窗口面积比例，范围为 0~1。
    pub largest_invalidation_ratio: f64,
    /// 本帧活跃动画节点数量。
    pub animation_count: u32,
    /// 本帧是否执行整树协调。
    pub reconcile_ran: bool,
    /// 最终归因后的失效来源。
    pub invalidation_source: InvalidationSource,
}

impl DebugFrameSnapshot {
    /// 生成 HUD 的稳定文本事实，不包含业务文本、路径或资源身份。
    pub fn hud_lines(&self, metrics: Option<&RenderMetrics>) -> Vec<String> {
        let frame_ms = duration_ms(self.frame_time);
        let fps = if frame_ms > 0.0 {
            1000.0 / frame_ms
        } else {
            0.0
        };
        let correlation = self
            .correlation_id
            .map_or_else(|| "none".to_string(), |id| id.to_string());
        let damage_kind = if self.dirty_full { "full" } else { "partial" };
        let largest = self.largest_invalidation_slot.map_or_else(
            || "none".to_string(),
            |slot| {
                format!(
                    "#{slot} {:.1}%",
                    self.largest_invalidation_ratio.clamp(0.0, 1.0) * 100.0
                )
            },
        );
        let mut lines = vec![
            format!(
                "frame #{}  corr {}  tree {} (+{})",
                self.frame_sequence, correlation, self.tree_version, self.tree_version_delta
            ),
            format!("total {frame_ms:.2} ms  ~{fps:.0} fps"),
            format!(
                "layout {:.2}  render {:.2} ms",
                duration_ms(self.layout_time),
                duration_ms(self.render_time)
            ),
            format!(
                "submit {:.2}  present {:.2} ms",
                duration_ms(self.submit_time),
                duration_ms(self.present_time)
            ),
            format!(
                "damage {damage_kind} {:.1}%  rects {}",
                self.dirty_area_ratio.clamp(0.0, 1.0) * 100.0,
                self.dirty_rect_count
            ),
            format!(
                "inv {}  count {}  max {largest}",
                self.invalidation_source.label(),
                self.invalidation_count
            ),
            format!(
                "animation {}  reconcile {}",
                self.animation_count,
                yes_no(self.reconcile_ran)
            ),
        ];
        if let Some(metrics) = metrics {
            lines.push(format!(
                "calls layout {}  paint {}  present {}  idle {}",
                metrics.layout_calls,
                metrics.paint_calls,
                metrics.present_calls,
                metrics.idle_frames
            ));
        }
        lines
    }
}

/// 调试渲染服务。
pub struct DebugRenderService {
    /// 是否启用调试边框、检查器和帧 HUD。
    pub debug_mode: bool,
}

#[derive(Clone)]
struct OverlayLine {
    text: String,
    color: Color,
}

impl OverlayLine {
    fn new(text: impl Into<String>, color: Color) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
}

impl DebugRenderService {
    /// 使用指定启用状态创建调试绘制服务。
    pub fn new(debug_mode: bool) -> Self {
        Self { debug_mode }
    }

    /// 更新调试绘制启用状态。
    pub fn set_debug_mode(&mut self, mode: bool) {
        self.debug_mode = mode;
    }

    /// 调试深度颜色；检查器路径色块与节点轮廓严格共用。
    pub const DEBUG_COLORS: [Color; 8] = [
        Color::from_rgba(220, 60, 60, 230),
        Color::from_rgba(60, 140, 220, 230),
        Color::from_rgba(60, 180, 80, 230),
        Color::from_rgba(220, 160, 40, 230),
        Color::from_rgba(160, 60, 220, 230),
        Color::from_rgba(220, 80, 140, 230),
        Color::from_rgba(40, 200, 200, 230),
        Color::from_rgba(180, 180, 60, 230),
    ];

    /// HUD 面板相对表面边缘的外边距。
    pub const HUD_MARGIN: f32 = 8.0;
    /// HUD 首选宽度。
    pub const HUD_PANEL_W: f32 = 390.0;
    /// HUD 标题与正文使用的行高。
    pub const HUD_LINE_H: f32 = 17.0;

    /// 在所有应用内容与浮层之后绘制唯一最终调试 Pass。
    #[allow(clippy::too_many_arguments)]
    pub fn draw_final_overlay(
        &self,
        canvas: &mut dyn Canvas2D,
        scene: &impl ScenePaint,
        hover_pos: Option<Point>,
        frame: Option<&DebugFrameSnapshot>,
        metrics: Option<&RenderMetrics>,
        font: FontHandle,
        font_service: &FontService,
    ) {
        if !self.debug_mode {
            return;
        }
        let surface_w = canvas.width();
        let surface_h = canvas.height();
        if surface_w <= 24 || surface_h <= 24 {
            return;
        }

        // 最终 Pass 必须从根画布状态开始，不能继承应用节点的裁剪、变换或透明度。
        canvas.save();
        canvas.set_offset(0.0, 0.0);
        canvas.set_transform(Transform::identity());
        canvas.set_opacity(1.0);

        let mut text = TextRenderService::new(font, font_service, 520.0);
        let snapshot = hover_pos.and_then(|pos| {
            let leaf = scene.hit_test(pos)?;
            scene.hover_inspector(leaf).map(|snapshot| (pos, snapshot))
        });
        let hud_line_count = frame.map_or(1, |value| value.hud_lines(metrics).len()) + 1;
        let hud_rect = frame_hud_rect(hud_line_count, surface_w, surface_h);
        if let Some((pointer, snapshot)) = snapshot.as_ref() {
            self.draw_hover_outlines(canvas, scene, snapshot);
            self.draw_inspector_panel(
                canvas, &mut text, scene, *pointer, snapshot, hud_rect, surface_w, surface_h,
            );
        }
        // 固定 HUD 最后绘制，祖先轮廓与极端小窗下的检查器都不能降低其可读性。
        self.draw_frame_hud(canvas, &mut text, frame, metrics, hud_rect);
        canvas.restore();
    }

    /// 绘制节点调试边框（兼容 PaintContext 的公开调试入口）。
    pub fn draw_debug_border(
        &self,
        canvas: &mut dyn Canvas2D,
        rect: Rect,
        depth: usize,
        hovered: bool,
    ) {
        if !self.debug_mode || !hovered {
            return;
        }
        let base = Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()];
        canvas.stroke_rect(rect, base, 1.5, None);
    }

    /// 在节点左上角显示调试标签背景（兼容既有公开入口）。
    pub fn draw_debug_label(
        &self,
        canvas: &mut dyn Canvas2D,
        node_slot: usize,
        depth: usize,
        rect: Rect,
    ) {
        if !self.debug_mode {
            return;
        }
        let label = format!("#{node_slot} d{depth}");
        let label_w = label.chars().count() as f32 * 7.0 + 6.0;
        canvas.fill_rect(
            Rect::new(rect.x, rect.y, label_w, 16.0),
            Color::from_rgba(0, 0, 0, 200),
            None,
        );
    }

    /// 在节点下方显示 frame 信息背景（兼容既有公开入口）。
    pub fn draw_debug_frame_info(&self, canvas: &mut dyn Canvas2D, node_slot: usize, rect: Rect) {
        if !self.debug_mode {
            return;
        }
        let info = format!(
            "#{node_slot} ({:.0},{:.0}) {:.0}x{:.0}",
            rect.x, rect.y, rect.w, rect.h
        );
        let info_w = info.chars().count() as f32 * 6.5 + 6.0;
        canvas.fill_rect(
            Rect::new(rect.x, rect.y + rect.h, info_w, 15.0),
            Color::from_rgba(0, 0, 0, 190),
            None,
        );
    }

    /// 保留旧 HUD 文本入口，但明确这些值均为累计计数。
    pub fn telemetry_hud_lines(metrics: &RenderMetrics) -> [String; 6] {
        [
            format!("last inv: {}", metrics.last_invalidation.label()),
            format!("layout calls: {}", metrics.layout_calls),
            format!("paint calls: {}", metrics.paint_calls),
            format!("present calls: {}", metrics.present_calls),
            format!("idle frames: {}", metrics.idle_frames),
            "toggle: Ctrl+Shift+D".to_string(),
        ]
    }

    fn draw_frame_hud(
        &self,
        canvas: &mut dyn Canvas2D,
        text: &mut TextRenderService<'_>,
        frame: Option<&DebugFrameSnapshot>,
        metrics: Option<&RenderMetrics>,
        panel: Rect,
    ) {
        const PAD_X: f32 = 12.0;
        const PAD_TOP: f32 = 10.0;
        const FONT_SIZE: f32 = 12.5;
        let mut lines = frame.map_or_else(
            || vec!["last presented frame: waiting for first sample".to_string()],
            |frame| frame.hud_lines(metrics),
        );
        lines.push("Ctrl+Shift+D  |  hover a component to inspect".to_string());
        let x = panel.x;
        let y = panel.y;
        draw_panel_shell(canvas, panel, Color::from_rgba(69, 183, 255, 235));
        text.draw_text(
            canvas,
            "UIX DEBUG  /  LAST PRESENTED FRAME",
            Point::new(x + PAD_X, y + PAD_TOP + FONT_SIZE * 0.8),
            Color::from_rgba(112, 211, 255, 255),
            FONT_SIZE,
        );
        let max_chars = ((panel.w - PAD_X * 2.0) / 7.0).floor().max(1.0) as usize;
        for (index, line) in lines.iter().enumerate() {
            let color = if index == 1 && frame.is_some() {
                frame_time_color(frame.expect("frame checked above").frame_time)
            } else if line.starts_with("inv ") {
                frame
                    .map(|value| invalidation_color(value.invalidation_source))
                    .unwrap_or(Color::from_rgba(220, 224, 232, 255))
            } else if index + 1 == lines.len() {
                Color::from_rgba(154, 164, 180, 255)
            } else {
                Color::from_rgba(226, 232, 240, 255)
            };
            text.draw_text(
                canvas,
                &truncate_line(line, max_chars),
                Point::new(
                    x + PAD_X,
                    y + PAD_TOP + (index as f32 + 1.0) * Self::HUD_LINE_H + FONT_SIZE * 0.8,
                ),
                color,
                FONT_SIZE,
            );
        }
    }

    fn draw_hover_outlines(
        &self,
        canvas: &mut dyn Canvas2D,
        scene: &impl ScenePaint,
        snapshot: &HoverInspectorSnapshot,
    ) {
        let leaf_index = snapshot.nodes.len().saturating_sub(1);
        for (depth, node) in snapshot.nodes.iter().enumerate() {
            if !node.visible {
                continue;
            }
            let rect = node_viewport_frame(scene, node.node_id);
            if rect.w <= 0.0 || rect.h <= 0.0 {
                continue;
            }
            let color = Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()];
            let width = if depth == leaf_index { 2.5 } else { 1.5 };
            canvas.stroke_rect(rect, color, width, None);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_inspector_panel(
        &self,
        canvas: &mut dyn Canvas2D,
        text: &mut TextRenderService<'_>,
        scene: &impl ScenePaint,
        pointer: Point,
        snapshot: &HoverInspectorSnapshot,
        hud_rect: Rect,
        surface_w: i32,
        surface_h: i32,
    ) {
        const PANEL_W: f32 = 460.0;
        const PAD_X: f32 = 12.0;
        const PAD_TOP: f32 = 10.0;
        const LINE_H: f32 = 17.0;
        const FONT_SIZE: f32 = 12.5;
        const MAX_PATH_NODES: usize = 6;
        let Some(leaf) = snapshot.leaf() else {
            return;
        };
        let leaf_viewport = node_viewport_frame(scene, leaf.node_id);
        let leaf_visible = visible_viewport_rect(scene, leaf.node_id);
        let clipped = leaf_visible.is_none_or(|visible| visible != leaf_viewport);
        let type_name = short_type_name(leaf.type_name);
        let transform = scene.node_transform(leaf.node_id);
        let transform_kind = if transform.is_identity() {
            "identity"
        } else {
            "affine"
        };
        let cache = match scene.node_picture_policy(leaf.node_id) {
            PicturePolicy::Never => "never",
            PicturePolicy::Eligible => "eligible",
        };
        let scroll = scene.scroll_offset(leaf.node_id).unwrap_or((0.0, 0.0));
        let mut clip_regions = 0;
        // 调试面板只读取片段数量，不为只读查询复制矩形集合。
        scene.visit_node_clip_regions(leaf.node_id, &mut |regions| {
            clip_regions = regions.len();
        });

        let mut lines = vec![
            OverlayLine::new(
                format!(
                    "type {type_name}  node #{}  depth {}",
                    leaf.node_id.slot(),
                    snapshot.nodes.len()
                ),
                Color::from_rgba(116, 211, 255, 255),
            ),
            OverlayLine::new(
                format!(
                    "pointer {:.0},{:.0}  hit #{}",
                    pointer.x,
                    pointer.y,
                    leaf.node_id.slot()
                ),
                Color::from_rgba(184, 232, 192, 255),
            ),
            OverlayLine::new(
                format_rect("layout", leaf.frame),
                Color::from_rgba(226, 232, 240, 255),
            ),
            OverlayLine::new(
                format_rect("viewport", leaf_viewport),
                Color::from_rgba(226, 232, 240, 255),
            ),
            OverlayLine::new(
                leaf_visible.map_or_else(
                    || "visible none  clipped yes".to_string(),
                    |rect| {
                        format!(
                            "{}  clipped {}",
                            format_rect("visible", rect),
                            yes_no(clipped)
                        )
                    },
                ),
                if clipped {
                    Color::from_rgba(255, 190, 91, 255)
                } else {
                    Color::from_rgba(184, 232, 192, 255)
                },
            ),
            OverlayLine::new(
                format!(
                    "layer overlay {}  z {}  opacity {:.2}  children {}",
                    yes_no(scene.node_is_overlay(leaf.node_id)),
                    leaf.z_index,
                    scene.node_opacity(leaf.node_id),
                    leaf.child_count
                ),
                Color::from_rgba(226, 232, 240, 255),
            ),
            OverlayLine::new(
                format!(
                    "transform {transform_kind}  scroll {:.0},{:.0}  clip regions {clip_regions}",
                    scroll.0, scroll.1
                ),
                Color::from_rgba(226, 232, 240, 255),
            ),
            OverlayLine::new(
                format!(
                    "paint dirty {}  cache {cache}  dynamic {}  after {}",
                    yes_no(leaf.dirty),
                    yes_no(scene.node_has_dynamic_content(leaf.node_id)),
                    yes_no(scene.node_paints_after_children(leaf.node_id))
                ),
                if leaf.dirty {
                    Color::from_rgba(255, 190, 91, 255)
                } else {
                    Color::from_rgba(226, 232, 240, 255)
                },
            ),
            OverlayLine::new(
                format!(
                    "input focusable {}  handlers {}  interactive {}  continuous {}",
                    yes_no(scene.node_focusable(leaf.node_id)),
                    yes_no(scene.node_has_semantic_handlers(leaf.node_id)),
                    yes_no(scene.node_has_interactive_state(leaf.node_id)),
                    yes_no(scene.node_wants_continuous_pointer_move(leaf.node_id))
                ),
                Color::from_rgba(226, 232, 240, 255),
            ),
            OverlayLine::new(
                format!(
                    "state hover {}  press {}  focus {}  disabled {}  visible {}",
                    yes_no(leaf.hovered),
                    yes_no(leaf.pressed),
                    yes_no(leaf.focused),
                    yes_no(leaf.disabled),
                    yes_no(leaf.visible)
                ),
                Color::from_rgba(226, 232, 240, 255),
            ),
            OverlayLine::new(
                format!(
                    "life attached {}  mounted {}  active {}  removal {}",
                    yes_no(leaf.attached),
                    yes_no(leaf.mounted),
                    yes_no(leaf.active),
                    yes_no(leaf.pending_removal)
                ),
                Color::from_rgba(226, 232, 240, 255),
            ),
            OverlayLine::new(
                "PATH  root -> leaf  (color matches outline)",
                Color::from_rgba(154, 164, 180, 255),
            ),
        ];
        let first = snapshot.nodes.len().saturating_sub(MAX_PATH_NODES);
        if first > 0 {
            lines.push(OverlayLine::new(
                format!("... {first} ancestors omitted"),
                Color::from_rgba(154, 164, 180, 255),
            ));
        }
        for (depth, node) in snapshot.nodes.iter().enumerate().skip(first) {
            lines.push(OverlayLine::new(
                format!(
                    "[{depth:02}] {}  #{}",
                    short_type_name(node.type_name),
                    node.node_id.slot()
                ),
                Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()],
            ));
        }

        let panel_w = PANEL_W.min(surface_w as f32 - Self::HUD_MARGIN * 2.0);
        let wanted_h = PAD_TOP * 2.0 + LINE_H * lines.len() as f32;
        let panel_h = wanted_h.min(surface_h as f32 - Self::HUD_MARGIN * 2.0);
        let anchor = leaf_visible.unwrap_or(leaf_viewport);
        let panel = place_inspector_panel(anchor, hud_rect, panel_w, panel_h, surface_w, surface_h);
        draw_panel_shell(canvas, panel, Color::from_rgba(80, 180, 255, 245));
        let max_lines = ((panel.h - PAD_TOP * 2.0) / LINE_H).floor().max(1.0) as usize;
        let max_chars = ((panel.w - PAD_X * 2.0) / 7.0).floor().max(1.0) as usize;
        for (index, line) in lines.iter().take(max_lines).enumerate() {
            text.draw_text(
                canvas,
                &truncate_line(&line.text, max_chars),
                Point::new(
                    panel.x + PAD_X,
                    panel.y + PAD_TOP + index as f32 * LINE_H + FONT_SIZE * 0.8,
                ),
                line.color,
                FONT_SIZE,
            );
        }
    }
}

fn draw_panel_shell(canvas: &mut dyn Canvas2D, panel: Rect, accent: Color) {
    canvas.fill_rect(panel, Color::from_rgba(12, 16, 24, 242), None);
    canvas.stroke_rect(panel, accent, 1.0, None);
    canvas.fill_rect(Rect::new(panel.x, panel.y, 4.0, panel.h), accent, None);
}

fn frame_hud_rect(line_count: usize, surface_w: i32, surface_h: i32) -> Rect {
    const PAD_TOP: f32 = 10.0;
    let margin = DebugRenderService::HUD_MARGIN;
    let panel_w = DebugRenderService::HUD_PANEL_W.min(surface_w as f32 - margin * 2.0);
    let panel_h = PAD_TOP * 2.0 + DebugRenderService::HUD_LINE_H * (line_count as f32 + 1.0);
    let x = (surface_w as f32 - panel_w - margin).max(margin);
    let y = margin;
    Rect::new(x, y, panel_w, panel_h.min(surface_h as f32 - y))
}

fn place_inspector_panel(
    anchor: Rect,
    hud: Rect,
    panel_w: f32,
    panel_h: f32,
    surface_w: i32,
    surface_h: i32,
) -> Rect {
    const GAP: f32 = 10.0;
    let margin = DebugRenderService::HUD_MARGIN;
    let candidates = [
        Rect::new(anchor.x + anchor.w + GAP, anchor.y, panel_w, panel_h),
        Rect::new(anchor.x - panel_w - GAP, anchor.y, panel_w, panel_h),
        Rect::new(anchor.x, anchor.y + anchor.h + GAP, panel_w, panel_h),
        Rect::new(anchor.x, anchor.y - panel_h - GAP, panel_w, panel_h),
    ];
    let max_x = (surface_w as f32 - panel_w - margin).max(margin);
    let max_y = (surface_h as f32 - panel_h - margin).max(margin);
    candidates
        .into_iter()
        .map(|candidate| {
            Rect::new(
                candidate.x.clamp(margin, max_x),
                candidate.y.clamp(margin, max_y),
                panel_w,
                panel_h,
            )
        })
        .min_by(|left, right| {
            panel_overlap_score(*left, anchor, hud)
                .total_cmp(&panel_overlap_score(*right, anchor, hud))
        })
        .unwrap_or(Rect::new(margin, max_y, panel_w, panel_h))
}

fn panel_overlap_score(panel: Rect, anchor: Rect, hud: Rect) -> f32 {
    intersection_area(panel, anchor) * 4.0 + intersection_area(panel, hud) * 8.0
}

fn intersection_area(left: Rect, right: Rect) -> f32 {
    left.intersect(&right).map_or(0.0, |rect| rect.w * rect.h)
}

fn format_rect(label: &str, rect: Rect) -> String {
    format!(
        "{label} {:.0},{:.0}  {:.0}x{:.0}",
        rect.x, rect.y, rect.w, rect.h
    )
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn short_type_name(value: &str) -> &str {
    value.rsplit("::").next().unwrap_or(value)
}

fn truncate_line(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut text = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    text.push('…');
    text
}

fn frame_time_color(duration: Duration) -> Color {
    if duration >= Duration::from_millis(34) {
        Color::from_rgba(255, 111, 111, 255)
    } else if duration >= Duration::from_millis(17) {
        Color::from_rgba(255, 199, 95, 255)
    } else {
        Color::from_rgba(125, 224, 151, 255)
    }
}

fn invalidation_color(source: InvalidationSource) -> Color {
    match source {
        InvalidationSource::None => Color::from_rgba(154, 164, 180, 255),
        InvalidationSource::FirstFrame => Color::from_rgba(245, 196, 77, 255),
        InvalidationSource::DirtyRegion => Color::from_rgba(111, 218, 139, 255),
        InvalidationSource::AnimationPolling => Color::from_rgba(98, 168, 255, 255),
        InvalidationSource::LayoutEvent => Color::from_rgba(242, 112, 181, 255),
    }
}
