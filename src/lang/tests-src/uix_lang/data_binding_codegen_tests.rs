// 引入表达式解析与生成入口及跨度构造。
use super::{SourceSpan, generate_expression, parse_expression};

// 构造数据构造测试使用的绝对起点。
fn origin() -> SourceSpan {
    // 返回非零位置以验证跨度映射。
    SourceSpan {
        // 模拟文档内绝对字节起点。
        start: 200,
        // 表达式入口只使用起点定位。
        end: 200,
        // 模拟第二十行。
        line: 20,
        // 模拟第五列。
        column: 5,
    }
}

// 把表达式源码生成 Rust 令牌字符串快照。
fn generate(source: &str) -> String {
    // 解析受限表达式。
    let expression = parse_expression(source, origin())
        // 测试输入必须成功。
        .expect("数据构造表达式应成功解析");
    // 生成确定性令牌流。
    generate_expression(&expression, None)
        // 生成必须成功。
        .expect("数据构造生成应成功")
        // 转为字符串快照。
        .to_string()
}

// 验证八类登记数据类型生成公开 API 构造调用。
#[test]
fn generates_registered_data_constructor_calls() {
    // 单选选项生成 SelectOption::new 调用。
    let select = generate("SelectOption('中国', 'cn')");
    // 快照必须包含公开构造路径与 new 调用。
    assert!(
        select.contains("prelude") && select.contains("SelectOption") && select.contains("new"),
        "{select}"
    );
    // 描述列表条目生成双参数构造。
    let descriptions = generate("DescriptionsItem('姓名', 'Ada')");
    // 快照必须包含公开构造路径。
    assert!(
        descriptions.contains("prelude") && descriptions.contains("DescriptionsItem"),
        "{descriptions}"
    );
    // 面包屑条目支持链式 active 成员调用。
    let breadcrumb = generate("BreadcrumbItem('导航').active()");
    // 快照必须包含构造与成员链。
    assert!(
        breadcrumb.contains("BreadcrumbItem") && breadcrumb.contains("active"),
        "{breadcrumb}"
    );
    // 锚点目标生成双参数构造。
    let anchor = generate("AnchorItem('基础', '#basic')");
    // 快照必须包含公开构造路径。
    assert!(
        anchor.contains("prelude") && anchor.contains("AnchorItem"),
        "{anchor}"
    );
    // 标签页元数据支持稳定 key 成员链。
    let tab = generate("Tab('账户').key('account')");
    // 快照必须包含公开构造路径与 key 配置。
    assert!(tab.contains("Tab") && tab.contains("key"), "{tab}");
    // 菜单项生成显式 label/key 构造并支持递归 children 链。
    let menu = generate("MenuItem('设置', 'settings').children([MenuItem('账户', 'account')])");
    // 快照必须包含公开构造入口与递归成员链。
    assert!(
        menu.contains("MenuItem") && menu.contains("from_text") && menu.contains("children"),
        "{menu}"
    );
    // DropdownItem 显式 label/key 构造必须与展示文字解耦。
    let dropdown = generate("DropdownItem('编辑', 'edit').icon('edit')");
    // 快照必须包含 keyed 构造入口与公开成员链。
    assert!(
        dropdown.contains("DropdownItem")
            && dropdown.contains("from_text")
            && dropdown.contains("icon"),
        "{dropdown}"
    );
    // 时间轴事件支持 description 成员链。
    let timeline = generate("TimelineItem('创建').description('2026-08-01')");
    // 快照必须包含构造与成员链。
    assert!(
        timeline.contains("TimelineItem") && timeline.contains("description"),
        "{timeline}"
    );
    // 级联选项支持嵌套数组 children 链。
    let cascader =
        generate("CascaderOption('浙江', 'zj').children([CascaderOption('杭州', 'hz')])");
    // 快照必须包含嵌套构造、数组与成员链。
    assert!(
        cascader.contains("CascaderOption")
            && cascader.contains("children")
            && cascader.contains("vec !"),
        "{cascader}"
    );
    // 树节点生成双参数构造。
    let tree = generate("TreeNode('部门', 'dept').children([TreeNode('成员', 'member')])");
    // 快照必须包含嵌套构造与成员链。
    assert!(
        tree.contains("TreeNode") && tree.contains("children"),
        "{tree}"
    );
    // 数组字面量生成 vec 宏调用。
    let array = generate("['female', 'male']");
    // 快照必须包含 vec 宏与两个元素。
    assert!(array.contains("vec !"), "{array}");
    // 步骤状态字符串语义值映射公开枚举。
    let step = generate("Step('注册').status('finish')");
    // 快照必须包含构造路径与枚举变体。
    assert!(
        step.contains("Step") && step.contains("StepStatus") && step.contains("Finish"),
        "{step}"
    );
    // 进行中状态映射 Process 变体。
    let processing = generate("Step('验证').status('process')");
    // 快照必须包含 Process 变体。
    assert!(
        processing.contains("StepStatus") && processing.contains("Process"),
        "{processing}"
    );
}

// 验证未知状态语义值与非法参数形状返回定位诊断。
#[test]
fn rejects_invalid_data_constructor_usage() {
    // 未知状态语义值必须给出登记表诊断。
    let error = generate_expression(
        // 解析非法状态值。
        &parse_expression("Step('注册').status('done')", origin())
            // 语法合法但语义非法。
            .expect("状态调用应成功解析"),
        // 普通生成上下文。
        None,
    )
    // 生成必须失败。
    .expect_err("未知状态值必须失败");
    // 原因必须说明登记表边界。
    assert!(error.message.contains("不在登记表"), "{}", error.message);
    // 非字面量状态参数必须给出形状诊断。
    let error = generate_expression(
        // 解析动态状态参数。
        &parse_expression("Step('注册').status(value)", origin())
            // 语法合法但形状非法。
            .expect("状态调用应成功解析"),
        // 普通生成上下文。
        None,
    )
    // 生成必须失败。
    .expect_err("动态状态参数必须失败");
    // 原因必须指向字符串语义值要求。
    assert!(error.message.contains("字符串语义值"), "{}", error.message);
    // 未登记类型名保持普通 Rust 调用语义，由 Rust 类型检查兜底。
    let custom = generate("DemoRow('1')");
    // 快照不能展开为 uix 公开构造路径。
    assert!(!custom.contains(":: uix_app :: prelude ::"), "{custom}");
}
