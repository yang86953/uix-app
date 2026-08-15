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

    // 协调下一棵声明树的受控句柄与本地日期投影。
    pub(super) fn sync_value_binding(&mut self, next_binding: Option<State<Date>>) {
        // 首次进入受控模式时必须建立完整受控投影。
        let entering_controlled = self.value_binding.is_none() && next_binding.is_some();
        // 在替换句柄前读取下一轮唯一外部日期事实。
        let controlled_date = next_binding.as_ref().map(State::get);
        // 替换绑定能力；None 表示继续保留最后一次运行态选择。
        self.value_binding = next_binding;
        // 仅在进入受控模式或外部日期实际变化时重置显示月份。
        if let Some(date) = controlled_date.map(Self::normalize_date)
            && (entering_controlled || self.selected_date.get() != Some(date))
        {
            // 同步完整选中日期。
            self.selected_date.set(Some(date));
            // 同步展示年份。
            self.year.set(date.year);
            // 同步展示月份。
            self.month.set(date.month);
            // 同步键盘焦点日。
            self.focused_day.set(date.day);
        }
    }
}
