    // 引入当前分隔线组件的私有契约。
    use super::*;

    // 验证 UIX 声明融合不新增节点且保留原有快照与固有几何。
    #[test]
    fn uix_shell_fuses_visuals_without_snapshot_or_geometry_drift() {
        // 构造带文字的垂直虚线，覆盖所有运行配置传递。
        let node = Divider::new()
            // 保留标签数据所有权。
            .with_text("Section")
            // 保留垂直方向。
            .vertical()
            // 保留虚线运行状态。
            .dashed()
            // 通过公开 View 契约进入 `.uix` 文件。
            .build();
        // 融合路径不得物化标签、线段或包装容器。
        assert!(node.children.is_empty());
        // 精确窄化根内核以读取声明配置与快照。
        let kernel = node
            // 借用根组件运行时类型视图。
            .widget
            // 窄化前转换为 Any。
            .as_any()
            // 精确窄化为 Divider。
            .downcast_ref::<Divider>()
            // 类型变化表示 UIX 融合路径破坏。
            .expect("UIX Divider 根必须保持 Divider 内核");
        assert!(std::ptr::eq(kernel.visual, DIVIDER_VISUAL_REF));
        assert!(
            std::mem::size_of::<DividerVisual>()
                > std::mem::size_of::<&'static DividerVisual>()
        );
        // UIX 必须保留标签字号与带标签固有高度。
        assert_eq!(
            (kernel.visual.text_size, kernel.visual.labelled_extent),
            (14.0, 24.0)
        );
        // 虚线段长与间距必须保留原契约。
        assert_eq!(
            (kernel.visual.dash_segment, kernel.visual.dash_gap),
            (6.0, 4.0)
        );
        // 垂直固有尺寸必须使用 UIX 声明的单像素厚度。
        assert_eq!(kernel.intrinsic_size(), Size::new(1.0, 0.0));
        // 公开快照仍必须反映 Rust 运行配置与 UIX 字号。
        assert_eq!(
            kernel.snapshot_fields(),
            SnapshotFields::Divider {
                text: Some(String::from("Section")),
                orientation: DividerOrientation::Center,
                direction: DividerDirection::Vertical,
                color: None,
                text_size: 14.0,
                dashed: true,
            }
        );
    }
