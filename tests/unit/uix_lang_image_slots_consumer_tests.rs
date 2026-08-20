// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 验证 Image 命名状态 View 只依赖公开 UIX 运行时契约。
#[test]
fn image_named_slots_compile_against_public_uix_api() {
    // 展开同时声明加载占位与错误 View 工厂的真实消费者文档。
    let _view: ViewNode = uix!(
        r#"<Image src="assets/hero.png"><Container slot="placeholder"><Text>正在加载</Text></Container><Container slot="error"><Text>加载失败</Text></Container></Image>"#
    );
}
