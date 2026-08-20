// 引入被测后端，验证卸载边界而不依赖应用初始化。
use super::AbGlyphBackend;
// 引入字体后端 trait，使测试可以调用加载、卸载和内存统计接口。
use crate::draw::TextBackend;
// 引入字体布局选项与对齐枚举，验证 shaping 到光栅的完整链路。
#[cfg(target_os = "windows")]
use crate::draw::{HAlign, VAlign};
// 引入后端内部布局选项类型。
#[cfg(target_os = "windows")]
use super::TextLayoutOptions;

#[test]
fn unload_releases_owned_font_data() {
    // 使用仓库内稳定的 Lucide 字体作为最小可解析输入。
    let data = include_bytes!("../../../../../../assets/fonts/lucide.ttf");
    // 创建独立后端，避免测试之间共享字体槽位。
    let mut backend = AbGlyphBackend::new();
    // 加载字体并记录稳定句柄。
    let handle = backend
        .load_font(data)
        .expect("Lucide font must load in the ab_glyph backend");
    // 加载后底层字体数据必须计入后端内存统计。
    assert!(backend.memory_usage() > 0);
    // 卸载字体应释放底层数据而不是只使句柄失效。
    backend.unload_font(&handle);
    // 卸载后句柄不可用，避免继续访问已释放的借用。
    assert!(!backend.is_valid(&handle));
    // 卸载后不应残留字体数据占用。
    assert_eq!(backend.memory_usage(), 0);
}

// 验证 rustybuzz glyph id 可由既有 ab_glyph 光栅路径直接消费。
#[cfg(target_os = "windows")]
#[test]
fn shaped_glyph_ids_rasterize_with_existing_backend() {
    // 读取包含阿拉伯 GSUB/GPOS 的标准 Windows 字体。
    let data = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf")
        // 缺少标准字体时明确暴露环境不满足 Windows 目标矩阵。
        .expect("应能读取 Segoe UI 复杂脚本测试字体");
    // 创建独立后端，隔离其他字体槽位。
    let mut backend = AbGlyphBackend::new();
    // 通过正式加载入口同时建立 ab_glyph 与 rustybuzz 所需数据。
    let handle = backend
        // 加载完整字体文件。
        .load_font(&data)
        // 解析失败时明确指出字体后端契约。
        .expect("Segoe UI 应能由字体后端加载");
    // 构造不换行的标准 shaping 约束。
    let options = TextLayoutOptions {
        // 使用足够大的有限宽度。
        max_width: 4096.0,
        // 高度由文本自身决定。
        max_height: 0.0,
        // 使用稳定行高。
        line_height: 30.0,
        // 本测试不触发自动换行。
        word_wrap: false,
        // 使用左对齐观察原始视觉字形。
        h_align: HAlign::Left,
        // 使用顶部对齐观察原始基线。
        v_align: VAlign::Top,
        // 使用稳定测试字号。
        font_size: 20.0,
        // 结束布局选项构造。
    };
    // 通过正式 TextBackend 接口执行阿拉伯 shaping。
    let layout = backend.layout_text(&handle, "سلام", &options);
    // shaping 必须产生可供光栅化的字形。
    assert!(!layout.glyphs.is_empty());
    // 至少一个真实字形必须由既有轮廓光栅路径成功解析。
    assert!(
        layout
            // 遍历 shaping 输出字形。
            .glyphs
            // 尝试通过同一字体句柄光栅化。
            .iter()
            // 要求至少一个字形产生非空像素或轮廓网格。
            .any(|glyph| {
                // 执行既有统一字形光栅入口。
                let raster = backend.rasterize_glyph(&handle, glyph.glyph_id, options.font_size);
                // 面积覆盖或轮廓网格任一存在即证明编号兼容。
                !raster.coverage.is_empty() || raster.outline_mesh.is_some()
            })
    );
    // 结束 shaping 到光栅兼容测试。
}
