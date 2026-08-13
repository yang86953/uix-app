// 提供 UI 线程窗口循环期间的主题切换请求通道。
//
// uix-lang 的 setTheme 内置操作生成对 `uix_set_theme` 的调用；该入口通过
// 线程局部请求器把主题名称提交给当前窗口会话。请求器由 App 组合根在窗口
// 事件循环前后安装与卸载，与组件状态捕获的线程局部模式一致：生命周期由
// 框架持有，不存在调用方全局注册表。

// 引入单元类型转换与互斥借用。
use std::cell::RefCell;

// 保存当前 UI 线程窗口循环的主题切换请求器。
thread_local! {
    // 事件循环外没有请求器，调用保持无副作用。
    static THEME_REQUESTER: RefCell<Option<Box<dyn Fn(&str)>>> = const { RefCell::new(None) };
}

// 安装当前 UI 线程的主题切换请求器。
#[doc(hidden)]
pub fn uix_install_theme_requester(requester: Box<dyn Fn(&str)>) {
    // 写入请求器，覆盖此前残留（App 层保证成对安装与卸载）。
    THEME_REQUESTER.with(|slot| {
        // 借用线程局部槽。
        *slot.borrow_mut() = Some(requester);
    });
}

// 卸载当前 UI 线程的主题切换请求器。
#[doc(hidden)]
pub fn uix_clear_theme_requester() {
    // 清空请求器，避免事件循环结束后继续提交。
    THEME_REQUESTER.with(|slot| {
        // 借用线程局部槽。
        *slot.borrow_mut() = None;
    });
}

// 按名称提交主题切换请求；无请求器时保持声明式行为无副作用。
#[doc(hidden)]
pub fn uix_set_theme(name: &str) {
    // 读取当前线程的请求器。
    THEME_REQUESTER.with(|slot| {
        // 借用请求器槽。
        let slot = slot.borrow();
        // 存在请求器时提交主题名称。
        if let Some(requester) = slot.as_ref() {
            // 委托 App 侧主题切换通道。
            requester(name);
        }
    });
}

// 集中验证主题请求通道的安装、提交与卸载语义。
#[cfg(test)]
mod tests {
    // 引入父模块入口。
    use super::*;
    // 引入引用计数。
    use std::rc::Rc;

    // 验证无请求器时调用保持无副作用。
    #[test]
    fn set_theme_without_requester_is_side_effect_free() {
        // 直接调用不 panic、不产生任何效果。
        uix_set_theme("dark");
    }

    // 验证安装请求器后按名称提交，卸载后恢复无副作用。
    #[test]
    fn requester_receives_names_and_clears_after_uninstall() {
        // 记录提交名称（RefCell 支持非 Copy 值）。
        let received = Rc::new(RefCell::new(String::new()));
        // 复制记录句柄供请求器捕获。
        let received_clone = received.clone();
        // 安装请求器。
        uix_install_theme_requester(Box::new(move |name: &str| {
            // 记录收到的主题名。
            *received_clone.borrow_mut() = name.to_string();
        }));
        // 提交暗色主题。
        uix_set_theme("dark");
        // 请求器必须收到名称。
        assert_eq!(received.borrow().as_str(), "dark");
        // 卸载请求器。
        uix_clear_theme_requester();
        // 卸载后的调用保持无副作用。
        uix_set_theme("light");
        // 记录值保持不变。
        assert_eq!(received.borrow().as_str(), "dark");
    }
}
