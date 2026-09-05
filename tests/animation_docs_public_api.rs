// 声明本文件只编译动画使用文档，不启动窗口或帧循环。
#![allow(dead_code)]

// 隔离 animation-animated 围栏中的声明式动画值。
mod animation_animated {
    // 引入文档承诺的公开动画 prelude。
    use uix::prelude::*;

    // 编译缓动、弹簧和关键帧三种 Animated 目标声明。
    fn compile_example() {
        // 声明从零过渡到一的缓动动画。
        let _fade = Animated::new(0.0_f32).to(1.0, 0.3, Easing::ease_out);

        // 声明使用默认弹簧参数的目标动画。
        let _bounce = Animated::new(0.0_f32).to_spring(1.0, Spring::default());

        // 声明包含三个采样点的关键帧动画。
        let _pulse = Animated::new(0.0_f32).to_keyframes(
            // 按归一化时间声明关键帧序列。
            vec![
                // 从零值开始。
                Keyframe::new(0.0, 0.0),
                // 在中点达到一并使用缓入缓出。
                Keyframe::new(0.5, 1.0).easing(Easing::ease_in_out),
                // 在终点返回零值。
                Keyframe::new(1.0, 0.0),
            ],
            // 声明总时长。
            1.2,
        );
    }
}

// 隔离 animation-bind 围栏中的 View 属性绑定。
mod animation_bind {
    // 引入文档承诺的公开动画与 View prelude。
    use uix::prelude::*;

    // 编译 Animated 当前值到 View 透明度属性的绑定。
    fn compile_example() {
        // 创建淡入动画源。
        let opacity = Animated::new(0.0_f32).to(1.0, 0.4, Easing::ease_out);
        // 克隆共享动画句柄供声明节点读取。
        let view_opacity = opacity.clone();

        // 构造读取动画当前值的内容列。
        let _view = column((
            // 把动画值绑定到标签透明度。
            label("淡入内容").opacity(view_opacity.value()),
        ))
        // 声明列内节点间距。
        .gap(8.0);
    }
}

// 隔离 animation-widget 围栏中的指令式动画组件。
mod animation_widget {
    // 引入文档承诺的公开组件、动画与绘制 prelude。
    use uix::prelude::*;

    // 声明由逐窗 frame opportunity 推进的脉冲圆环。
    widget! {
        // 公开 PulseRing 类型及其相位时间。
        pub struct PulseRing {
            // 保存累计动画秒数。
            pub time: f32,
        }
        // 声明公开默认构造入口。
        @new -> Self {
            // 从零相位开始。
            Self { time: 0.0 }
        }

        // 声明有限固有尺寸并服从父级约束。
        measure => (&self, constraints: Constraints) -> Size {
            // 把四十八像素固有尺寸夹入约束。
            constraints.clamp(Size::new(48.0, 48.0))
        }

        // 声明自绘圆环的重绘矩形。
        dirty_rect => (&self, frame: Rect) -> Rect {
            // 计算组件中心横坐标。
            let center_x = frame.x + frame.w * 0.5;
            // 计算组件中心纵坐标。
            let center_y = frame.y + frame.h * 0.5;
            // 返回覆盖最大圆环范围的矩形。
            Rect::new(center_x - 24.0, center_y - 24.0, 48.0, 48.0)
        }

        // 使用框架提供的真实帧间隔推进动画。
        update_animation => (&mut self, delta_seconds: f64) -> bool {
            // 累加当前帧经过的秒数。
            self.time += delta_seconds as f32;
            // 脉冲动画持续请求下一次 frame opportunity。
            true
        }

        // 声明只消费当前相位与主题令牌的绘制能力。
        render => (&self, frame: Rect, ctx: &mut PaintContext) {
            // 把累计时间映射为零到一的脉冲相位。
            let phase = (self.time * 1.5).sin() * 0.5 + 0.5;
            // 从相位计算当前圆环半径。
            let radius = 6.0 + phase * 16.0;
            // 从主题令牌解析主色。
            let color = ctx.tokens().color_primary();
            // 记录圆环描边绘制意图。
            ctx.stroke_circle(
                // 使用组件中心横坐标。
                frame.x + frame.w * 0.5,
                // 使用组件中心纵坐标。
                frame.y + frame.h * 0.5,
                // 使用当前脉冲半径。
                radius,
                // 使用主题主色。
                color,
                // 使用固定描边宽度。
                3.0,
            );
        }
    }
}

// 隔离 animation-enter 围栏中的挂载与离场过渡。
mod animation_enter {
    // 引入文档承诺的公开过渡与 View prelude。
    use uix::prelude::*;

    // 编译进场、离场和列表交错进场声明。
    fn compile_example() {
        // 声明首次挂载时播放的淡入动画。
        let _enter = label("淡入标题").enter_animation(AnimationConfig::fade_in(0.3));

        // 声明 keyed 移除时播放的淡出动画。
        let _leave = label("离场内容").leave_animation(AnimationConfig::fade_out(0.2));

        // 声明两个子项从底部交错进场的列表。
        let _staggered = column((
            // 声明首项滑入动画。
            label("A").enter_animation(AnimationConfig::slide_in(Placement::Bottom, 0.25)),
            // 声明次项滑入动画。
            label("B").enter_animation(AnimationConfig::slide_in(Placement::Bottom, 0.25)),
        ))
        // 在子项动画之间加入五十毫秒间隔。
        .stagger_enter(0.05, AnimationConfig::fade_in(0.25));
    }
}

// 隔离 animation-transition 围栏中的属性 Transition。
mod animation_transition {
    // 引入文档承诺的公开过渡、状态与 View prelude。
    use uix::prelude::*;

    // 编译静态淡入与响应式列表交错滑入声明。
    fn transition_view(items: &State<Vec<String>>) -> ViewNode {
        // 纵向组合静态标题和响应式列表。
        column((
            // 通过紧凑 Transition 入口声明标题淡入。
            label("列表").enter(Transition::fade_in(0.3)),
            // 把业务列表状态映射为声明式子树。
            items.map(|list| {
                // 按当前条目顺序构造交错过渡列。
                column(
                    // 遍历当前列表快照并保留索引延迟。
                    list.iter()
                        // 为每个条目创建对应的滑入节点。
                        .enumerate()
                        // 把索引映射为交错延迟。
                        .map(|(index, item)| {
                            // 为当前文本声明交错上滑过渡。
                            label(item).enter(Transition::stagger(
                                // 根据索引计算开始延迟。
                                0.05 * index as f64,
                                // 声明二百五十毫秒上滑过渡。
                                Transition::slide_up(0.25),
                            ))
                        })
                        // 收集为 column 接受的公开 View 集合。
                        .collect::<Vec<_>>(),
                )
            }),
        ))
    }
}
