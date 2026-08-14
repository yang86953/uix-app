// 导入应用、视图、主题与反馈组件公开门面。
use uix::prelude::*;
// 显式导入尚未进入通用 prelude 的浮层公开契约。
use uix::ui::{Modal, OverlayBackdropBlur};

// 构造 backdrop blur 真窗验收根视图。
fn acceptance_view() -> ViewNode {
    // Modal 从首帧开始保持打开。
    let open = State::new(true);
    // 创建纯 RHI 原生命令背景与居中 Modal。
    column((
        // Canvas 只产生 GPU-native fill_rect，不引入 CPU raster segment。
        canvas(900.0, 560.0, |_frame, ctx| {
            // 绘制覆盖全窗口的明暗交错竖条。
            for column in 0..9 {
                // 偶数列使用暖红色，奇数列使用冷蓝色。
                let color = if column % 2 == 0 {
                    // 暖色提供强烈红通道边界。
                    Color::from_rgb(238, 64, 64)
                } else {
                    // 冷色提供强烈蓝通道边界。
                    Color::from_rgb(66, 133, 244)
                };
                // 每列固定一百逻辑像素，便于像素梯度断言。
                ctx.fill_rect(
                    // 覆盖完整高度让 Modal 周围始终可见背景边界。
                    Rect::new(column as f32 * 100.0, 0.0, 100.0, 560.0),
                    // 使用当前交错颜色。
                    color,
                    // 直角矩形保持 RHI 原生命令。
                    None,
                );
            }
        }),
        // Modal 使用显式二十四逻辑像素并跟随全窗口 mask 区域，形成可判定证据。
        Modal::builder()
            // 绑定首帧打开状态。
            .open(&open)
            // 设置可访问标题供 Agent 验收。
            .title("Overlay Backdrop Blur 已启用")
            // 让面板覆盖足够背景边界。
            .size(520.0, 260.0)
            // 使用强效果半径让真窗验收能明确区分模糊与 mask-only。
            .backdrop_blur(OverlayBackdropBlur::radius(24.0))
            // 添加清晰前景内容，验证只模糊 backdrop。
            .content(|| {
                // 前景文字必须保持锐利可读。
                column((
                    // 说明效果来源。
                    label("背景采用 D3D11 retained snapshot + 24px 高斯模糊。"),
                    // 说明验收重点。
                    label("核对：背景色块边界柔化；对话框文字、边框与按钮保持清晰。"),
                    // 真实按钮验证 overlay 前景仍正常绘制。
                    embed(Button::new("前景按钮保持清晰")),
                ))
            }),
    ))
}

// 启动独立 D3D11 真窗验收应用。
fn main() {
    // 使用公开 App 组合根，不建立第二 UI 或图形路径。
    App::new()
        // 设置稳定窗口标题。
        .title("UIX Overlay Backdrop Blur 视觉验收")
        // 固定逻辑尺寸便于像素审阅。
        .size(900, 560)
        // 明确选择参考 D3D11 adapter。
        .graphics_backend(GraphicsBackend::Direct3D11)
        // 根工厂每窗构造独立状态与视图。
        .root(acceptance_view)
        // 专用验收程序显式开放同用户本机 Agent Adapter。
        .enable_agent_control()
        // 进入现有原生窗口事件循环。
        .run();
}
