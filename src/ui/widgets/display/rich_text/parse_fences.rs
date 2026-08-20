// 富文本行首围栏代码边界解析辅助。

// 保存一个完整围栏代码块在当前输入切片中的稳定字节边界。
pub(super) struct FencedBlock {
    // 开围栏首字节，用于把围栏前的普通 Markdown 交回行解析器。
    pub(super) open: usize,
    // 代码正文首字节，开围栏同一行的语言标注不会进入正文。
    pub(super) code_start: usize,
    // 闭围栏首字节，代码正文切片在这里结束。
    pub(super) close_start: usize,
    // 闭围栏行尾字节，允许的尾随空白会与标记一起消费。
    pub(super) after_close: usize,
}

// 保存开围栏采用的 ASCII 标记与连续长度。
struct FenceMarker {
    // 反引号或波浪号字节，闭围栏必须使用同一种标记。
    byte: u8,
    // 开围栏连续标记长度，闭围栏不得更短。
    len: usize,
}

// 查找一个完整的行首围栏代码块。
pub(super) fn find_fenced_block(text: &str) -> Option<FencedBlock> {
    // 先找到最早的合法开围栏，未闭合时保持整段原文语义。
    let (open, marker, code_start) = find_opening_fence(text)?;
    // 再从正文首行开始寻找同类且足够长的闭围栏。
    let (close_start, after_close) = find_closing_fence(text, code_start, &marker)?;
    // 返回调用方切分普通文本、代码正文和剩余内容所需的全部边界。
    Some(FencedBlock {
        // 保留开围栏的位置。
        open,
        // 保留正文起点。
        code_start,
        // 保留闭围栏起点。
        close_start,
        // 保留闭围栏行尾位置。
        after_close,
    })
}

// 查找最早的三个及以上反引号或波浪号开围栏。
fn find_opening_fence(text: &str) -> Option<(usize, FenceMarker, usize)> {
    // 扫描位置始终指向一行的首字节。
    let mut line_start = 0usize;
    // 逐行检查，避免行内连续标记抢占块级语义。
    while line_start < text.len() {
        // 找到当前行换行符或输入结尾。
        let line_end = find_line_end(text, line_start);
        // 统计当前行首可作为围栏的连续标记。
        if let Some(marker) = parse_fence_marker(&text.as_bytes()[line_start..line_end]) {
            // 开围栏必须拥有下一行，正文从换行符后开始。
            if line_end < text.len() {
                // 返回开围栏、标记契约与正文起点。
                return Some((line_start, marker, line_end + 1));
            }
            // 文件末尾的孤立开围栏无法形成完整代码块。
            return None;
        }
        // 没有下一行时已经扫描完全部输入。
        if line_end == text.len() {
            // 当前输入不存在合法开围栏。
            return None;
        }
        // 跳过当前换行符并检查下一行行首。
        line_start = line_end + 1;
    }
    // 空输入或扫描结束时没有合法开围栏。
    None
}

// 从代码正文起点寻找与开围栏匹配的闭围栏。
fn find_closing_fence(
    // 完整输入切片用于返回绝对字节边界。
    text: &str,
    // 搜索从代码正文首行开始。
    from: usize,
    // 开围栏标记定义闭合字符和最短长度。
    opening: &FenceMarker,
) -> Option<(usize, usize)> {
    // 搜索位置始终保持在代码正文的行首。
    let mut line_start = from;
    // 逐行查找，异类或更短围栏继续作为代码正文保留。
    while line_start < text.len() {
        // 定位当前候选行的 LF 或输入结尾。
        let line_end = find_line_end(text, line_start);
        // CR 只有作为 CRLF 的一部分时才从闭合后缀检查中排除。
        let content_end = line_content_end(text, line_end);
        // 读取当前行首与开围栏相同的连续标记数量。
        let marker_len = count_marker_run(&text.as_bytes()[line_start..content_end], opening.byte);
        // 闭围栏长度不得短于开围栏。
        if marker_len >= opening.len
            // 标记后的当前行只允许 ASCII 空格或制表符。
            && text.as_bytes()[line_start + marker_len..content_end]
                .iter()
                .all(|byte| matches!(byte, b' ' | b'\t'))
        {
            // 消费闭围栏和尾随 ASCII 空白，但把 LF 留给统一换行解析。
            return Some((line_start, line_end));
        }
        // 当前行已经是输入末尾时不存在后续闭围栏。
        if line_end == text.len() {
            // 开围栏保持未闭合状态。
            return None;
        }
        // 继续检查下一行行首。
        line_start = line_end + 1;
    }
    // 没有找到满足字符、长度和尾随空白契约的闭围栏。
    None
}

// 解析行首围栏标记，长度不足时返回空。
fn parse_fence_marker(line: &[u8]) -> Option<FenceMarker> {
    // 只有反引号和波浪号可以启动 Markdown 围栏代码块。
    let byte = *line.first()?;
    // 其他行首字符直接保持普通文本。
    if !matches!(byte, b'`' | b'~') {
        // 当前行不是开围栏。
        return None;
    }
    // 统计同一种行首标记的连续长度。
    let len = count_marker_run(line, byte);
    // Markdown 围栏至少需要三个相同标记。
    if len < 3 {
        // 一至两个标记继续交给内联解析器。
        return None;
    }
    // 返回供闭围栏匹配使用的稳定标记契约。
    Some(FenceMarker {
        // 保存标记字符。
        byte,
        // 保存开围栏长度。
        len,
    })
}

// 统计切片开头连续出现的指定 ASCII 标记。
fn count_marker_run(line: &[u8], marker: u8) -> usize {
    // 只有连续的相同字节属于同一围栏。
    line.iter().take_while(|byte| **byte == marker).count()
}

// 返回当前行 LF 的字节位置，不存在 LF 时返回输入长度。
fn find_line_end(text: &str, line_start: usize) -> usize {
    // 相对位置加回行首偏移即可得到绝对字节边界。
    text[line_start..]
        // 查找统一的源码行分隔符。
        .find('\n')
        // 把相对索引转换成绝对索引。
        .map_or(text.len(), |relative| line_start + relative)
}

// 返回用于闭合后缀检查的当前行内容终点。
fn line_content_end(text: &str, line_end: usize) -> usize {
    // 仅当 LF 前紧邻 CR 时把它视为 CRLF 行结束的一部分。
    if line_end > 0 && line_end < text.len() && text.as_bytes()[line_end - 1] == b'\r' {
        // 排除 CR，避免把合法 Windows 行结束误判成围栏后缀。
        return line_end - 1;
    }
    // LF 之前或输入结尾的其他字节都属于当前行内容。
    line_end
}
