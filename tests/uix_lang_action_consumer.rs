// 引入真实使用方可见的 UIX 公开 API 与过程宏。
use uix::prelude::*;

// 提供 action 返回值的真实 Rust 消费方。
fn consume_count(_count: f64) {}

// 验证多语句同步 action 的生成 Rust 通过真实 consumer 类型检查。
#[test]
fn structured_widget_actions_compile_as_external_consumer() {
    let _view: ViewNode = uix!(
        r#"
        resting { color: #333333; }
        active { color: #1677ff; }
        <Widget name="ActionCounter" state="count: 0, limit: 10" external="consume_count"
                actions="cap: do { if count >= limit { setStyle('active'); return; } setState(count: count + 1); }, run: do { let next = count + 1; next = next + 1; cap(); setState(count: next); return; }, read: do { if count > 0 { return count; } return 0; }">
          <Button class="resting" @click="run()">{count}</Button>
          <Button @click="consume_count(read())">读取</Button>
        </Widget>
        <ActionCounter />
        "#
    );
}
