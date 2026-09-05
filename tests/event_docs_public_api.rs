// 声明本文件只编译事件使用文档，不启动窗口或原生资源。
#![allow(dead_code)]

// 隔离 event-semantic 围栏中的语义事件入口。
mod event_semantic {
    // 引入文档承诺的公开事件与状态 prelude。
    use uix::prelude::*;

    // 编译绑定状态、精确语义匹配与无状态点击三种入口。
    fn compile_example() {
        // 创建由业务作用域拥有的计数状态。
        let count = State::new(0);

        // 构造通过公开状态句柄更新计数的按钮。
        let _state_button = button("+1")
            // 点击后在 State 所属线程更新整数值。
            .on_click(&count, |current| current.update(|value| *value += 1));

        // 先物化 ViewNode 再登记精确语义事件处理器。
        let _submit = button("提交").build().on_semantic(
            // 只匹配点击语义。
            SemanticKind::Click,
            // 保留应用提交回调的公开形态。
            |_| {
                // 文档示例不执行额外副作用。
            },
        );

        // 构造无需绑定 State 的自定义点击回调。
        let _notification = button("通知").on_click_fn(|| println!("clicked"));
    }
}

// 隔离 event-keyboard 围栏中的原始键盘事件组件。
mod event_keyboard {
    // 引入文档承诺的公开组件、事件与绘制 prelude。
    use uix::prelude::*;

    // 声明只在 Enter 按下后激活的自定义组件。
    widget! {
        // 公开 ShortcutPad 类型及其激活状态。
        pub struct ShortcutPad {
            // 保存 Enter 是否已触发。
            pub activated: bool,
        }
        // 声明公开默认构造入口。
        @new -> Self {
            // 初始化为尚未激活。
            Self { activated: false }
        }

        // 声明直接消费原始键盘输入的组件事件能力。
        on_event => (&mut self, event: &SystemEvent) -> EventResult {
            // 按公开系统事件类型分派。
            match event {
                // 只消费 Enter 键按下事实。
                SystemEvent::KeyDown { key, .. } if *key == KeyCode::Enter => {
                    // 更新组件私有激活状态。
                    self.activated = true;
                    // 告知框架事件已处理。
                    EventResult::Handled
                }
                // 其它事件继续交给外层路由。
                _ => EventResult::NotHandled,
            }
        }

        // 声明按激活状态消费主题令牌的绘制能力。
        render => (&self, frame: Rect, ctx: &mut PaintContext) {
            // 在成功色和普通文本色之间选择语义颜色。
            let color = if self.activated {
                // 激活后使用成功色。
                ctx.tokens().color_success()
            } else {
                // 未激活时使用普通文本色。
                ctx.tokens().color_text()
            };
            // 绘制公开快捷键提示文字。
            ctx.draw_text(
                // 声明固定展示文本。
                "ENTER",
                // 在组件左上角保留八像素内边距。
                Point::new(frame.x + 8.0, frame.y + 8.0),
                // 使用当前语义颜色。
                color,
                // 使用固定逻辑字号。
                14.0,
            );
        }
    }
}

// 隔离 event-a11y 围栏中的无障碍角色覆写。
mod event_a11y {
    // 引入文档承诺的公开组件与无障碍 prelude。
    use uix::prelude::*;

    // 编译图标与进度文本的公开角色声明。
    fn compile_example() {
        // 为设置图标声明可读图像角色。
        let _settings = embed(Icon::new("settings").size(16.0)).role(AccessibilityRole::Image);
        // 为进度文本声明进度条角色。
        let _progress = label("进度：60%").role(AccessibilityRole::ProgressBar);
    }
}

// 隔离 event-semantic-actions 围栏中的统一语义入口。
mod event_semantic_actions {
    // 引入文档承诺的公开语义动作与组件 prelude。
    use uix::prelude::*;

    // 为文档中的业务确认调用提供最小无副作用宿主。
    fn confirm_delete() {}

    // 编译键盘与指针汇入同一 Click 语义的按钮。
    fn semantic_delete_button() -> ViewNode {
        // 先物化 ViewNode 再登记公开语义处理器。
        button("删除").build().on_semantic(
            // 只消费点击语义。
            SemanticKind::Click,
            // 把统一语义入口交给业务确认函数。
            |_| {
                // 调用当前模块拥有的删除确认逻辑。
                confirm_delete();
            },
        )
    }

    // 声明无需私有 WidgetTree 类型的无障碍调整动作组件。
    widget! {
        // 公开音量滑块及其当前值。
        pub struct VolumeSlider {
            // 保存归一化音量值。
            pub level: f32,
        }
        // 声明公开默认构造入口。
        @new -> Self {
            // 使用中间音量作为初始值。
            Self { level: 0.5 }
        }

        // 声明连续值调整语义动作及其公开范围。
        semantic_actions => [SemanticAction::Adjust { min: 0.0, max: 1.0 }]

        // 使用公开三参数 render 形态声明无自绘内容。
        render => (&self, _frame: Rect, _ctx: &mut PaintContext) {}
    }
}
