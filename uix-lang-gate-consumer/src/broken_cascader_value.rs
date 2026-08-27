// Widget 状态注解引用 CascaderValue 而 tree-widgets capability 未启用：
// 必须在生成前由值类型门禁定向拒绝；普通元素引用不先触发组件级需求。

use ::uix_derive::uix;

// 只渲染 Text 引用状态名，避免先撞上其他门禁而丢失 UIX2000 主证据。
fn probe() {
    let _view = uix!(
        r#"<Widget name="GateValueProbe" state="region: CascaderValue = { labels: [], values: [] }">
  <Text>gate</Text>
</Widget><GateValueProbe />"#
    );
}
