// 声明本文件只编译自定义组件使用文档，不启动窗口或原生资源。
#![allow(dead_code)]

// 隔离 widget-function 围栏中的函数组件。
mod widget_function {
    // 引入文档承诺的公开组件 prelude。
    use uix::prelude::*;

    // 组合公开节点并返回无私有状态的用户卡片。
    fn user_card(name: &str, role: &str) -> ViewNode {
        // 纵向组合用户名与角色文本。
        column((
            // 声明主要文本字号。
            label(name).font_size(16.0),
            // 声明次要文本字号和灰色语义。
            label(role).font_size(12.0).color(Color::gray()),
        ))
        // 声明两个文本节点之间的间距。
        .gap(4.0)
        // 声明卡片内容内边距。
        .padding(12.0)
        // 声明卡片背景色。
        .bg(Color::WHITE)
        // 声明卡片圆角。
        .radius(8.0)
    }
}

// 隔离 widget-canvas 围栏中的轻量绘制闭包。
mod widget_canvas {
    // 引入文档承诺的公开绘制 prelude。
    use uix::prelude::*;

    // 编译固定尺寸 canvas 的公开绘制闭包。
    fn compile_example() {
        // 构造只消费 frame 与 PaintContext 的画布节点。
        let _canvas = canvas(120.0, 40.0, |frame, ctx| {
            // 从当前主题令牌解析主色。
            let color = ctx.tokens().color_primary();
            // 记录圆角矩形绘制意图。
            ctx.fill_rounded_rect(frame, color, Radius::uniform(8.0));
        });
    }
}

// 隔离 widget-full 围栏中的完整事件组件。
mod widget_full {
    // 引入文档承诺的公开组件、事件与绘制 prelude。
    use uix::prelude::*;

    // 声明带计数状态、测量、事件和绘制能力的完整组件。
    widget! {
        // 公开 Counter 类型及其状态字段。
        pub struct Counter {
            // 保存当前点击计数。
            pub count: u32,
        }
        // 声明公开默认构造入口。
        @new -> Self {
            // 初始化计数状态。
            Self { count: 0 }
        }

        // 声明有限固有尺寸并服从父级约束。
        measure => (&self, constraints: Constraints) -> Size {
            // 把固有尺寸夹入当前约束。
            constraints.clamp(Size::new(120.0, 36.0))
        }

        // 声明指针按下时递增的同步事件处理。
        on_event => (&mut self, event: &SystemEvent) -> EventResult {
            // 按公开系统事件类型分派。
            match event {
                // 指针按下建立一次点击事实。
                SystemEvent::PointerDown { .. } => {
                    // 更新组件私有计数状态。
                    self.count += 1;
                    // 告知框架事件已经处理。
                    EventResult::Handled
                }
                // 其它事件继续交给外层路由。
                _ => EventResult::NotHandled,
            }
        }

        // 声明只记录绘制意图的 render 能力。
        render => (&self, frame: Rect, ctx: &mut PaintContext) {
            // 从当前主题取得主色背景。
            let bg = ctx.tokens().color_primary_bg();
            // 从当前主题取得主色前景。
            let color = ctx.tokens().color_primary();
            // 绘制组件背景。
            ctx.fill_rect(frame, bg, None);
            // 绘制当前计数文本。
            ctx.draw_text(
                // 格式化拥有型计数展示文本。
                &format!("Count: {}", self.count),
                // 在组件左上角保留八像素内边距。
                Point::new(frame.x + 8.0, frame.y + 8.0),
                // 使用主题主色绘制文字。
                color,
                // 使用固定逻辑字号。
                14.0,
            );
        }
    }
}

// 隔离 widget-paint-context 围栏中的 render 片段。
mod widget_paint_context {
    // 引入片段宿主所需的公开组件与绘制 prelude。
    use uix::prelude::*;

    // 为文档中的 render 片段提供最小公开 widget 宿主。
    widget! {
        // 声明无需私有字段的绘制探针。
        pub struct PaintContextProbe {}
        // 声明最小公开构造入口。
        @new -> Self {
            // 构造无状态绘制探针。
            Self {}
        }

        // 按围栏原样编译令牌与绘制原语调用。
        render => (&self, frame: Rect, ctx: &mut PaintContext) {
            // 语义色来自主题令牌，不硬编码。
            let primary = ctx.tokens().color_primary();
            // 背景语义色同样来自主题令牌。
            let bg = ctx.tokens().color_primary_bg();

            // 绘制圆角矩形背景。
            ctx.fill_rounded_rect(frame, bg, Radius::uniform(8.0));
            // 绘制圆形装饰。
            ctx.fill_circle(frame.x + 20.0, frame.y + 20.0, 10.0, primary);
            // 绘制公开文本原语。
            ctx.draw_text(
                // 声明静态展示文本。
                "hello",
                // 声明文本左上角位置。
                Point::new(frame.x + 8.0, frame.y + 8.0),
                // 使用主题主色。
                primary,
                // 使用固定逻辑字号。
                14.0,
            );
        }
    }
}

// 隔离 widget-timer-badge 围栏中的动画脏区组件。
mod widget_timer_badge {
    // 引入文档承诺的公开组件与绘制 prelude。
    use uix::prelude::*;

    // 声明显式状态、脏区与绘制能力的计时徽标。
    widget! {
        // 公开 TimerBadge 类型及其状态字段。
        pub struct TimerBadge {
            // 保存当前秒数。
            pub seconds: u32,
            // 保存脉冲强调状态。
            pub pulse: bool,
        }
        // 声明公开默认构造入口。
        @new -> Self {
            // 初始化计时与脉冲状态。
            Self {
                // 从零秒开始。
                seconds: 0,
                // 默认不显示警告脉冲。
                pulse: false,
            }
        }

        // 声明有限固有尺寸并服从父级约束。
        measure => (&self, constraints: Constraints) -> Size {
            // 把固有尺寸夹入当前约束。
            constraints.clamp(Size::new(160.0, 48.0))
        }

        // 声明只覆盖左侧脉冲区域的动画脏矩形。
        dirty_bounds => (&self, frame: Rect) -> Rect {
            // 返回与组件左侧四十八像素对齐的重绘区域。
            Rect::new(frame.x, frame.y, 48.0, frame.h)
        }

        // 声明根据脉冲状态选择语义色的绘制能力。
        render => (&self, frame: Rect, ctx: &mut PaintContext) {
            // 在警告色和主色之间选择当前文本颜色。
            let color = if self.pulse {
                // 脉冲阶段使用主题警告色。
                ctx.tokens().color_warning()
            } else {
                // 普通阶段使用主题主色。
                ctx.tokens().color_primary()
            };
            // 绘制当前秒数字符串。
            ctx.draw_text(
                // 格式化秒数展示文本。
                &format!("{}s", self.seconds),
                // 在组件左上角保留八像素内边距。
                Point::new(frame.x + 8.0, frame.y + 8.0),
                // 使用当前语义色。
                color,
                // 使用固定逻辑字号。
                16.0,
            );
        }
    }
}
