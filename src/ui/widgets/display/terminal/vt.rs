//! 真实终端的 VT 屏幕状态：把 PTY 字节流经 vte 状态机解析为行列网格。
//!
//! SMC 职责：本文件是纯数据 Component——只消费字节、维护网格与光标，
//! 不拥有进程、fd 或 UI；PTY 会话（`super::session`）是唯一写入口，
//! 绘制组件（`super::screen_widget`）是只读消费者。跨读取边界的 UTF-8
//! 由 vte 解析器的部分码点缓冲处理，本层不重复拆包。

use vte::{Params, Perform};

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
/// ED/EL/ECH/DCH/ICH、IL/DL/SU/SD、DECSC/DECRC、RI/IND/NEL、RIS 与 SGR
/// （0/1/7/22/27、30-37/39/90-97、40-47/49/100-107、38/48 的 5 与 2 扩展）。
pub(crate) struct VtScreen {
    cols: usize,
    rows: usize,
    grid: Vec<VtCell>,
    cursor_row: usize,
    cursor_col: usize,
    saved_cursor: Option<(usize, usize)>,
    style: TerminalSpanStyle,
    /// 收到但未实现的序列计数，供诊断与文档限制核对。
    pub(crate) ignored_sequences: usize,
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
            saved_cursor: None,
            style: TerminalSpanStyle::default(),
            ignored_sequences: 0,
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
        let mut grid = vec![VtCell::blank(); cols * rows];
        let keep_rows = rows.min(self.rows);
        let keep_cols = cols.min(self.cols);
        for row in 0..keep_rows {
            let source = row * self.cols;
            let target = row * cols;
            grid[target..target + keep_cols]
                .copy_from_slice(&self.grid[source..source + keep_cols]);
        }
        self.grid = grid;
        self.cols = cols;
        self.rows = rows;
        self.cursor_row = self.cursor_row.min(rows - 1);
        self.cursor_col = self.cursor_col.min(cols - 1);
    }

    /// 把整屏导出为按行文本段快照；宽字符右侧续格不产生文本。
    pub(crate) fn snapshot(&self) -> Vec<TerminalRow> {
        (0..self.rows)
            .map(|row| {
                let mut spans: Vec<TerminalRowSpan> = Vec::new();
                let mut column = 0;
                while column < self.cols {
                    let cell = self.grid[row * self.cols + column];
                    if cell.width == 0 {
                        column += 1;
                        continue;
                    }
                    let style = cell.style;
                    let mut text = String::new();
                    text.push(cell.ch);
                    // 合并右侧续格与后续同风格窄格。
                    let mut scan = column + cell.width as usize;
                    while scan < self.cols {
                        let next = self.grid[row * self.cols + scan];
                        if next.width == 0 {
                            // 宽字符续格不产出文本，仅越过。
                            scan += 1;
                            continue;
                        }
                        if next.style != style {
                            break;
                        }
                        text.push(next.ch);
                        scan += 1;
                    }
                    spans.push(TerminalRowSpan { text, style });
                    column = scan;
                }
                if spans.is_empty() {
                    spans.push(TerminalRowSpan {
                        text: String::new(),
                        style: TerminalSpanStyle::default(),
                    });
                }
                TerminalRow { spans }
            })
            .collect()
    }

    // 光标列可能因自动换行悬停在虚拟 cols 位置；写入前先落回最后实列。
    fn settle_cursor_col(&mut self) {
        if self.cursor_col >= self.cols {
            self.cursor_col = self.cols - 1;
        }
    }

    fn scroll_up(&mut self, count: usize) {
        for _ in 0..count {
            self.grid.drain(0..self.cols);
            self.grid
                .extend(std::iter::repeat_n(VtCell::blank(), self.cols));
        }
    }

    fn scroll_down(&mut self, count: usize) {
        for _ in 0..count {
            let tail = self.grid.len() - self.cols;
            self.grid.drain(tail..);
            let mut head = vec![VtCell::blank(); self.cols];
            head.extend_from_slice(&self.grid);
            self.grid = head;
        }
    }

    // 换行到下一行；底行触发向上滚动。
    fn linefeed(&mut self) {
        if self.cursor_row + 1 >= self.rows {
            self.scroll_up(1);
            self.cursor_row = self.rows - 1;
        } else {
            self.cursor_row += 1;
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
        if self.cursor_col >= self.cols {
            self.wrap_line();
        } else if self.cursor_col + width > self.cols {
            self.wrap_line();
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

    fn csi_dispatch(
        &mut self,
        params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        match action {
            'H' | 'f' => {
                let row = Self::param(params, 0, 1).saturating_sub(1) as usize;
                let column = Self::param(params, 1, 1).saturating_sub(1) as usize;
                self.cursor_row = row.min(self.rows - 1);
                self.cursor_col = column.min(self.cols - 1);
            }
            'A' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(count);
            }
            'B' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                self.cursor_row = (self.cursor_row + count).min(self.rows - 1);
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
                self.cursor_row = row.min(self.rows - 1);
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
                    _ => {
                        for target in 0..self.rows {
                            self.clear_cells(target, 0, self.cols);
                        }
                    }
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
                let start = self.cursor_row * self.cols;
                for _ in 0..count {
                    self.grid.drain(start..start + self.cols);
                    self.grid
                        .extend(std::iter::repeat_n(VtCell::blank(), self.cols));
                }
            }
            'M' => {
                self.settle_cursor_col();
                let count = Self::param(params, 0, 1) as usize;
                let start = self.cursor_row * self.cols;
                let end = (self.cursor_row + count).min(self.rows) * self.cols;
                let removed = end.saturating_sub(start);
                self.grid.drain(start..end);
                self.grid
                    .extend(std::iter::repeat_n(VtCell::blank(), removed));
            }
            'S' => {
                let count = Self::param(params, 0, 1) as usize;
                self.scroll_up(count);
            }
            'T' => {
                let count = Self::param(params, 0, 1) as usize;
                self.scroll_down(count);
            }
            'm' => self.apply_sgr(params),
            _ => self.ignored_sequences += 1,
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'M' => {
                if self.cursor_row == 0 {
                    self.scroll_down(1);
                } else {
                    self.cursor_row -= 1;
                }
            }
            b'D' => self.linefeed(),
            b'E' => {
                self.cursor_col = 0;
                self.linefeed();
            }
            b'7' => self.saved_cursor = Some((self.cursor_row, self.cursor_col)),
            b'8' => {
                if let Some((row, column)) = self.saved_cursor {
                    self.cursor_row = row.min(self.rows - 1);
                    self.cursor_col = column.min(self.cols - 1);
                }
            }
            b'c' => {
                let cols = self.cols;
                let rows = self.rows;
                *self = Self::new(cols, rows);
            }
            _ => self.ignored_sequences += 1,
        }
    }
}
