// 引入文档解析与普通 View 生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 把 UIX 源码生成 Rust 令牌字符串快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析测试文档。
    let document = parse_document(source)?;
    // 生成根 View 并转成可断言字符串。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Dropdown 生成 keyed 数据、完整 trigger 子树、触发方式与 Change 事件。
#[test]
fn generates_owned_trigger_and_keyed_items_contract() {
    // 生成覆盖首版全部专有属性的下拉菜单。
    let tokens = generate(
        "<Dropdown items={[DropdownItem('编辑', 'edit'), DropdownItem('复制', 'copy')]} trigger=\"contextMenu\" @change=\"on_action($event)\" width=\"180px\"><Button>更多</Button></Dropdown>",
    )
    // 合法组合契约必须生成成功。
    .expect("Dropdown 应生成成功");
    // 快照必须包含 keyed 数据门禁、trigger owner 与右键模式。
    assert!(
        tokens.contains("Dropdown :: new")
            && tokens.contains("keyed_items")
            && tokens.contains("DropdownItem :: from_text")
            && tokens.contains("trigger_view")
            && tokens.contains("TriggerMode :: ContextMenu"),
        "{tokens}"
    );
    // 唯一按钮必须进入运行时拥有的 trigger ViewNode。
    assert!(
        tokens.contains("button") && tokens.contains("更多"),
        "{tokens}"
    );
    // 事件必须通过公开 Change 入口发布稳定 key。
    assert!(tokens.contains("on_change_fn") && tokens.contains("on_action"));
    // 公共尺寸属性必须继续应用到外层 View。
    assert!(tokens.contains("width (180.0)"), "{tokens}");
}

// 验证 Dropdown 对 trigger 静态基数和类型给出确定诊断。
#[test]
fn rejects_missing_multiple_or_dynamic_triggers() {
    // 零 trigger 不满足组合 owner 契约。
    let missing = generate("<Dropdown items={items} />")
        // 生成必须失败。
        .expect_err("零 trigger 必须失败");
    // 诊断必须说明唯一直接 View。
    assert!(missing.message.contains("仅包含一个"), "{missing:?}");
    // 多个直接 View 会产生不确定触发区域。
    let multiple =
        generate("<Dropdown items={items}><Button>一</Button><Button>二</Button></Dropdown>")
            // 生成必须失败。
            .expect_err("多 trigger 必须失败");
    // 诊断必须继续指向唯一基数。
    assert!(multiple.message.contains("仅包含一个"), "{multiple:?}");
    // 直接控制流会改变 trigger 身份和基数。
    let dynamic =
        generate("<Dropdown items={items}><If {show}><Button>更多</Button></If></Dropdown>")
            // 生成必须失败。
            .expect_err("动态 trigger 必须失败");
    // 诊断必须点名 If/For 边界。
    assert!(
        dynamic.message.contains("If") && dynamic.message.contains("For"),
        "{dynamic:?}"
    );
}

// 验证缺失数据、未知触发方式与旧 @select 不得静默降级。
#[test]
fn rejects_missing_items_unknown_trigger_and_select_alias() {
    // keyed items 是必需数据所有权边界。
    let missing = generate("<Dropdown><Button>更多</Button></Dropdown>")
        // 生成必须失败。
        .expect_err("缺失 items 必须失败");
    // 诊断必须点名缺失属性。
    assert!(missing.message.contains("items"), "{missing:?}");
    // blur 不在批准的四种触发模式中。
    let trigger =
        generate("<Dropdown items={items} trigger=\"blur\"><Button>更多</Button></Dropdown>")
            // 生成必须失败。
            .expect_err("未知 trigger 必须失败");
    // 诊断必须列出批准模式。
    assert!(
        trigger.message.contains("click") && trigger.message.contains("contextMenu"),
        "{trigger:?}"
    );
    // @select 未登记，避免产生第二套语义事件名。
    let select = generate(
        "<Dropdown items={items} @select=\"on_action($event)\"><Button>更多</Button></Dropdown>",
    )
    // 生成必须失败。
    .expect_err("旧 @select 必须失败");
    // 诊断必须点名未登记别名。
    assert!(select.message.contains("@select"), "{select:?}");
}
