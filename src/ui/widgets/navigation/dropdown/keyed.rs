// 引入父模块公开数据类型与运行时组件。
use super::{Dropdown, DropdownItem};
// keyed UIX 数据在运行时执行全树唯一性门禁。
use std::collections::HashSet;

// 实现 Dropdown 的 keyed 数据身份辅助边界。
impl Dropdown {
    /// 规范化 UIX keyed 选项并按全树首项优先拒绝非法身份。
    pub(super) fn normalize_keyed_items<I>(items: I) -> (Vec<DropdownItem>, Vec<String>)
    where
        // 接受任意拥有型 DropdownItem 集合。
        I: IntoIterator<Item = DropdownItem>,
    {
        /// 递归验证当前层与全部后代。
        fn visit(
            items: impl IntoIterator<Item = DropdownItem>,
            seen: &mut HashSet<String>,
            diagnostics: &mut Vec<String>,
        ) -> Vec<DropdownItem> {
            // 保存当前层通过身份门禁的选项。
            let mut output = Vec::new();
            // 按调用方原始顺序稳定遍历。
            for mut item in items {
                // 分隔线没有交互身份，可原样保留。
                if item.divider {
                    // 分隔线不参与全树 key 唯一性。
                    output.push(item);
                    // 继续处理下一条数据。
                    continue;
                }
                // 空 key 无法承担选择、展开或 reconcile 身份。
                if item.key.is_empty() {
                    // 记录被忽略条目的展示文字以便定位动态数据。
                    diagnostics.push(format!("Dropdown 忽略空 key 选项 {:?}", item.label));
                    // 非法条目及其子树不再物化。
                    continue;
                }
                // 全树重复 key 保留源码或集合中的首项。
                if !seen.insert(item.key.clone()) {
                    // 记录确定的重复身份诊断。
                    diagnostics.push(format!("Dropdown 忽略重复 key {:?}，保留首项", item.key));
                    // 后续重复条目及其子树不再物化。
                    continue;
                }
                // 后代共享同一个全局 seen 集合。
                item.children = visit(item.children, seen, diagnostics);
                // 保存完整且身份稳定的选项。
                output.push(item);
            }
            // 返回当前层的稳定选项集合。
            output
        }

        // 初始化全树 key 集合。
        let mut seen = HashSet::new();
        // 初始化可观察诊断集合。
        let mut diagnostics = Vec::new();
        // 递归规范化完整输入树。
        let items = visit(items, &mut seen, &mut diagnostics);
        // 返回稳定树与全部身份诊断。
        (items, diagnostics)
    }

    /// 判断完整选项树是否仍含指定 keyed 子菜单组。
    pub(super) fn contains_group_key(items: &[DropdownItem], key: &str) -> bool {
        // 当前层或任意递归子树命中时返回真。
        items.iter().any(|item| {
            // 只有含 children 的同 key 选项才拥有展开状态。
            (!item.children.is_empty() && item.key == key)
                // 继续检查未命中的递归子树。
                || Self::contains_group_key(&item.children, key)
        })
    }
}
