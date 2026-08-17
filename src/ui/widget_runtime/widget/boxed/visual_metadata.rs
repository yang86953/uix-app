// 引入父模块运行时节点与视觉变换类型。
use super::*;

// 集中实现不属于组件本体的视觉变换和指针光标元数据。
impl BoxedWidget {
    // 返回节点声明的基础视觉变换。
    pub(crate) fn visual_transform(&self) -> ViewTransform {
        // 复制小型值类型，避免暴露内部可变状态。
        self.visual_transform
    }

    // 替换节点声明的基础视觉变换。
    pub(crate) fn set_visual_transform(&mut self, transform: ViewTransform) {
        // 过渡动画仍由独立覆盖层组合。
        self.visual_transform = transform;
    }

    // 返回当前节点的显式光标覆盖。
    pub(crate) fn cursor(&self) -> Option<crate::platform::windowing::CursorType> {
        // 复制小型平台无关枚举，避免向查询方泄漏内部存储。
        self.cursor
    }

    // 替换当前节点的显式光标覆盖。
    pub(crate) fn set_cursor(&mut self, cursor: Option<crate::platform::windowing::CursorType>) {
        // None 恢复沿父链继承。
        self.cursor = cursor;
    }

    // 解析当前布局帧上的完整视觉变换矩阵。
    pub(crate) fn visual_transform_matrix(&self) -> crate::draw::Transform {
        // 读取可选过渡动画覆盖层。
        let transition = self
            // 借用当前过渡播放器。
            .view_transition
            // 仅在过渡存在时生成覆盖变换。
            .as_ref()
            // 把播放器状态转换为不含额外仿射矩阵的覆盖层。
            .map(|player| ViewTransform {
                // 使用过渡播放器的平移。
                offset: player.offset,
                // 使用过渡播放器的缩放。
                scale: player.scale,
                // 过渡动画仍只覆盖平移与缩放，不注入额外仿射内容。
                affine: crate::draw::Transform::identity(),
                // 过渡覆盖层使用默认值，占位但不覆盖基础声明原点。
                origin: crate::ui::TransformOrigin::default(),
            })
            // 无过渡时使用中性覆盖层。
            .unwrap_or_default();
        // 在实际帧上组合基础声明与过渡覆盖层。
        self.visual_transform
            // 保持基础原点并组合过渡平移缩放。
            .combined(transition)
            // 最终解析为绘制与命中共享矩阵。
            .matrix(self.frame)
    }

    // 判断当前节点是否存在非单位视觉变换。
    pub(crate) fn has_effective_visual_transform(&self) -> bool {
        // 统一通过完整矩阵判断，避免遗漏过渡覆盖层。
        !self.visual_transform_matrix().is_identity()
    }
}
