// 导入应用、视图、主题与反馈组件公开门面。
use uix::prelude::*;
// 导入 Drawing 层 API 无关的规范 surface 快照。
#[cfg(feature = "test-harness")]
use uix::draw::SurfaceReadback;
// 显式导入尚未进入通用 prelude 的浮层公开契约。
use uix::ui::{Modal, OverlayBackdropBlur};
// 引入有界 GPU 回读等待时长。
#[cfg(feature = "test-harness")]
use std::time::Duration;

// 固定参考窗口宽度，保证两个 Adapter 使用相同逻辑采样坐标。
const REFERENCE_WIDTH: i32 = 900;
// 固定参考窗口高度，保证两个 Adapter 使用相同逻辑采样坐标。
const REFERENCE_HEIGHT: i32 = 560;

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
                    label("背景采用 RHI retained snapshot + 24px 高斯模糊。"),
                    // 说明验收重点。
                    label("核对：背景色块边界柔化；对话框文字、边框与按钮保持清晰。"),
                    // 真实按钮验证 overlay 前景仍正常绘制。
                    embed(Button::new("前景按钮保持清晰")),
                ))
            }),
    ))
}

// 安排下一次真实 GPU 帧的最终 surface 回读。
#[cfg(feature = "test-harness")]
fn schedule_readback(handle: AppHandle) {
    // on_start 在首帧前登记一次性 Drawing 快照票据。
    let ticket = match handle.request_surface_readback_for_test() {
        // 保存由 owner-thread Renderer 完成的票据。
        Ok(ticket) => ticket,
        // 请求失败必须使自动验收返回非零状态。
        Err(error) => {
            // 输出跨平台 runner 可识别的失败标记。
            eprintln!("UIX_BACKDROP_READBACK_FAILED request={}", error.what());
            // 终止无法继续的专用验收进程。
            std::process::exit(2);
        }
    };
    // 等待不能阻塞 UI owner thread，否则首帧无法提交。
    let spawn = std::thread::Builder::new()
        // 使用稳定线程名便于原生调试器定位。
        .name("uix-backdrop-readback-test".to_string())
        // 把一次性票据移动到后台线程。
        .spawn(move || {
            // 为真窗创建、双 pass blur、最终合成和 GPU 同步保留有界时间。
            let readback = match ticket.recv_timeout(Duration::from_secs(15)) {
                // owner thread 已交付规范快照。
                Ok(readback) => readback,
                // 超时或 Adapter 失败必须保留具体原因。
                Err(error) => {
                    // 输出稳定失败标记供 runner 抽取。
                    eprintln!("UIX_BACKDROP_READBACK_FAILED receive={}", error.what());
                    // 使用独立非零状态结束等待失败。
                    std::process::exit(3);
                }
            };
            // 用条纹远端与边界混色共同证明真实 blur shader 已执行。
            if let Err(reason) = validate_blurred_stripes(&readback) {
                // 报告真实 drawable 尺寸与首个失败断言。
                eprintln!(
                    // 使用单行稳定格式便于 Linux 与 Windows 对照。
                    "UIX_BACKDROP_READBACK_FAILED width={} height={} reason={reason}",
                    // 插入物理宽度。
                    readback.width,
                    // 插入物理高度。
                    readback.height,
                );
                // 像素证据不满足契约时返回独立状态。
                std::process::exit(4);
            }
            // 读取成功证据中使用的四个规范像素。
            let warm = sample_reference_pixel(&readback, 50, 50);
            // 读取暖色侧靠近边界的混色像素。
            let seam_warm = sample_reference_pixel(&readback, 96, 50);
            // 读取冷色侧靠近边界的混色像素。
            let seam_cool = sample_reference_pixel(&readback, 104, 50);
            // 读取冷色条纹远离边界的基准像素。
            let cool = sample_reference_pixel(&readback, 150, 50);
            // 输出可由不同平台 runner 共用的成功证据。
            println!(
                // 保留规范 AARRGGBB 样本，便于直接比较 Adapter 结果。
                "UIX_BACKDROP_READBACK_OK width={} height={} format=AARRGGBB warm={warm:#010X} seam-warm={seam_warm:#010X} seam-cool={seam_cool:#010X} cool={cool:#010X}",
                // 插入物理宽度。
                readback.width,
                // 插入物理高度。
                readback.height,
            );
            // 专用验收已完成，不继续占用事件循环。
            std::process::exit(0);
        });
    // 线程创建失败同样不能留下悬空测试进程。
    if let Err(error) = spawn {
        // 输出稳定线程失败诊断。
        eprintln!("UIX_BACKDROP_READBACK_FAILED spawn={error}");
        // 使用独立非零状态终止验收。
        std::process::exit(5);
    }
}

// 验证远端条纹保持主色，同时边界两侧都被高斯核混入另一通道。
#[cfg(feature = "test-harness")]
fn validate_blurred_stripes(readback: &SurfaceReadback) -> Result<(), String> {
    // 尺寸与像素载荷必须先满足 Drawing 快照契约。
    if readback.width <= 0 || readback.height <= 0 {
        // 返回不依赖平台的稳定错误。
        return Err("surface extent is not positive".to_string());
    }
    // 计算紧密顶到底行序的期望像素数量。
    let expected_len = (readback.width as usize).saturating_mul(readback.height as usize);
    // 拒绝任何裁切或带行距载荷。
    if readback.pixels.len() != expected_len {
        // 报告实际与期望长度。
        return Err(format!(
            // 使用稳定载荷诊断。
            "payload length {} does not match {expected_len}",
            // 插入实际像素数量。
            readback.pixels.len(),
        ));
    }
    // 读取第一条暖色条纹中央，作为未跨边界混色基准。
    let warm = sample_reference_pixel(readback, 50, 50);
    // 读取第二条冷色条纹中央，作为未跨边界混色基准。
    let cool = sample_reference_pixel(readback, 150, 50);
    // 读取暖色一侧距离边界四像素的位置。
    let seam_warm = sample_reference_pixel(readback, 96, 50);
    // 读取冷色一侧距离边界四像素的位置。
    let seam_cool = sample_reference_pixel(readback, 104, 50);
    // 抽取四个样本的 RGB 通道。
    let warm_rgb = rgb(warm);
    // 抽取冷色远端 RGB 通道。
    let cool_rgb = rgb(cool);
    // 抽取暖色边界 RGB 通道。
    let seam_warm_rgb = rgb(seam_warm);
    // 抽取冷色边界 RGB 通道。
    let seam_cool_rgb = rgb(seam_cool);
    // 远端暖色必须继续明显由红通道主导。
    let warm_is_distinct = i32::from(warm_rgb.0) - i32::from(warm_rgb.2) >= 30;
    // 远端冷色必须继续明显由蓝通道主导。
    let cool_is_distinct = i32::from(cool_rgb.2) - i32::from(cool_rgb.0) >= 30;
    // 暖色边界必须从相邻冷色条纹获得显著额外蓝通道。
    let warm_side_is_mixed = i32::from(seam_warm_rgb.2) - i32::from(warm_rgb.2) >= 8;
    // 冷色边界必须从相邻暖色条纹获得显著额外红通道。
    let cool_side_is_mixed = i32::from(seam_cool_rgb.0) - i32::from(cool_rgb.0) >= 8;
    // 四项同时成立才能排除未执行 blur 的 mask-only 画面。
    if !(warm_is_distinct && cool_is_distinct && warm_side_is_mixed && cool_side_is_mixed) {
        // 报告全部规范像素，方便判断坐标、通道或 shader 差异。
        return Err(format!(
            // 使用固定宽度十六进制格式。
            "expected blurred red-blue seam, got warm={warm:#010X} seam-warm={seam_warm:#010X} seam-cool={seam_cool:#010X} cool={cool:#010X}",
        ));
    }
    // 远端与边界关系满足统一视觉契约。
    Ok(())
}

// 从规范 AARRGGBB 像素提取 RGB 通道。
#[cfg(feature = "test-harness")]
fn rgb(pixel: u32) -> (u8, u8, u8) {
    // 返回红、绿、蓝三个八位通道。
    (
        // 提取红通道。
        ((pixel >> 16) & 0xFF) as u8,
        // 提取绿通道。
        ((pixel >> 8) & 0xFF) as u8,
        // 提取蓝通道。
        (pixel & 0xFF) as u8,
    )
}

// 读取参考逻辑坐标对应的规范 surface 像素。
#[cfg(feature = "test-harness")]
fn sample_reference_pixel(readback: &SurfaceReadback, reference_x: i32, reference_y: i32) -> u32 {
    // 将参考横坐标按真实 drawable 宽度缩放。
    let x = scale_coordinate(reference_x, readback.width, REFERENCE_WIDTH);
    // 将参考纵坐标按真实 drawable 高度缩放。
    let y = scale_coordinate(reference_y, readback.height, REFERENCE_HEIGHT);
    // 载荷长度已经验证，按紧密顶到底行序返回目标像素。
    readback.pixels[(y as usize) * (readback.width as usize) + x as usize]
}

// 把参考逻辑坐标映射到当前物理 surface 并限制在有效范围。
#[cfg(feature = "test-harness")]
fn scale_coordinate(reference: i32, actual_extent: i32, reference_extent: i32) -> i32 {
    // 使用六十四位乘法避免高 DPI 中间值溢出。
    let scaled = (reference as i64)
        // 按当前物理 extent 缩放。
        .saturating_mul(actual_extent as i64)
        // 以参考 extent 归一化。
        / reference_extent as i64;
    // 防御性限制到最后一个有效像素。
    scaled.clamp(0, actual_extent.saturating_sub(1) as i64) as i32
}

// 启动由平台 registry 选择原生 Adapter 的同一真窗验收应用。
fn main() {
    // 只有显式参数才启用自动回读并在得到证据后退出。
    #[cfg(feature = "test-harness")]
    let readback_test = std::env::args().any(|argument| argument == "--test-readback");
    // 使用公开 App 组合根，不建立第二 UI 或图形路径。
    let app = App::new()
        // 设置稳定窗口标题。
        .title("UIX Overlay Backdrop Blur 视觉验收")
        // 固定逻辑尺寸便于像素审阅。
        .size(REFERENCE_WIDTH, REFERENCE_HEIGHT)
        // 根工厂每窗构造独立状态与视图。
        .root(acceptance_view)
        // 专用验收程序显式开放同用户本机 Agent Adapter。
        .enable_agent_control();
    // 自动模式在首帧前登记唯一 surface 回读。
    #[cfg(feature = "test-harness")]
    let app = if readback_test {
        // 返回带有一次性回读调度的应用构建器。
        app.on_start(schedule_readback)
    } else {
        // 手工视觉模式不改变原有真窗生命周期。
        app
    };
    // 进入由平台实现提供的原生窗口事件循环。
    app.run();
}
