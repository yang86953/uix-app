//! 轻量使用方消费者探针：只挂 `uix-derive` 过程宏前端，不引入图形栈或运行时符号。
//!
//! 平时保持可编译空库；任一 `gate-broken-*` feature 打开时，对应模块嵌入注定
//! 被共享 Compiler System 定向拒绝的声明，由集成测试以子进程构建并断言真实
//! rustc 失败输出中出现稳定诊断文本。未打开任何场景 feature 时本库没有内容。

#[cfg(feature = "gate-broken-cascader-value")]
mod broken_cascader_value;
#[cfg(feature = "gate-broken-mixed-member-forms")]
mod broken_mixed_member_forms;
