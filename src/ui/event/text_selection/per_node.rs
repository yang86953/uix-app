//! 单节点文字选区状态与命中测试 — Label / Typography 共享的逐节点实现。
//!
//! 跨节点协调层在 [`super`]（text_selection.rs），本模块只负责单个文字节点：
//! 布局缓存（字形 x / advance / 字符下标 / 行信息）、二维字符命中、
//! 扩展字素簇归一选区与切片、拖选事件序列，以及跨节点选区读写。
//! 两组件文本字段名不同（`text` / `content`），统一由调用方以
//! `text: &str` 参数传入，本模块不感知具体组件。

use std::cell::Cell;
use std::cell::RefCell;

use crate::core::Point;
// 引入共享扩展字素簇边界模型。
use crate::draw::resources::font::text_index::{
    BoundaryBias, CharIndex, TextIndexCursor, TextIndexMap,
};
// 引入布局产出的字形与行信息容器。
use crate::draw::resources::font::text_backend::TextLayout;

/// 文本行的字符范围与字形范围，用于二维命中测试。
#[derive(Debug, Clone, Copy)]
struct TextLineHit {
    y: f32,
    glyph_start: usize,
    glyph_count: usize,
    start_char: usize,
    end_char: usize,
}

/// 单节点共用的文字选区状态：布局缓存 + 选区 + 拖选锚点 + 绘制偏移。
pub(crate) struct PerNodeTextSelection {
    /// 渲染时缓存的字形 x 位置（文本局部坐标）。
    glyph_xs: RefCell<Vec<f32>>,
    /// 与 glyph_xs 平行的字形 advance，用于按中点选择最近光标边界。
    glyph_widths: RefCell<Vec<f32>>,
    /// 与 glyph_xs 平行的字符下标（`chars()` 序）。
    glyph_char_indices: RefCell<Vec<usize>>,
    /// 每行的字符范围与字形范围，用于二维命中测试。
    line_info: RefCell<Vec<TextLineHit>>,
    /// 当前选区（字符边界，起点小于终点；空选区为 None）。
    selection: Cell<Option<(usize, usize)>>,
    /// 拖选锚点（Shift 扩展与跨节点扩展的基准）。
    sel_anchor: Cell<usize>,
    /// 是否正在拖选。
    sel_dragging: Cell<bool>,
    /// 上次渲染时的文本 draw_pos（用于事件命中测试）。
    draw_pos: Cell<Point>,
}

impl PerNodeTextSelection {
    pub(crate) fn new() -> Self {
        // 全部初始化为空状态。
        Self {
            glyph_xs: RefCell::new(Vec::new()),
            glyph_widths: RefCell::new(Vec::new()),
            glyph_char_indices: RefCell::new(Vec::new()),
            line_info: RefCell::new(Vec::new()),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            draw_pos: Cell::new(Point::new(0.0, 0.0)),
        }
    }

    /// 从单次布局结果重建字形与行缓存（渲染阶段调用一次）。
    pub(crate) fn cache_layout(&self, layout: &TextLayout) {
        // 缓存字形 x、advance 与字符下标（选区/命中用字符序）。
        let mut xs = self.glyph_xs.borrow_mut();
        let mut widths = self.glyph_widths.borrow_mut();
        let mut cis = self.glyph_char_indices.borrow_mut();
        xs.clear();
        widths.clear();
        cis.clear();
        for g in &layout.glyphs {
            xs.push(g.x);
            widths.push(g.width.max(0.0));
            cis.push(g.char_index);
        }
        // 缓存行信息（用于 y 轴命中测试）。
        let mut li = self.line_info.borrow_mut();
        li.clear();
        for line in &layout.lines {
            li.push(TextLineHit {
                y: line.y,
                glyph_start: line.glyph_start,
                glyph_count: line.glyph_count,
                start_char: line.start_char,
                end_char: line.end_char,
            });
        }
    }

    /// 清空字形与行缓存（文本变更后重建）。
    pub(crate) fn clear_caches(&self) {
        self.glyph_xs.borrow_mut().clear();
        self.glyph_widths.borrow_mut().clear();
        self.glyph_char_indices.borrow_mut().clear();
        self.line_info.borrow_mut().clear();
    }

    /// 清空选区、锚点与拖选状态（文本变更 / 失焦 / 关闭可选中）。
    pub(crate) fn reset_selection(&self) {
        self.selection.set(None);
        self.sel_anchor.set(0);
        self.sel_dragging.set(false);
    }

    /// 当前选区（字符边界，起点小于终点；无选区为 None）。
    pub(crate) fn selection(&self) -> Option<(usize, usize)> {
        self.selection.get()
    }

    /// 是否正在拖选。
    pub(crate) fn is_dragging(&self) -> bool {
        self.sel_dragging.get()
    }

    /// 记录本次渲染的文本绘制起点（事件命中测试换算用）。
    pub(crate) fn set_draw_pos(&self, draw_pos: Point) {
        self.draw_pos.set(draw_pos);
    }

    /// PointerDown 的选区起始处理：Shift 按锚点扩展，否则重设锚点并清空选区。
    pub(crate) fn pointer_down(&self, text: &str, pos: Point, shift: bool) {
        // 命中当前位置对应的字符下标。
        let ci = self.hit_char_at(text, pos);
        if shift {
            // Shift 按下：从既有锚点扩展到当前命中位置。
            let anchor = self.sel_anchor.get();
            self.set_selection_range(text, anchor, ci);
        } else {
            // 普通按下：清空选区并以当前位置为新锚点。
            self.selection.set(None);
            self.sel_anchor.set(ci);
        }
        // 进入拖选状态。
        self.sel_dragging.set(true);
    }

    /// PointerMove 的拖选扩展；非拖选中的移动返回 false 表示未消费。
    pub(crate) fn pointer_move(&self, text: &str, pos: Point) -> bool {
        // 非拖选中的移动不消费事件。
        if !self.sel_dragging.get() {
            return false;
        }
        // 拖选中按当前位置从锚点扩展选区。
        let ci = self.hit_char_at(text, pos);
        let anchor = self.sel_anchor.get();
        self.set_selection_range(text, anchor, ci);
        true
    }

    /// PointerUp：结束拖选，起点终点重合的拖选视为未选中。
    pub(crate) fn pointer_up(&self) {
        self.sel_dragging.set(false);
        // 空选择（起点 == 终点）不保留选区。
        if let Some((s, e)) = self.selection.get() {
            if s == e {
                self.selection.set(None);
            }
        }
    }

    /// Ctrl+A 全选：锚点置于行首并选中全部字符。
    pub(crate) fn select_all(&self, text: &str) {
        let len = text.chars().count();
        self.sel_anchor.set(0);
        self.set_selection_range(text, 0, len);
    }

    /// 当前选区文本；无选区或选区为空时返回 `None`。
    pub(crate) fn selected_text(&self, text: &str) -> Option<String> {
        self.selection
            .get()
            .map(|(s, e)| self.slice_range(text, s, e))
    }

    /// 跨节点选区使用的字符长度（`chars()` 序）。
    pub(crate) fn cross_text_len(&self, text: &str) -> usize {
        text.chars().count()
    }

    /// 跨节点选区使用的锚点。
    pub(crate) fn cross_text_anchor(&self) -> usize {
        self.sel_anchor.get()
    }

    /// 设置跨节点选区范围；仍复用本节点完整字素簇归一规则。
    pub(crate) fn set_cross_text_range(&self, text: &str, range: Option<(usize, usize)>) {
        match range {
            // 跨节点选择仍复用本节点完整字素簇归一规则。
            Some((a, b)) if a != b => self.set_selection_range(text, a, b),
            _ => self.selection.set(None),
        }
    }

    /// 跨节点命中：把节点帧内坐标换算到文本局部坐标后做字符命中。
    pub(crate) fn cross_text_char_at(&self, text: &str, frame_local: Point) -> usize {
        self.hit_char_at(text, frame_local)
    }

    // 测试目标保留渲染行起点观测入口，供排版布局测试按需调用。
    #[cfg(test)]
    pub(crate) fn rendered_line_origins_for_test(&self) -> Vec<Point> {
        let glyph_xs = self.glyph_xs.borrow();
        self.line_info
            .borrow()
            .iter()
            .map(|line| {
                let x = glyph_xs.get(line.glyph_start).copied().unwrap_or_default();
                Point::new(x, line.y)
            })
            .collect()
    }

    /// 把节点帧内坐标换算到文本局部坐标并返回命中字符下标。
    fn hit_char_at(&self, text: &str, pos: Point) -> usize {
        let dp = self.draw_pos.get();
        self.char_at_xy(text, pos.x - dp.x, pos.y - dp.y)
    }

    // 先保留布局层给出的原始 shaping cluster 字符边界。
    fn raw_char_at_xy(&self, text: &str, text_x: f32, text_y: f32) -> usize {
        let xs = self.glyph_xs.borrow();
        let widths = self.glyph_widths.borrow();
        let cis = self.glyph_char_indices.borrow();
        let li = self.line_info.borrow();
        // 无字形缓存时按行首处理。
        if xs.is_empty() {
            return 0;
        }
        // 单行（无行信息）时按 x 顺序命中。
        if li.is_empty() {
            for i in 0..xs.len() {
                let boundary = xs[i] + widths.get(i).copied().unwrap_or_default() * 0.5;
                if text_x < boundary {
                    return cis.get(i).copied().unwrap_or(i);
                }
            }
            return cis.last().map(|c| c + 1).unwrap_or(text.chars().count());
        }
        // 多行：先按 y 落在哪一行，再在该行内按 x 命中。
        let target_y = text_y.max(li[0].y);
        let Some(line) = li
            .iter()
            .enumerate()
            .find(|(index, line)| {
                let next_y = li.get(index + 1).map_or(f32::MAX, |next| next.y);
                target_y >= line.y && target_y < next_y
            })
            .map(|(_, line)| line)
            .or_else(|| li.last())
        else {
            return 0;
        };
        let start = line.glyph_start.min(xs.len());
        let end = (start + line.glyph_count).min(xs.len());
        for i in start..end {
            let boundary = xs[i] + widths.get(i).copied().unwrap_or_default() * 0.5;
            if text_x < boundary {
                return cis.get(i).copied().unwrap_or(i);
            }
        }
        if end > start {
            line.end_char
        } else {
            line.start_char
        }
    }

    // 把普通文本命中统一约束到扩展字素簇边界。
    fn char_at_xy(&self, text: &str, text_x: f32, text_y: f32) -> usize {
        // 查询现有布局几何给出的原始字符位置。
        let raw_index = self.raw_char_at_xy(text, text_x, text_y);
        // 命中采用最近合法字素簇边界。
        TextIndexCursor::new(text)
            // 归一显式字符位置。
            .normalize_char(CharIndex(raw_index), BoundaryBias::Nearest)
            // 返回兼容字符下标。
            .0
    }

    fn set_selection_range(&self, text: &str, a: usize, b: usize) {
        // 同一逻辑位置始终表示空选择，不因旧位置非法而扩展文本。
        if a == b {
            // 清除空选择。
            self.selection.set(None);
            // 无需构造范围。
            return;
        }
        // 把无方向选择向外扩展到完整字素簇边界。
        let (start, end) = TextIndexCursor::new(text)
            // 归一显式字符范围。
            .normalize_selection(CharIndex(a), CharIndex(b));
        // 空范围不保留选择。
        if start == end {
            self.selection.set(None);
        } else {
            // 保存合法字符边界组成的选择范围。
            self.selection.set(Some((start.0, end.0)));
        }
    }

    fn slice_range(&self, text: &str, start_char: usize, end_char: usize) -> String {
        // 建立字符到 UTF-8 字节的显式转换表。
        let index_map = TextIndexMap::new(text);
        // 防御性地把调用范围扩展到完整字素簇。
        let (start, end) = index_map.normalize_selection(
            // 包装字符起点。
            CharIndex(start_char),
            // 包装字符终点。
            CharIndex(end_char),
        );
        // 转换合法字符起点为字节偏移。
        let byte_start = index_map.char_to_byte(start).0;
        // 转换合法字符终点为字节偏移。
        let byte_end = index_map.char_to_byte(end).0;
        // 返回完整 UTF-8 字素簇片段。
        text[byte_start..byte_end].to_owned()
    }
}
