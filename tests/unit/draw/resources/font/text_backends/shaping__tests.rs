    // 引入被测试的 shaping 入口。
    use super::layout_text;
    // 引入字体句柄与布局选项。
    use crate::draw::{FontHandle, HAlign, VAlign};
    // 引入内部文本布局约束类型。
    use crate::draw::resources::font::text_backend::TextLayoutOptions;

    // 构造稳定的单行 shaping 约束。
    fn options() -> TextLayoutOptions {
        // 返回不触发自动换行的标准字号选项。
        TextLayoutOptions {
            // 使用足够大的有限宽度方便验证 advance。
            max_width: 4096.0,
            // 高度由文本自身决定。
            max_height: 0.0,
            // 使用与字号成比例的稳定行高。
            line_height: 30.0,
            // 本测试聚焦 shaping，不触发换行。
            word_wrap: false,
            // 使用左对齐观察原始视觉顺序。
            h_align: HAlign::Left,
            // 使用顶部对齐观察原始基线。
            v_align: VAlign::Top,
            // 使用足够大的字号避免浮点精度噪声。
            font_size: 20.0,
            // 结束布局选项构造。
        }
        // 结束测试选项辅助函数。
    }

    // 从标准 Windows 字体目录读取指定字体。
    #[cfg(target_os = "windows")]
    fn windows_font(name: &str) -> Vec<u8> {
        // 拼接系统字体目录中的稳定文件名。
        let path = std::path::Path::new(r"C:\Windows\Fonts").join(name);
        // 缺少目标系统字体时让测试明确失败，而不是静默跳过覆盖。
        std::fs::read(&path).unwrap_or_else(|error| {
            // 输出精确字体路径与读取错误便于诊断环境差异。
            panic!("无法读取复杂脚本测试字体 {}：{error}", path.display())
            // 结束读取失败处理。
        })
        // 结束系统字体读取辅助函数。
    }

    // 验证阿拉伯连写会产生 RTL cluster 与 ligature 区间。
    #[cfg(target_os = "windows")]
    #[test]
    fn arabic_shaping_uses_visual_clusters() {
        // Segoe UI 提供稳定的阿拉伯 GSUB/GPOS 表。
        let font = windows_font("segoeui.ttf");
        // 使用含 lam-alef 连写的阿拉伯单词。
        let text = "سلام";
        // 执行真实 OpenType shaping。
        let layout = layout_text(
            // 传入完整字体文件。
            &font,
            // 使用字体集合的首个字体面。
            0,
            // 使用独立测试句柄。
            FontHandle::new(7),
            // 传入阿拉伯文本。
            text,
            // 传入标准测试约束。
            &options(),
            // 测试字体按 20px/UPEM 的稳定比例缩放。
            20.0 / 2048.0,
            // 使用稳定 ascent。
            16.0,
            // 使用稳定字体行盒。
            24.0,
            // 使用稳定调用方行高。
            30.0,
            // 独立 shaping 测试保留自动方向推断。
            None,
            // 字体解析或 shaping 失败应直接暴露。
        )
        // 说明本测试要求完整 shaping 路径。
        .expect("Segoe UI 阿拉伯 shaping 应成功");
        // 阿拉伯文本必须产生可绘制字形。
        assert!(!layout.glyphs.is_empty());
        // 每个字形都必须保留合法非空源 cluster。
        assert!(
            layout
                // 遍历全部定位字形。
                .glyphs
                // 检查每个源区间。
                .iter()
                // 要求排他终点严格大于起点。
                .all(|glyph| glyph.char_end > glyph.char_index)
        );
        // RTL 视觉输出应包含递减 cluster 或覆盖多个字符的连写。
        assert!(
            layout
            // 检查相邻字形的视觉 cluster 顺序。
            .glyphs
            // 形成相邻窗口。
            .windows(2)
            // 接受 RTL 递减顺序。
            .any(|pair| pair[0].char_index > pair[1].char_index)
            // 或接受单个 ligature 覆盖多个源字符。
            || layout
                // 再次遍历字形。
                .glyphs
                // 检查 cluster 跨度。
                .iter()
                // lam-alef 等连写应覆盖多个字符。
                .any(|glyph| glyph.char_end - glyph.char_index > 1)
        );
        // RTL 视觉顺序不能改变行信息中的逻辑源范围。
        assert_eq!(
            // 读取唯一视觉行的逻辑起止索引。
            (layout.lines[0].start_char, layout.lines[0].end_char),
            // 原始阿拉伯文本包含四个逻辑字符。
            (0, text.chars().count()),
            // 结束逻辑源范围断言。
        );
        // 结束阿拉伯 shaping 测试。
    }

    // 验证 Indic 重排与组合符保留 cluster 源区间。
    #[cfg(target_os = "windows")]
    #[test]
    fn indic_and_combining_marks_preserve_clusters() {
        // Leelawadee UI 提供 Malayalam OpenType shaping 表。
        let indic_font = windows_font("LeelawUI.ttf");
        // Malayalam ka + virama + ssa 形成一个复杂辅音簇。
        let indic_text = "ക്ഷ";
        // 执行 Indic shaping。
        let indic = layout_text(
            // 传入 Malayalam 字体。
            &indic_font,
            // 使用字体首个面。
            0,
            // 使用独立测试句柄。
            FontHandle::new(8),
            // 传入 Indic 文本。
            indic_text,
            // 传入标准测试约束。
            &options(),
            // 测试字体按 20px/UPEM 的稳定比例缩放。
            20.0 / 2048.0,
            // 使用稳定 ascent。
            16.0,
            // 使用稳定字体高度。
            24.0,
            // 使用稳定行高。
            30.0,
            // Indic 独立测试保留自动方向推断。
            None,
            // Indic shaping 必须成功。
        )
        // 失败时明确指出字体与脚本契约。
        .expect("Leelawadee UI Malayalam shaping 应成功");
        // Indic 文本必须产生字形。
        assert!(!indic.glyphs.is_empty());
        // 至少一个输出 cluster 应覆盖整个组合序列的一部分以上。
        assert!(
            indic
                // 遍历 Indic 字形。
                .glyphs
                // 检查源 cluster 跨度。
                .iter()
                // 复杂辅音簇必须出现多字符 cluster。
                .any(|glyph| glyph.char_end - glyph.char_index > 1)
        );
        // Segoe UI 提供 Latin 基字与组合音标定位。
        let latin_font = windows_font("segoeui.ttf");
        // 使用分解形式而不是预组合字符。
        let combining_text = "a\u{0301}";
        // 执行组合音标 shaping。
        let combining = layout_text(
            // 传入 Latin 字体。
            &latin_font,
            // 使用字体首个面。
            0,
            // 使用独立测试句柄。
            FontHandle::new(9),
            // 传入分解文本。
            combining_text,
            // 传入标准测试约束。
            &options(),
            // 测试字体按 20px/UPEM 的稳定比例缩放。
            20.0 / 2048.0,
            // 使用稳定 ascent。
            16.0,
            // 使用稳定字体高度。
            24.0,
            // 使用稳定行高。
            30.0,
            // 组合文本独立测试保留自动方向推断。
            None,
            // 组合音标 shaping 必须成功。
        )
        // 失败时明确指出组合文本契约。
        .expect("Segoe UI 组合音标 shaping 应成功");
        // 至少一个字形必须覆盖基字与组合符的同一 cluster。
        assert!(
            combining
                // 遍历组合文本字形。
                .glyphs
                // 检查源 cluster 跨度。
                .iter()
                // 两个标量必须属于同一 shaping cluster。
                .any(|glyph| glyph.char_index == 0 && glyph.char_end == 2)
        );
        // 结束 Indic 与组合符测试。
    }

    // 验证默认 Latin ligature 由 GSUB 合并并保留完整源区间。
    #[cfg(target_os = "windows")]
    #[test]
    fn latin_ligature_keeps_source_span() {
        // Calibri 提供默认启用的标准 fi/ffi ligature。
        let font = windows_font("calibri.ttf");
        // 使用常见 ffi 连写输入。
        let text = "ffi";
        // 执行默认 OpenType 特性 shaping。
        let layout = layout_text(
            // 传入 Latin 字体。
            &font,
            // 使用字体首个面。
            0,
            // 使用独立测试句柄。
            FontHandle::new(10),
            // 传入 ligature 文本。
            text,
            // 传入标准测试约束。
            &options(),
            // 测试字体按 20px/UPEM 的稳定比例缩放。
            20.0 / 2048.0,
            // 使用稳定 ascent。
            16.0,
            // 使用稳定字体高度。
            24.0,
            // 使用稳定行高。
            30.0,
            // Latin 独立测试保留自动方向推断。
            None,
            // Latin shaping 必须成功。
        )
        // 失败时明确指出 ligature 契约。
        .expect("Calibri ligature shaping 应成功");
        // 至少一个输出字形必须覆盖多个源字符。
        assert!(
            layout
                // 遍历 Latin 定位字形。
                .glyphs
                // 检查 GSUB 合并后的源区间。
                .iter()
                // fi 或 ffi ligature 必须跨越至少两个标量。
                .any(|glyph| glyph.char_end - glyph.char_index > 1)
        );
        // 结束 Latin ligature 测试。
    }
    // 结束 shaping 测试模块。
