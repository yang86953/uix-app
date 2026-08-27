// 定义 UI System 拥有的闭合文本装饰值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDecoration {
    // 不绘制文本装饰线，并可显式覆盖继承值。
    #[default]
    None,
    // 在每个视觉行的文字下缘绘制下划线。
    Underline,
    // 在每个视觉行的文字上缘绘制上划线。
    Overline,
    // 在每个视觉行的文字中部绘制删除线。
    LineThrough,
}
