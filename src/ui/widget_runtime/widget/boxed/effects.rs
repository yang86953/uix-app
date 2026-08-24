// 节点捕获 Effect 的替换与调度访问保持在 BoxedWidget 生命周期边界内。
impl super::BoxedWidget {
    // 整体替换此节点捕获的 Effect，保持协调时的声明替换语义。
    pub(crate) fn replace_captured_effects(
        // 接收将被更新的已挂载节点。
        &mut self,
        // 接收本轮声明捕获的完整 Effect 集合。
        effects: Vec<crate::ui::reactive::state::Effect>,
    ) {
        // 旧 Effect 随向量替换释放，不再被树调度。
        self.effects = effects.into_boxed_slice();
    }

    // 返回节点仍应被树调度的 Effect。
    pub(crate) fn effects(
        // 借用当前节点。
        &self,
        // 返回仅与当前节点生命周期绑定的 Effect 集合。
    ) -> &[crate::ui::reactive::state::Effect] {
        // 借用节点自身拥有的 Effect。
        &self.effects
    }
}
