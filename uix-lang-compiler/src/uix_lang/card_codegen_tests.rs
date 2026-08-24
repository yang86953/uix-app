// 引入与公开宏一致的完整文档测试生成入口。
use super::generate_test_document_view as generate;

// 验证 Card 标题、操作项、子树与公共属性的完整生成契约。
#[test]
// 声明完整 Card 生成测试。
fn generates_card_contract() {
    // 生成覆盖全部专有属性和异质有序子节点的卡片。
    let snapshot = generate(r#"<Card title="用户信息" actions={card_actions} width="320px" automationId="profile-card"><Text>姓名</Text><Button>编辑</Button></Card>"#)
        // 合法 Card 必须成功生成。
        .expect("文档属性与子树应映射到公开 Card API");
    // 卡片必须从公开无参构造器开始。
    assert!(snapshot.contains("Card :: new ()"));
    // 标题必须进入公开复制型构建器。
    assert!(snapshot.contains("title (& * (\"用户信息\"))"));
    // 操作项表达式必须保留调用侧标识符。
    assert!(snapshot.contains("actions ((card_actions) . clone ())"));
    // 卡片与子节点必须进入组件自己的 UIX 根声明，不能直接构造运行时节点。
    assert!(snapshot.contains("build_view_with_children"));
    assert!(!snapshot.contains("ViewNode :: new"));
    // 公共宽度必须继续应用到组合节点。
    assert!(snapshot.contains("width (320.0)"));
    // 自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("automation_id"));
    // 文本与按钮必须保持源码顺序。
    let text = snapshot.find("label (\"姓名\")").expect("应生成文本子节点");
    // 定位后续按钮子节点。
    let button = snapshot
        // 查找按钮构造器快照。
        .find("button (\"编辑\")")
        // 缺失按钮即说明子树生成不完整。
        .expect("应生成按钮子节点");
    // 子节点顺序不得在生成阶段重排。
    assert!(text < button);
    // 结束完整 Card 生成测试。
}

// 验证 Card 动态标题、空子树与控制子节点生成。
#[test]
// 声明动态与控制流测试。
fn generates_dynamic_and_controlled_card_values() {
    // 生成只含动态标题的空卡片。
    let empty = generate(r#"<Card title={card_title} />"#)
        // 空子树是合法卡片形状。
        .expect("空 Card 应保留公开容器契约");
    // 动态标题必须进入公开构建器。
    assert!(empty.contains("title (& * (card_title))"));
    // 未声明操作项时不能凭空生成 actions 调用。
    assert!(!empty.contains("actions ("));
    // 生成含条件子节点的卡片。
    let controlled = generate(r#"<Card><If {show_details}><Text>详情</Text></If><For {item} in {items}><Text>{item}</Text></For></Card>"#)
        // Card 必须接受核心控制节点。
        .expect("Card 子树应保留条件生成语义");
    // 条件表达式必须出现在子节点追加作用域。
    assert!(controlled.contains("if show_details"));
    // 条件内文本必须生成公开 View。
    assert!(controlled.contains("label (\"详情\")"));
    // 循环绑定必须通过内部位置枚举在 Card 子节点作用域内展开。
    assert!(
        controlled.contains("__uix_for_ordinal")
            && controlled.contains("enumerate")
            && controlled.contains("items")
    );
    // 循环项插值必须保留当前绑定表达式。
    assert!(controlled.contains("ToString") && controlled.contains("item"));
    // 结束动态与控制流测试。
}

// 验证 Card 拒绝非数组表达式的操作项属性。
#[test]
// 声明非法操作项形状测试。
fn rejects_literal_card_actions() {
    // 字符串不能伪装成操作项数组引用。
    let diagnostic = generate(r#"<Card actions="编辑"><Text>内容</Text></Card>"#)
        // 非表达式 actions 必须失败。
        .expect_err("Card actions 字符串字面量必须被拒绝");
    // 诊断必须点明数组表达式契约。
    assert!(diagnostic.message.contains("数组表达式"));
    // 修复建议必须给出规范属性写法。
    assert!(diagnostic.suggestion.contains("actions={card_actions}"));
    // 结束非法操作项形状测试。
}
