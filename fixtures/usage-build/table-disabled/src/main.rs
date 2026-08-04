// 故意导入未启用 capability 的基础与泛型表格类型，门禁要求此行无法编译。
use uix::prelude::{DataTable, Table};

// 提供 compile-fail fixture 的稳定入口。
fn main() {
    // 若基础表格公开面意外泄漏，构造值会使门禁检测到编译成功并失败。
    let table = Table::new();
    // 若泛型表格公开面意外泄漏，类型标注会阻止部分门控静默通过。
    let data_table: Option<DataTable<()>> = None;
    // 显式消费值，避免公开面泄漏时只产生未使用警告。
    drop((table, data_table));
}
