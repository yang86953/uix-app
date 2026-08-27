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
