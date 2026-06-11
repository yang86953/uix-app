use crate::graphics::{compute_flex_layout, AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent};
use crate::graphics::{Color};
use crate::base::{EdgeInsets, Rect, Size};
use crate::ui::widget::{Widget, WidgetId, WidgetTree};

/// Builder for constructing widget trees declaratively.
pub struct WidgetBuilder {
    children: Vec<Box<dyn Widget>>,
    direction: FlexDirection,
    justify: JustifyContent,
    align: AlignItems,
    padding: EdgeInsets,
    gap: f32,
    bg_color: Option<Color>,
    width: Option<f32>,
    height: Option<f32>,
    #[allow(dead_code)]
    min_width: Option<f32>,
    #[allow(dead_code)]
    min_height: Option<f32>,
    flex_grow: f32,
    flex_shrink: f32,
    visible: bool,
    opacity: f32,
}

impl Default for WidgetBuilder {
    fn default() -> Self {
        Self {
            children: Vec::new(),
            direction: FlexDirection::Row,
            justify: JustifyContent::Start,
            align: AlignItems::Stretch,
            padding: EdgeInsets::zero(),
            gap: 0.0,
            bg_color: None,
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            visible: true,
            opacity: 1.0,
        }
    }
}

impl WidgetBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn child(mut self, w: impl Widget + 'static) -> Self {
        self.children.push(Box::new(w));
        self
    }

    pub fn children(mut self, widgets: Vec<Box<dyn Widget>>) -> Self {
        self.children.extend(widgets);
        self
    }

    pub fn direction(mut self, d: FlexDirection) -> Self {
        self.direction = d;
        self
    }
    pub fn justify(mut self, j: JustifyContent) -> Self {
        self.justify = j;
        self
    }
    pub fn align(mut self, a: AlignItems) -> Self {
        self.align = a;
        self
    }
    pub fn padding(mut self, p: EdgeInsets) -> Self {
        self.padding = p;
        self
    }
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = Some(w);
        self.height = Some(h);
        self
    }
    pub fn flex(mut self, grow: f32, shrink: f32) -> Self {
        self.flex_grow = grow;
        self.flex_shrink = shrink;
        self
    }
    pub fn visible(mut self, v: bool) -> Self {
        self.visible = v;
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = o;
        self
    }

    /// Build the widget tree as a two-phase construction result.
    /// Returns (widget, children) — the caller adds children to the tree
    /// separately via `WidgetTree::add_child_with_children()`.
    /// This eliminates the `RefCell<Option<Vec<...>>>` workaround.
    pub fn build(self) -> (Box<dyn Widget>, Vec<Box<dyn Widget>>) {
        let children = self.children;
        let widget = Box::new(ContainerWidget {
            direction: self.direction,
            justify: self.justify,
            align: self.align,
            padding: self.padding,
            gap: self.gap,
            bg_color: self.bg_color,
            width: self.width,
            height: self.height,
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            visible: self.visible,
            opacity: self.opacity,
        });
        (widget, children)
    }
}

// ── Container widget built by the builder ──

#[allow(dead_code)]
struct ContainerWidget {
    direction: FlexDirection,
    justify: JustifyContent,
    align: AlignItems,
    padding: EdgeInsets,
    gap: f32,
    bg_color: Option<Color>,
    width: Option<f32>,
    height: Option<f32>,
    flex_grow: f32,
    flex_shrink: f32,
    visible: bool,
    opacity: f32,
}

impl Widget for ContainerWidget {
    fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let w = self.width.unwrap_or(0.0);
        let h = self.height.unwrap_or(0.0);
        Size::new(w, h)
    }

    fn render(
        &self,
        frame: Rect,
        ctx: &mut crate::ui::render_context::RenderContext,
        _tree: &WidgetTree,
    ) {
        if !self.visible {
            return;
        }
        // Fill background
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        if children.is_empty() { return Vec::new(); }

        let child_sizes: Vec<Size> = children
            .iter()
            .map(|&cid| {
                tree.get(cid)
                    .map(|c| c.preferred_size(None))
                    .unwrap_or_default()
            })
            .collect();

        let input = FlexInput {
            direction: self.direction,
            gap: self.gap,
            padding: self.padding,
            container: frame,
            children: vec![FlexChild::default(); children.len()],
            child_sizes,
            justify_content: self.justify,
            align_items: AlignItems::Stretch,
            ..FlexInput::default()
        };

        let output = compute_flex_layout(&input);
        children
            .iter()
            .zip(output.child_rects)
            .map(|(&cid, rect)| (cid, rect))
            .collect()
    }
}
