// 富文本行首围栏代码边界解析辅助。

// 查找一个完整的行首围栏代码块，返回开标记、正文和闭标记的字节位置。
pub(super) fn find_fenced_block(text: &str) -> Option<(usize, usize, usize)> {
    // 只接受行首的三个反引号作为围栏开标记。
    let open = find_line_start_fence(text, 0, false)?;
    // 围栏开标记所在行的剩余内容作为可选语言标注。
    let after_open = open + 3;
    // 正文必须从开标记后的下一行开始。
    let first_newline = text[after_open..].find('\n')?;
    // 跳过语言标注行和换行符，得到代码正文起点。
    let code_start = after_open + first_newline + 1;
    // 只接受后续行首且行尾仅有空白的三个反引号作为闭标记。
    let close = find_line_start_fence(text, code_start, true)?;
    // 返回三个稳定的切片边界，调用方负责生成 Code 段。
    Some((open, code_start, close))
}

// 从指定字节位置开始查找位于行首的三个反引号。
fn find_line_start_fence(text: &str, from: usize, closing: bool) -> Option<usize> {
    // 从调用方给出的合法 UTF-8 边界开始扫描候选标记。
    let mut search_from = from;
    // 允许跳过行内反引号，继续寻找下一个行首围栏。
    while let Some(relative) = text[search_from..].find("```") {
        // 计算候选反引号的绝对字节位置。
        let candidate = search_from + relative;
        // 文本起点或换行后的标记才属于块级围栏。
        if candidate == 0 || text.as_bytes().get(candidate - 1) == Some(&b'\n') {
            // 开围栏允许语言标注，闭围栏只允许三反引号后的空白。
            if !closing || has_only_fence_trailing_whitespace(text, candidate + 3) {
                // 返回当前行首围栏的位置。
                return Some(candidate);
            }
        }
        // 行内反引号不参与围栏配对，继续向后搜索。
        search_from = candidate + 3;
    }
    // 没有找到符合行首边界的围栏。
    None
}

// 判断闭合围栏的同一行是否只剩空白字符。
fn has_only_fence_trailing_whitespace(text: &str, after_fence: usize) -> bool {
    // 只检查当前行，下一行内容不参与闭合标记判断。
    text[after_fence..]
        .chars()
        .take_while(|ch| *ch != '\n')
        .all(|ch| ch.is_whitespace())
}
