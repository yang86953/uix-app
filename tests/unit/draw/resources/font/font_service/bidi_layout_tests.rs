//! 验证 FontService 的 UAX #9 视觉顺序与方向感知交互几何。

// 引入可控等宽测试字体服务与布局选项。
use super::line_break_tests::{options, service};
// 引入共享选择片段几何辅助。
use crate::draw::resources::font::text_backend::{
    glyph_selection_x_ranges, visit_glyph_selection_x_ranges,
};
// 引入命中测试坐标。
use crate::core::Point;
// 引入测试字体句柄。
use crate::draw::FontHandle;
// 引入共享字素簇边界模型以核对交互结果。
use crate::draw::resources::font::text_index::{CharIndex, TextIndexMap};

// 验证 LTR 段落中的 Hebrew 与数字按 UAX #9 视觉顺序排列。
#[test]
fn mixed_runs_preserve_logical_indices_and_number_order() {
    // 使用可控单字符六像素 advance 的字体服务。
    let service = service();
    // 构造 Latin、Hebrew、中性空格与欧洲数字混排。
    let text = "abc אבג 123";
    // 执行完整字体分段、逻辑换行与视觉 run 重排。
    let layout = service.layout_text(&FontHandle::new(0), text, &options(4096.0));
    // 提取最终视觉字形保存的逻辑源索引。
    let visual_indices = layout
        // 遍历视觉顺序字形。
        .glyphs
        // 借用以保留布局供后续几何断言。
        .iter()
        // 读取稳定逻辑字符起点。
        .map(|glyph| glyph.char_index)
        // 收集视觉到逻辑索引映射。
        .collect::<Vec<_>>();
    // 数字保持 LTR，Hebrew 字母按视觉顺序反向。
    assert_eq!(visual_indices, vec![0, 1, 2, 3, 8, 9, 10, 7, 6, 5, 4]);
    // 全部视觉字形 x 坐标必须保持单调，供绘制与命中共享。
    assert!(
        layout
            // 遍历相邻视觉字形窗口。
            .glyphs
            // 取得连续二元窗口。
            .windows(2)
            // 每个后继字形都不得位于前驱左侧。
            .all(|pair| pair[0].x <= pair[1].x)
    );
    // Hebrew 字形必须保存奇数嵌入级别。
    assert!(
        layout
            // 遍历 Hebrew 逻辑源索引对应字形。
            .glyphs
            // 只保留三个 Hebrew 字符。
            .iter()
            // 判断逻辑源范围。
            .filter(|glyph| (4..7).contains(&glyph.char_index))
            // 全部 Hebrew 字形必须为 RTL 奇数级。
            .all(|glyph| glyph.bidi_level % 2 == 1)
    );
    // 欧洲数字必须保留偶数嵌入级别和内部 LTR 顺序。
    assert!(
        layout
            // 遍历数字逻辑源索引对应字形。
            .glyphs
            // 只保留三个数字。
            .iter()
            // 判断逻辑源范围。
            .filter(|glyph| (8..11).contains(&glyph.char_index))
            // 全部数字字形必须为偶数级。
            .all(|glyph| glyph.bidi_level % 2 == 0)
    );
}

// 验证 RTL cluster 的左右半区命中返回相反逻辑边界。
#[test]
fn rtl_hit_testing_and_cursor_use_visual_cluster_direction() {
    // 使用可控等宽字体服务。
    let service = service();
    // 构造包含 RTL run 的稳定单行混排。
    let text = "abc אבג 123";
    // 使用足够宽约束避免自动换行。
    let options = options(4096.0);
    // RTL 字符 ג 位于视觉区间 48..54，左半区对应逻辑排他终点七。
    assert_eq!(
        service.hit_test_text(
            // 使用测试字体。
            &FontHandle::new(0),
            // 使用混排源文本。
            text,
            // 复用同一布局选项。
            &options,
            // 命中 RTL cluster 左半区。
            Point::new(49.0, 1.0),
        ),
        // RTL 左缘必须返回逻辑排他终点。
        Some(7),
    );
    // 同一 RTL cluster 右半区对应逻辑起点六。
    assert_eq!(
        service.hit_test_text(
            // 使用测试字体。
            &FontHandle::new(0),
            // 使用同一混排源文本。
            text,
            // 复用同一布局选项。
            &options,
            // 命中 RTL cluster 右半区。
            Point::new(53.0, 1.0),
        ),
        // RTL 右缘必须返回逻辑起点。
        Some(6),
    );
    // RTL 逻辑边界六位于字符 ג 的视觉右缘五十四像素。
    assert_eq!(
        service.text_cursor_x(&FontHandle::new(0), text, &options, 6),
        54.0,
    );
}

// 验证跨双向 run 的逻辑选择形成多个连续视觉片段。
#[test]
fn mixed_selection_returns_disjoint_visual_ranges() {
    // 使用可控等宽字体服务。
    let service = service();
    // 构造包含分离视觉区域的逻辑连续选择源文本。
    let text = "abc אבג 123";
    // 执行完整混排布局。
    let layout = service.layout_text(&FontHandle::new(0), text, &options(4096.0));
    // 选择逻辑区间 5..9，覆盖 Hebrew 尾部、空格与首个数字。
    let ranges = glyph_selection_x_ranges(&layout.glyphs, 5, 9);
    // 首个数字与 RTL 片段之间存在未选数字形成的视觉间隔。
    assert_eq!(ranges, vec![(24.0, 30.0), (42.0, 60.0)]);
    // 流式热路径必须产生与拥有型兼容接口相同的视觉片段。
    let mut streamed = Vec::new();
    // 立即收集仅用于验证回调顺序和边界。
    visit_glyph_selection_x_ranges(&layout.glyphs, 5, 9, |left, right| {
        streamed.push((left, right));
    });
    // 双向分离片段不得因流式消费而被合并或重排。
    assert_eq!(streamed, ranges);
}

// 验证混合 RTL 组合序列的命中与光标查询不会暴露内部标量位置。
#[test]
fn mixed_rtl_grapheme_hit_and_cursor_stay_on_grapheme_boundaries() {
    // 使用可控等宽测试字体服务。
    let service = service();
    // 希伯来字母与元音点组成一个 RTL 扩展字素簇。
    let text = "Aא\u{05B7}בZ";
    // 使用足够宽约束保持单行混排。
    let options = options(4096.0);
    // 执行完整 shaping 与 UAX #9 视觉重排。
    let layout = service.layout_text(&FontHandle::new(0), text, &options);
    // 建立合法扩展字素簇边界表。
    let index_map = TextIndexMap::new(text);
    // 覆盖文本左右外侧和全部视觉像素位置。
    for sample in -4..=(layout.width.ceil() as i32 + 4) {
        // 查询当前视觉位置对应的逻辑字符边界。
        let hit = service
            // 执行方向感知命中。
            .hit_test_text(
                // 使用测试字体句柄。
                &FontHandle::new(0),
                // 使用混合方向组合文本。
                text,
                // 复用单行布局选项。
                &options,
                // 采样当前水平像素中心。
                Point::new(sample as f32 + 0.5, 1.0),
            )
            // 非空文本命中必须产生字符位置。
            .expect("混合方向非空文本应返回命中边界");
        // 任意视觉命中都不得返回希伯来组合序列内部位置二。
        assert!(index_map.is_grapheme_boundary(CharIndex(hit)));
    }
    // 查询旧调用方传入的组合序列内部字符位置。
    let internal_x = service.text_cursor_x(&FontHandle::new(0), text, &options, 2);
    // 查询该位置按最近规则归一后的合法字素簇终点。
    let boundary_x = service.text_cursor_x(&FontHandle::new(0), text, &options, 3);
    // 内部位置必须与合法边界共享同一主光标几何。
    assert_eq!(internal_x, boundary_x);
}
