// 声明本文件只编译输入组件使用文档，不启动窗口或原生资源。
#![allow(dead_code)]

// 隔离 input-basic 围栏中的文本输入模式。
mod input_basic {
    // 引入文档承诺的公开输入与状态 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "input-basic";

    // 编译单行、多行和密码输入的受控状态入口。
    fn compile_example() {
        // 创建由业务作用域拥有的已提交文本状态。
        let text = State::new(String::new());

        // 构造可横向伸缩的单行受控输入节点。
        let _single_line = input()
            // 声明空值时的占位文本。
            .placeholder("请输入")
            // 绑定业务已提交值状态。
            .value(&text)
            // 声明容器中的伸缩权重。
            .flex_grow(1.0);

        // 构造四行受控多行输入组件。
        let _textarea = Input::textarea().rows(4).value(&text);

        // 构造受控密码输入组件。
        let _password = Input::password().value(&text);
    }
}

// 隔离 input-number 围栏中的数值与区间选择组件。
mod input_number {
    // 引入文档承诺的公开数值输入 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "input-number";

    // 编译受控数值输入、单值滑块和区间滑块。
    fn compile_example() {
        // 创建由业务作用域拥有的整数状态。
        let number = State::new(50_i32);

        // 构造带范围、步长和精度约束的数值输入。
        let _number_input = embed(
            // 创建公开数值输入组件。
            InputNumber::new()
                // 绑定业务整数状态。
                .value(&number)
                // 声明最小提交值。
                .min(0.0)
                // 声明最大提交值。
                .max(100.0)
                // 声明递增步长。
                .step(5.0)
                // 声明整数显示与量化精度。
                .precision(0),
        );

        // 构造带顶部优先提示的单值滑块。
        let _slider = embed(
            // 创建零到一百的公开滑块。
            Slider::new(0.0..=100.0)
                // 使用区间中点作为初始值。
                .default_value(50.0)
                // 声明提示的优先方向。
                .tooltip(TooltipPlacement::Top),
        );

        // 构造步长为五的区间滑块。
        let _range_slider = embed(RangeSlider::new(0.0..=100.0).step(5.0));
    }
}

// 隔离 input-choice 围栏中的受控选择组件。
mod input_choice {
    // 引入文档承诺的公开选择与状态 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "input-choice";

    // 编译复选框、单选组和开关公开入口。
    fn compile_example() {
        // 创建由业务作用域拥有的布尔状态。
        let checked = State::new(false);

        // 构造绑定业务状态的记住我复选框。
        let _checkbox = embed(Checkbox::new("记住我").checked(&checked));
        // 构造带两个稳定选项的单选组。
        let _radio = embed(
            // 创建公开 Radio 组件。
            Radio::new()
                // 声明管理员与普通用户选项。
                .options(["管理员", "普通用户"])
                // 默认选择首项。
                .default_selected(0),
        );
        // 构造带可读名称的受控开关。
        let _switch = embed(
            // 创建公开 Switch 组件。
            Switch::new()
                // 为无文字控件声明无障碍名称。
                .label("启用通知")
                // 绑定业务布尔状态。
                .checked(&checked),
        );
    }
}

// 隔离 input-select 围栏中的受控下拉选择器。
mod input_select {
    // 引入文档承诺的公开选择与状态 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "input-select";

    // 编译稳定选项与业务值状态绑定。
    fn compile_example() {
        // 创建由业务作用域拥有的当前语言值。
        let selected = State::new("cn".to_string());

        // 构造受控单选下拉组件。
        let _select = embed(
            // 创建公开 Select 组件。
            Select::new()
                // 声明两个展示选项。
                .options(["中文", "English"])
                // 绑定业务选择值状态。
                .value(&selected),
        );
    }
}

// 隔离 input-date 围栏中的日期、时间与颜色选择器。
mod input_date {
    // 引入文档承诺的公开选择器与状态 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "input-date";

    // 编译日期、时间和颜色三种受控值入口。
    fn compile_example() {
        // 创建由业务作用域拥有的日期状态。
        let date = State::new(Date::new(2026, 7, 31));
        // 构造绑定日期状态的选择器。
        let _date_picker = embed(DatePicker::new().value(&date));

        // 创建由业务作用域拥有的时间状态。
        let time = State::new(Time::new(14, 30));
        // 构造绑定时间状态的选择器。
        let _time_picker = embed(TimePicker::new().value(&time));

        // 创建由业务作用域拥有的颜色状态。
        let color = State::new(Color::hex("#1677ff"));
        // 构造绑定颜色状态的选择器。
        let _color_picker = embed(ColorPicker::new().value(&color));
    }
}

// 运行无原生副作用的标记测试，让 Cargo 显式执行本编译消费者。
#[test]
// 确认本批外部消费者覆盖输入组件文档的五个围栏。
fn input_rust_fences_compile_as_external_consumers() {
    // 收集五个已经由编译器类型检查的公开示例标识。
    let compile_ids = [
        // 登记文本输入围栏。
        input_basic::COMPILE_ID,
        // 登记数值与滑块围栏。
        input_number::COMPILE_ID,
        // 登记选择控件围栏。
        input_choice::COMPILE_ID,
        // 登记下拉选择器围栏。
        input_select::COMPILE_ID,
        // 登记日期时间颜色围栏。
        input_date::COMPILE_ID,
    ];
    // 运行阶段核对消费者覆盖标识与 Markdown 围栏一致。
    assert_eq!(
        // 使用实际模块暴露的标识作为结果。
        compile_ids,
        // 使用输入文档当前声明的稳定标识作为期望。
        [
            // 文本输入围栏标识。
            "input-basic",
            // 数值与滑块围栏标识。
            "input-number",
            // 选择控件围栏标识。
            "input-choice",
            // 下拉选择器围栏标识。
            "input-select",
            // 日期时间颜色围栏标识。
            "input-date",
        ],
    );
}

#[cfg(feature = "test-harness")]
#[test]
fn input_view_width_and_flex_grow_reach_public_layout() {
    use uix::ui::test_harness::TestApp;

    let app = TestApp::new((320.0, 100.0), || {
        column((
            input()
                .placeholder("固定宽度")
                .width(180.0)
                .automation_id("input.fixed"),
            row((
                input()
                    .placeholder("弹性宽度")
                    .flex_grow(1.0)
                    .automation_id("input.flex"),
                space(40.0).width(40.0),
            )),
        ))
    });
    let snapshot = app.snapshot();
    let fixed = snapshot.find("input.fixed").expect("固定输入框应存在");
    let flex = snapshot.find("input.flex").expect("弹性输入框应存在");

    assert!((fixed.frame.w - 180.0).abs() < 0.01);
    assert!((flex.frame.w - 280.0).abs() < 0.01);
}
