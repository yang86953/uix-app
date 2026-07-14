//! 组件库页面 — page_input（输入控件全覆盖）。
//! 对照 [`使用.md`](../../docs/使用.md#输入与表单) 输入与表单组件。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

pub fn page_input(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    // ━━ State 定义 ━━

    // 文本输入
    let text_small = State::new(String::new());
    let text_medium = State::new(String::new());
    let text_large = State::new(String::new());
    let text_plain = State::new(String::new());
    let password = State::new(String::new());
    let textarea = State::new(String::new());

    // 数值输入
    let num_qty = State::new(0i32);
    let num_price = State::new(0.0f64);

    // 选择器
    let select_options = vec!["选项 A", "选项 B", "选项 C"];
    let long_options: Vec<String> = (0..100).map(|i| format!("选项 {i}")).collect();
    let sel_single = State::new(String::new());
    let sel_multi = State::new(std::collections::HashSet::new());
    let sel_search = State::new(String::new());
    let sel_long = State::new(String::new());

    // 树选择 & 级联
    let tree_nodes = vec![
        TreeNode::new("前端").children(vec![TreeNode::new("React"), TreeNode::new("Vue")]),
        TreeNode::new("后端").children(vec![TreeNode::new("Rust"), TreeNode::new("Go")]),
    ];
    let tree_sel = State::new(String::new());

    let city_tree = vec![
        TreeNode::new("北京").children(vec![TreeNode::new("海淀"), TreeNode::new("朝阳")]),
        TreeNode::new("上海").children(vec![TreeNode::new("浦东"), TreeNode::new("静安")]),
    ];
    let cascader_path = State::new(Vec::new());

    // 自动完成 & 提及
    let auto_text = State::new(String::new());
    let mentions_text = State::new(String::new());
    let auto_suggestions = vec!["北京", "上海", "广州", "深圳"];
    let mention_users = vec!["Alice", "Bob", "Charlie"];

    // 开关与勾选
    let checked_a = State::new(true);
    let checked_b = State::new(false);
    let radio_selected = State::new(String::new());
    let radio_options = vec!["苹果", "香蕉", "樱桃"];
    let switch_on = State::new(true);
    let switch_off = State::new(false);

    // 滑块 / 评分
    let slider_val = State::new(42.0f64);
    let rating_val = State::new(3u32);

    // 日期 / 时间 / 颜色
    let date_val = State::new(Date::today());
    let time_val = State::new(Time::now());
    let color_val = State::new(Color::from_rgb(22, 119, 255));

    // 分段
    let segmented_options = vec!["每日", "每周", "每月", "每年"];
    let segmented_val = State::new(String::new());

    // ━━ 页面构建 ━━

    PageBuilder::new(tk)
        .gap()
        // 1. Input — 尺寸
        .section("Input — 尺寸")
        .push(
            demo_row(32.0)
                .child(
                    input()
                        .placeholder("Small...")
                        .value(&text_small)
                        .size(Size::Small),
                )
                .child(
                    input()
                        .placeholder("Medium...")
                        .value(&text_medium)
                        .size(Size::Middle),
                )
                .child(
                    input()
                        .placeholder("Large...")
                        .value(&text_large)
                        .size(Size::Large),
                ),
        )
        // 2. Input — 文本 / 密码 / 多行
        .section("Input — 文本 / 密码 / 多行")
        .push(
            demo_row(32.0)
                .child(input().placeholder("请输入用户名").value(&text_plain))
                .child(Input::password().placeholder("请输入密码").value(&password)),
        )
        .push(
            Input::textarea()
                .placeholder("请输入描述")
                .value(&textarea)
                .rows(4)
                .max_length(500),
        )
        // 3. InputNumber
        .section("InputNumber")
        .push(
            demo_row(36.0)
                .child(InputNumber::new().value(&num_qty).min(0).max(100).step(1))
                .child(
                    InputNumber::new()
                        .value(&num_price)
                        .min(0.0)
                        .max(999.0)
                        .step(0.5),
                ),
        )
        // 4. Select
        .section("Select — 单选 / 多选 / 可搜索")
        .push(
            demo_row(36.0)
                .child(
                    Select::new()
                        .options(&select_options)
                        .value(&sel_single)
                        .placeholder("请选择"),
                )
                .child(
                    Select::multiple()
                        .options(&select_options)
                        .value(&sel_multi)
                        .placeholder("多选"),
                ),
        )
        .push(labeled_row(
            tk,
            36.0,
            "Select (100 项)",
            Select::new()
                .options(&long_options)
                .value(&sel_long)
                .placeholder("选择..."),
        ))
        .push(
            Select::searchable()
                .options(&select_options)
                .value(&sel_search)
                .placeholder("可搜索选择"),
        )
        // 5. TreeSelect
        .section("TreeSelect")
        .push(labeled_row(
            tk,
            36.0,
            "TreeSelect",
            TreeSelect::new()
                .options(&tree_nodes)
                .value(&tree_sel)
                .placeholder("选择技术栈"),
        ))
        .push(labeled_row(
            tk,
            36.0,
            "TreeSelect (80 项)",
            TreeSelect::new()
                .options(
                    &(0..80)
                        .map(|i| TreeNode::new(format!("节点 {i}")))
                        .collect::<Vec<_>>(),
                )
                .value(&tree_sel)
                .placeholder("大列表树选择"),
        ))
        // 6. Cascader
        .section("Cascader")
        .push(labeled_row(
            tk,
            36.0,
            "Cascader",
            Cascader::new()
                .options(&city_tree)
                .value(&cascader_path)
                .placeholder("选择地区"),
        ))
        // 7. AutoComplete / Mentions
        .section("AutoComplete / Mentions")
        .push(labeled_row(
            tk,
            36.0,
            "AutoComplete",
            AutoComplete::new()
                .suggestions(&auto_suggestions)
                .value(&auto_text)
                .placeholder("输入城市..."),
        ))
        .push(labeled_row(
            tk,
            36.0,
            "Mentions",
            Mentions::new()
                .suggestions(&mention_users)
                .value(&mentions_text)
                .placeholder("输入 @ 提及"),
        ))
        // 8. Checkbox / Radio / Switch
        .section("Checkbox / Radio / Switch")
        .push(
            demo_row(30.0)
                .child(Checkbox::new("选项 A").checked(&checked_a))
                .child(Checkbox::new("选项 B").checked(&checked_b))
                .child(
                    Checkbox::new("禁用")
                        .checked(&State::new(true))
                        .disabled(true),
                ),
        )
        .push(demo_row(32.0).child(Radio::group("fruits", &radio_options, &radio_selected)))
        .push(
            demo_row(30.0)
                .child(Switch::new().checked(&switch_on))
                .child(Switch::new().checked(&switch_off))
                .child(Switch::new().checked(&State::new(true)).disabled(true)),
        )
        // 9. Slider / Rate
        .section("Slider / Rate")
        .push(demo_row(40.0).child(Slider::new(0.0..=100.0).value(&slider_val)))
        .push(
            demo_row(30.0)
                .child(Rate::new().count(5).value(&rating_val))
                .child(Rate::new().count(7).value(&State::new(5u32)))
                .child(Rate::new().count(5).value(&State::new(2u32)).allow_half()),
        )
        // 10. DatePicker / TimePicker / ColorPicker
        .section("DatePicker / TimePicker / ColorPicker")
        .push(demo_row(36.0).child(DatePicker::new().value(&date_val)))
        .push(demo_row(36.0).child(TimePicker::new().value(&time_val)))
        .push(labeled_row(
            tk,
            36.0,
            "ColorPicker",
            ColorPicker::new().value(&color_val),
        ))
        // 11. Segmented
        .section("Segmented")
        .push(demo_row(36.0).child(Segmented::new(&segmented_options).value(&segmented_val)))
        // 12. Form — 声明式校验
        .section("Form — 声明式校验")
        .push({
            let form = Form::new()
                .field("user", "用户名")
                .default("")
                .required("请输入用户名")
                .custom(|v| {
                    if v.len() < 3 {
                        Err("至少 3 个字符".into())
                    } else {
                        Ok(())
                    }
                })
                .field("email", "邮箱")
                .default("")
                .required("请输入邮箱")
                .validate_email("邮箱格式不正确")
                .field("age", "年龄")
                .default(0i32)
                .validate_range(1..=120, "年龄范围 1-120")
                .field("password", "密码")
                .default("")
                .required("请输入密码")
                .field("confirm", "确认密码")
                .default("")
                .depends_on("password", |v, deps| {
                    let pwd = deps.get::<String>("password").unwrap();
                    if v != pwd {
                        Err("两次密码不一致".into())
                    } else {
                        Ok(())
                    }
                })
                .build();

            // 演示：展示表单校验结果
            column((
                form,
                button("校验表单").primary().on_click_fn(|| {
                    // 实际校验逻辑在此
                }),
            ))
            .gap(8.0)
        })
        // 13. Upload
        .section("Upload")
        .push(
            Upload::new()
                .accept(".jpg,.png,.pdf")
                .max_size(10 * 1024 * 1024)
                // 10 MB
                .on_select(|files| {
                    // 处理上传文件列表
                    let _ = files;
                }),
        )
        .build()
}
