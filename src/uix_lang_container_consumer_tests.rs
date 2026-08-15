// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 验证 Container 复合视觉简写只依赖公开 UIX 运行时契约。
#[test]
fn container_border_and_shadow_compile_against_public_uix_api() {
    // 展开包含具体边框、完整双轴阴影和明确清除阴影的真实消费者文档。
    let _view: ViewNode = uix!(
        r##"<Container border="1px #d9d9d9" shadow="-2px 4px 8px rgba(0, 0, 0, 0.15)"><Container shadow="none"><Text>内容</Text></Container></Container>"##
    );
}
