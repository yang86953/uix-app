    // 引入当前主题切换组件的私有契约。
    use super::*;
    // 引入构造主指针事件所需的坐标和修饰键。
    use crate::core::Point;
    // 引入调用组件事件契约所需的 trait 与修饰键。
    use crate::ui::{EventHandler, KeyMod};

    // 验证 UIX 声明融合不新增节点且保留主题切换事实。
    #[test]
    fn uix_shell_fuses_visuals_and_preserves_toggle_behavior() {
        // 从暗色初始状态物化真实 UIX 声明壳。
        let mut node = ThemeToggle::new().dark(true).build();
        // 融合路径不得物化两个 Icon 子节点或包装容器。
        assert!(node.children.is_empty());
        // 可变借用根内核以同时验证配置与交互。
        let kernel = node
            // 借用根组件的可变运行时类型视图。
            .widget
            // 窄化前转换为 Any。
            .as_any_mut()
            // 精确窄化为 ThemeToggle。
            .downcast_mut::<ThemeToggle>()
            // 类型变化表示 UIX 融合路径破坏。
            .expect("UIX ThemeToggle 根必须保持 ThemeToggle 内核");
        assert!(std::ptr::eq(kernel.visual, THEME_TOGGLE_VISUAL_REF));
        assert!(
            std::mem::size_of::<ThemeToggleVisual>()
                > std::mem::size_of::<&'static ThemeToggleVisual>()
        );
        // UIX 声明必须保留两个无分配静态图标角色。
        assert_eq!(
            (kernel.visual.dark_icon, kernel.visual.light_icon),
            ("sun", "moon")
        );
        // 固有尺寸和图标尺寸必须与声明一致。
        assert_eq!(
            (kernel.visual.extent, kernel.visual.icon_size),
            (32.0, 18.0)
        );
        // 焦点环必须保留原有线宽与圆形比例。
        assert_eq!(
            (kernel.visual.focus_width, kernel.visual.focus_radius_ratio),
            (1.5, 0.5)
        );
        // 初始暗色状态必须保留。
        assert!(kernel.is_dark());
        // 主指针激活必须继续由 Rust 内核消费。
        assert_eq!(
            kernel.on_event(&SystemEvent::PointerDown {
                // 坐标不改变叶内核的切换语义。
                pos: Point::zero(),
                // 使用主指针键。
                button: MouseButton::Left,
                // 本测试不使用修饰键。
                mods: KeyMod::NONE,
            }),
            EventResult::Handled
        );
        // 激活后必须切换回亮色状态。
        assert!(!kernel.is_dark());
    }
