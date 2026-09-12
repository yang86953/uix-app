// 引入确定性名称映射。
use std::collections::BTreeMap;

// 引入应用可安装的公开主题值。
use crate::ui::Theme;

// 保存单个 App 实例可解析的 UIX 具名主题。
#[derive(Clone)]
pub(super) struct NamedThemes {
    // 保存名称到拥有所有权主题值的确定性映射。
    themes: BTreeMap<String, Theme>,
}

// 为普通 Rust App 保留 light 与 dark 两个内建名称。
impl Default for NamedThemes {
    // 创建包含两个内建主题的作用域表。
    fn default() -> Self {
        // 创建空的确定性映射。
        let mut themes = BTreeMap::new();
        // 登记内建亮色主题。
        themes.insert("light".to_string(), Theme::light());
        // 登记内建暗色主题。
        themes.insert("dark".to_string(), Theme::dark());
        // 返回完整默认表。
        Self { themes }
    }
}

// 提供 App builder 与运行期请求通道共用的解析能力。
impl NamedThemes {
    // 插入文档主题；同名声明显式覆盖内建预设。
    pub(super) fn insert(&mut self, name: String, theme: Theme) {
        // 写入拥有所有权的主题值。
        self.themes.insert(name, theme);
    }

    // 按精确名称克隆主题供 App 或运行期安装。
    pub(super) fn resolve(&self, name: &str) -> Option<Theme> {
        // 克隆 Arc 包装的轻量主题值。
        self.themes.get(name).cloned()
    }
}
