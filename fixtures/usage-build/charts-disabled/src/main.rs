// 故意导入未启用 capability 的基础与高级图表类型，门禁要求此行无法编译。
use uix::prelude::{BarChart, Gauge};

// 提供 compile-fail fixture 的稳定入口。
fn main() {
    // 若基础图表公开面意外泄漏，构造值会使门禁检测到编译成功并失败。
    let bar_chart = BarChart::new();
    // 若高级图表公开面意外泄漏，构造值会阻止部分门控静默通过。
    let gauge = Gauge::new();
    // 显式消费值，避免公开面泄漏时只产生未使用警告。
    drop((bar_chart, gauge));
}
