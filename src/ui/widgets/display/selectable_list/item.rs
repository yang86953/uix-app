//! 可选择列表条目的无状态构造方法。

// 引入父模块拥有的公开条目值类型。
use super::SelectableItem;

// 条目子模块只负责构造数据，不拥有列表选择或滚动状态。
impl SelectableItem {
    /// 创建具有稳定业务标识、显示文字且没有图标的条目。
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        // 将调用方输入转换为条目拥有的字符串。
        Self {
            // 保存用于选择调和的稳定业务标识。
            id: id.into(),
            // 保存向用户显示的文字。
            text: text.into(),
            // 默认不声明行内图标。
            icon: None,
        }
    }

    /// 设置条目使用的图标名称。
    pub fn icon(mut self, icon: &str) -> Self {
        // 条目拥有图标名称，避免借用调用方生命周期。
        self.icon = Some(icon.to_string());
        // 返回完成配置的条目。
        self
    }
}
