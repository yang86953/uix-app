// 引入真实使用方可见的 UIX 公开宏与运行时门面。
use uix::prelude::*;

// 在模块级生成与 Rust 业务代码共享的 Record 类型。
uix_items!("tests/fixtures/uix_lang/records.uix");

// 验证 App、递归导入与 Record 三条正式 AOT 路径通过真实 consumer 类型检查。
#[test]
fn app_and_items_compile_as_external_consumer() {
    // App builder 由 Rust 组合根持有；测试不进入原生事件循环。
    let _app = uix_app!("tests/fixtures/uix_lang/imports/app-root.uix");
    // 生成类型必须能被同一 Rust 模块直接引用。
    let _profile: Option<Profile> = None;
}
