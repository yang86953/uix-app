//! RichText 同字符样式 delimiter 扫描辅助。

// 查找单字符斜体的闭合位置，同时跳过内部成对的同字符粗体标记。
pub(super) fn find_single_closing_marker(
    // 接收从外层开标记开始的完整文本切片。
    text: &str,
    // 接收外层开标记之后的首个搜索位置。
    start: usize,
    // 接收单星号或单下划线标记。
    marker: &str,
) -> Option<usize> {
    // 将受支持的单字符标记映射为同字符粗体标记。
    let double_marker = match marker {
        // 星号斜体内部使用双星号粗体。
        "*" => "**",
        // 下划线斜体内部使用双下划线粗体。
        "_" => "__",
        // 其他标记不属于本辅助的窄契约。
        _ => return None,
    };
    // 读取单字节 ASCII delimiter，供连续 run 扫描使用。
    let marker_byte = marker.as_bytes()[0];
    // 从外层正文起点开始寻找候选 delimiter run。
    let mut search_from = start;
    // 记录扫描位置是否处于一个已开启的同字符粗体区间。
    let mut inside_double = false;
    // 记录未闭合内层粗体之后仍可闭合外层斜体的单字符候选。
    let mut fallback_close = None;
    // 持续扫描剩余文本中的同字符 delimiter。
    while let Some(relative) = text[search_from..].find(marker) {
        // 计算当前 delimiter run 的绝对起点。
        let candidate = search_from + relative;
        // 一次读取完整连续 run，避免把双标记拆成两个外层闭合候选。
        let run_end = marker_run_end(text, candidate, marker_byte);
        // 计算当前连续 run 的标记数量。
        let run_len = run_end - candidate;
        // 被奇数反斜杠转义的 run 不参与任何样式闭合。
        if super::is_escaped_at(text, candidate) {
            // 跳过整个已转义 run，继续寻找真实 delimiter。
            search_from = run_end;
            // 当前候选保持字面值。
            continue;
        }
        // 已进入内层粗体时只寻找合法的双字符闭合。
        if inside_double {
            // 至少两个同字符标记且满足闭合边界时结束内层粗体。
            if run_len >= double_marker.len() && can_close_double(text, candidate, run_end, marker)
            {
                // 三字符 run 的前两个标记闭合粗体，最后一个闭合外层斜体。
                if run_len == double_marker.len() + marker.len() {
                    // 返回共享 run 中最后一个单字符标记的位置。
                    return Some(candidate + double_marker.len());
                }
                // 分离的双字符闭合只退出内层粗体扫描状态。
                inside_double = false;
                // 内层已经正常闭合，不再需要未闭合回退候选。
                fallback_close = None;
            } else if run_len == marker.len() && super::can_close_marker(text, candidate, marker) {
                // 暂存单字符闭合；仅在内层粗体最终未闭合时使用。
                fallback_close = Some(candidate);
            }
            // 内层区间尚未产生外层闭合，跳过当前完整 run。
            search_from = run_end;
            // 继续扫描外层真正的闭合标记。
            continue;
        }
        // 恰好两个标记且满足开启边界时进入同字符粗体区间。
        if run_len == double_marker.len() && can_open_double(text, candidate, run_end, marker) {
            // 后续单字符候选必须先越过对应粗体闭合。
            inside_double = true;
            // 跳过内层粗体开标记。
            search_from = run_end;
            // 继续寻找内层闭合标记。
            continue;
        }
        // 非内层标记 run 使用末尾字符作为外层闭合候选。
        let close = run_end - marker.len();
        // 复用现有空白与下划线单词边界校验。
        if close > start && super::can_close_marker(text, close, marker) {
            // 返回第一个满足外层边界的闭合位置。
            return Some(close);
        }
        // 当前 run 不能闭合外层，继续扫描后续文本。
        search_from = run_end;
    }
    // 未闭合的内层双标记按字面保留时，允许最后的单标记闭合外层。
    if inside_double {
        // 返回扫描期间记录的合法外层回退闭合位置。
        return fallback_close;
    }
    // 没有找到合法外层闭合标记。
    None
}

// 返回从指定位置开始的连续同字符 delimiter 末尾。
fn marker_run_end(text: &str, start: usize, marker_byte: u8) -> usize {
    // 从候选起点开始向后扫描 ASCII 单字节标记。
    let mut end = start;
    // 连续相同字节仍属于当前 delimiter run。
    while text.as_bytes().get(end) == Some(&marker_byte) {
        // 将 run 末尾向后推进一个字节。
        end += 1;
    }
    // 返回当前连续 run 的独占结束位置。
    end
}

// 判断双字符 run 是否可以开启内层同字符粗体。
fn can_open_double(text: &str, candidate: usize, run_end: usize, marker: &str) -> bool {
    // 读取完整双字符 run 后的正文起点。
    let after = text[run_end..].chars().next();
    // 空正文或空白正文不能开启内层粗体。
    if after.is_none_or(char::is_whitespace) {
        // 保留不满足开启边界的双字符 run。
        return false;
    }
    // 星号粗体不需要额外的单词内边界判断。
    if marker == "*" {
        // 当前双星号满足开启条件。
        return true;
    }
    // 读取双下划线前的相邻字符。
    let before = text[..candidate].chars().next_back();
    // 单词内部双下划线不能开启粗体。
    !(before.is_some_and(super::is_word_char) && after.is_some_and(super::is_word_char))
}

// 判断连续 run 的前两个字符是否可以闭合内层同字符粗体。
fn can_close_double(text: &str, candidate: usize, run_end: usize, marker: &str) -> bool {
    // 读取双字符闭合前的相邻正文字符。
    let before = text[..candidate].chars().next_back();
    // 空正文或尾随空白不能闭合内层粗体。
    if before.is_none_or(char::is_whitespace) {
        // 保留不满足闭合边界的双字符 run。
        return false;
    }
    // 星号粗体不需要额外的单词内边界判断。
    if marker == "*" {
        // 当前双星号满足闭合条件。
        return true;
    }
    // 共享三字符 run 时，边界字符位于整个 run 之后。
    let after_index = if run_end - candidate > 2 {
        // 跳过共享 run，避免把外层下划线误算为单词字符。
        run_end
    } else {
        // 分离闭合只跳过内层双下划线。
        candidate + 2
    };
    // 读取双下划线闭合后的相邻字符。
    let after = text[after_index..].chars().next();
    // 单词内部双下划线不能闭合粗体。
    !(before.is_some_and(super::is_word_char) && after.is_some_and(super::is_word_char))
}
