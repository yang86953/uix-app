// Agent transport 帧划分纯逻辑测试。
// 只测 read_bounded_line 的边界语义（EOF/未终止/超限/分片），不启动线程与 IO。

// 引入被测的帧划分函数与结果枚举。
use super::{BoundedLine, MAX_AGENT_MESSAGE_BYTES, read_bounded_line};
// 构造内存缓冲读取器。
use std::io::{BufReader, Cursor};

/// 把输入字节封装为带 8KB 缓冲的行读取器。
fn reader(input: &[u8]) -> BufReader<Cursor<&[u8]>> {
    BufReader::new(Cursor::new(input))
}

/// 每次只暴露 3 字节的分片读取器，强制跨 fill_buf 边界累积。
struct ChunkedCursor<'a> {
    data: &'a [u8],
    consumed: usize,
}

impl std::io::Read for ChunkedCursor<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        // 每轮最多交付 3 字节，模拟内核小包到达。
        let chunk = 3.min(output.len()).min(self.data.len() - self.consumed);
        output[..chunk].copy_from_slice(&self.data[self.consumed..self.consumed + chunk]);
        self.consumed += chunk;
        Ok(chunk)
    }
}

// 单行完整帧必须返回 Line 并填充行内容。
#[test]
fn single_line_frame_reads_line_without_newline() {
    // 构造不带换行符的行内容。
    let mut line = Vec::new();
    let result = read_bounded_line(&mut reader(b"hello\n"), &mut line);
    // 必须判定为完整行。
    assert_eq!(result.unwrap(), BoundedLine::Line);
    // 行内容不得包含换行符。
    assert_eq!(line, b"hello");
}

// 空输入必须返回 Eof。
#[test]
fn empty_input_reports_eof() {
    // 构造空输入。
    let mut line = Vec::new();
    let result = read_bounded_line(&mut reader(b""), &mut line);
    // 空流必须判定为 EOF。
    assert_eq!(result.unwrap(), BoundedLine::Eof);
    assert!(line.is_empty());
}

// 流结束后仍有半行内容必须返回 Unterminated。
#[test]
fn trailing_partial_line_reports_unterminated() {
    // 构造无换行结尾的残留内容。
    let mut line = Vec::new();
    let result = read_bounded_line(&mut reader(b"partial"), &mut line);
    // 无换行结束的半行必须判定为未终止。
    assert_eq!(result.unwrap(), BoundedLine::Unterminated);
    // 已读内容仍要保留供诊断。
    assert_eq!(line, b"partial");
}

// 跨多个 fill_buf 分片的行必须完整累积。
#[test]
fn line_accumulates_across_chunk_boundaries() {
    // 用 3 字节分片输入构造 10 字节行。
    let mut line = Vec::new();
    let mut stream = BufReader::new(ChunkedCursor {
        data: b"abcdefghij\n",
        consumed: 0,
    });
    let result = read_bounded_line(&mut stream, &mut line);
    // 分片输入也必须判定为完整行。
    assert_eq!(result.unwrap(), BoundedLine::Line);
    assert_eq!(line, b"abcdefghij");
}

// 连续调用必须逐行消费输入流。
#[test]
fn consecutive_calls_consume_lines_in_order() {
    // 构造三行输入。
    let mut stream = reader(b"first\nsecond\nthird\n");
    let mut line = Vec::new();
    // 第一行。
    assert_eq!(
        read_bounded_line(&mut stream, &mut line).unwrap(),
        BoundedLine::Line
    );
    assert_eq!(line, b"first");
    // 第二行。
    assert_eq!(
        read_bounded_line(&mut stream, &mut line).unwrap(),
        BoundedLine::Line
    );
    assert_eq!(line, b"second");
    // 第三行。
    assert_eq!(
        read_bounded_line(&mut stream, &mut line).unwrap(),
        BoundedLine::Line
    );
    assert_eq!(line, b"third");
    // 行读尽后必须判定为 EOF。
    assert_eq!(
        read_bounded_line(&mut stream, &mut line).unwrap(),
        BoundedLine::Eof
    );
}

// 累计长度超过协议上限必须立即返回 TooLarge。
#[test]
fn oversized_line_reports_too_large() {
    // 构造超过上限的单行输入（含换行）。
    let mut line = Vec::new();
    let input = vec![b'x'; MAX_AGENT_MESSAGE_BYTES + 1];
    let result = read_bounded_line(&mut reader(&input), &mut line);
    // 超限帧必须判定为过大。
    assert_eq!(result.unwrap(), BoundedLine::TooLarge);
}

// 恰好达到上限的行必须被接受。
#[test]
fn exactly_at_limit_line_is_accepted() {
    // 构造恰好等于上限的行内容。
    let mut line = Vec::new();
    let mut input = vec![b'x'; MAX_AGENT_MESSAGE_BYTES];
    input.push(b'\n');
    let result = read_bounded_line(&mut reader(&input), &mut line);
    // 上限边界内的行必须判定为完整行。
    assert_eq!(result.unwrap(), BoundedLine::Line);
    assert_eq!(line.len(), MAX_AGENT_MESSAGE_BYTES);
}

// 超限判定发生在换行之前：超限行即使带换行也必须判定为 TooLarge。
#[test]
fn oversized_line_with_newline_still_reports_too_large() {
    // 构造超限且带换行结尾的输入。
    let mut line = Vec::new();
    let mut input = vec![b'x'; MAX_AGENT_MESSAGE_BYTES + 10];
    input.push(b'\n');
    let result = read_bounded_line(&mut reader(&input), &mut line);
    // 超限优先于行结束判定。
    assert_eq!(result.unwrap(), BoundedLine::TooLarge);
}

// 空行帧必须返回 Line 且内容为空。
#[test]
fn empty_line_frame_reports_line() {
    // 构造只含换行符的输入。
    let mut line = Vec::new();
    let result = read_bounded_line(&mut reader(b"\n"), &mut line);
    // 空行也是合法帧。
    assert_eq!(result.unwrap(), BoundedLine::Line);
    assert!(line.is_empty());
}

// 连续空行必须逐行消费。
#[test]
fn consecutive_empty_lines_are_consumed_separately() {
    // 构造两个连续换行。
    let mut stream = reader(b"\n\n");
    let mut line = Vec::new();
    // 第一个空行。
    assert_eq!(
        read_bounded_line(&mut stream, &mut line).unwrap(),
        BoundedLine::Line
    );
    // 第二个空行。
    assert_eq!(
        read_bounded_line(&mut stream, &mut line).unwrap(),
        BoundedLine::Line
    );
    // 之后 EOF。
    assert_eq!(
        read_bounded_line(&mut stream, &mut line).unwrap(),
        BoundedLine::Eof
    );
}
