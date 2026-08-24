    // 引入全部被测索引类型。
    use super::*;

    /// 收集便于断言的全部字符边界。
    fn grapheme_boundaries(text: &str) -> Vec<usize> {
        // 建立源文本索引模型。
        let map = TextIndexMap::new(text);
        // 遍历从起点到文本末尾的全部字符位置。
        (0..=map.char_len().0)
            // 只保留合法扩展字素簇边界。
            .filter(|index| map.is_grapheme_boundary(CharIndex(*index)))
            // 固化为测试向量。
            .collect()
    }

    /// 流式选择归一必须与缓存索引模型保持完全相同的边界语义。
    #[test]
    fn streaming_selection_normalization_matches_index_map() {
        // 覆盖空文本、ASCII、组合字符、ZWJ emoji、Indic 与混合双向文本。
        let samples = ["", "steady", "a\u{0301}b", "👩🏽‍💻Z", "क्ष", "Aא\u{05B7}בZ"];
        // 逐段核对全部有效位置、反向选择和两个越界位置。
        for text in samples {
            // 建立重复查询使用的参考模型。
            let map = TextIndexMap::new(text);
            // 把越界行为也纳入对比范围。
            let limit = map.char_len().0 + 2;
            // 穷举选择的两个有向端点。
            for a in 0..=limit {
                for b in 0..=limit {
                    // 流式路径必须逐项等于既有缓存模型。
                    assert_eq!(
                        normalize_selection_in_text(text, CharIndex(a), CharIndex(b)),
                        map.normalize_selection(CharIndex(a), CharIndex(b)),
                        "文本 {text:?} 的选择 {a}..{b} 归一结果不一致"
                    );
                }
            }
        }
    }

    /// 流式单点归一必须与缓存索引模型保持完全相同的边界语义。
    #[test]
    fn streaming_char_normalization_matches_index_map() {
        // 覆盖空文本、ASCII、组合字符、ZWJ emoji、Indic 与混合双向文本。
        let samples = ["", "steady", "a\u{0301}b", "👩🏽‍💻Z", "क्ष", "Aא\u{05B7}בZ"];
        // 三种偏向都必须覆盖全部有效位置和两个越界位置。
        let biases = [
            BoundaryBias::Backward,
            BoundaryBias::Forward,
            BoundaryBias::Nearest,
        ];
        for text in samples {
            // 缓存模型作为现有语义参考。
            let map = TextIndexMap::new(text);
            let limit = map.char_len().0 + 2;
            for index in 0..=limit {
                for bias in biases {
                    assert_eq!(
                        normalize_char_in_text(text, CharIndex(index), bias),
                        map.normalize_char(CharIndex(index), bias),
                        "文本 {text:?} 的位置 {index} 使用 {bias:?} 归一结果不一致"
                    );
                }
            }
        }
    }

    /// 组合音标必须与基础字母形成单一可编辑单元。
    #[test]
    // 验证 combining mark 不会产生内部光标位置。
    fn combining_mark_has_no_internal_cursor_boundary() {
        // 基础拉丁字母与组合锐音包含两个标量。
        let text = "a\u{0301}";
        // 只能在完整字素簇的两端停靠。
        assert_eq!(grapheme_boundaries(text), vec![0, 2]);
        // 建立索引模型以验证移动行为。
        let map = TextIndexMap::new(text);
        // 向右移动必须一次越过整个组合序列。
        assert_eq!(map.next_grapheme_boundary(CharIndex(0)), CharIndex(2));
        // 向左移动必须一次返回整个组合序列之前。
        assert_eq!(map.previous_grapheme_boundary(CharIndex(2)), CharIndex(0));
    }

    /// 肤色修饰符与 ZWJ emoji 必须保持为单一可编辑单元。
    #[test]
    // 验证 emoji 序列不会暴露内部标量边界。
    fn emoji_zwj_and_skin_tone_have_no_internal_cursor_boundary() {
        // 女性、肤色、ZWJ 与电脑组成四标量 emoji。
        let text = "👩🏽‍💻";
        // 只能在整个 emoji 两端停靠。
        assert_eq!(grapheme_boundaries(text), vec![0, 4]);
        // 建立索引模型以验证内部命中归一。
        let map = TextIndexMap::new(text);
        // 靠近前半区的内部位置归一到起点。
        assert_eq!(
            map.normalize_char(CharIndex(1), BoundaryBias::Nearest),
            CharIndex(0)
        );
        // 靠近后半区的内部位置归一到终点。
        assert_eq!(
            map.normalize_char(CharIndex(3), BoundaryBias::Nearest),
            CharIndex(4)
        );
    }

    /// Indic 连写必须保持标准扩展字素簇边界。
    #[test]
    // 验证天城文辅音连写不会被标量索引拆开。
    fn indic_conjunct_has_no_internal_cursor_boundary() {
        // Ka、virama 与 Ssa 组成一个连写字素簇。
        let text = "क्ष";
        // 三个标量共享同一对编辑边界。
        assert_eq!(grapheme_boundaries(text), vec![0, 3]);
        // 建立索引模型以验证选择归一。
        let map = TextIndexMap::new(text);
        // 任意内部选择必须扩展为完整连写。
        assert_eq!(
            // 归一中间标量形成的选择。
            map.normalize_selection(CharIndex(1), CharIndex(2)),
            // 期望完整覆盖三标量连写。
            (CharIndex(0), CharIndex(3))
        );
    }

    /// 混合 RTL 文本仍使用逻辑字符索引表达稳定字素簇边界。
    #[test]
    // 验证希伯来组合音标与周边 LTR 文本之间的边界。
    fn mixed_rtl_text_preserves_logical_grapheme_boundaries() {
        // 拉丁前缀、希伯来组合序列与拉丁后缀形成混合方向文本。
        let text = "Aא\u{05B7}בZ";
        // 希伯来字母与元音点共享一个逻辑字素簇。
        assert_eq!(grapheme_boundaries(text), vec![0, 1, 3, 4, 5]);
        // 建立索引模型以验证 shaping cluster 区间转换。
        let map = TextIndexMap::new(text);
        // 模拟后端只覆盖组合序列内部标量的错误簇范围。
        let cluster = ShapingCluster {
            // cluster 从希伯来基字符后的内部位置开始。
            start: CharIndex(2),
            // cluster 在组合序列后结束。
            end: CharIndex(3),
        };
        // shaping cluster 必须扩展回完整希伯来字素簇。
        assert_eq!(
            // 归一后端簇范围。
            map.normalize_shaping_cluster(cluster),
            // 期望覆盖基字符与组合点。
            ShapingCluster {
                // 合法字素簇起点。
                start: CharIndex(1),
                // 合法字素簇终点。
                end: CharIndex(3),
            }
        );
    }

    /// UTF-8 字节、字符和字素簇转换必须保持各自单位。
    #[test]
    // 验证多字节字符内部的向前与向后字符边界转换。
    fn byte_char_and_grapheme_units_convert_explicitly() {
        // ASCII、二字节字符与 emoji 共同覆盖不同 UTF-8 宽度。
        let text = "Aé👩🏽‍💻";
        // 建立显式索引模型。
        let map = TextIndexMap::new(text);
        // 三个字素簇分别覆盖一、一和四个字符。
        assert_eq!(map.grapheme_len(), GraphemeIndex(3));
        // 第二个字符边界对应三个 UTF-8 字节。
        assert_eq!(map.char_to_byte(CharIndex(2)), ByteIndex(3));
        // 落在 é 内部的字节向后收敛到字符一。
        assert_eq!(map.byte_to_char_floor(ByteIndex(2)), CharIndex(1));
        // 落在 é 内部的字节向前收敛到字符二。
        assert_eq!(map.byte_to_char_ceil(ByteIndex(2)), CharIndex(2));
        // 第三个字素簇从逻辑字符二开始。
        assert_eq!(map.grapheme_to_char(GraphemeIndex(2)), CharIndex(2));
    }
