    // 引入当前模块公开的按钮组位置类型。
    use super::*;
    // 引入按钮构建器入口与组件快照字段。
    use crate::ui::{SnapshotFields, button};

    // 验证构建器物化时同时保留连体位置与事件处理器。
    #[test]
    fn button_builder_group_position_preserves_button_contract() {
        // 构造带点击处理器的中间位置按钮。
        let node = button("Middle")
            // 先登记按钮点击处理器。
            .on_click_fn(|| {})
            // 再把按钮物化为连体中间节点。
            .group_position(ButtonGroupPosition::Middle);
        // 物化不能丢失已登记的点击处理器。
        assert_eq!(node.handlers.len(), 1);
        // 底层组件快照必须记录中间位置。
        match node.widget.snapshot_fields() {
            // 检查 Button 快照中的连体位置。
            SnapshotFields::Button { group_position, .. } => {
                // 位置必须与构建器输入一致。
                assert_eq!(group_position, Some(ButtonGroupPosition::Middle));
            }
            // 其他组件类型表示构建器物化契约被破坏。
            _ => panic!("ButtonBuilder::group_position 必须生成 Button 节点"),
        }
    }

    // 验证 ButtonBuilder 把加载态交给既有 Button 生命周期。
    #[test]
    fn button_builder_loading_reaches_button_snapshot() {
        // 构造加载中的按钮并物化公开 ViewNode。
        let node: ViewNode = button("Loading")
            // 启用加载旋转器与交互禁用语义。
            .loading(true)
            // 使用公开 View 契约完成构建。
            .build();
        // 读取底层按钮的类型化快照。
        match node.widget.snapshot_fields() {
            // 检查加载字段保持为真。
            SnapshotFields::Button { loading, .. } => {
                // 构建器输入必须抵达既有 Button。
                assert!(loading);
            }
            // 其他组件类型表示构建器物化契约被破坏。
            _ => panic!("ButtonBuilder::loading 必须生成 Button 节点"),
        }
    }
