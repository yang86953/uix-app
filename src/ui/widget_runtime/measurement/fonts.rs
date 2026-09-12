//! 布局借用所属 UI 循环的真实字体服务；不另建字体注册表或布局缓存。

use std::{cell::RefCell, rc::Rc};

use crate::draw::resources::font::text_backend::{EstimatedTextMetrics, TextLayoutOptions};
use crate::draw::{FontService, HAlign, VAlign};
use crate::ui::theme::style::FontFamily;

thread_local! {
    static LAYOUT_FONTS: RefCell<Option<Rc<FontService>>> = const { RefCell::new(None) };
}

/// 由 UI 组合根在建树前进入，覆盖同一循环内全部主/副窗；嵌套退出恢复原服务。
pub(crate) struct LayoutFontScope(Option<Rc<FontService>>);

impl LayoutFontScope {
    pub(crate) fn enter(fonts: Rc<FontService>) -> Self {
        Self(LAYOUT_FONTS.with(|current| current.replace(Some(fonts))))
    }
}

impl Drop for LayoutFontScope {
    fn drop(&mut self) {
        LAYOUT_FONTS.with(|current| current.replace(self.0.take()));
    }
}

/// 与绘制共享字体族、fallback、shaping 和 UAX #14；无窗口字体时才允许估算。
pub fn text_metrics(
    text: &str,
    max_width: f32,
    font_size: f32,
    line_height: f32,
    family: Option<&FontFamily>,
    word_wrap: bool,
) -> EstimatedTextMetrics {
    // 在调用字体服务之前释放 TLS 借用，避免嵌套测量占住可变上下文。
    let fonts = LAYOUT_FONTS.with(|current| current.borrow().clone());
    let Some(fonts) = fonts.filter(|fonts| fonts.font_family(&fonts.loaded_font_handle).is_some())
    else {
        return crate::draw::resources::font::text_backend::estimate_text_metrics(
            text,
            if word_wrap { max_width } else { f32::INFINITY },
            font_size,
        );
    };
    let font = family.map_or(fonts.loaded_font_handle, |family| {
        fonts.resolve_font_families(family.iter(), fonts.loaded_font_handle)
    });
    let mut options = TextLayoutOptions {
        max_width: 0.0,
        max_height: 0.0,
        font_size,
        line_height,
        word_wrap: false,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
    };
    let natural = fonts.layout_text_shared(&font, text, &options);
    let width_wrapped =
        word_wrap && max_width.is_finite() && max_width > 0.0 && natural.width > max_width;
    let layout = if width_wrapped {
        options.max_width = max_width;
        options.word_wrap = true;
        fonts.layout_text_shared(&font, text, &options)
    } else {
        natural
    };
    EstimatedTextMetrics {
        // 向外取整到逻辑像素，避免精确 hug 宽度经坐标减法后少一个浮点 ULP。
        max_line_width: layout.width.ceil(),
        line_count: layout.lines.len().max(1),
        width_wrapped,
    }
}
