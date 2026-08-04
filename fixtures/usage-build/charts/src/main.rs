// 导入应用、基础图表与高级图表，覆盖单 capability 公开入口。
use uix::prelude::{App, BarChart, BarData, Color, Gauge};

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 构造应用以保留主 crate 基础门面编译验证。
    let app = App::new();
    // 构造基础柱状图以证明数据模型与组件公开面可达。
    let bar_chart = BarChart::new().data(vec![BarData::new("A", 1.0, Color::BLUE)]);
    // 构造高级仪表盘以证明高级图表构造器同步进入公开面。
    let gauge = Gauge::new().value(50.0).min(0.0).max(100.0);
    // 显式消费全部值，避免无意义的未使用警告。
    drop((app, bar_chart, gauge));
}
