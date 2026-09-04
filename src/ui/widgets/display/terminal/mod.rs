use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};

// 保存由 UIX 声明的 Terminal 固有尺寸与内容留白。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TerminalGeometryVisual {
    default_width: f32,
    default_height: f32,
    row_height: f32,
    padding_x: f32,
    padding_y: f32,
}

// 保存由 UIX 声明的输入行光标、提示符间距与焦点装饰视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TerminalChromeVisual {
    font_size: f32,
    cursor_width: f32,
    cursor_height_ratio: f32,
    cursor_char_width: f32,
    cursor_base_width: f32,
    prompt_spacing: f32,
    focus_inset: f32,
    focus_stroke: f32,
    border_stroke: f32,
}

// 保存由 UIX 声明的 Terminal 主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TerminalPaletteVisual {
    background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    muted: ColorValue,
    prompt: ColorValue,
    cursor: ColorValue,
    primary: ColorValue,
    success: ColorValue,
    warning: ColorValue,
    error: ColorValue,
    info: ColorValue,
}

// 全部 Terminal 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TerminalVisual {
    geometry: TerminalGeometryVisual,
    chrome: TerminalChromeVisual,
    palette: TerminalPaletteVisual,
}

// 同目录 UIX 生成几何、装饰、色板与根视觉记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/terminal/terminal.uix");

// 保存 Terminal 每帧只解析一次的主题颜色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedTerminalVisual {
    pub(crate) background: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) muted: Color,
    pub(crate) prompt: Color,
    pub(crate) cursor: Color,
    pub(crate) primary: Color,
    pub(crate) success: Color,
    pub(crate) warning: Color,
    pub(crate) error: Color,
    pub(crate) info: Color,
}

impl TerminalVisual {
    pub(crate) fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedTerminalVisual {
        ResolvedTerminalVisual {
            background: self.palette.background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            muted: self.palette.muted.resolve(tokens),
            prompt: self.palette.prompt.resolve(tokens),
            cursor: self.palette.cursor.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            success: self.palette.success.resolve(tokens),
            warning: self.palette.warning.resolve(tokens),
            error: self.palette.error.resolve(tokens),
            info: self.palette.info.resolve(tokens),
        }
    }
}

// 向 UIX 提供主题角色。
const fn terminal_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
const fn terminal_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
const fn terminal_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
const fn terminal_muted_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
const fn terminal_prompt_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn terminal_cursor_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
const fn terminal_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn terminal_success_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Success)
}
const fn terminal_warning_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}
const fn terminal_error_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}
const fn terminal_info_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Info)
}

/// 终端文本段的语义颜色；全部经主题 token 解析，不暴露原始色值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalColor {
    /// 默认正文颜色。
    Default,
    /// 次级弱化颜色。
    Muted,
    /// 品牌主色。
    Primary,
    /// 成功状态颜色。
    Success,
    /// 警告状态颜色。
    Warning,
    /// 错误状态颜色。
    Error,
    /// 信息状态颜色。
    Info,
}

// 终端输出行内部的着色文本段。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TerminalSpan {
    text: String,
    color: TerminalColor,
}

/// 终端输出行：按声明顺序拼接的着色文本段集合。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TerminalLine {
    pub(crate) spans: Vec<TerminalSpan>,
}

impl TerminalLine {
    /// 创建不含任何文本段的空输出行。
    pub fn new() -> Self {
        Self::default()
    }

    /// 用单段默认颜色文本创建输出行。
    pub fn text(text: impl Into<String>) -> Self {
        Self::default().push(text)
    }

    /// 以默认颜色追加一个文本段。
    pub fn push(mut self, text: impl Into<String>) -> Self {
        self.spans.push(TerminalSpan {
            text: text.into(),
            color: TerminalColor::Default,
        });
        self
    }

    /// 以指定语义颜色追加一个文本段。
    pub fn styled(mut self, text: impl Into<String>, color: TerminalColor) -> Self {
        self.spans.push(TerminalSpan {
            text: text.into(),
            color,
        });
        self
    }

    /// 返回拼接全部文本段后的纯文本。
    pub fn plain(&self) -> String {
        let mut plain = String::new();
        for span in &self.spans {
            plain.push_str(&span.text);
        }
        plain
    }

    // 渲染内核按声明顺序读取的文本段视图。
    pub(crate) fn spans(&self) -> &[TerminalSpan] {
        &self.spans
    }
}

mod methods;
mod widget;

pub use widget::*;

// 把输出缓冲、交互状态与 UIX 静态视觉融合为单一根节点。
fn build_terminal_view(mut kernel: Terminal, visual: &'static TerminalVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Terminal {
    fn build(self) -> ViewNode {
        // UIX 拥有公开根与静态视觉；Rust 保留输出缓冲、命令输入与滚动算法。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/terminal/terminal.uix")
    }
}
