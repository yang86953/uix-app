// 引入真实窗口逻辑矩形以固定父级验收表面。
use crate::core::Rect;
// 使用声明适配器物化完整父子组件树。
use crate::ui::adapter::ViewAdapter;
// 引入组件 frame 的只读与测试写入入口。
use crate::ui::widget_runtime::widget::WidgetCore;
// 使用声明节点组合父级 Row 与两个 Popconfirm。
use crate::ui::view::ViewNode;
// 使用真实按钮和行容器验证自然尺寸向上传递。
use crate::ui::widgets::{button, row};

// 引入同模块被测的 Popconfirm 组件。
use super::Popconfirm;

// 标记透明包装组件必须在父级测量轮次暴露真实 trigger 尺寸。
#[test]
// 两个组合 trigger 在 Row 中必须占用互不重叠的自然 border-box。
fn composite_triggers_report_natural_size_to_parent_row() {
    // 构造两个长文本 trigger，旧零尺寸代理会把它们安排在同一原点。
    let view = row((
        // 第一个 Popconfirm 持有真实危险按钮。
        ViewNode::leaf(Popconfirm::new().trigger_view(button("很长的自定义删除按钮 · Left"))),
        // 第二个 Popconfirm 持有独立的真实主按钮。
        ViewNode::leaf(Popconfirm::new().trigger_view(button("自定义按钮 · Right"))),
    ));
    // 通过真实适配器发布父行、透明包装节点与按钮子树。
    let mut tree = ViewAdapter::build(view);
    // 取得父级 Row 的稳定根身份。
    let root = tree.root_id().expect("父级 Row 必须存在");
    // 读取两个直接 Popconfirm 包装节点。
    let wrappers = tree
        // 根节点必须在发布后保持可读。
        .get(root)
        // 缺失根节点表示适配器发布失败。
        .expect("父级 Row 必须可读")
        // 只复制两个轻量组件身份。
        .children()
        // 测试需要在后续可变布局期间保留身份列表。
        .to_vec();
    // 验证测试树确实包含两个独立包装节点。
    assert_eq!(wrappers.len(), 2);
    // 为父级提供足够大的真实逻辑表面。
    tree.get_mut(root)
        // 根节点在布局前必须仍可写。
        .expect("父级 Row 必须可写")
        // 固定表面以隔离自然尺寸代理语义。
        .set_frame(Rect::new(0.0, 0.0, 600.0, 100.0));
    // 执行真实多阶段布局收敛。
    tree.layout();
    // 读取第一个包装节点的最终 frame。
    let first = tree
        // 第一个稳定身份必须继续可寻址。
        .get(wrappers[0])
        // 缺失包装节点表示布局错误删除了子树。
        .expect("第一个包装节点必须存在")
        // 取得最终 border-box。
        .frame();
    // 读取第二个包装节点的最终 frame。
    let second = tree
        // 第二个稳定身份必须继续可寻址。
        .get(wrappers[1])
        // 缺失包装节点表示布局错误删除了子树。
        .expect("第二个包装节点必须存在")
        // 取得最终 border-box。
        .frame();
    // 长 trigger 不得回退到零宽或旧八十像素兼容尺寸。
    assert!(first.w > 80.0);
    // 父级 Row 必须获得真实正高度。
    assert!(first.h > 0.0 && second.h > 0.0);
    // 第二个包装节点必须排在第一个真实 border-box 之后。
    assert!(second.x >= first.x + first.w);
}
