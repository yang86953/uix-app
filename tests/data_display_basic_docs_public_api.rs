// 声明本文件只编译基础数据展示文档，不启动窗口、计时循环或图片 I/O。
#![allow(dead_code)]

// 隔离 data-label 围栏中的短文本与多行文本。
mod data_label {
    // 引入文档承诺的文本与样式公开 prelude。
    use uix_app::prelude::*;

    // 编译字号与行高样式配置。
    fn compile_example() {
        // 构造固定字号短文本。
        let _short = label("短文本").font_size(14.0);
        // 由 Label 的统一样式持有经过校验的行高倍率。
        let _multiline = embed(Label::new("多行文本\n第二行").style(
            // 从公开样式入口构造行高样式。
            Style::default().with_line_height(
                // 校验正数倍率后再应用到标签。
                LineHeight::factor(1.5).expect("行高倍率必须为正值"),
            ),
        ));
    }
}

// 隔离 data-avatar 围栏中的文字回退与方形图片头像。
mod data_avatar {
    // 引入文档承诺的头像公开 prelude。
    use uix_app::prelude::*;

    // 编译文字头像与图片回退配置。
    fn compile_example() {
        // 构造四十像素的文字回退头像。
        let _text = embed(Avatar::new("AL").size(40.0));
        // 构造带可读回退名称的方形图片头像。
        let _image = embed(
            // 创建头像组件。
            Avatar::new("图片用户")
                // 声明图片资源路径。
                .src("assets/")
                // 启用方形裁剪。
                .square(true),
        );
    }
}

// 隔离 data-badge 围栏中的计数与透明装饰器。
mod data_badge {
    // 引入文档承诺的徽标与视图公开 prelude。
    use uix_app::prelude::*;

    // 编译颜色、计数上限与唯一子 View 配置。
    fn compile_example() {
        // 构造红色计数徽标。
        let _red = embed(Badge::new().count(5).color(BadgeColor::Red));
        // 构造超限显示为九十九加的徽标。
        let _capped = embed(Badge::new().count(120).max(99));
        // 构造装饰真实按钮且不接管点击的徽标。
        let _decorated = embed(Badge::new().count(5).child(button("通知")));
    }
}

// 隔离 calendar-basic 围栏中的非受控初值。
mod calendar_basic {
    // 引入文档承诺的日历公开 prelude。
    use uix_app::prelude::*;

    // 编译日期格尺寸与非受控默认日期。
    fn compile_example() {
        // 构造二〇二六年七月末的日历。
        let _calendar = embed(
            // 创建默认日历。
            Calendar::new()
                // 声明日期格首选尺寸。
                .cell_size(22.0)
                // 声明非受控初值。
                .default_date(Date::new(2026, 7, 31)),
        );
    }
}

// 隔离 calendar-controlled 围栏中的受控选择值。
mod calendar_controlled {
    // 引入文档承诺的日历与状态公开 prelude。
    use uix_app::prelude::*;

    // 编译业务 State 到日历选择值的双向绑定。
    fn compile_example() {
        // 创建由业务拥有的选中日期。
        let selected = State::new(Date::new(2026, 8, 15));
        // 把同一日期 State 绑定给日历组件。
        let _calendar = embed(Calendar::new().value(&selected));
    }
}

// 隔离 calendar-policy 围栏中的显示月份与日期策略。
mod calendar_policy {
    // 引入文档承诺的日历、状态与颜色公开 prelude。
    use uix_app::prelude::*;

    // 编译选择值、显示月份、事件快照与禁用策略。
    fn compile_example() {
        // 创建由业务拥有的选中日期。
        let selected = State::new(Date::new(2026, 8, 15));
        // 创建由组件消费的拥有型事件快照。
        let events = vec![CalendarEvent::new(
            // 声明产品发布日期。
            Date::new(2026, 9, 3),
            // 声明事件标题。
            "产品发布",
            // 声明事件颜色。
            Color::BLUE,
        )];
        // 构造带导航与日期策略的日历。
        let _calendar = embed(
            // 创建默认日历。
            Calendar::new()
                // 双向绑定选中日期。
                .value(&selected)
                // 独立声明初始显示月份。
                .default_displayed(2026, 9)
                // 声明日期格尺寸。
                .cell_size(36.0)
                // 启用年份跳转。
                .year_jump(true)
                // 提交事件快照。
                .events(events)
                // 禁用每月第一天。
                .disabled_date(|date| date.day == 1),
        );
    }
}

// 隔离 calendar-date-cell 围栏中的真实日期格子树。
mod calendar_date_cell {
    // 引入文档承诺的日期上下文与 View 公开 prelude。
    use uix_app::prelude::*;

    // 为一个日期构造由 Calendar 拥有的自定义格子 View。
    fn calendar_date_cell(date: Date, info: CalendarCellInfo) -> ViewNode {
        // 根据只读上下文选择稳定状态标记。
        let marker = if info.is_selected {
            // 选中日期显示圆点。
            "●"
        } else if info.is_today {
            // 今日显示今字。
            "今"
        } else {
            // 普通日期不显示标记。
            ""
        };
        // 返回包含状态标记和日期的真实 Label 子树。
        ViewNode::leaf(Label::new(format!("{marker}{}", date.day)))
    }

    // 编译类型化日期格工厂扩展入口。
    fn compile_example() {
        // 把日期格工厂交给 Calendar 运行时所有者。
        let _calendar = embed(Calendar::new().date_cell(calendar_date_cell));
    }
}

// 隔离 carousel-basic 围栏中的尺寸与自动播放配置。
mod carousel_basic {
    // 引入公开时间类型。
    use std::time::Duration;
    // 引入文档承诺的轮播公开 prelude。
    use uix_app::prelude::*;

    // 编译轮播尺寸与三秒自动播放间隔。
    fn compile_example() {
        // 构造固定尺寸的自动播放轮播组件。
        let _carousel = embed(
            // 创建默认轮播。
            Carousel::new()
                // 声明轮播尺寸。
                .size(300.0, 160.0)
                // 声明自动播放间隔。
                .autoplay(Duration::from_secs(3)),
        );
    }
}

// 隔离 data-card 围栏中的标题、正文与操作区。
mod data_card {
    // 引入文档承诺的卡片公开 prelude。
    use uix_app::prelude::*;

    // 编译卡片复合内容配置。
    fn compile_example() {
        // 构造包含用户详情与编辑动作的卡片。
        let _card = embed(
            // 创建默认卡片。
            Card::new()
                // 声明卡片标题。
                .title("用户")
                // 提交正文 Label 子组件。
                .child(Label::new("详细信息"))
                // 声明操作区文本。
                .actions(vec!["编辑"]),
        );
    }
}

// 隔离 collapse 围栏中的稳定折叠面板。
mod collapse {
    // 引入文档承诺的折叠面板公开 prelude。
    use uix_app::prelude::*;

    // 编译展开初值与面板列表。
    fn compile_example() {
        // 构造两个折叠面板并展开首项。
        let _collapse = embed(Collapse::new().panels(vec![
            // 添加默认展开的首项。
            CollapsePanel::new("标题一", "内容一").expanded(),
            // 添加默认收起的次项。
            CollapsePanel::new("标题二", "内容二"),
        ]));
    }
}

// 隔离 data-image-advanced 围栏中的图片与可读回退。
mod data_image_advanced {
    // 引入文档承诺的图片公开 prelude。
    use uix_app::prelude::*;

    // 编译资源路径、替代文本、回退、圆角、预览与适配配置。
    fn compile_example() {
        // 构造带完整可读失败状态的图片组件。
        let _image = embed(
            // 创建固定尺寸图片。
            Image::new(128.0, 88.0)
                // 声明资源路径。
                .src("assets/")
                // 声明无障碍替代文本。
                .alt("演示图片")
                // 声明加载失败回退文本。
                .fallback("图片加载失败")
                // 声明圆角。
                .radius(8.0)
                // 启用点击预览。
                .preview(true)
                // 保持图片比例适配。
                .fit(true),
        );
    }
}
