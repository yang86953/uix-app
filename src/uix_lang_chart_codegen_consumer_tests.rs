// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 验证图表共同高级配置在真实公开宏消费者中通过类型检查。
#[test]
fn chart_advanced_config_compiles_against_public_uix_api() {
    // 提供图表公开动画配置快照。
    let chart_animation = AnimationConfig::fade_in(0.2);
    // 提供图表公开点击、缩放、平移与十字线配置快照。
    let chart_interaction = InteractionConfig {
        // 启用滚轮缩放。
        zoom: true,
        // 启用拖拽平移。
        pan: true,
        // 启用十字参考线。
        crosshair: true,
        // 本编译证明不触发业务点击回调。
        on_click: None,
    };
    // 提供图表公开刷选配置快照。
    let chart_brush = BrushConfig {
        // 启用刷选。
        enabled: true,
        // 本编译证明不触发业务刷选回调。
        on_select: None,
    };
    // 提供图表公开 tooltip 模板配置快照。
    let chart_tooltip = TooltipConfig::new().template("{label}: {value}");
    // 提供动态公开图例位置。
    let chart_legend = LegendPosition::Right;
    // 展开五项配置并要求结果类型为公开 ViewNode。
    let _chart: ViewNode = crate::uix!(
        r##"<BarChart data={[BarData('Q1', 12, Color('#1677ff'))]} legend={chart_legend} animation={chart_animation} interactive={chart_interaction} brush={chart_brush} tooltip={chart_tooltip} />"##
    );
}
