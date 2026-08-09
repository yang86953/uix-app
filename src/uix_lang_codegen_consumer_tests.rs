// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 表示消费者循环中的拥有所有权数据项。
#[derive(Clone)]
struct ConsumerItem {
    // 保存稳定行身份。
    id: u64,
    // 保存可插值名称。
    name: String,
}

// 提供无事件参数的 Rust 侧回调。
fn on_confirm() {}

// 提供带 bool 返回值的组件回调 prop。
fn confirm_delete() -> bool {
    // 返回确定的确认结果。
    true
}

// 提供带 String 参数的组件回调 prop。
fn submit_search(
    // 接收组件传出的拥有所有权字符串。
    _query: String,
) {
}

// 提供接收点击坐标的 Rust 侧回调。
fn on_point(
    // 接收点击 x 坐标。
    _x: f32,
    // 接收点击 y 坐标。
    _y: f32,
) {
}

// 验证核心生成物在真实 uix 公开 API 消费者中通过类型检查。
#[test]
fn generated_view_compiles_against_public_uix_api() {
    // 提供文本插值变量。
    let count = 2_i32;
    // 提供表达式布尔属性变量。
    let busy = false;
    // 提供 If 条件变量。
    let visible = true;
    // 提供 For 数据源。
    let items = vec![ConsumerItem {
        // 设置稳定行身份。
        id: 7,
        // 设置文本插值内容。
        name: "Alpha".to_string(),
    }];
    // 在真实 crate 中展开并要求结果类型为公开 ViewNode。
    let _view: ViewNode = uix_derive::__uix_view_internal!(
        r#"
        <Column gap="8px">
          <Text fontSize="heading2" key={count}>Count: {count}</Text>
          <Button type="primary" disabled={busy} @click="on_confirm()">Save</Button>
          <Button @click="on_point($event.x, $event.y)">Point</Button>
          <If {visible}><Icon name="star" size="16px" /></If>
          <For {item} {index} in {items} key={item.id}>
            <Row><Text>{index}</Text><Text>{item.name}</Text></Row>
          </For>
        </Column>
    "#
    );
}

// 验证 crate 根与 prelude 导出的公开 uix! 内嵌及文件入口。
#[test]
fn public_uix_macro_compiles_inline_and_file_entries() {
    // 通过 prelude 导入的公开宏展开内嵌 UIX 源码。
    let _inline: ViewNode = uix!(r#"<Text>Inline entry</Text>"#);
    // 以调用 crate 清单目录为基准读取并展开 .uix 文件。
    let _file: ViewNode = uix!("tests/fixtures/uix_lang/public_entry.uix");
    // 通过 crate 根路径再次验证宏导出。
    let _root_export: ViewNode = crate::uix!(r#"<Button>Root export</Button>"#);
    // 验证通用组件文档中的 Label 别名真实生成。
    let _label: ViewNode = crate::uix!(r#"<Label>Documented label</Label>"#);
}

// 验证已映射内联样式在真实公开 API 消费者中通过类型检查。
#[test]
fn mapped_inline_styles_compile_against_public_uix_api() {
    // 展开覆盖布局、Grid、盒模型、状态色、阴影和可见性的代表样式。
    let _styled: ViewNode = uix!(
        // 只使用样式参考中当前标记为已映射的值。
        r##"<Column style="display: grid; gridTemplateColumns: 1fr 120px; gridTemplateRows: auto; gridColumnGap: 8px; gridRowGap: 4px; flexWrap: true; justifyContent: space-between; alignItems: center; margin: 1px 2px 3px 4px; padding: 8px 12px; borderColor: #336699; borderWidth: 1px; borderRadius: 6px; width: 320px; height: auto; backgroundColor: rgba(10,20,30,0.5); backgroundColor:hover: #fff; opacity: 0.9; overflow: hidden; boxShadow: 0 2px 4px rgba(0,0,0,0.2); visible: true;"><Text style="color: blue; fontSize: heading3; flexGrow: 1; flexShrink: 0; alignSelf: flex-start; gridColumnSpan: 2; gridRowSpan: 1;">Styled</Text></Column>"##
    );
}

// 验证样式类继承和内联优先级在公开宏消费者中通过类型检查。
#[test]
fn style_classes_compile_against_public_uix_api() {
    // 展开父类、子类、多 class 与内联覆盖组合。
    let _styled: ViewNode = uix!(
        // 后声明类覆盖前一类，内联样式具有最高优先级。
        r##"baseCard { padding: 4px; color: red; } elevatedCard { extends: baseCard; boxShadow: 0 2px 4px rgba(0,0,0,0.2); } blueCard { color: blue; } <Text class="elevatedCard blueCard" style="padding: 8px;">Class style</Text>"##
    );
}

// 验证组件私有状态、共享状态、回调与组合在真实公开 API 中通过类型检查。
#[test]
fn generated_components_compile_against_public_uix_api() {
    // 创建 Rust 侧持有的共享 number 状态。
    let shared_count = State::new(4_f64);
    // 展开多层组件并要求最终结果为公开 ViewNode。
    let _view: ViewNode = uix_derive::__uix_view_internal!(
        r#"
        <Component name="Counter" props="label: String, onConfirm: () -> bool" state="count: 0">
          <Column>
            <Text>{label}: {count}</Text>
            <Button @click="setState(count: count + 1)">+1</Button>
            <Button @click="onConfirm()">Confirm</Button>
          </Column>
        </Component>
        <Component name="Panel" props="title: String, onClose: () -> bool">
          <Counter label={title} onConfirm={onClose} />
        </Component>
        <Component name="SharedCounter" props="count: State<number>">
          <Column>
            <Text>Shared: {count}</Text>
            <Button @click="setState(count: count + 1)">Share +1</Button>
          </Column>
        </Component>
        <Component name="Message" state="text: 'A'">
          <Button @click="setState(text: 'B')">{text}</Button>
        </Component>
        <Component name="SearchAction" props="onSearch: (String)">
          <Button @click="onSearch('needle')">Search</Button>
        </Component>
        <Column>
          <Panel title="Clicks" onClose={confirm_delete} />
          <SharedCounter count={shared_count} />
          <Message />
          <SearchAction onSearch={submit_search} />
        </Column>
        "#
    );
}
