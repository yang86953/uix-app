// 引入 UIX 公开 View 类型。
use uix::prelude::ViewNode;

// 构造只用于验证生成文件诊断位置的编译失败入口。
fn main() {
    // 未定义名称应由 rustc 在 UIX 生成文件中报告。
    let _view: ViewNode = uix::uix!("src/main.uix");
}
