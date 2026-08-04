// 导入基础表格、泛型表格与列定义，覆盖单 capability 公开入口。
use uix::prelude::{DataTable, Table, TableColumn};

// 声明用于验证泛型表格入口的最小行模型。
#[derive(Clone)]
struct TableFixtureRow {
    // 保存稳定行键。
    id: u64,
    // 保存列投影使用的名称。
    name: &'static str,
}

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 构造基础表格以证明列与行构造器公开面可达。
    let table = Table::new()
        // 声明一个基础文本列。
        .columns(vec![TableColumn::new("Name", 120.0)])
        // 写入一行稳定文本数据。
        .rows(vec![vec!["Ada".to_owned()]]);
    // 准备泛型表格使用的结构化行数据。
    let rows = vec![TableFixtureRow { id: 1, name: "Ada" }];
    // 构造泛型表格以证明 DataTable 与类型化列投影公开面可达。
    let data_table: DataTable<TableFixtureRow> = match Table::data(rows, |row| row.id.to_string()) {
        // 绑定成功时投影名称列。
        Ok(table) => table.columns(vec![TableColumn::new("Name", 120.0)
            // 将结构化行映射为稳定文本。
            .bind(|row: &TableFixtureRow| row.name.to_owned())]),
        // 行键校验失败时结束 fixture。
        Err(_) => return,
    };
    // 显式消费全部值，避免无意义的未使用警告。
    drop((table, data_table));
}
