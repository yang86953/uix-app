//! 真实终端的 VT 屏幕状态：把 PTY 字节流经 vte 状态机解析为行列网格。
//!
//! SMC 职责：本文件是纯数据 Component——只消费字节、维护网格与光标，
//! 不拥有进程、fd 或 UI；PTY 会话（`super::session`）是唯一写入口，
//! 绘制组件（`super::screen_widget`）是只读消费者。跨读取边界的 UTF-8
//! 由 vte 解析器的部分码点缓冲处理，本层不重复拆包。

use std::collections::VecDeque;
use vte::{Params, Perform};

pub(crate) const MAX_SCROLLBACK_ROWS: usize = 100_000;
const MAX_SCROLLBACK_CELLS: usize = 1_048_576;

// Navigation state belongs to each view, not to the shared session. These
// counters let a view keep its anchor as new rows arrive or old rows expire.
#[derive(Clone, Copy, Default)]
pub(crate) struct ScrollbackState {
    pub(crate) rows: usize,
    pub(crate) total: u64,
    pub(crate) epoch: u64,
    pub(crate) alternate: bool,
}

/// 子进程控制序列声明的会话模式；查询快照，不是宿主配置或光标保存槽。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TerminalModes {
    /// DECCKM：无修饰方向/Home/End 键发送 SS3，而不是普通 CSI。
    pub application_cursor_keys: bool,
    /// DECTCEM：允许绘制活动光标；焦点、会话与历史浏览仍有独立门禁。
    pub cursor_visible: bool,
    /// 2004：粘贴事件使用 ESC[200~ 与 ESC[201~ 包围。
    pub bracketed_paste: bool,
}

impl Default for TerminalModes {
    fn default() -> Self {
        Self {
            application_cursor_keys: false,
            cursor_visible: true,
            bracketed_paste: false,
        }
    }
}

/// 单元格前景/背景的颜色指定方式。
///
/// `Palette` 索引标准 xterm 色板：0-15 为 16 基础色，16-255 为 256 色扩展板；
/// 具体色值由绘制层解析，公开面不出现原始色值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalColorSpec {
    /// 终端默认前景/背景，由主题 token 解析。
    Default,
    /// xterm 色板索引（0-255）。
    Palette(u8),
    /// SGR 38/48 直接色。
    Rgb(u8, u8, u8),
}

/// 一段连续同风格屏幕文本的完整绘制风格。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSpanStyle {
    /// 前景色指定。
    pub fg: TerminalColorSpec,
    /// 背景色指定。
    pub bg: TerminalColorSpec,
    /// SGR 1 粗体（22 取消）。
    pub bold: bool,
    /// SGR 7 反转（27 取消）；绘制层交换前景/背景。
    pub inverse: bool,
}

impl Default for TerminalSpanStyle {
    fn default() -> Self {
        Self {
            fg: TerminalColorSpec::Default,
            bg: TerminalColorSpec::Default,
            bold: false,
            inverse: false,
        }
    }
}

/// 一段连续同风格屏幕文本。
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalRowSpan {
    /// 该段文本（宽字符按显示宽度 2 计列）。
    pub text: String,
    /// 该段风格。
    pub style: TerminalSpanStyle,
}

/// 真实终端屏幕的一行：按列序拼接的同风格文本段集合。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TerminalRow {
    /// 按列序声明的文本段。
    pub spans: Vec<TerminalRowSpan>,
}

impl TerminalRow {
    /// 返回拼接全部文本段后的纯文本。
    pub fn plain(&self) -> String {
        let mut plain = String::new();
        for span in &self.spans {
            plain.push_str(&span.text);
        }
        plain
    }

    /// 用单段文本构造整行（测试与工具用）。
    pub fn text(value: impl Into<String>) -> Self {
        Self {
            spans: vec![TerminalRowSpan {
                text: value.into(),
                style: TerminalSpanStyle::default(),
            }],
        }
    }
}

// 网格单元：一个字符及其占列数；width 为 0 表示宽字符右侧续格。
#[derive(Debug, Clone, Copy, PartialEq)]
struct VtCell {
    ch: char,
    width: u8,
    style: TerminalSpanStyle,
}

// DECSC/DECRC keeps the pending wrap column as well as the visible position.
#[derive(Clone, Copy)]
struct SavedCursor {
    row: usize,
    column: usize,
    style: TerminalSpanStyle,
    origin_mode: bool,
    auto_wrap: bool,
}

impl VtCell {
    fn blank() -> Self {
        Self {
            ch: ' ',
            width: 1,
            style: TerminalSpanStyle::default(),
        }
    }
}

/// VT 屏幕状态：行列网格、光标与当前打印风格。
///
/// 支持的控制函数（其余序列被吞掉并计入 `ignored_sequences` 观察计数）：
/// 打印与自动换行（DECAWM）、CR/LF/BS/TAB、CUP/CUU/CUD/CUF/CUB/CHA/VPA、
/// ED/EL/ECH/DCH/ICH、IL/DL/SU/SD、DECSC/DECRC、RI/IND/NEL、RIS、DECSTBM、
/// DECOM/DECAWM、47/1047/1048/1049 交替屏与光标，以及 SGR
/// （0/1/7/22/27、30-37/39/90-97、40-47/49/100-107、38/48 的 5 与 2 扩展）。
pub(crate) struct VtScreen {
    cols: usize,
    rows: usize,
    grid: Vec<VtCell>,
    cursor_row: usize,
    cursor_col: usize,
    saved_cursor: [Option<SavedCursor>; 2],
    style: TerminalSpanStyle,
    // At most one inactive grid; allocate it only when alternate mode is used.
    inactive_grid: Option<Vec<VtCell>>,
    alternate: bool,
    scroll_top: usize,
    scroll_bottom: usize,
    origin_mode: bool,
    auto_wrap: bool,
    /// 收到但未实现的序列计数，供诊断与文档限制核对。
    pub(crate) ignored_sequences: usize,
    scrollback: VecDeque<Vec<VtCell>>,
    scrollback_cells: usize,
    scrollback_limit: usize,
    scrollback_total: u64,
    scrollback_epoch: u64,
    modes: TerminalModes,
}

impl VtScreen {
    pub(crate) fn new(cols: usize, rows: usize) -> Self {
        let cols = cols.max(2);
        let rows = rows.max(2);
        Self {
            cols,
            rows,
            grid: vec![VtCell::blank(); cols * rows],
            cursor_row: 0,
            cursor_col: 0,
            saved_cursor: [None, None],
            style: TerminalSpanStyle::default(),
            inactive_grid: None,
            alternate: false,
            scroll_top: 0,
            scroll_bottom: rows - 1,
            origin_mode: false,
            auto_wrap: true,
            ignored_sequences: 0,
            scrollback: VecDeque::new(),
            scrollback_cells: 0,
            scrollback_limit: 1_000,
            scrollback_total: 0,
            scrollback_epoch: 0,
            modes: TerminalModes::default(),
        }
    }

    /// 网格列数。
    pub(crate) fn cols(&self) -> usize {
        self.cols
    }

    /// 网格行数。
    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    /// 光标位置（行、列）。
    pub(crate) fn cursor(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col.min(self.cols - 1))
    }

    pub(crate) fn modes(&self) -> TerminalModes {
        self.modes
    }

    pub(crate) fn scrollback_state(&self) -> ScrollbackState {
        ScrollbackState {
            rows: self.scrollback.len(),
            total: self.scrollback_total,
            epoch: self.scrollback_epoch,
            alternate: self.alternate,
        }
    }

    pub(crate) fn scrollback_limit(&self) -> usize {
        self.scrollback_limit
    }

    pub(crate) fn set_scrollback_limit(&mut self, limit: usize) {
        self.scrollback_limit = limit;
        while self.scrollback.len() > limit {
            self.evict_scrollback_row();
        }
    }

    fn evict_scrollback_row(&mut self) {
        if let Some(row) = self.scrollback.pop_front() {
            self.scrollback_cells -= row.len();
        }
    }

    pub(crate) fn clear_scrollback(&mut self) {
        self.scrollback = VecDeque::new();
        self.scrollback_cells = 0;
        self.scrollback_epoch = self.scrollback_epoch.saturating_add(1);
    }

    pub(crate) fn scrollback_rows(&self, start: usize, count: usize) -> Vec<TerminalRow> {
        self.scrollback
            .iter()
            .skip(start)
            .take(count)
            .map(|row| Self::snapshot_row(row, row.len()))
            .collect()
    }

    pub(crate) fn viewport_rows(&self, offset: usize) -> Vec<TerminalRow> {
        if self.alternate || offset == 0 {
            return self.snapshot();
        }
        let offset = offset.min(self.scrollback.len());
        let start = self.scrollback.len() - offset;
        let mut result: Vec<_> = self
            .scrollback
            .iter()
            .skip(start)
            .take(self.rows)
            .map(|row| Self::snapshot_row(row, self.cols))
            .collect();
        for row in 0..self.rows - result.len() {
            result.push(Self::snapshot_row(
                &self.grid[row * self.cols..(row + 1) * self.cols],
                self.cols,
            ));
        }
        result
    }

    fn retain_scrolled_rows(&mut self, count: usize) {
        self.scrollback_total = self.scrollback_total.saturating_add(count as u64);
        if self.scrollback_limit == 0 {
            return;
        }
        for row in 0..count {
            while self.scrollback.len() >= self.scrollback_limit
                || self.scrollback_cells + self.cols > MAX_SCROLLBACK_CELLS
            {
                self.evict_scrollback_row();
            }
            self.scrollback
                .push_back(self.grid[row * self.cols..(row + 1) * self.cols].to_vec());
            self.scrollback_cells += self.cols;
        }
    }

    /// 喂入一段 PTY 字节；UTF-8 边界由解析器内部缓冲。
    ///
    /// 解析器由调用方（会话核心）持有，避免 `Perform` 回调与解析器互相借用。
    pub(crate) fn feed(&mut self, parser: &mut vte::Parser, bytes: &[u8]) {
        parser.advance(self, bytes);
    }

    /// 调整网格尺寸：保留左上角既有内容，新增区域为空白。
    pub(crate) fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(2);
        let rows = rows.max(2);
        if cols == self.cols && rows == self.rows {
            return;
        }
        Self::resize_grid(&mut self.grid, self.cols, self.rows, cols, rows);
        if let Some(inactive) = self.inactive_grid.as_mut() {
            Self::resize_grid(inactive, self.cols, self.rows, cols, rows);
        }
        for saved in self.saved_cursor.iter_mut().flatten() {
            saved.row = saved.row.min(rows - 1);
            // A width change cancels a pending wrap, not a height-only resize.
            if cols != self.cols {
                saved.column = saved.column.min(cols - 1);
            }
        }
        if cols != self.cols {
            self.cursor_col = self.cursor_col.min(cols - 1);
        }
        self.cols = cols;
        self.rows = rows;
        self.cursor_row = self.cursor_row.min(rows - 1);
        // Resizing invalidates margins, as in a newly sized terminal page.
        self.scroll_top = 0;
        self.scroll_bottom = rows - 1;
    }

    fn resize_grid(
        grid: &mut Vec<VtCell>,
        old_cols: usize,
        old_rows: usize,
        cols: usize,
        rows: usize,
    ) {
        let mut resized = vec![VtCell::blank(); cols * rows];
        let keep_cols = cols.min(old_cols);
        for row in 0..rows.min(old_rows) {
            let source = row * old_cols;
            let target = row * cols;
            resized[target..target + keep_cols].copy_from_slice(&grid[source..source + keep_cols]);
            // Do not leave a two-column glyph cut in half at the right edge.
            if resized[target + cols - 1].width == 2 {
                resized[target + cols - 1] = VtCell::blank();
            }
        }
        *grid = resized;
    }

    fn save_cursor(&mut self) {
        self.saved_cursor[usize::from(self.alternate)] = Some(SavedCursor {
            row: self.cursor_row,
            column: self.cursor_col,
            style: self.style,
            origin_mode: self.origin_mode,
            auto_wrap: self.auto_wrap,
        });
    }

    fn restore_cursor(&mut self) {
        if let Some(saved) = self.saved_cursor[usize::from(self.alternate)] {
            self.origin_mode = saved.origin_mode;
            self.auto_wrap = saved.auto_wrap;
            let (top, bottom) = self.vertical_bounds();
            self.cursor_row = saved.row.clamp(top, bottom);
            self.cursor_col = saved.column.min(self.cols);
            self.style = saved.style;
        }
    }

    fn switch_screen(&mut self, alternate: bool) {
        if self.alternate == alternate {
            return;
        }
        let inactive = self
            .inactive_grid
            .get_or_insert_with(|| vec![VtCell::blank(); self.cols * self.rows]);
        std::mem::swap(&mut self.grid, inactive);
        self.alternate = alternate;
    }

    fn vertical_bounds(&self) -> (usize, usize) {
        if self.origin_mode {
            (self.scroll_top, self.scroll_bottom)
        } else {
            (0, self.rows - 1)
        }
    }

    fn home_cursor(&mut self) {
        self.cursor_row = self.vertical_bounds().0;
        self.cursor_col = 0;
    }

    fn set_private_mode(&mut self, mode: u16, enabled: bool) {
        match mode {
            1 => self.modes.application_cursor_keys = enabled,
            6 => {
                self.origin_mode = enabled;
                self.home_cursor();
            }
            7 => {
                self.auto_wrap = enabled;
                if !enabled {
                    self.settle_cursor_col();
                }
            }
            47 => self.switch_screen(enabled),
            25 => self.modes.cursor_visible = enabled,
            2004 => self.modes.bracketed_paste = enabled,
            1047 => {
                if !enabled && self.alternate {
                    self.grid.fill(VtCell::blank());
                }
                self.switch_screen(enabled);
            }
            1048 => {
                if enabled {
                    self.save_cursor();
                } else {
                    self.restore_cursor();
                }
            }
            1049 => {
                if enabled && !self.alternate {
                    self.save_cursor();
                    self.switch_screen(true);
                    self.grid.fill(VtCell::blank());
                } else if !enabled && self.alternate {
                    self.switch_screen(false);
                    self.restore_cursor();
                }
            }
            _ => self.ignored_sequences = self.ignored_sequences.saturating_add(1),
        }
    }

    /// 把整屏导出为按行文本段快照；宽字符右侧续格不产生文本。
    pub(crate) fn snapshot(&self) -> Vec<TerminalRow> {
        (0..self.rows)
            .map(|row| {
                Self::snapshot_row(
                    &self.grid[row * self.cols..(row + 1) * self.cols],
                    self.cols,
                )
            })
            .collect()
    }

    fn snapshot_row(cells: &[VtCell], cols: usize) -> TerminalRow {
        let mut spans: Vec<TerminalRowSpan> = Vec::new();
        let mut column = 0;
        while column < cols {
            let mut cell = cells.get(column).copied().unwrap_or_else(VtCell::blank);
            if cell.width == 0 {
                column += 1;
                continue;
            }
            if column + usize::from(cell.width) > cols {
                cell = VtCell::blank();
            }
            match spans.last_mut() {
                Some(span) if span.style == cell.style => span.text.push(cell.ch),
                _ => spans.push(TerminalRowSpan {
                    text: cell.ch.to_string(),
                    style: cell.style,
                }),
            }
            column += usize::from(cell.width);
        }
        TerminalRow { spans }
    }

    // 光标列可能因自动换行悬停在虚拟 cols 位置；写入前先落回最后实列。
    fn settle_cursor_col(&mut self) {
        if self.cursor_col >= self.cols {
            self.cursor_col = self.cols - 1;
        }
    }

    fn scroll_up_from(&mut self, top: usize, count: usize, save: bool) {
        let count = count.min(self.scroll_bottom + 1 - top);
        if save && !self.alternate && top == 0 && self.scroll_bottom == self.rows - 1 {
            self.retain_scrolled_rows(count);
        }
        let start = top * self.cols;
        let end = (self.scroll_bottom + 1) * self.cols;
        let shift = count.min(self.scroll_bottom + 1 - top) * self.cols;
        self.grid.copy_within(start + shift..end, start);
        self.grid[end - shift..end].fill(VtCell::blank());
    }

    fn scroll_down_from(&mut self, top: usize, count: usize) {
        let start = top * self.cols;
        let end = (self.scroll_bottom + 1) * self.cols;
        let shift = count.min(self.scroll_bottom + 1 - top) * self.cols;
        self.grid.copy_within(start..end - shift, start + shift);
        self.grid[start..start + shift].fill(VtCell::blank());
    }

    // 换行到下一行；底行触发向上滚动。
    fn linefeed(&mut self) {
        self.settle_cursor_col();
        if self.cursor_row == self.scroll_bottom {
            self.scroll_up_from(self.scroll_top, 1, true);
        } else {
            self.cursor_row = (self.cursor_row + 1).min(self.rows - 1);
        }
    }

    // 自动换行：先回到行首再换行（DECAWM）。
    fn wrap_line(&mut self) {
        self.cursor_col = 0;
        self.linefeed();
    }

    fn clear_cells(&mut self, row: usize, from: usize, to: usize) {
        let to = to.min(self.cols);
        for column in from..to {
            self.grid[row * self.cols + column] = VtCell::blank();
        }
    }

    fn put_char(&mut self, ch: char, width: usize) {
        // 宽度 0 的组合字符暂不支持并入前格，直接跳过（限制见文档）。
        if width == 0 {
            return;
        }
        if self.cursor_col + width > self.cols {
            if self.auto_wrap {
                self.wrap_line();
            } else {
                self.settle_cursor_col();
                if self.cursor_col + width > self.cols {
                    return;
                }
            }
        }
        let row = self.cursor_row;
        let column = self.cursor_col;
        self.grid[row * self.cols + column] = VtCell {
            ch,
            width: width as u8,
            style: self.style,
        };
        if width == 2 && column + 1 < self.cols {
            self.grid[row * self.cols + column + 1] = VtCell {
                ch: ' ',
                width: 0,
                style: self.style,
            };
        }
        self.cursor_col += width;
        if !self.auto_wrap {
            self.settle_cursor_col();
        }
    }

    // 按主参数读取第 index 个参数的默认值。
    fn param(params: &Params, index: usize, fallback: u16) -> u16 {
        params
            .iter()
            .nth(index)
            .and_then(|param| param.first().copied())
            .filter(|value| *value != 0)
            .unwrap_or(fallback)
    }

    fn apply_sgr(&mut self, params: &Params) {
        // 子参数（冒号形式）与分号形式在此统一展开处理。
        let mut flat: Vec<u16> = Vec::new();
        for param in params.iter() {
            flat.extend_from_slice(param);
        }
        let mut index = 0;
        while index < flat.len() {
            let value = flat[index];
            match value {
                0 => self.style = TerminalSpanStyle::default(),
                1 => self.style.bold = true,
                22 => self.style.bold = false,
                7 => self.style.inverse = true,
                27 => self.style.inverse = false,
                30..=37 => self.style.fg = TerminalColorSpec::Palette((value - 30) as u8),
                39 => self.style.fg = TerminalColorSpec::Default,
                90..=97 => self.style.fg = TerminalColorSpec::Palette((value - 90 + 8) as u8),
                40..=47 => self.style.bg = TerminalColorSpec::Palette((value - 40) as u8),
                49 => self.style.bg = TerminalColorSpec::Default,
                100..=107 => self.style.bg = TerminalColorSpec::Palette((value - 100 + 8) as u8),
                38 | 48 => {
                    let Some(kind) = flat.get(index + 1).copied() else {
                        index = flat.len();
                        continue;
                    };
                    let target = if value == 38 {
                        &mut self.style.fg
                    } else {
                        &mut self.style.bg
                    };
                    match kind {
                        5 => {
                            if let Some(color) = flat.get(index + 2).copied() {
                                *target = TerminalColorSpec::Palette(color as u8);
                                index += 2;
                            } else {
                                index = flat.len();
                            }
                        }
                        2 => {
                            if let (Some(r), Some(g), Some(b)) = (
                                flat.get(index + 2).copied(),
                                flat.get(index + 3).copied(),
                                flat.get(index + 4).copied(),
                            ) {
                                *target = TerminalColorSpec::Rgb(r as u8, g as u8, b as u8);
                                index += 4;
                            } else {
                                index = flat.len();
                            }
                        }
                        _ => index = flat.len(),
                    }
                }
                _ => {}
            }
            index += 1;
        }
    }
}

impl Perform for VtScreen {
    fn print(&mut self, ch: char) {
        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        self.put_char(ch, width);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\r' => self.cursor_col = 0,
            b'\n' | 0x0b | 0x0c => self.linefeed(),
            b'\x08' => {
                self.settle_cursor_col();
                self.cursor_col = self.cursor_col.saturating_sub(1);
            }
            b'\t' => {
                self.settle_cursor_col();
                let next = (self.cursor_col / 8 + 1) * 8;
                self.cursor_col = next.min(self.cols - 1);
            }
            // BEL、ENQ 与字符集切换在本轮明确忽略。
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if !ignore && intermediates == b"?" && matches!(action, 'h' | 'l') {
            for param in params.iter() {
                if let [mode] = param {
                    self.set_private_mode(*mode, action == 'h');
                } else {
                    self.ignored_sequences = self.ignored_sequences.saturating_add(1);
                }
            }
            return;
        }
        // Prefixes and intermediates are part of a control's identity. For
        // example, DECSED (?J) must never accidentally execute ordinary ED (J).
        if ignore
            || !intermediates.is_empty()
            || (action != 'm' && params.iter().any(|param| param.len() != 1))
        {
            self.ignored_sequences = self.ignored_sequences.saturating_add(1);
            return;
        }
        match action {
            'H' | 'f' => {
                let row = Self::param(params, 0, 1).saturating_sub(1) as usize;
                let column = Self::param(params, 1, 1).saturating_sub(1) as usize;
                let (top, bottom) = self.vertical_bounds();
                self.cursor_row = (top + row).min(bottom);
                self.cursor_col = column.min(self.cols - 1);
            }
            'A' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                let top = if self.cursor_row >= self.scroll_top {
                    self.scroll_top
                } else {
                    0
                };
                self.cursor_row = self.cursor_row.saturating_sub(count).max(top);
            }
            'B' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                let bottom = if self.cursor_row <= self.scroll_bottom {
                    self.scroll_bottom
                } else {
                    self.rows - 1
                };
                self.cursor_row = (self.cursor_row + count).min(bottom);
            }
            'C' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                self.cursor_col = (self.cursor_col + count).min(self.cols - 1);
            }
            'D' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                self.cursor_col = self.cursor_col.saturating_sub(count);
            }
            'G' => {
                let column = Self::param(params, 0, 1).saturating_sub(1) as usize;
                self.cursor_col = column.min(self.cols - 1);
            }
            'd' => {
                let row = Self::param(params, 0, 1).saturating_sub(1) as usize;
                let (top, bottom) = self.vertical_bounds();
                self.cursor_row = (top + row).min(bottom);
                self.settle_cursor_col();
            }
            'J' => {
                let mode = Self::param(params, 0, 0);
                let (row, column) = self.cursor();
                match mode {
                    0 => {
                        self.clear_cells(row, column, self.cols);
                        for target in row + 1..self.rows {
                            self.clear_cells(target, 0, self.cols);
                        }
                    }
                    1 => {
                        for target in 0..row {
                            self.clear_cells(target, 0, self.cols);
                        }
                        self.clear_cells(row, 0, column + 1);
                    }
                    2 => {
                        for target in 0..self.rows {
                            self.clear_cells(target, 0, self.cols);
                        }
                    }
                    3 => self.clear_scrollback(),
                    _ => self.ignored_sequences = self.ignored_sequences.saturating_add(1),
                }
            }
            'K' => {
                let mode = Self::param(params, 0, 0);
                let (row, column) = self.cursor();
                match mode {
                    0 => self.clear_cells(row, column, self.cols),
                    1 => self.clear_cells(row, 0, column + 1),
                    _ => self.clear_cells(row, 0, self.cols),
                }
            }
            'X' => {
                let count = Self::param(params, 0, 1) as usize;
                let (row, column) = self.cursor();
                self.clear_cells(row, column, column + count);
            }
            'P' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                let row = self.cursor_row;
                let column = self.cursor_col;
                for target in column..self.cols {
                    self.grid[row * self.cols + target] = if target + count < self.cols {
                        self.grid[row * self.cols + target + count]
                    } else {
                        VtCell::blank()
                    };
                }
            }
            '@' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                let row = self.cursor_row;
                let column = self.cursor_col;
                for target in (column..self.cols).rev() {
                    self.grid[row * self.cols + target] = if target >= column + count {
                        self.grid[row * self.cols + target - count]
                    } else {
                        VtCell::blank()
                    };
                }
            }
            'L' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                if (self.scroll_top..=self.scroll_bottom).contains(&self.cursor_row) {
                    self.scroll_down_from(self.cursor_row, count);
                }
            }
            'M' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                if (self.scroll_top..=self.scroll_bottom).contains(&self.cursor_row) {
                    self.scroll_up_from(self.cursor_row, count, false);
                }
            }
            'S' => {
                let count = Self::param(params, 0, 1) as usize;
                self.scroll_up_from(self.scroll_top, count, true);
            }
            'T' => {
                let count = Self::param(params, 0, 1) as usize;
                self.scroll_down_from(self.scroll_top, count);
            }
            'r' => {
                let top = Self::param(params, 0, 1).saturating_sub(1) as usize;
                let bottom = Self::param(params, 1, self.rows as u16).saturating_sub(1) as usize;
                // A scrolling region needs at least two rows. Invalid requests
                // do not disturb the last accepted region or cursor.
                if params.len() <= 2 && top < bottom && bottom < self.rows {
                    self.scroll_top = top;
                    self.scroll_bottom = bottom;
                    self.home_cursor();
                }
            }
            'm' => self.apply_sgr(params),
            _ => self.ignored_sequences = self.ignored_sequences.saturating_add(1),
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        if ignore || !intermediates.is_empty() {
            self.ignored_sequences = self.ignored_sequences.saturating_add(1);
            return;
        }
        match byte {
            b'M' => {
                self.settle_cursor_col();
                if self.cursor_row == self.scroll_top {
                    self.scroll_down_from(self.scroll_top, 1);
                } else {
                    self.cursor_row = self.cursor_row.saturating_sub(1);
                }
            }
            b'D' => self.linefeed(),
            b'E' => {
                self.cursor_col = 0;
                self.linefeed();
            }
            b'7' => self.save_cursor(),
            b'8' => self.restore_cursor(),
            b'c' => {
                let cols = self.cols;
                let rows = self.rows;
                let limit = self.scrollback_limit;
                let total = self.scrollback_total;
                let epoch = self.scrollback_epoch.saturating_add(1);
                *self = Self::new(cols, rows);
                self.scrollback_limit = limit;
                self.scrollback_total = total;
                self.scrollback_epoch = epoch;
            }
            _ => self.ignored_sequences = self.ignored_sequences.saturating_add(1),
        }
    }
}
