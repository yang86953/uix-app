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
