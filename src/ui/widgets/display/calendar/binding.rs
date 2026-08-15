// 引入 Calendar 及其日期类型。
use super::{Calendar, Date};
// 引入公开响应式状态句柄。
use crate::ui::State;

// 为 Calendar 隔离受控状态构造适配。
impl Calendar {
    /// 绑定受控选中日期；用户提交会先回写状态，再发出 Change 事件。
    pub fn value(mut self, state: &State<Date>) -> Self {
        // Calendar 仅缓存外部状态投影，不成为第二个业务状态所有者。
        self.value_binding = Some(state.clone());
        // 读取外部唯一状态并归一化到 Calendar 支持范围。
        let date = Self::normalize_date(state.get());
        // 缓存受支持范围内的选中日期。
        self.selected_date.set(Some(date));
        // 同步当前展示年份。
        self.year.set(date.year);
        // 同步当前展示月份。
        self.month.set(date.month);
        // 同步键盘焦点日。
        self.focused_day.set(date.day);
        // 返回受控组件构造器。
        self
    }
}
