// 引入被测试的字体服务实现。
use super::FontService;
// 引入错误类型以实现测试后端加载契约。
use crate::core::Error;
// 引入统一文本后端 trait。
use crate::draw::TextBackend;
// 引入字体句柄与对齐枚举。
use crate::draw::{FontHandle, HAlign, VAlign};
// 引入测试后端所需的布局与光栅类型。
use crate::draw::resources::font::text_backend::{
    // 引入空光栅返回类型。
    GlyphRaster,
    // 引入行信息类型。
    LineInfo,
    // 引入水平行度量类型。
    LineMetrics,
    // 引入定位字形类型。
    PositionedGlyph,
    // 引入布局结果类型。
    TextLayout,
    // 引入布局选项类型。
    TextLayoutOptions,
    // 结束测试类型导入。
};

// 使用可控覆盖矩阵模拟主字体与回退字体。
#[derive(Debug)]
struct CoverageBackend;

// 实现最小文本后端以隔离字体文件差异。
impl TextBackend for CoverageBackend {
    // 测试不依赖真实字体数据，始终返回主句柄。
    fn load_font(&mut self, _data: &[u8]) -> Result<FontHandle, Error> {
        // 返回稳定主字体句柄。
        Ok(FontHandle::new(0))
        // 结束测试加载实现。
    }
    // 测试后端没有需要释放的外部资源。
    fn unload_font(&mut self, _handle: &FontHandle) {}
    // 句柄零和一分别代表主字体与回退字体。
    fn is_valid(&self, handle: &FontHandle) -> bool {
        // 只接受测试矩阵中的两个句柄。
        matches!(handle.0, 0 | 1)
        // 结束句柄有效性判断。
    }
    // 主字体只覆盖基字，回退字体同时覆盖组合符。
    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool {
        // 按句柄返回可控字符覆盖。
        match font.0 {
            // 主字体故意缺少组合音标。
            0 => ch == 'a',
            // 回退字体完整覆盖扩展字素簇。
            1 => matches!(ch, 'a' | '\u{0301}'),
            // 其他句柄均无字符覆盖。
            _ => false,
            // 结束覆盖矩阵匹配。
        }
        // 结束字形覆盖判断。
    }
    // 返回两个字形组成的同一 shaping cluster。
    fn layout_text(
        // 测试后端不依赖自身状态。
        &self,
        // 保留调用方选择的回退字体句柄。
        font: &FontHandle,
        // 测试文本仅用于计算逻辑终点。
        text: &str,
        // 复用调用方字号与行高。
        opts: &TextLayoutOptions,
        // 返回可被 FontService 二次拼接的段布局。
    ) -> TextLayout {
        // 当前测试文本固定包含两个 Unicode 标量。
        let char_end = text.chars().count();
        // 构造同一 cluster 的基字与组合符字形。
        let glyphs = vec![
            // 第一个字形消费主要 advance。
            PositionedGlyph {
                // 基字从段起点开始。
                x: 0.0,
                // 使用稳定测试基线。
                y: 8.0,
                // 基字宽度小于测试最大行宽。
                width: 6.0,
                // 使用调用方字号作为行盒高度。
                height: opts.font_size,
                // 使用稳定测试字形编号。
                glyph_id: 1,
                // cluster 从第一个字符开始。
                char_index: 0,
                // cluster 覆盖基字与组合符。
                char_end,
                // 测试后端默认使用 LTR，FontService 会回填行级双向级别。
                bidi_level: 0,
                // 保留回退字体句柄。
                font: *font,
                // 结束基字字形构造。
            },
            // 第二个字形仍属于同一 cluster。
            PositionedGlyph {
                // 组合符从基字 advance 后开始，用于触发潜在拆行。
                x: 6.0,
                // 使用同一测试基线。
                y: 8.0,
                // 第二字形使完整 cluster 超过测试最大行宽。
                width: 6.0,
                // 使用调用方字号作为行盒高度。
                height: opts.font_size,
                // 使用另一个稳定测试字形编号。
                glyph_id: 2,
                // 两个输出字形共享 cluster 起点。
                char_index: 0,
                // 两个输出字形共享 cluster 终点。
                char_end,
                // 测试后端默认使用 LTR，FontService 会回填行级双向级别。
                bidi_level: 0,
                // 保留回退字体句柄。
                font: *font,
                // 结束组合符字形构造。
            },
            // 结束测试字形数组。
        ];
        // 返回单段原始布局，最终行信息由 FontService 重建。
        TextLayout {
            // 返回两个 cluster 字形。
            glyphs,
            // 返回占位单行信息。
            lines: vec![LineInfo {
                // 原始段行位于顶部。
                y: 0.0,
                // 使用调用方行高。
                height: opts.line_height,
                // 完整 cluster advance 为十二像素。
                width: 12.0,
                // 源区间从零开始。
                start_char: 0,
                // 源区间覆盖完整文本。
                end_char: char_end,
                // 字形范围从零开始。
                glyph_start: 0,
                // 字形范围包含两个字形。
                glyph_count: 2,
                // 结束占位行构造。
            }],
            // 返回完整 cluster 宽度。
            width: 12.0,
            // 返回稳定字体高度。
            height: opts.font_size,
            // 结束测试布局构造。
        }
        // 结束测试布局实现。
    }
    // 测试不执行光栅化，返回稳定空光栅。
    fn rasterize_glyph(
        // 测试后端不依赖自身状态。
        &self,
        // 测试不读取字体句柄。
        _font: &FontHandle,
        // 测试不读取字形编号。
        _glyph_id: u32,
        // 测试不读取像素字号。
        _pixel_size: f32,
        // 返回空光栅。
    ) -> GlyphRaster {
        // 使用内部稳定空值构造。
        GlyphRaster::empty()
        // 结束测试光栅实现。
    }
    // 返回稳定水平度量以驱动 FontService 行盒。
    fn horizontal_line_metrics(
        // 测试后端不依赖自身状态。
        &self,
        // 测试不区分字体句柄。
        _font: &FontHandle,
        // 使用调用方像素字号。
        pixel_size: f32,
        // 返回可用行度量。
    ) -> Option<LineMetrics> {
        // 构造八成 ascent 与两成 descent。
        Some(LineMetrics {
            // ascent 使用字号八成。
            ascent: pixel_size * 0.8,
            // descent 使用字号两成。
            descent: pixel_size * 0.2,
            // 换行步长使用完整字号。
            new_line_size: pixel_size,
            // 结束测试行度量构造。
        })
        // 结束测试行度量实现。
    }
    // 结束测试后端实现。
}

// 构造使用可控覆盖后端的字体服务。
fn service() -> FontService {
    // 从默认服务复用注册表与缓存初始化。
    let mut service = FontService::new();
    // 替换为不依赖系统字体的覆盖后端。
    service.text_backend = Box::new(CoverageBackend);
    // 显式登记主字体句柄以允许添加回退字体。
    service.loaded_font_handle = FontHandle::new(0);
    // 设置唯一回退字体句柄。
    service.set_fallback_chain(&[FontHandle::new(1)]);
    // 返回已配置服务。
    service
    // 结束测试服务构造。
}

// 验证扩展字素簇只能整体选择一个回退字体。
#[test]
fn fallback_does_not_split_extended_grapheme_cluster() {
    // 创建可控字体覆盖服务。
    let service = service();
    // 基字由主字体覆盖，组合符只由回退字体覆盖。
    let text = "a\u{0301}";
    // 执行真实字体分段逻辑。
    let segments = service.segment_text(&FontHandle::new(0), text);
    // 完整扩展字素簇只能形成一个字体段。
    assert_eq!(segments.len(), 1);
    // 整个 cluster 必须选择能完整覆盖它的回退字体。
    assert_eq!(segments[0].font, FontHandle::new(1));
    // 字体段必须覆盖完整 UTF-8 文本而不是只覆盖基字。
    assert_eq!(
        (segments[0].byte_start, segments[0].byte_end),
        (0, text.len())
    );
    // 结束字素簇回退测试。
}

// 验证超宽 shaping cluster 宁可整簇溢出也不能在内部拆行。
#[test]
fn wrapping_keeps_all_glyphs_of_cluster_on_one_line() {
    // 创建可控字体覆盖服务。
    let service = service();
    // 构造小于完整 cluster 宽度的换行约束。
    let options = TextLayoutOptions {
        // 八像素只能容纳 cluster 的首个六像素字形。
        max_width: 8.0,
        // 高度由文本自身决定。
        max_height: 0.0,
        // 使用稳定行高。
        line_height: 12.0,
        // 显式启用自动换行。
        word_wrap: true,
        // 使用左对齐观察原始行。
        h_align: HAlign::Left,
        // 使用顶部对齐观察原始行。
        v_align: VAlign::Top,
        // 使用稳定测试字号。
        font_size: 10.0,
        // 结束换行选项构造。
    };
    // 执行完整 FontService 字体分段与行拼接。
    let layout = service.layout_text(&FontHandle::new(0), "a\u{0301}", &options);
    // 两个同 cluster 字形必须保留在同一视觉行。
    assert_eq!(layout.lines.len(), 1);
    // 唯一行必须同时引用两个字形。
    assert_eq!(layout.lines[0].glyph_count, 2);
    // 超宽 cluster 可以整簇溢出，但宽度必须保持完整十二像素。
    assert_eq!(layout.lines[0].width, 12.0);
    // 两个字形必须保留相同的全局 cluster 源区间。
    assert!(
        layout
            // 遍历最终定位字形。
            .glyphs
            // 检查完整源区间。
            .iter()
            // 两个字形都覆盖两个源字符。
            .all(|glyph| glyph.char_index == 0 && glyph.char_end == 2)
    );
    // 结束 cluster 原子换行测试。
}
// 结束字体布局测试模块。
