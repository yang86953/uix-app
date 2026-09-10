//! 主演示真实 GPU surface 的跨 Adapter 像素验收。

// 引入有界等待时长。
use std::time::Duration;

// 引入 App、AppHandle、ViewNode 与声明式 View 入口。
use uix_app::prelude::*;
// 引入 Drawing 层 API 无关的回读结果。
use uix_app::draw::SurfaceReadback;

// 使用专用参考页固定逻辑布局的比例坐标抽样 Shape 与 Gradient。
const REFERENCE_WIDTH: i32 = 400;
// 保存专用参考页固定逻辑高度。
const REFERENCE_HEIGHT: i32 = 500;
// 保存主题强调色的规范 0xAARRGGBB 值。
const ACCENT_ORANGE: u32 = 0xFFFF_7716;
// 保存标题卡片内部背景的规范像素值。
const HEADER_FILL: u32 = 0xFFF5_F5F5;
// 保存标题卡片外部画布背景的规范像素值。
const CANVAS_WHITE: u32 = 0xFFFF_FFFF;
// 保存确定性零模糊阴影的规范不透明黑色。
const SHADOW_BLACK: u32 = 0xFF00_0000;
// 保存固定文本参考区允许的最少非背景像素，拒绝空字形或缺少 CJK fallback。
const MIN_TEXT_INK_PIXELS: usize = 80;

// 构造只存在于 test-harness 的真实 GPU 像素参考应用。
pub(super) fn build_app() -> App {
    // 创建由声明式 ScrollView 双向绑定的 API 无关逻辑偏移。
    let scroll_offset = State::new(Point::new(0.0, 0.0));
    // 克隆状态供每次声明协调重建同一个受控滚动视图。
    let root_scroll_offset = scroll_offset.clone();
    // 保留另一份句柄供测试线程通过 UI 队列触发第二帧。
    let scheduled_scroll_offset = scroll_offset;
    // 返回由 Application System 持有的独立窗口与生命周期。
    App::new()
        // 使用明确标题区分普通主演示与自动像素验收。
        .title("UIX 跨平台图形像素验收")
        // 固定逻辑尺寸，使所有平台使用同一参考坐标。
        .size(REFERENCE_WIDTH, REFERENCE_HEIGHT)
        // 每次协调重建同一个受控参考 View，偏移变化只属于组件状态语义。
        .root(move || build_reference_view(root_scroll_offset.clone()))
        // 首帧前通过公开 AppHandle 安排初始帧与滚动帧两次最终 surface 回读。
        .on_start(move |handle| schedule(handle, scheduled_scroll_offset.clone()))
}

// 从独立声明文件构造 Shape 与半透明径向 Gradient 参考页。
fn build_reference_view(scroll_offset: State<Point>) -> ViewNode {
    // 编译期生成普通 UI View，不让验收代码接触 Renderer 或原生 API。
    uix!("src/graphics_readback.uix")
}

// 安排初始与受控滚动帧回读，并在独立线程执行有界像素和执行路径断言。
fn schedule(handle: AppHandle, scroll_offset: State<Point>) {
    // on_start 发生在首帧前，当前窗口已经装配真实 Renderer 消费者。
    let initial_ticket = match handle.request_surface_readback_for_test() {
        // 保存由 owner-thread Renderer 完成的一次性票据。
        Ok(ticket) => ticket,
        // 请求失败时立即输出稳定标记并终止专用验收进程。
        Err(error) => {
            // 使用 stderr 区分验收失败与普通演示日志。
            eprintln!("UIX_GRAPHICS_READBACK_FAILED request={}", error.what());
            // 专用运行参数明确授权以非零状态结束测试进程。
            std::process::exit(2);
        }
    };
    // 等待动作不能占用 UI 线程，否则首帧无法执行。
    let spawn = std::thread::Builder::new()
        // 使用稳定线程名便于原生调试器定位验收逻辑。
        .name("uix-graphics-readback-test".to_string())
        // 移动一次性票据进入后台等待线程。
        .spawn(move || {
            // 为真实窗口创建、绘制和 GPU 同步保留充足但有限的时间。
            let initial_readback = match initial_ticket.recv_timeout(Duration::from_secs(15)) {
                // owner thread 成功交付规范 surface 快照。
                Ok(readback) => readback,
                // 超时、呈现或 Adapter 错误都必须使验收失败。
                Err(error) => {
                    // 输出机器可识别的失败标记与完整原因。
                    eprintln!("UIX_GRAPHICS_READBACK_FAILED receive={}", error.what());
                    // 使用固定非零状态结束专用验收进程。
                    std::process::exit(3);
                }
            };
            // 核对 Shape 四边、内部填充、画布与半透明径向 Gradient。
            if let Err(reason) = validate_reference_pixels(&initial_readback) {
                // 输出尺寸与首个失败断言，便于跨平台 CI 定位。
                eprintln!(
                    // 使用单行稳定标记供 runner 抽取。
                    "UIX_GRAPHICS_READBACK_FAILED width={} height={} reason={reason}",
                    // 报告真实物理宽度。
                    initial_readback.width,
                    // 报告真实物理高度。
                    initial_readback.height,
                );
                // 像素不一致使用独立非零状态。
                std::process::exit(4);
            }
            // 在更新状态前安排下一次成功内容帧的最终 surface 回读。
            let moved_ticket = match handle.request_surface_readback_for_test() {
                // 保存只会由滚动更新帧完成的一次性票据。
                Ok(ticket) => ticket,
                // 重复请求或窗口关闭都必须作为明确失败返回。
                Err(error) => {
                    // 输出稳定阶段名与 Drawing 层错误。
                    eprintln!("UIX_GRAPHICS_READBACK_FAILED move-request={}", error.what());
                    // 第二次请求失败使用独立退出状态。
                    std::process::exit(7);
                }
            };
            // 受控 State 必须在 UI 线程更新，由正常声明协调产生滚动合成记录。
            let update_handle = handle.clone();
            // 通过窗口 UI 队列串行发布状态与声明根更新。
            handle.post_to_ui(move || {
                // 向下滚动四十逻辑像素，触发受控偏移对应的局部纹理搬移。
                scroll_offset.set(Point::new(0.0, 40.0));
                // 克隆已更新状态供下一轮声明根读取受控偏移。
                let updated_scroll_offset = scroll_offset.clone();
                // 显式请求正常 ViewAdapter 协调，避免测试依赖隐式绘制订阅唤醒时序。
                update_handle.update_view(move || build_reference_view(updated_scroll_offset));
            });
            // 有界等待状态协调、共享 FramePlan、Adapter 执行与最终 present。
            let moved_readback = match moved_ticket.recv_timeout(Duration::from_secs(15)) {
                // owner thread 成功交付滚动后的规范 surface 快照。
                Ok(readback) => readback,
                // 超时或底层 typed failure 都不能伪装成执行证据。
                Err(error) => {
                    // 输出机器可识别的第二帧失败标记。
                    eprintln!("UIX_GRAPHICS_READBACK_FAILED move-receive={}", error.what());
                    // 第二帧回读失败使用独立退出状态。
                    std::process::exit(8);
                }
            };
            // 第二帧仍需满足不受滚动影响的 Shape、Gradient、Shadow 与字体契约。
            if let Err(reason) = validate_reference_pixels(&moved_readback) {
                // 输出滚动后完整参考页的尺寸与首个失败断言。
                eprintln!(
                    // 使用单行稳定标记供 runner 抽取。
                    "UIX_GRAPHICS_READBACK_FAILED phase=after-move width={} height={} reason={reason}",
                    // 报告第二帧真实物理宽度。
                    moved_readback.width,
                    // 报告第二帧真实物理高度。
                    moved_readback.height,
                );
                // 滚动污染其它图元时使用独立退出状态。
                std::process::exit(9);
            }
            // 核对 retained 搬移后的像素确实来自初始帧对应内容位置。
            if let Err(reason) = validate_scroll_move_pixels(&initial_readback, &moved_readback) {
                // 输出独立阶段，区分最终图元正确但像素搬移内容错误。
                eprintln!("UIX_GRAPHICS_READBACK_FAILED phase=texture-move-pixels reason={reason}");
                // 搬移像素关系失败使用独立退出状态。
                std::process::exit(10);
            }
            // 只有共享 FramePlan 真正成功执行 TextureMove 才能通过路径验收。
            if moved_readback.executed_texture_moves == 0 {
                // 禁止把整帧重绘后相同的最终像素误报成局部搬移成功。
                eprintln!("UIX_GRAPHICS_READBACK_FAILED phase=texture-move count=0");
                // 缺少真实移动命令证据时使用独立退出状态。
                std::process::exit(11);
            }
            // 对滚动后的固定文本区域生成可供 Windows 与 Linux 直接比较的像素签名。
            let (_, text_hash) = text_region_signature(&moved_readback);
            // 输出可由 Linux 与 Windows runner 共用的成功证据标记。
            println!(
                // 成功行只包含稳定键值。
                "UIX_GRAPHICS_READBACK_OK width={} height={} format=AARRGGBB checks=shape,radial-gradient,box-shadow,glyph-coverage,texture-move-pixels,texture-move texture_moves={} text_hash={text_hash:016X}",
                // 报告真实物理宽度。
                moved_readback.width,
                // 报告真实物理高度。
                moved_readback.height,
                // 报告本帧共享 FramePlan 实际执行的移动次数。
                moved_readback.executed_texture_moves,
            );
            // 专用验收已完成，避免正常演示事件循环继续占用 runner。
            std::process::exit(0);
        });
    // 线程创建失败同样必须让专用验收得到非零结果。
    if let Err(error) = spawn {
        // 输出稳定的线程创建失败标记。
        eprintln!("UIX_GRAPHICS_READBACK_FAILED spawn={error}");
        // 使用独立状态结束无法继续的验收进程。
        std::process::exit(12);
    }
}

// 按参考逻辑布局的比例抽样像素，兼容不同 DPI 的物理 surface。
fn validate_reference_pixels(readback: &SurfaceReadback) -> Result<(), String> {
    // 非正尺寸或载荷缺失不能进入比例坐标换算。
    if readback.width <= 0 || readback.height <= 0 {
        // 返回不依赖 Adapter 的稳定失败原因。
        return Err("surface extent is not positive".to_string());
    }
    // 载荷必须与公开快照尺寸精确一致。
    let expected_len = (readback.width as usize).saturating_mul(readback.height as usize);
    // 拒绝任何裁切或带行距结果。
    if readback.pixels.len() != expected_len {
        // 报告实际与期望长度。
        return Err(format!(
            // 使用稳定的紧密载荷诊断。
            "payload length {} does not match {expected_len}",
            // 插入实际像素数量。
            readback.pixels.len(),
        ));
    }
    // 定义覆盖 Shape 卡片四条边、内部和外部的精确参考样本。
    let samples = [
        // 上边中段应为完整强调色。
        ("top", 100, 21, ACCENT_ORANGE),
        // 下边中段应为完整强调色。
        ("bottom", 100, 107, ACCENT_ORANGE),
        // 左边中段应为完整强调色。
        ("left", 21, 60, ACCENT_ORANGE),
        // 右边中段应为完整强调色。
        ("right", 388, 60, ACCENT_ORANGE),
        // 卡片内部应保持固定浅灰填充。
        ("inner", 100, 60, HEADER_FILL),
        // 两个参考图元右侧应保持画布白色。
        ("outside", 300, 200, CANVAS_WHITE),
        // 零模糊阴影向右偏移十二像素后露出的条带必须保持不透明黑色。
        ("shadow-offset", 186, 330, SHADOW_BLACK),
    ];
    // 逐个核对所有结构位置，首个不一致即返回。
    for (name, reference_x, reference_y, expected) in samples {
        // 将参考横坐标按真实 drawable 宽度缩放。
        let x = scale_coordinate(reference_x, readback.width, REFERENCE_WIDTH);
        // 将参考纵坐标按真实 drawable 高度缩放。
        let y = scale_coordinate(reference_y, readback.height, REFERENCE_HEIGHT);
        // 使用紧密顶到底行序计算像素索引。
        let index = (y as usize)
            // 定位目标行起点。
            .saturating_mul(readback.width as usize)
            // 加上目标列偏移。
            .saturating_add(x as usize);
        // 载荷长度已验证，因此该索引必须存在。
        let actual = readback.pixels[index];
        // 精确颜色值能同时发现通道顺序和绘制缺边问题。
        if actual != expected {
            // 搜索期望颜色的真实边界，帮助识别布局偏移与颜色语义错误。
            let bounds = exact_color_bounds(readback, expected);
            // 返回包含样本名、坐标和规范像素的诊断。
            return Err(format!(
                // 使用固定十六进制宽度便于平台间比较。
                "{name}@({x},{y}) expected {expected:#010X}, got {actual:#010X}, expected-bounds={bounds:?}",
            ));
        }
    }
    // 读取半透明红色径向 Gradient 中心像素。
    let radial = sample_reference_pixel(readback, 100, 200);
    // 拆出规范 AARRGGBB 的四个通道。
    let alpha = ((radial >> 24) & 0xFF) as u8;
    // 红色通道在正确 straight-alpha SrcOver 后必须接近不透明红。
    let red = ((radial >> 16) & 0xFF) as u8;
    // 绿色通道只来自白色背景的剩余一半。
    let green = ((radial >> 8) & 0xFF) as u8;
    // 蓝色通道与绿色通道应保持相同范围。
    let blue = (radial & 0xFF) as u8;
    // 目标为不透明白底，因此最终 alpha 必须保持完全不透明。
    let radial_matches = alpha == 0xFF
        // 正确结果约为 255，旧的双乘 alpha 结果约为 191。
        && red >= 250
        // 允许八位颜色解析与驱动舍入产生一个小范围差异。
        && (120..=135).contains(&green)
        // 两个非红通道必须落在同一容差范围。
        && (120..=135).contains(&blue);
    // 明确拒绝 OpenGL 径向分支曾经出现的二次 premultiply 结果。
    if !radial_matches {
        // 返回完整像素和通道诊断供真实平台 runner 比较。
        return Err(format!(
            // 使用稳定样本名和十六进制格式。
            "radial-center expected straight-alpha red-over-white, got {radial:#010X} rgba=({red},{green},{blue},{alpha})",
        ));
    }
    // 统计固定卡片内部的正文覆盖率并生成完整区域签名。
    let (text_ink_pixels, _) = text_region_signature(readback);
    // 字体未安装、CJK 缺字或 R8 coverage 未绘制都会留下接近纯背景的区域。
    if text_ink_pixels < MIN_TEXT_INK_PIXELS {
        // 报告实际覆盖像素数量，避免只得到笼统的颜色失败。
        return Err(format!(
            // 使用稳定字段供跨平台 runner 提取。
            "glyph coverage region has only {text_ink_pixels} non-background pixels",
        ));
    }
    // 所有结构样本都满足统一视觉契约。
    Ok(())
}

// 验证四十逻辑像素滚动后，保留区域与初始帧对应来源像素完全一致。
fn validate_scroll_move_pixels(
    // 借用执行移动前的最终 surface 快照。
    initial: &SurfaceReadback,
    // 借用执行移动后的最终 surface 快照。
    moved: &SurfaceReadback,
) -> Result<(), String> {
    // 两帧必须观察同一个未 resize 的物理 surface。
    if initial.width != moved.width || initial.height != moved.height {
        // 尺寸变化会使坐标关系无法证明，必须明确拒绝。
        return Err(format!(
            // 报告两帧完整尺寸供真实平台定位。
            "surface extent changed from {}x{} to {}x{}",
            // 初始帧宽度。
            initial.width,
            // 初始帧高度。
            initial.height,
            // 移动帧宽度。
            moved.width,
            // 移动帧高度。
            moved.height,
        ));
    }
    // 选取滚动视口保留区域内的多列、多行稳定样本。
    let samples = [
        // 上部左侧目标应来自初始帧下方四十像素。
        ("upper-left", 40, 398, 438),
        // 上部中间目标覆盖渐变主体。
        ("upper-center", 100, 398, 438),
        // 下部中间目标仍位于非暴露保留区域。
        ("lower-center", 100, 418, 458),
        // 下部右侧目标验证整条横向搬移范围。
        ("lower-right", 300, 418, 458),
    ];
    // 逐个比较移动帧目标与初始帧来源，禁止只凭执行计数通过。
    for (name, reference_x, destination_y, source_y) in samples {
        // 读取初始帧中将被 TextureMove 复制的来源像素。
        let expected = sample_reference_pixel(initial, reference_x, source_y);
        // 读取移动帧中对应的 retained 目标像素。
        let actual = sample_reference_pixel(moved, reference_x, destination_y);
        // 同一纹理搬移必须逐位保持规范 AARRGGBB 值。
        if actual != expected {
            // 返回稳定样本名、逻辑坐标与两侧规范值。
            return Err(format!(
                // 十六进制值可直接发现通道、方向或暴露区错误。
                "{name} source=({reference_x},{source_y}) destination=({reference_x},{destination_y}) expected {expected:#010X}, got {actual:#010X}",
            ));
        }
    }
    // 全部保留样本满足同一滚动位移关系。
    Ok(())
}

// 对固定正文逻辑区域统计墨迹像素并计算 FNV-1a 像素签名。
fn text_region_signature(readback: &SurfaceReadback) -> (usize, u64) {
    // 横向区域避开四像素橙色边框。
    let left = scale_coordinate(50, readback.width, REFERENCE_WIDTH);
    // 右边界覆盖居中的完整中英文文本。
    let right = scale_coordinate(350, readback.width, REFERENCE_WIDTH);
    // 纵向区域避开卡片上边框。
    let top = scale_coordinate(35, readback.height, REFERENCE_HEIGHT);
    // 下边界保持在卡片内部浅灰背景内。
    let bottom = scale_coordinate(85, readback.height, REFERENCE_HEIGHT);
    // 从零个非背景像素开始统计。
    let mut ink_pixels = 0_usize;
    // 使用标准 64 位 FNV-1a offset basis。
    let mut hash = 0xCBF2_9CE4_8422_2325_u64;
    // 按规范顶到底行序扫描固定逻辑区域。
    for y in top..bottom {
        // 每行按从左到右顺序进入签名。
        for x in left..right {
            // 从紧密 surface 载荷取得当前 AARRGGBB 像素。
            let pixel = readback.pixels
                // 使用已经限制在有效范围内的物理坐标计算索引。
                [(y as usize) * (readback.width as usize) + x as usize];
            // 将完整规范像素混入 FNV-1a 状态。
            hash ^= pixel as u64;
            // 使用标准 64 位 FNV prime 推进有序签名。
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
            // 与卡片浅灰不同的覆盖或抗锯齿像素都属于正文墨迹。
            if pixel != HEADER_FILL {
                // 饱和计数避免异常大 surface 理论上的 usize 回绕。
                ink_pixels = ink_pixels.saturating_add(1);
            }
        }
    }
    // 返回结构断言所需数量和跨平台比较所需哈希。
    (ink_pixels, hash)
}

// 查找一个精确规范颜色在真实 surface 中的最小包围盒与像素数量。
fn exact_color_bounds(
    readback: &SurfaceReadback,
    expected: u32,
) -> Option<(i32, i32, i32, i32, usize)> {
    // 从尚未命中任何像素的空边界开始。
    let mut bounds: Option<(i32, i32, i32, i32, usize)> = None;
    // 按紧密顶到底行序扫描全部像素。
    for (index, pixel) in readback.pixels.iter().copied().enumerate() {
        // 只统计与规范值完全相同的像素。
        if pixel != expected {
            // 跳过其它颜色。
            continue;
        }
        // 从紧密索引恢复物理横坐标。
        let x = (index % readback.width as usize) as i32;
        // 从紧密索引恢复物理纵坐标。
        let y = (index / readback.width as usize) as i32;
        // 扩张已有边界或建立首个命中边界。
        bounds = Some(match bounds {
            // 合并当前像素并推进计数。
            Some((left, top, right, bottom, count)) => (
                // 更新最左坐标。
                left.min(x),
                // 更新最上坐标。
                top.min(y),
                // 更新最右坐标。
                right.max(x),
                // 更新最下坐标。
                bottom.max(y),
                // 记录精确像素数量。
                count + 1,
            ),
            // 第一个命中像素同时建立四条边。
            None => (x, y, x, y, 1),
        });
    }
    // 返回空或已经完成的确定边界。
    bounds
}

// 读取一个参考逻辑坐标对应的规范 surface 像素。
fn sample_reference_pixel(readback: &SurfaceReadback, reference_x: i32, reference_y: i32) -> u32 {
    // 将参考横坐标按真实 drawable 宽度缩放。
    let x = scale_coordinate(reference_x, readback.width, REFERENCE_WIDTH);
    // 将参考纵坐标按真实 drawable 高度缩放。
    let y = scale_coordinate(reference_y, readback.height, REFERENCE_HEIGHT);
    // 载荷长度已由调用方验证，直接返回紧密顶到底行序中的像素。
    readback.pixels[(y as usize) * (readback.width as usize) + x as usize]
}

// 把参考逻辑坐标映射到当前物理 surface 并限制在有效范围内。
fn scale_coordinate(reference: i32, actual_extent: i32, reference_extent: i32) -> i32 {
    // 使用 64 位乘法避免高 DPI surface 上的中间值溢出。
    let scaled = (reference as i64)
        // 按当前物理 extent 缩放。
        .saturating_mul(actual_extent as i64)
        // 以参考 extent 归一化。
        / reference_extent as i64;
    // 防御性限制到最后一个有效像素。
    scaled.clamp(0, actual_extent.saturating_sub(1) as i64) as i32
}
