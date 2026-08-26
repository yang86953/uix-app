// 复用通知布局实现与父模块导入的组件类型。
use super::*;
// 引入通知状态级别以构造最小条目。
use crate::platform::capabilities::StatusLevel;

// 标记窄窗口双侧留白回归契约。
#[test]
// 验证固定宽度通知在不足 432 像素时主动缩窄。
fn notification_preserves_horizontal_insets_in_narrow_frame() {
    // 构造默认右上角通知容器。
    let notification = Notification::new();
    // 放入一条不自动消失的单行通知。
    notification.add(NotificationItem {
        // 使用信息级别避免业务差异影响几何。
        type_: StatusLevel::Info,
        // 提供最小标题内容。
        title: "窄窗口".to_owned(),
        // 保持描述为空以固定 48 像素条目高度。
        description: String::new(),
        // 禁用自动超时以保持测试条目稳定。
        duration_ms: 0,
        // 保留默认关闭控件以覆盖真实几何路径。
        closable: true,
    });
    // 选择无法同时容纳 384 像素通知与双侧留白的窗口。
    let frame = Rect::new(0.0, 0.0, 400.0, 200.0);
    // 读取绘制与交互共同消费的通知矩形。
    let rects = notification.interaction_rects_for_test(frame);
    // 确认唯一条目没有被可见性裁剪淘汰。
    assert_eq!(rects.len(), 1);
    // 取出动画变换后的实际条目矩形。
    let toast = rects[0].0;
    // 双侧各保留 24 像素，条目因此缩窄为 352 像素。
    assert_eq!(toast, Rect::new(24.0, 12.0, 352.0, 48.0));
    // 命中边界必须复用同一个动画后条目矩形。
    assert_eq!(notification.hit_bounds(frame), Some(toast));
    // 结束窄窗口通知几何契约。
}
// 结束通知布局单元测试模块。
