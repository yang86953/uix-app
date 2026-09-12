// 集中验证 reactive 组件的作用域包装、准备语句收归与拒绝路径。

// 引入共享文档解析入口。
use super::parser::parse_document;
// 引入共享 View 生成入口。
use super::widget_codegen::generate_document_view;

// 验证 reactive 组件生成 scoped 闭包且准备语句收归闭包内。
#[test]
fn generates_reactive_widget_scoped_wrapping() {
    // 解析响应式共享状态组件。
    let document = parse_document(
        // 声明 reactive 与 State<number> prop。
        r#"
        <Widget name="ReactiveCounter" reactive props="count: State<number>">
          <Column>
            <Text>{count}</Text>
            <Button @click="setState(count: count + 1)">+</Button>
          </Column>
        </Widget>
        <ReactiveCounter count={shared_count} />
        "#,
    )
    // 合法响应式文档必须解析成功。
    .expect("响应式文档应解析成功");
    // 生成完整令牌。
    let tokens = generate_document_view(&document)
        // 合法响应式组件应生成成功。
        .expect("响应式组件应生成成功")
        // 转换为文本。
        .to_string();
    // 应调用公开 scoped 组合器。
    assert!(tokens.contains(":: uix_app :: prelude :: scoped"));
    // scoped 闭包应使用 move 捕获准备语句建立的绑定。
    assert!(tokens.contains("move ||"));
    // 调用方句柄求值位于 scoped 闭包外（调用方帧），组件体读值在闭包内。
    let scoped_at = tokens
        .find("scoped")
        .expect("scoped 组合器应存在");
    // 句柄克隆的准备语句文本在调用方帧：先于 scoped 出现。
    let clone_at = tokens
        .find("shared_count")
        .expect("共享句柄应出现在准备语句");
    assert!(
        clone_at < scoped_at,
        "调用参数求值应位于调用方帧（scoped 闭包之前）"
    );
    // 组件体读值 get 调用位于闭包内：句柄的闭包内克隆副本再读值。
    assert!(
        tokens[scoped_at..].contains(". get"),
        "组件体读值应在 scoped 闭包内"
    );
    // 普通（非 reactive）文档不应出现 scoped。
    let plain = parse_document(
        // 不声明 reactive 的对照组件。
        r#"
        <Widget name="PlainCounter" props="count: State<number>">
          <Column><Text>{count}</Text></Column>
        </Widget>
        <PlainCounter count={shared_count} />
        "#,
    )
    // 合法对照文档必须解析成功。
    .expect("对照文档应解析成功");
    // 生成完整令牌。
    let plain_tokens = generate_document_view(&plain)
        // 合法对照组件应生成成功。
        .expect("对照组件应生成成功")
        // 转换为文本。
        .to_string();
    // 未声明 reactive 时不得出现 scoped 组合器。
    assert!(!plain_tokens.contains(":: uix_app :: prelude :: scoped"));
}

// 验证 reactive 组件内 external 投影函数进入 scoped 闭包。
#[test]
fn generates_reactive_widget_with_external_snapshot() {
    // 解析带 external 投影的响应式组件。
    let document = parse_document(
        // external 声明列表投影函数。
        r#"
        <Widget name="ReactiveList" reactive external="visible_items">
          <Column>
            <For {item} in {visible_items()}>
              <Text>{item}</Text>
            </For>
          </Column>
        </Widget>
        <ReactiveList />
        "#,
    )
    // 合法响应式列表文档必须解析成功。
    .expect("响应式列表文档应解析成功");
    // 生成完整令牌。
    let tokens = generate_document_view(&document)
        // 合法响应式列表应生成成功。
        .expect("响应式列表应生成成功")
        // 转换为文本。
        .to_string();
    // 应调用公开 scoped 组合器。
    assert!(tokens.contains(":: uix_app :: prelude :: scoped"));
    // external 投影函数调用应保留原名。
    assert!(tokens.contains("visible_items"));
}

// 验证多根 reactive 组件自动收敛为合成 Column 唯一根。
#[test]
fn generates_reactive_multi_root_wrapped_in_column() {
    // 解析多根响应式组件。
    let document = parse_document(
        // 组件体有两个可渲染根。
        r#"
        <Widget name="ReactiveMulti" reactive>
          <Text>第一行</Text>
          <Text>第二行</Text>
        </Widget>
        <ReactiveMulti />
        "#,
    )
    // 合法多根文档必须解析成功。
    .expect("多根响应式文档应解析成功");
    // 生成完整令牌。
    let tokens = generate_document_view(&document)
        // 多根响应式应生成成功。
        .expect("多根响应式组件应生成成功")
        // 转换为文本。
        .to_string();
    // 应生成 scoped 包装。
    assert!(tokens.contains(":: uix_app :: prelude :: scoped"));
    // 应保留两行文本。
    assert!(tokens.contains("第一行"));
    assert!(tokens.contains("第二行"));
}

// 验证直接嵌套与空组件的 reactive 声明被拒绝。
#[test]
fn rejects_reactive_nesting_and_empty_shapes() {
    // 解析根直接为另一 reactive 组件调用的文档。
    let nested = parse_document(
        // 内层先声明。
        r#"
        <Widget name="Inner" reactive><Text>内层</Text></Widget>
        <Widget name="Outer" reactive><Inner /></Widget>
        <Outer />
        "#,
    )
    // 声明阶段合法。
    .expect("嵌套声明应可解析");
    // 生成阶段必须拒绝直接嵌套 scoped 根。
    let nested_error = generate_document_view(&nested)
        // 直接嵌套必须失败。
        .expect_err("reactive 直接嵌套必须失败");
    // 诊断应说明 scoped 节点约束。
    assert!(nested_error.message.contains("不能直接是另一个 reactive"));
    // 解析无可渲染根的响应式组件。
    let empty = parse_document(
        // 组件体只有排版空白。
        r#"<Widget name="Empty" reactive>   </Widget><Empty />"#,
    )
    // 声明阶段合法。
    .expect("空组件声明应可解析");
    // 生成阶段必须拒绝无可渲染根。
    let empty_error = generate_document_view(&empty)
        // 空组件必须失败。
        .expect_err("空 reactive 组件必须失败");
    // 诊断应说明根要求。
    assert!(empty_error.message.contains("没有可渲染节点"));
    // 解析文本根的响应式组件。
    let text_root = parse_document(
        // 组件体只有纯文本节点。
        r#"<Widget name="TextRoot" reactive>纯文本</Widget><TextRoot />"#,
    )
    // 声明阶段合法。
    .expect("文本根声明应可解析");
    // 生成阶段必须拒绝非元素根。
    let text_error = generate_document_view(&text_root)
        // 文本根必须失败。
        .expect_err("reactive 文本根必须失败");
    // 诊断应指向元素根要求。
    assert!(text_error.message.contains("必须是元素节点"));
}

// 验证 reactive="false" 显式关闭后保持普通内联展开。
#[test]
fn reactive_false_keeps_plain_expansion() {
    // 解析显式关闭响应式的组件。
    let document = parse_document(
        // reactive="false" 与缺省一致。
        r#"
        <Widget name="OptOut" reactive="false" props="count: State<number>">
          <Text>{count}</Text>
        </Widget>
        <OptOut count={shared_count} />
        "#,
    )
    // 合法文档必须解析成功。
    .expect("显式关闭文档应解析成功");
    // 生成完整令牌。
    let tokens = generate_document_view(&document)
        // 合法组件应生成成功。
        .expect("显式关闭组件应生成成功")
        // 转换为文本。
        .to_string();
    // 未声明 reactive 时不得出现 scoped 组合器。
    assert!(!tokens.contains(":: uix_app :: prelude :: scoped"));
}

// 验证 reactive 属性的非法值形状被拒绝。
#[test]
fn rejects_invalid_reactive_values() {
    // 解析 reactive 用非法字面量。
    let document = parse_document(
        // reactive 只接受布尔值。
        r#"<Widget name="Bad" reactive="yes"><Text>x</Text></Widget><Bad />"#,
    );
    // 解析阶段必须拒绝非法布尔。
    let error = document
        // 非法值必须失败。
        .expect_err("reactive 非法值必须失败");
    // 诊断应说明布尔形状。
    assert!(error.message.contains("布尔"));
}