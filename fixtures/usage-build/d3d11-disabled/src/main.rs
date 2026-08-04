// 导入仍然常驻的应用 builder 与 backend 类型。
use uix::prelude::{App, GraphicsBackend};

// 提供故意无法编译的公开面验证入口。
fn main() {
    // D3D11 feature 关闭后该公开枚举变体必须不存在。
    let app = App::new().graphics_backend(GraphicsBackend::Direct3D11);
    // 保留完整使用形态，防止错误来自无关的未使用代码。
    drop(app);
}
