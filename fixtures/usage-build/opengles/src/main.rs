// 导入应用 builder 与受 OpenGL ES feature 门控的公开 backend 变体。
use uix::prelude::{App, GraphicsBackend};

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 显式选择 OpenGL ES，证明对应公开变体与应用配置入口可共同编译。
    let app = App::new().graphics_backend(GraphicsBackend::OpenGlEs);
    // 显式消费应用值，避免无意义的未使用警告。
    drop(app);
}
