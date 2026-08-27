// 定义 UI System 拥有并保留声明顺序的字体族回退列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFamily {
    // 保存已经去除外围空白且通过验证的字体族名称。
    names: Vec<String>,
}

// 提供受控构造与只读遍历能力。
impl FontFamily {
    // 从任意字符串迭代器构造非空字体族回退列表。
    pub fn from_names<I, S>(names: I) -> Option<Self>
    where
        // 接受数组、切片或其他有序迭代器。
        I: IntoIterator<Item = S>,
        // 只借用调用方名称并在验证后取得所有权。
        S: AsRef<str>,
    {
        // 按声明顺序收集经过验证的名称。
        let mut normalized = Vec::new();
        // 逐项验证，任何非法名称都拒绝整个列表。
        for name in names {
            // 去除列表分隔符外的无语义空白。
            let name = name.as_ref().trim();
            // 空名称或控制字符不能成为字体注册表键。
            if name.is_empty() || name.chars().any(char::is_control) {
                // 不静默删除非法项，以免改变回退顺序。
                return None;
            }
            // 保存规范化名称并维持源码顺序。
            normalized.push(name.to_owned());
        }
        // 空列表没有可选择的字体族。
        if normalized.is_empty() {
            // 要求调用方显式保留未声明状态。
            return None;
        }
        // 返回完整有效的字体族列表。
        Some(Self { names: normalized })
    }

    // 按声明顺序借用全部字体族名称。
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &str> {
        // 隐藏内部 String 所有权，只公开稳定字符串视图。
        self.names.iter().map(String::as_str)
    }

    // 返回列表中的字体族数量。
    pub fn len(&self) -> usize {
        // 返回已验证集合的长度。
        self.names.len()
    }
}
