use super::tree_core::WidgetTree;
use super::*;

impl WidgetTree {
    pub fn render_geometry(&self, ctx: &mut crate::ui::render_context::RenderContext) {
        if let Some(root_id) = self.root_id { self.render_pass(root_id, ctx); }
    }

    pub fn render_overlays(&self, ctx: &mut crate::ui::render_context::RenderContext) {
        if let Some(root_id) = self.root_id { self.post_render_pass(root_id, ctx); }
    }

    pub fn render_tree(&self, ctx: &mut crate::ui::render_context::RenderContext) {
        self.render_geometry(ctx);
        self.render_overlays(ctx);
    }

    fn render_pass(&self, id: WidgetId, ctx: &mut crate::ui::render_context::RenderContext) {
        if let Some(node) = self.get(id) {
            if !node.visible() { return; }
            let frame = node.frame();
            ctx.save();
            node.inner().render(frame, ctx, self);
            let clip = node.inner().children_clip(frame);
            if let Some(rect) = clip { ctx.engine().push_clip_rect(rect); }
            let mut sorted: Vec<WidgetId> = node.children().to_vec();
            sorted.sort_by_key(|&cid| self.get(cid).map_or(0, |c| c.z_index()));
            for &child_id in &sorted { self.render_pass(child_id, ctx); }
            if let Some(_) = clip { ctx.engine().pop_clip_rect(); }
            ctx.restore();
        }
    }

    fn post_render_pass(&self, id: WidgetId, ctx: &mut crate::ui::render_context::RenderContext) {
        if let Some(node) = self.get(id) {
            if !node.visible() { return; }
            let frame = node.frame();
            ctx.save();
            node.inner().post_render(frame, ctx, self);
            let mut sorted: Vec<WidgetId> = node.children().to_vec();
            sorted.sort_by_key(|&cid| self.get(cid).map_or(0, |c| c.z_index()));
            for &child_id in &sorted { self.post_render_pass(child_id, ctx); }
            ctx.restore();
        }
    }
}
