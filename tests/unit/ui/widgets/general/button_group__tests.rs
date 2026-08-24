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

    // 验证 ButtonGroup 由 UIX 物化横向零间距容器，同时保留 Rust 位置语义。
    #[test]
    fn button_group_builds_uix_row_with_positioned_children() {
        // 构造三个可区分的连体按钮。
        let node = ButtonGroup::new()
            // 交给按钮组按源顺序标记位置。
            .buttons(vec![Button::new("A"), Button::new("B"), Button::new("C")])
            // 触发 UIX 声明壳物化。
            .build();
        // UIX 根必须是零间距横向 Container。
        match node.widget.snapshot_fields() {
            // 读取容器样式快照。
            SnapshotFields::Container { style } => {
                // 方向必须为横向。
                assert_eq!(
                    style.flex_direction,
                    crate::ui::theme::style::FlexDirection::Row
                );
                // 连体按钮之间不留空隙。
                assert_eq!(style.gap, 0.0);
            }
            // 其他根类型表示 UIX 声明壳未生效。
            _ => panic!("ButtonGroup 必须物化为 UIX Container"),
        }
        // 列表桥接必须不增减按钮。
        assert_eq!(node.children.len(), 3);
        // 依次验证左、中、右位置语义。
        for (child, expected) in node.children.iter().zip([
            ButtonGroupPosition::Left,
            ButtonGroupPosition::Middle,
            ButtonGroupPosition::Right,
        ]) {
            // 按钮快照必须保留 Rust 内核计算的位置。
            match child.widget.snapshot_fields() {
                // 读取连体位置。
                SnapshotFields::Button { group_position, .. } => {
                    // 位置必须与子项顺序一致。
                    assert_eq!(group_position, Some(expected));
                }
                // 列表桥接不得替换子按钮类型。
                _ => panic!("ButtonGroup 子项必须保持 Button"),
            }
        }
    }
