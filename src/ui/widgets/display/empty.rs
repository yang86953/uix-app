//! Empty widget — 空状态占位（图标 + 描述居中）。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

component! {
    /// Empty — 空状态展示。
    pub struct Empty {
        description: String,
        icon_name: String,
        image: String,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        let desc = if self.description.is_empty() { loc.empty_description } else { &self.description };
        let text_secondary = ctx.tokens().color_text_quaternary();
        let text_tertiary = ctx.tokens().color_text_tertiary();

        // image preset
        if !self.image.is_empty() {
            let icon_str = match self.image.as_str() {
                "default" => "📦",
                "search" => "🔍",
                "file" => "📄",
                "folder" => "📁",
                "network" => "🌐",
                _ => "📦",
            };
            let icon_frame = Rect::new(frame.x, frame.y + 4.0, frame.w, frame.h * 0.4);
            ctx.text_center(icon_str, icon_frame, text_tertiary, 36.0);
        }

        // 图标（25% 高度位置）
        if !self.icon_name.is_empty() && self.image.is_empty() {
            let icon_str = crate::ui::widgets::icon::icon_char(&self.icon_name);
            let saved = *ctx.font();
            if let Some(fh) = crate::ui::widgets::icon::lucide_handle() {
                ctx.set_font(fh);
            }
            let icon_frame = Rect::new(frame.x, frame.y + 8.0, frame.w, frame.h * 0.35);
            ctx.text_center(icon_str, icon_frame, text_secondary, 28.0);
            ctx.set_font(saved);
        }
        // 描述文字（图标下方居中）
        let desc_frame = Rect::new(frame.x, frame.y + frame.h * 0.45, frame.w, frame.h * 0.5);
        ctx.text_center(desc, desc_frame, text_secondary, 13.0);
    }
}

impl Default for Empty {
    fn default() -> Self {
        Self::new()
    }
}

impl Empty {
    pub fn new() -> Self {
        Self {
            description: String::new(),
            icon_name: String::new(),
            image: String::new(),
        }
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }
    pub fn icon(mut self, name: impl Into<String>) -> Self {
        self.icon_name = name.into();
        self
    }
    pub fn image(mut self, name: impl Into<String>) -> Self {
        self.image = name.into();
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(160.0, 100.0)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Empty {
            description: self.description.clone(),
            icon_name: self.icon_name.clone(),
            image: self.image.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.description = next.description;
        self.icon_name = next.icon_name;
        self.image = next.image;
    }
}
