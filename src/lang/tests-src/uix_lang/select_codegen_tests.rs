// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Select 结构化选项、状态模式与可选属性生成。
#[test]
fn generates_bound_select_contract() {
    // 生成文档登记的可搜索多选选择器。
    let snapshot = generate(
        r#"<Select value={cities} options={city_options} multiple searchable={can_search} placeholder="请选择城市" width="240px" />"#,
    )
    // 合法选择器必须成功生成。
    .expect("文档属性应映射到公开 Select API");
    // 结构化选项必须进入独立公开入口。
    let options = snapshot
        // 查找选项数据引用。
        .find("select_options ((city_options) . clone ())")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成 Select 结构化选项：{snapshot}"));
    // 搜索表达式必须保留动态布尔类型检查。
    let searchable = snapshot
        // 查找公开搜索开关入口。
        .find("searchable_enabled (can_search)")
        // 失败表示动态属性被丢弃。
        .expect("应生成 Select 搜索开关");
    // 多选绑定必须通过 const 泛型选择集合状态类型。
    let value = snapshot
        // 查找公开模式绑定入口。
        .find("value_mode :: < true , _ > (& (cities))")
        // 失败表示多选类型约束未生成。
        .expect("应生成 Select 多选值绑定");
    // 构造生命周期必须先设置选项和搜索，再同步外部状态。
    assert!(options < searchable && searchable < value);
    // 占位文本与公共宽度必须继续映射。
    assert!(snapshot.contains("placeholder (\"请选择城市\")"));
    // 公共尺寸属性必须保留。
    assert!(snapshot.contains("width (240.0)"));
}

// 验证 Select 值提交事件的文本载荷生成。
#[test]
fn generates_select_change_event_with_payload() {
    // 生成带值提交事件的单选选择器。
    let snapshot = generate(
        r#"<Select value={city} options={city_options} @change="on_city_change($event)" />"#,
    )
    // 合法事件必须成功生成。
    .expect("Select @change 应映射到公开 View Change 入口");
    // Change 事件必须把选中值文本载荷交给处理器。
    assert!(
        snapshot.contains("on_change_fn")
            && snapshot.contains("(on_city_change) (__uix_change_value)"),
        "{snapshot}"
    );
    // 裸零参数处理器也必须成立。
    let bare = generate(
        r#"<Select value={city} options={city_options} @change="on_city_committed()" />"#,
    )
    .expect("零参数 Select @change 应生成");
    assert!(bare.contains("on_change_fn") && bare.contains("(on_city_committed) ()"), "{bare}");
}

// 验证 Select 默认单选模式和拒绝路径。
#[test]
fn validates_select_contracts() {
    // 未声明 multiple 时必须生成单选状态约束。
    let single = generate(r#"<Select value={city} options={city_options} />"#)
        // 默认单选选择器必须成功生成。
        .expect("缺省 multiple 应生成单选 Select");
    // 默认模式必须约束 State<String>。
    assert!(single.contains("value_mode :: < false , _ > (& (city))"));
    // 动态 multiple 无法在编译期选择状态类型。
    let dynamic =
        generate(r#"<Select value={city} options={city_options} multiple={is_multiple} />"#)
            // 动态模式必须失败。
            .expect_err("动态 multiple 必须失败");
    // 诊断必须说明静态类型选择边界。
    assert!(dynamic.message.contains("静态布尔值"));
    // options 字面量不能提供结构化集合。
    let literal_options = generate(r#"<Select value={city} options="cities" />"#)
        // 字面量选项必须失败。
        .expect_err("字面量 options 必须失败");
    // 诊断必须指出公开选项模型。
    assert!(literal_options.message.contains("SelectOption"));
    // value 字面量不能提供 State 所有权。
    let literal_value = generate(r#"<Select value="cn" options={city_options} />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量 value 必须失败");
    // 诊断必须明确两种受支持状态类型。
    assert!(literal_value.message.contains("State<String>"));
    // Select 子节点不能被生成器丢弃。
    let child =
        generate(r#"<Select value={city} options={city_options}><Text>lost</Text></Select>"#)
            // 子树形状必须失败。
            .expect_err("Select 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
