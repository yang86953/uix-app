// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 验证组件状态可同时服务动态文本、结构条件与事件闭包。
#[test]
fn dynamic_text_compiles_against_public_uix_api() {
    // 创建窗口级动态计时状态。
    let tick = State::new(0.0_f64);
    // 创建真正决定结构的页面状态。
    let page = State::new(0.0_f64);
    // 创建只由动态文本与点击事件消费的计数状态。
    let count = State::new(0.0_f64);
    // 展开完整组件文档并只依赖公开 prelude。
    let _view: ViewNode = uix!(
        r#"
        <Widget name="DynamicStatus" props="tick: State<number>, page: State<number>, count: State<number>">
          <Column>
            <Text fontSize="small" automationId="live-tick">tick: {tick}s</Text>
            <Text automationId="live-count">count: {count}</Text>
            <If {page == 0}><Text>首页</Text></If>
            <Button @click="setState(count: count + 1)">增加</Button>
          </Column>
        </Widget>
        <DynamicStatus tick={tick} page={page} count={count} />
        "#
    );
    // 宏展开必须只克隆句柄，调用方仍可更新原状态。
    tick.set(1.0);
    // 调用方继续拥有结构状态。
    page.set(1.0);
    // 调用方继续拥有事件状态。
    count.set(2.0);
    // 三个共享槽都必须保留最新调用方值。
    assert_eq!((tick.get(), page.get(), count.get()), (1.0, 1.0, 2.0));
}
