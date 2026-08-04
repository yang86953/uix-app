// 导入应用和设置服务，覆盖单 capability 公开入口。
use uix::prelude::{App, SettingsService};

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 构造应用以保留主 crate 基础门面编译验证。
    let app = App::new();
    // 构造设置服务以触发 settings-serde capability 的公开类型。
    let settings = SettingsService::new();
    // 显式消费两个值，避免无意义的未使用警告。
    drop((app, settings));
}
