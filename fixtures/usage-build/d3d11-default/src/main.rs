// 只导入默认图形入口需要的 App 类型。
use uix::prelude::App;

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 构造应用以触发默认 feature 公开门面编译。
    let app = App::new();
    // 显式消费应用值，避免无意义的未使用警告。
    drop(app);
}
