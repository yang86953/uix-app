    // 引入待验证组件与事件处理 trait。
    use super::*;
    // 引入构造指针事件所需的几何与修饰键类型。
    use crate::core::Point;
    // 引入事件处理 trait 以调用组件契约。
    use crate::ui::{EventHandler, KeyMod, MouseButton};

    // 验证绑定状态决定可见性且激活写回顶部。
    #[test]
    fn bound_state_activation_writes_top() {
        // 创建超过默认阈值的应用滚动状态。
        let scroll_y = State::new(450.0_f32);
        // 绑定公开 BackTop 组件。
        let mut back_top = BackTop::new().scroll_state(&scroll_y);
        // 初始滚动位置应使按钮可见。
        assert!(back_top.is_visible());
        // 构造主指针激活事件。
        let result = back_top.on_event(&SystemEvent::PointerDown {
            // 坐标不影响组件自身的激活语义。
            pos: Point::zero(),
            // 只允许主按钮激活。
            button: MouseButton::Left,
            // 本测试不使用修饰键。
            mods: KeyMod::NONE,
        });
        // 激活必须由组件消费。
        assert_eq!(result, EventResult::Handled);
        // 应用拥有的状态必须被写回顶部。
        assert_eq!(scroll_y.get(), 0.0);
        // 当前组件必须立即隐藏。
        assert!(!back_top.is_visible());
    }

    // 验证 BackTop 通过 UIX 声明融合静态视觉且仍保持单叶节点。
    #[test]
    fn uix_shell_fuses_visual_contract_without_extra_nodes() {
        // 使用可见滚动快照物化真实 UIX 声明壳。
        let node = BackTop::new()
            // 超过默认阈值以保留固有尺寸。
            .scroll_y(480.0)
            // 调用公开 View 契约进入 `.uix` 文件。
            .build();
        // 融合路径不得创建 Icon 子节点或包装容器。
        assert!(node.children.is_empty());
        // 根组件仍必须是拥有滚动与交互状态的 BackTop 内核。
        let kernel = node
            // 借用根组件的运行时类型视图。
            .widget
            // 转换为 Any 以供精确窄化。
            .as_any()
            // 窄化为 BackTop。
            .downcast_ref::<BackTop>()
            // 类型变化表示 UIX 融合路径破坏。
            .expect("UIX BackTop 根必须保持 BackTop 内核");
        // 构建前后必须共享 UIX 生成的唯一静态地址，且实例只保存一个指针。
        assert!(std::ptr::eq(kernel.visual, BACK_TOP_VISUAL_REF));
        assert!(
            std::mem::size_of::<BackTopVisual>()
                > std::mem::size_of::<&'static BackTopVisual>()
        );
        // UIX 声明必须保留原有图标与尺寸。
        assert_eq!(kernel.visual.icon_name, "chevron-up");
        // 图标大小必须来自 UIX 参数。
        assert_eq!(kernel.visual.icon_size, 14.0);
        // 固有边长必须保持四十像素。
        assert_eq!(kernel.intrinsic_size(), Size::new(40.0, 40.0));
        // 环形几何必须保留旧版四成半径比例和两像素环宽。
        assert_eq!(
            (
                kernel.visual.ring_radius_ratio,
                kernel.visual.ring_width
            ),
            (0.4, 2.0)
        );
        // 主题色角色必须保留主色环和浮层内部。
        assert_eq!(
            (kernel.visual.ring_color, kernel.visual.fill_color),
            (
                ColorValue::Palette(PaletteColor::Primary),
                ColorValue::Neutral(NeutralRole::BgElevated),
            )
        );
    }
