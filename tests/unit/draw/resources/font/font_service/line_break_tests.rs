//! 使用等宽纯内存后端验证 FontService 的 UAX #14 行拼接。

// 引入被测字体服务。
use super::FontService;
// 引入测试后端 trait 所需错误类型。
use crate::core::Error;
// 引入字体布局与光栅数据类型。
use crate::draw::resources::font::text_backend::{
    // 引入空光栅返回类型。
    GlyphRaster,
    // 引入占位行信息。
    LineInfo,
    // 引入稳定水平字体度量。
    LineMetrics,
    // 引入逐字符定位字形。
    PositionedGlyph,
    // 引入后端布局结果。
    TextLayout,
    // 引入布局约束。
    TextLayoutOptions,
    // 结束布局类型导入。
};
// 引入字体句柄、对齐枚举与文本后端 trait。
use crate::draw::{FontHandle, HAlign, TextBackend, VAlign};

// 使用固定六像素 advance 的纯内存后端。
#[derive(Debug)]
pub(super) struct MonospaceBackend;

// 实现最小文本后端以隔离系统字体差异。
impl TextBackend for MonospaceBackend {
    // 测试不读取字体数据并返回稳定句柄。
    fn load_font(&mut self, _data: &[u8]) -> Result<FontHandle, Error> {
        // 返回唯一有效句柄。
        Ok(FontHandle::new(0))
    }

    // 纯内存后端没有需要释放的字体资源。
    fn unload_font(&mut self, _handle: &FontHandle) {}

    // 仅句柄零代表有效测试字体。
    fn is_valid(&self, handle: &FontHandle) -> bool {
        // 精确检查稳定句柄。
        handle.0 == 0
    }

    // 测试字体覆盖全部 Unicode 标量。
    fn has_glyph(&self, _font: &FontHandle, _ch: char) -> bool {
        // 避免字体回退干扰断行测试。
        true
    }

    // 将每个 Unicode 标量映射为一个固定宽度字形。
    fn layout_text(
        // 测试后端不依赖自身状态。
        &self,
        // 保留调用方字体句柄。
        font: &FontHandle,
        // 接收当前不含强制换行的字体段。
        text: &str,
        // 复用调用方字号与行高。
        options: &TextLayoutOptions,
        // 返回可由 FontService 二次拼接的单段布局。
    ) -> TextLayout {
        // 为每个字符构造独立 cluster 字形。
        let glyphs = text
            // 枚举逻辑字符索引。
            .chars()
            // 保留源字符索引。
            .enumerate()
            // 构造稳定定位字形。
            .map(|(char_index, _)| PositionedGlyph {
                // 每个字符在前一字符后推进六像素。
                x: char_index as f32 * 6.0,
                // 使用稳定测试基线。
                y: 8.0,
                // 使用固定六像素 advance。
                width: 6.0,
                // 使用调用方字号作为字形行盒高度。
                height: options.font_size,
                // 使用非零稳定字形编号。
                glyph_id: char_index as u32 + 1,
                // 每个字形覆盖一个源字符。
                char_index,
                // 排他终点紧随源字符起点。
                char_end: char_index + 1,
                // 测试后端默认使用 LTR，FontService 会回填行级双向级别。
                bidi_level: 0,
                // 保留调用方字体句柄。
                font: *font,
                // 结束定位字形构造。
            })
            // 固化字形列表。
            .collect::<Vec<_>>();
        // 计算单段完整宽度。
        let width = glyphs.len() as f32 * 6.0;
        // 返回占位单行，最终行信息由 FontService 重建。
        TextLayout {
            // 返回逐字符定位字形。
            glyphs,
            // 返回稳定占位行。
            lines: vec![LineInfo {
                // 原始段位于顶部。
                y: 0.0,
                // 使用调用方行高。
                height: options.line_height,
                // 保存完整段宽度。
                width,
                // 源区间从零开始。
                start_char: 0,
                // 源区间覆盖当前段全部字符。
                end_char: text.chars().count(),
                // 字形范围从零开始。
                glyph_start: 0,
                // 占位行包含全部字形。
                glyph_count: text.chars().count(),
                // 结束占位行构造。
            }],
            // 返回完整段宽度。
            width,
            // 返回稳定字体高度。
            height: options.font_size,
            // 结束布局构造。
        }
    }

    // 测试不执行真实光栅化。
    fn rasterize_glyph(
        // 测试后端不依赖自身状态。
        &self,
        // 测试不读取字体句柄。
        _font: &FontHandle,
        // 测试不读取字形编号。
        _glyph_id: u32,
        // 测试不读取像素字号。
        _pixel_size: f32,
        // 返回稳定空光栅。
    ) -> GlyphRaster {
        // 使用共享空光栅构造。
        GlyphRaster::empty()
    }

    // 返回稳定水平行度量。
    fn horizontal_line_metrics(
        // 测试后端不依赖自身状态。
        &self,
        // 测试不区分字体句柄。
        _font: &FontHandle,
        // 使用调用方像素字号。
        pixel_size: f32,
        // 返回可用字体度量。
    ) -> Option<LineMetrics> {
        // 构造稳定 ascent、descent 与换行步长。
        Some(LineMetrics {
            // ascent 使用字号八成。
            ascent: pixel_size * 0.8,
            // descent 使用字号两成。
            descent: pixel_size * 0.2,
            // 换行步长使用完整字号。
            new_line_size: pixel_size,
            // 结束行度量构造。
        })
    }
}

// 构造使用纯内存等宽后端的字体服务。
pub(super) fn service() -> FontService {
    // 从默认服务复用注册表和缓存初始化。
    let mut service = FontService::new();
    // 替换为不依赖系统字体的等宽后端。
    service.text_backend = Box::new(MonospaceBackend);
    // 设置唯一有效字体句柄。
    service.loaded_font_handle = FontHandle::new(0);
    // 返回测试字体服务。
    service
}

// 构造启用自动换行的稳定布局选项。
pub(super) fn options(max_width: f32) -> TextLayoutOptions {
    // 返回固定字号、行高与左上对齐约束。
    TextLayoutOptions {
        // 使用调用方宽度。
        max_width,
        // 高度由文本自身决定。
        max_height: 0.0,
        // 使用十二像素稳定行高。
        line_height: 12.0,
        // 显式启用自动换行。
        word_wrap: true,
        // 使用左对齐观察原始行范围。
        h_align: HAlign::Left,
        // 使用顶部对齐观察原始行位置。
        v_align: VAlign::Top,
        // 使用十像素稳定字号。
        font_size: 10.0,
        // 结束选项构造。
    }
}

// 验证真实 FontService 行拼接遵守 CJK 标点禁则。
#[test]
fn cjk_close_punctuation_stays_on_previous_line() {
    // 执行六像素窄宽度布局。
    let layout = service().layout_text(&FontHandle::new(0), "天，地", &options(6.0));
    // 闭标点必须与前一汉字共同留在溢出的首行。
    assert_eq!(layout.lines.len(), 2);
    // 首行逻辑范围必须覆盖汉字和逗号。
    assert_eq!(
        (layout.lines[0].start_char, layout.lines[0].end_char),
        (0, 2)
    );
    // 第二行只包含最后一个汉字。
    assert_eq!(
        (layout.lines[1].start_char, layout.lines[1].end_char),
        (2, 3)
    );
}

// 验证不可断序列与超长单词使用不同兜底策略。
#[test]
fn non_breaking_sequences_overflow_but_long_words_emergency_wrap() {
    // NBSP 序列在窄宽度下必须保持单行溢出。
    let nbsp = service().layout_text(&FontHandle::new(0), "a\u{00a0}b", &options(6.0));
    // 非断空格两侧不得拆行。
    assert_eq!(nbsp.lines.len(), 1);
    // ZWJ emoji 序列同样必须保持原子单行。
    let emoji = service().layout_text(&FontHandle::new(0), "👩‍👩‍👧‍👦", &options(6.0));
    // emoji 内部不得紧急折行。
    assert_eq!(emoji.lines.len(), 1);
    // 普通超长字母词允许逐 cluster 紧急折行。
    let word = service().layout_text(&FontHandle::new(0), "abc", &options(6.0));
    // 三个字母应各占一行。
    assert_eq!(word.lines.len(), 3);
}

// 验证 CRLF 形成单个强制换行并保留逻辑源范围。
#[test]
fn crlf_is_one_mandatory_break_with_correct_ranges() {
    // 宽约束足够大，行数只能由 CRLF 决定。
    let layout = service().layout_text(&FontHandle::new(0), "a\r\nb", &options(100.0));
    // CRLF 只形成两个正文行。
    assert_eq!(layout.lines.len(), 2);
    // 首行覆盖 CRLF 前的字符。
    assert_eq!(
        (layout.lines[0].start_char, layout.lines[0].end_char),
        (0, 1)
    );
    // 第二行从完整 CRLF 之后开始。
    assert_eq!(
        (layout.lines[1].start_char, layout.lines[1].end_char),
        (3, 4)
    );
}

// 验证 FontService 只拉伸自动换行产生的段落非末行。
#[test]
fn text_align_justify_skips_paragraph_final_and_mandatory_break_lines() {
    // 使用二十四像素容器启用两端对齐。
    let mut justify_options = options(24.0);
    // 选择中性两端对齐模式。
    justify_options.h_align = HAlign::Justify;
    // 自动换行应产生一个可拉伸首行和自然末行。
    let wrapped = service().layout_text(&FontHandle::new(0), "a b c", &justify_options);
    // 首行必须精确占满容器。
    assert_eq!(wrapped.lines[0].width, 24.0);
    // 末行只有一个字符并保持六像素自然宽度。
    assert_eq!(wrapped.lines[1].width, 6.0);
    // 首行内部空白 advance 必须包含分配后的剩余宽度。
    assert_eq!(wrapped.glyphs[1].width, 12.0);
    // 显式换行前的段落末行不得拉伸。
    let mandatory = service().layout_text(&FontHandle::new(0), "a b\nc", &justify_options);
    // 显式换行前一行保持十八像素自然宽度。
    assert_eq!(mandatory.lines[0].width, 18.0);
}

// 验证左、居中与右对齐使用同一有限容器计算稳定偏移。
#[test]
fn text_align_left_center_right_use_expected_offsets() {
    // 逐个验证闭合非拉伸对齐值及其首字形横坐标。
    for (alignment, expected_x) in [
        // 左对齐保持自然起点。
        (HAlign::Left, 0.0),
        // 三十像素容器中的十二像素文本居中偏移九像素。
        (HAlign::Center, 9.0),
        // 右对齐把十八像素剩余宽度放在文本左侧。
        (HAlign::Right, 18.0),
    ] {
        // 使用三十像素稳定容器。
        let mut align_options = options(30.0);
        // 应用当前待验证对齐值。
        align_options.h_align = alignment;
        // 两字符文本保持十二像素自然宽度。
        let layout = service().layout_text(&FontHandle::new(0), "ab", &align_options);
        // 首字形必须位于对应对齐偏移。
        assert_eq!(layout.glyphs[0].x, expected_x);
    }
}
