// 同一成员混用属性形式与块级形式：必须在解析层被定向拒绝并点名冲突成员。

use ::uix_derive::uix;

// 触发混合双写诊断的最小文档；宏失败即本 crate 构建失败。
fn probe() {
    let _view = uix!(
        r#"<Widget name="Counter" state="count: 0">
  @state {
    draft: '',
  }
  <Text>t</Text>
</Widget><Counter />"#
    );
}
