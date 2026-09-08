//! 真实终端屏幕组件：把 `TerminalSession` 的 VT 网格投影为像素网格，
//! 并把键盘/文本输入原样编码转发给 PTY 中的程序。
//!
//! SMC 职责：本组件是纯投影与输入转发——不创建进程（会话必须显式传入）、
//! 不解释命令、不维护本地输入行；回显与编辑全部由 PTY 中的程序完成。
//! 与命令终端 `Terminal` 的关系：互不替代，本组件没有本地命令草稿或
//! 命令历史；仅独立浏览会话持有的滚回行，避免与程序自身回显重复。

use crate::core::{Constraints, Rect, Size};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{EventResult, KeyCode, KeyMod, SystemEvent, WidgetTree};
use crate::widget;
use std::cell::Cell;

use super::session::{TerminalSession, TerminalSessionStatus};
use super::vt::{ScrollbackState, TerminalColorSpec, TerminalRow, TerminalSpanStyle};
use super::{ResolvedTerminalVisual, TerminalScreenVisual};

widget! {
    /// 真实终端屏幕：渲染 PTY 会话的 VT 网格并转发原始按键。
    pub struct TerminalScreen {
        pub(crate) session: TerminalSession,
        pub(crate) focused: Cell<bool>,
        pub(crate) suppress_next_text: Cell<bool>,
        #[snapshot(skip)]
        pub(crate) alt_next_text: Cell<bool>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        pub(crate) synced_grid: Cell<(u16, u16)>,
        #[snapshot(skip)]
        pub(crate) history_offset: Cell<usize>,
        #[snapshot(skip)]
        pub(crate) observed_history: Cell<ScrollbackState>,
        #[snapshot(skip)]
        pub(crate) wheel_fraction: Cell<f64>,
        // 声明布局独立于会话运行态。
        #[snapshot(skip)]
        pub(crate) view_style: crate::ui::theme::style::Style,
        // 全部实例共享 UIX 声明固化后的只读视觉配置。
        #[snapshot(skip)]
        pub(crate) visual: &'static TerminalScreenVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    // 焦点在终端上时 Tab 是子进程输入（补全），不是焦点导航。
    consumes_tab_key => (&self) -> bool { true }

    // 屏幕接收文本输入与输入法（中文等经 TextInput 进入 PTY）。
    accepts_text_input => (&self) -> bool { true }

    // 只暴露当前视图的主屏历史滚动，不把共享会话变成共享导航状态。
    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        let offset = self.effective_history_offset();
        let history = self.observed_history.get();
        (!history.alternate && history.rows > 0).then_some((
            0.0,
            (history.rows - offset) as f32 * self.visual.geometry.row_height,
        ))
    }

    // 输入法候选窗定位到屏幕光标像素位置。
    text_input_cursor_rect => (&self) -> Rect {
        let frame = self.local_frame();
        let geometry = self.visual.geometry;
        let (row, column) = self.session.cursor();
        let (offset_y, cell_width) = self.grid_origin(frame);
        Rect::new(
            frame.x + geometry.padding_x + column as f32 * cell_width,
            frame.y + offset_y + row as f32 * geometry.row_height,
            cell_width,
            geometry.row_height,
        )
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_grow => (&self) -> f32 { self.view_style.flex_grow }
    flex_shrink => (&self) -> f32 { self.view_style.flex_shrink }
    layout_margin => (&self) -> crate::core::EdgeInsets { self.view_style.margin }
    align_self => (&self) -> Option<crate::ui::layout::AlignItems> { self.view_style.align_self }

    // 事件入口：真实按键编码进 PTY；可打印文本与粘贴走 TextInput/Paste。
    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::Wheel { pos, delta } => {
                if self.local_frame().contains(*pos) && self.scroll_history_wheel(delta.y) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => {
                self.focused.set(true);
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused.set(false);
                self.suppress_next_text.set(false);
                self.alt_next_text.set(false);
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, mods } => {
                self.alt_next_text.set(false);
                if self.browse_history_key(*key, *mods) {
                    self.suppress_next_text.set(false);
                    return EventResult::Handled;
                }
                let handled_as_control =
                    (*mods).intersects(KeyMod::CTRL | KeyMod::ALT | KeyMod::SUPER);
                match encode_key(*key, *mods, self.session.modes().application_cursor_keys) {
                    Some(bytes) => {
                        // 控制组合后的平台文本事件（如 Ctrl+A 附带 "a"）必须丢弃；
                        // 该文本只会紧随本次 KeyDown 到达，KeyUp 时统一清标记。
                        self.suppress_next_text.set(handled_as_control);
                        self.write_to_session(&bytes);
                        EventResult::Handled
                    }
                    None if mods.contains(KeyMod::ALT)
                        && !mods.intersects(KeyMod::CTRL | KeyMod::SUPER) => {
                        // 布局相关可打印字符由平台 TextInput 决定，不能从 KeyCode
                        // 猜字符；Alt 文本仍经同一事件添加 Meta 前缀。
                        self.suppress_next_text.set(false);
                        self.alt_next_text.set(true);
                        EventResult::Handled
                    }
                    None if handled_as_control => {
                        // 无法编码的控制组合也消费掉，避免字母落进程序。
                        self.suppress_next_text.set(true);
                        EventResult::Handled
                    }
                    None => {
                        // 可打印键走 TextInput；清掉可能残留的抑制标记。
                        self.suppress_next_text.set(false);
                        EventResult::NotHandled
                    }
                }
            }
            SystemEvent::KeyUp { .. } => {
                // 控制组合的伴随文本只存在于按下与抬起之间；抬起即过期，
                // 避免误杀后续输入（Agent 注入或无伴随文本的平台）。
                self.suppress_next_text.set(false);
                self.alt_next_text.set(false);
                EventResult::NotHandled
            }
            SystemEvent::TextInput { text } => {
                if self.suppress_next_text.replace(false) {
                    return EventResult::Handled;
                }
                let alt = self.alt_next_text.replace(false);
                if !text.is_empty() {
                    if alt {
                        let mut bytes = Vec::with_capacity(text.len() + 1);
                        bytes.push(0x1b);
                        bytes.extend_from_slice(text.as_bytes());
                        self.write_to_session(&bytes);
                    } else {
                        self.write_to_session(text.as_bytes());
                    }
                }
                EventResult::Handled
            }
            SystemEvent::Paste { text } => {
                self.suppress_next_text.set(false);
                self.alt_next_text.set(false);
                if !text.is_empty() {
                    self.paste_to_session(text);
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    // 渲染：容器底与边框、退出状态带、VT 网格逐字符绘制与焦点光标块。
    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 读取输出代际以建立本组件的精确重绘订阅（pump 后失效本节点）。
        let _revision = self.session.output_revision().get();
        let frame = Self::normalized_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            self.last_frame.set(Some(frame));
            return;
        }
        self.last_frame.set(Some(frame));
        let resolved = self.visual.resolve(ctx.tokens());
        let geometry = self.visual.geometry;

        // 容器背景与边框。
        ctx.fill_rect(frame, resolved.background, None);
        ctx.stroke_rect(frame, resolved.border, 1.0, None);

        // 网格尺寸同步：帧尺寸换算列行，变化时同步 PTY 与屏幕状态。
        let measured_cell = ctx.measure_text("M", geometry.font_size).w.max(1.0);
        let status = self.session.status();
        let status_band = matches!(status, TerminalSessionStatus::Exited { .. })
            .then_some(geometry.status_height)
            .unwrap_or(0.0);
        let inner_w = frame.w - geometry.padding_x * 2.0;
        let inner_h = frame.h - geometry.padding_y * 2.0 - status_band;
        let cols = ((inner_w / measured_cell).floor() as u16).max(2);
        let rows = ((inner_h / geometry.row_height).floor() as u16).max(2);
        if self.synced_grid.get() != (cols, rows) {
            self.synced_grid.set((cols, rows));
            if let Err(error) = self.session.resize(cols, rows) {
                crate::diagnostics::observe_boundary_error("ui::terminal-screen", &error);
            }
        }

        // 退出状态带：会话结束后在网格上方叠加一条关闭说明。
        if status_band > 0.0 {
            let band = Rect::new(
                frame.x + geometry.padding_x,
                frame.y + geometry.padding_y,
                frame.w - geometry.padding_x * 2.0,
                status_band,
            );
            match status {
                TerminalSessionStatus::Exited { code: Some(code) } => {
                    let text = format!("会话已退出（代码 {code}）");
                    ctx.draw_text_in_frame(&text, band, resolved.muted, geometry.font_size);
                }
                TerminalSessionStatus::Exited { code: None } => {
                    ctx.draw_text_in_frame(
                        "会话已被信号终止",
                        band,
                        resolved.muted,
                        geometry.font_size,
                    );
                }
                TerminalSessionStatus::Running => {}
            }
        }

        // VT 网格：逐字符按列定位绘制，保证网格对齐与宽字符占两列。
        let (grid_top, _) = self.grid_origin(frame);
        let grid_left = frame.x + geometry.padding_x;
        let grid_right = frame.x + frame.w - geometry.padding_x;
        ctx.push_clip(Rect::new(
            grid_left,
            grid_top,
            frame.w - geometry.padding_x * 2.0,
            inner_h.max(0.0),
        ));
        let history_offset = self.effective_history_offset();
        let snapshot = self.session.viewport_rows(history_offset);
        for (row_index, row) in snapshot.iter().enumerate() {
            let y = grid_top + row_index as f32 * geometry.row_height;
            if y + geometry.row_height < grid_top {
                continue;
            }
            if y > grid_top + inner_h {
                break;
            }
            draw_screen_row(ctx, row, grid_left, grid_right, y, measured_cell, geometry.row_height, geometry.font_size, &resolved);
        }
        ctx.pop_clip();

        // 焦点光标块：运行中且焦点可见时覆盖在光标格上。
        if self.focused.get()
            && history_offset == 0
            && self.session.modes().cursor_visible
            && tree.keyboard_focus_visible()
            && status == TerminalSessionStatus::Running
        {
            let (cursor_row, cursor_col) = self.session.cursor();
            let x = grid_left + cursor_col as f32 * measured_cell;
            let y = grid_top + cursor_row as f32 * geometry.row_height;
            ctx.fill_rect(
                Rect::new(x, y, measured_cell, geometry.row_height),
                with_alpha(resolved.cursor, 140),
                None,
            );
        }
    }
}

// 把一段带风格屏幕行按列网格逐字符绘制。
fn draw_screen_row(
    ctx: &mut PaintContext,
    row: &TerminalRow,
    grid_left: f32,
    grid_right: f32,
    y: f32,
    cell_width: f32,
    row_height: f32,
    font_size: f32,
    resolved: &ResolvedTerminalVisual,
) {
    let mut column = 0usize;
    let mut cell_text = [0u8; 4];
    for span in &row.spans {
        let (foreground, background) = resolve_paint_style(&span.style, resolved);
        for ch in span.text.chars() {
            let width = unicode_width::UnicodeWidthChar::width(ch)
                .unwrap_or(1)
                .max(1);
            let x = grid_left + column as f32 * cell_width;
            // 非默认背景按占列宽度铺底色。
            if background != resolved.background {
                ctx.fill_rect(
                    Rect::new(x, y, cell_width * width as f32, row_height),
                    background,
                    None,
                );
            }
            ctx.draw_text_in_frame(
                ch.encode_utf8(&mut cell_text),
                Rect::new(x, y, cell_width * width as f32, row_height),
                foreground,
                font_size,
            );
            column += width;
            if grid_left + column as f32 * cell_width >= grid_right {
                return;
            }
        }
    }
}

// 解析一段风格：inverse 交换前景背景，色板索引映射标准 xterm 颜色。
fn resolve_paint_style(
    style: &TerminalSpanStyle,
    resolved: &ResolvedTerminalVisual,
) -> (crate::draw::Color, crate::draw::Color) {
    let foreground = resolve_paint(style.fg, resolved, true, style.bold);
    let background = resolve_paint(style.bg, resolved, false, false);
    if style.inverse {
        (background, foreground)
    } else {
        (foreground, background)
    }
}

// 单个颜色指定映射为像素颜色；默认色取主题 token。
fn resolve_paint(
    spec: TerminalColorSpec,
    resolved: &ResolvedTerminalVisual,
    foreground: bool,
    _bold: bool,
) -> crate::draw::Color {
    match spec {
        TerminalColorSpec::Default => {
            if foreground {
                resolved.text
            } else {
                resolved.background
            }
        }
        TerminalColorSpec::Palette(index) => xterm_palette_color(index),
        TerminalColorSpec::Rgb(r, g, b) => crate::draw::Color::from_rgb(r, g, b),
    }
}

// 标准 xterm 256 色板：0-15 基础色、16-231 6×6×6 立方、232-255 灰阶。
fn xterm_palette_color(index: u8) -> crate::draw::Color {
    const BASIC: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    match index {
        0..=15 => {
            let (r, g, b) = BASIC[index as usize];
            crate::draw::Color::from_rgb(r, g, b)
        }
        16..=231 => {
            let cube = (index - 16) as usize;
            let levels = [0, 95, 135, 175, 215, 255];
            let r = levels[cube / 36];
            let g = levels[(cube / 6) % 6];
            let b = levels[cube % 6];
            crate::draw::Color::from_rgb(r, g, b)
        }
        _ => {
            let gray = 8 + (index - 232) as u8 * 10;
            crate::draw::Color::from_rgb(gray, gray, gray)
        }
    }
}

fn with_alpha(color: crate::draw::Color, alpha: u8) -> crate::draw::Color {
    let rgba = color.to_rgba();
    crate::draw::Color::from_rgba(
        (rgba >> 24) as u8,
        (rgba >> 16) as u8,
        (rgba >> 8) as u8,
        alpha,
    )
}

// 键盘事件到 PTY 字节序列的编码（xterm 惯例）。
// 可打印字符不在此编码，交给紧随其后的 TextInput 事件。
fn encode_key(key: KeyCode, mods: KeyMod, application_cursor: bool) -> Option<Vec<u8>> {
    // Super 组合留在宿主快捷键层；不能伪装成无修饰的终端按键。
    if mods.contains(KeyMod::SUPER) {
        return None;
    }
    let modifier = 1
        + u8::from(mods.contains(KeyMod::SHIFT))
        + 2 * u8::from(mods.contains(KeyMod::ALT))
        + 4 * u8::from(mods.contains(KeyMod::CTRL));
    let cursor = match key {
        KeyCode::Up => Some('A'),
        KeyCode::Down => Some('B'),
        KeyCode::Right => Some('C'),
        KeyCode::Left => Some('D'),
        KeyCode::Home => Some('H'),
        KeyCode::End => Some('F'),
        _ => None,
    };
    if let Some(final_char) = cursor {
        return Some(if modifier > 1 {
            format!("\x1b[1;{modifier}{final_char}").into_bytes()
        } else {
            format!(
                "\x1b{}{final_char}",
                if application_cursor { 'O' } else { '[' }
            )
            .into_bytes()
        });
    }
    let function = match key {
        KeyCode::F1 => Some('P'),
        KeyCode::F2 => Some('Q'),
        KeyCode::F3 => Some('R'),
        KeyCode::F4 => Some('S'),
        _ => None,
    };
    if let Some(final_char) = function {
        return Some(if modifier > 1 {
            format!("\x1b[1;{modifier}{final_char}").into_bytes()
        } else {
            format!("\x1bO{final_char}").into_bytes()
        });
    }
    let code = match key {
        KeyCode::Insert => Some(2),
        KeyCode::Delete => Some(3),
        KeyCode::PageUp => Some(5),
        KeyCode::PageDown => Some(6),
        KeyCode::F5 => Some(15),
        KeyCode::F6 => Some(17),
        KeyCode::F7 => Some(18),
        KeyCode::F8 => Some(19),
        KeyCode::F9 => Some(20),
        KeyCode::F10 => Some(21),
        KeyCode::F11 => Some(23),
        KeyCode::F12 => Some(24),
        _ => None,
    };
    if let Some(code) = code {
        return Some(if modifier > 1 {
            format!("\x1b[{code};{modifier}~").into_bytes()
        } else {
            format!("\x1b[{code}~").into_bytes()
        });
    }
    // Ctrl 组合：字母/数字/空格转控制字节。
    if mods.contains(KeyMod::CTRL) {
        let control = match key {
            KeyCode::A => Some(0x01),
            KeyCode::B => Some(0x02),
            KeyCode::C => Some(0x03),
            KeyCode::D => Some(0x04),
            KeyCode::E => Some(0x05),
            KeyCode::F => Some(0x06),
            KeyCode::G => Some(0x07),
            KeyCode::H => Some(0x08),
            KeyCode::I => Some(0x09),
            KeyCode::J => Some(0x0a),
            KeyCode::K => Some(0x0b),
            KeyCode::L => Some(0x0c),
            KeyCode::M => Some(0x0d),
            KeyCode::N => Some(0x0e),
            KeyCode::O => Some(0x0f),
            KeyCode::P => Some(0x10),
            KeyCode::Q => Some(0x11),
            KeyCode::R => Some(0x12),
            KeyCode::S => Some(0x13),
            KeyCode::T => Some(0x14),
            KeyCode::U => Some(0x15),
            KeyCode::V => Some(0x16),
            KeyCode::W => Some(0x17),
            KeyCode::X => Some(0x18),
            KeyCode::Y => Some(0x19),
            KeyCode::Z => Some(0x1a),
            KeyCode::Space => Some(0x00),
            KeyCode::Num0 | KeyCode::Num2 => Some(0x00),
            KeyCode::Num1 => Some(0x01),
            KeyCode::Num3 => Some(0x03),
            KeyCode::Num4 => Some(0x04),
            KeyCode::Num5 => Some(0x05),
            KeyCode::Num6 => Some(0x06),
            KeyCode::Num7 => Some(0x07),
            KeyCode::Num8 => Some(0x08),
            KeyCode::Num9 => Some(0x09),
            _ => None,
        };
        return control.map(|byte| alt_prefix(vec![byte], mods));
    }
    let base: Vec<u8> = match key {
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Backspace => b"\x7f".to_vec(),
        KeyCode::Tab => b"\t".to_vec(),
        KeyCode::Escape => b"\x1b".to_vec(),
        _ => return None,
    };
    // Shift+Tab 是反向 Tab（BackTab）。
    if key == KeyCode::Tab && mods.contains(KeyMod::SHIFT) {
        return Some(b"\x1b[Z".to_vec());
    }
    Some(alt_prefix(base, mods))
}

// Alt 修饰用 ESC 前缀表达（Meta 键惯例）。
fn alt_prefix(mut bytes: Vec<u8>, mods: KeyMod) -> Vec<u8> {
    if mods.contains(KeyMod::ALT) {
        bytes.insert(0, 0x1b);
    }
    bytes
}
