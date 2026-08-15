// 引入可插值颜色值。
use crate::draw::Color;
// 引入声明式动画源。
use crate::ui::animation::Animated;

// 引入实际声明节点。
use super::ViewNode;

// 为 ViewNode 集中提供不拥有调度生命周期的动画值适配。
impl ViewNode {
    /// 绑定动画宽度。
    pub fn width_animated(
        // 消费当前节点以延续 builder 链。
        self,
        // 借用由窗口帧调度器驱动的共享动画源。
        width: &Animated<f32>,
    ) -> Self {
        // 读取当前帧值并复用静态宽度入口。
        self.width(width.value())
    }

    /// 绑定动画高度。
    pub fn height_animated(
        // 消费当前节点以延续 builder 链。
        self,
        // 借用由窗口帧调度器驱动的共享动画源。
        height: &Animated<f32>,
    ) -> Self {
        // 读取当前帧值并复用静态高度入口。
        self.height(height.value())
    }

    /// 绑定动画圆角半径。
    pub fn radius_animated(
        // 消费当前节点以延续 builder 链。
        self,
        // 借用由窗口帧调度器驱动的共享动画源。
        radius: &Animated<f32>,
    ) -> Self {
        // 读取当前帧值并复用静态圆角入口。
        self.radius(radius.value())
    }

    /// 绑定动画透明度。
    pub fn opacity_animated(
        // 消费当前节点以延续 builder 链。
        self,
        // 借用由窗口帧调度器驱动的共享动画源。
        opacity: &Animated<f32>,
    ) -> Self {
        // 读取当前帧值并复用静态透明度入口。
        self.opacity(opacity.value())
    }

    /// 绑定动画前景或文本颜色。
    pub fn color_animated(
        // 消费当前节点以延续 builder 链。
        self,
        // 借用由窗口帧调度器驱动的共享动画源。
        color: &Animated<Color>,
    ) -> Self {
        // 读取当前帧值并复用静态前景色入口。
        self.color(color.value())
    }

    /// 绑定动画背景颜色。
    pub fn background_color_animated(
        // 消费当前节点以延续 builder 链。
        self,
        // 借用由窗口帧调度器驱动的共享动画源。
        color: &Animated<Color>,
    ) -> Self {
        // 读取当前帧值并复用静态背景色入口。
        self.background_color(color.value())
    }
}
