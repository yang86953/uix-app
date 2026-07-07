//! Breadcrumb widget — 面包屑导航路径。

use crate::core::{Constraints, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::ui::WidgetTree;

/// 面包屑的一项。
#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    pub title: String,
    pub active: bool,
}

impl BreadcrumbItem {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            active: false,
        }
    }
    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }
}

define_widget! {
    /// Breadcrumb — 导航路径指示器。
    pub struct Breadcrumb {
        items: Vec<BreadcrumbItem>,
        separator: String,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        self.intrinsic_size()
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_color = ctx.tokens().color_text();
        let mut x = frame.x;
        let h = frame.h;
        for (i, item) in self.items.iter().enumerate() {
            let color = if item.active { text_color } else { text_secondary };
            let item_w = item.title.len() as f32 * 7.5 + 8.0;
            ctx.text_center(&item.title, Rect::new(x, frame.y, item_w, h), color, 13.0);
            x += item_w;
            if i < self.items.len() - 1 {
                let sep_w = self.separator.len() as f32 * 8.0 + 8.0;
                ctx.text_center(loc.breadcrumb_separator, Rect::new(x, frame.y, sep_w, h), text_secondary, 12.0);
                x += sep_w;
            }
        }
    }
}

impl Default for Breadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

impl Breadcrumb {
    fn intrinsic_size(&self) -> Size {
        if self.items.is_empty() {
            return Size::zero();
        }
        let mut w = 0.0f32;
        for (i, item) in self.items.iter().enumerate() {
            w += item.title.len() as f32 * 7.5 + 8.0;
            if i < self.items.len() - 1 {
                w += self.separator.len() as f32 * 8.0 + 8.0;
            }
        }
        Size::new(w, 22.0)
    }

    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            separator: "/".to_string(),
        }
    }
    pub fn item(mut self, item: BreadcrumbItem) -> Self {
        self.items.push(item);
        self
    }
    pub fn items(mut self, items: Vec<BreadcrumbItem>) -> Self {
        self.items = items;
        self
    }
    pub fn separator(mut self, s: impl Into<String>) -> Self {
        self.separator = s.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_breadcrumb_size() {
        let measured = Breadcrumb::new()
            .item(BreadcrumbItem::new("Home"))
            .item(BreadcrumbItem::new("Docs").active())
            .measure(Constraints::loose(Size::new(60.0, 16.0)));

        assert_eq!(measured, Size::new(60.0, 16.0));
    }
}
