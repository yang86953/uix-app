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

    // 返回节点声明的视觉透明度。
    pub(crate) fn declared_opacity(&self) -> f32 {
        // 复制小型值类型，避免暴露内部可变状态。
        self.declared_opacity
    }

    // 替换节点声明的视觉透明度；非法值按无衰减归一。
    pub(crate) fn set_declared_opacity(&mut self, opacity: f32) {
        self.declared_opacity = normalize_declared_opacity(opacity);
    }

    // 解析当前布局帧上的完整视觉变换矩阵。
    pub(crate) fn visual_transform_matrix(&self) -> crate::draw::Transform {
        // 绝大多数节点没有过渡覆盖，直接解析基础声明以跳过中性矩阵组合。
        let Some(player) = self.view_transition.as_ref() else {
            return self.visual_transform.matrix(self.frame);
        };
        // 把过渡播放器状态转换为不含额外仿射矩阵的覆盖层。
        let transition = ViewTransform {
            // 使用过渡播放器的平移。
            offset: player.offset,
            // 使用过渡播放器的缩放。
            scale: player.scale,
            // 过渡动画仍只覆盖平移与缩放，不注入额外仿射内容。
            affine: crate::draw::Transform::identity(),
            // 过渡覆盖层使用默认值，占位但不覆盖基础声明原点。
            origin: crate::ui::TransformOrigin::default(),
        };
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

#[cfg(test)]
#[path = "../../../../../tests-src/ui/widget_runtime/widget/boxed/visual_metadata_tests.rs"]
mod tests;

