//! Menu typed 受控状态构造与诊断访问。

// 引入 Menu 运行时、数据项与擦除后的绑定实现。
use super::{Menu, MenuItem, StateMenuControlledBinding};
// 引入调用方拥有的响应式状态。
use crate::ui::State;
// 引入稳定 key 展示约束。
use std::fmt::Display;
// 引入受控绑定的共享所有权。
use std::rc::Rc;

// 扩展 Menu 的 typed 受控构造边界。
impl Menu {
    /// 使用 typed MenuItem 树建立单选与展开双向受控 Menu。
    pub fn controlled<K, I>(items: I, selected: &State<Option<K>>, open: &State<Vec<K>>) -> Self
    where
        // typed key 必须可复制、比较、稳定展示并进入响应式 State。
        K: Clone + PartialEq + Display + Send + Sync + 'static,
        // 调用方交付拥有型菜单数据树。
        I: IntoIterator<Item = MenuItem<K>>,
    {
        // 擦除绘制层不需要的 K 类型，同时保存稳定回写映射。
        let (items, values, diagnostics) = Self::erase_controlled_items(items);
        // 构造兼容运行时并安装 typed 状态绑定。
        let mut menu = Self::new();
        // 保存去重后的拥有型菜单树。
        menu.items = items;
        // 保存可观察的重复 key 诊断。
        menu.diagnostics = diagnostics;
        // 由 trait object 隔离具体 K 与非泛型组件绘制层。
        menu.controlled_binding = Some(Rc::new(StateMenuControlledBinding {
            // 克隆调用方单选状态句柄。
            selected: selected.clone(),
            // 克隆调用方展开状态句柄。
            open: open.clone(),
            // 保存首项优先的 typed key 映射。
            values,
        }));
        // 首次物化立即从唯一事实源同步界面状态。
        menu.sync_bound_keys();
        // 返回完整受控菜单。
        menu
    }

    /// 返回首次物化时产生的菜单树诊断。
    pub fn diagnostics(&self) -> &[String] {
        // 只读暴露稳定诊断集合。
        &self.diagnostics
    }
}
