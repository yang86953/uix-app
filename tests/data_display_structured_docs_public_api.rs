// 声明本文件只编译结构化数据展示文档，不启动窗口、计时器或外部 I/O。
#![allow(dead_code)]

// 隔离 data-list-slots 围栏中的列表插槽。
mod data_list_slots {
    // 引入文档承诺的列表与声明式视图公开 prelude。
    use uix_app::prelude::*;

    // 编译列表头部、尾部与加载入口的真实子视图。
    fn compile_example() {
        // 构造带稳定角色插槽的列表组件。
        let _list = embed(
            // 创建列表并注入业务文字项。
            List::new()
                // 设置当前可见的业务条目。
                .items(vec!["待处理", "已完成"])
                // 注入头部真实 View。
                .header_view(label("任务"))
                // 注入尾部真实 View。
                .footer_view(label("共 2 项"))
                // 注入保留自身事件状态的加载按钮。
                .load_more_view(button("加载更多").on_click_fn(|| {})),
        );
    }
}

// 隔离 data-tree 围栏中的可勾选层级树。
mod data_tree {
    // 引入文档承诺的树组件公开 prelude。
    use uix_app::prelude::*;

    // 编译稳定业务 key 驱动的层级树配置。
    fn compile_example() {
        // 构造可勾选且默认展开的树组件。
        let _tree = embed(
            // 用递归节点声明树的父子关系。
            Tree::new(vec![
                // 创建根节点并赋予稳定业务 key。
                TreeNode::new("root", "root").children(vec![
                    // 创建第一个叶节点。
                    TreeNode::new("child-a", "a"),
                    // 创建第二个叶节点。
                    TreeNode::new("child-b", "b"),
                ]),
            ])
            // 开启勾选能力。
            .checkable(true)
            // 让全部层级在首次构建时展开。
            .default_expand_all(true),
        );
    }
}

// 隔离 tree-basic 围栏中的最小层级关系。
mod tree_basic {
    // 引入文档承诺的树组件公开 prelude。
    use uix_app::prelude::*;

    // 编译最小的父子树结构。
    fn compile_example() {
        // 使用稳定 key 构造部门与成员两级树。
        let _tree = embed(Tree::new(vec![
            // 创建父节点并注入唯一子节点。
            TreeNode::new("部门", "dept")
                // 保留成员节点的独立业务身份。
                .children(vec![TreeNode::new("成员", "member")]),
        ]));
    }
}

// 隔离 data-qrcode 围栏中的二维码显示配置。
mod data_qrcode {
    // 引入文档承诺的二维码组件公开 prelude。
    use uix_app::prelude::*;

    // 编译二维码值、尺寸与纠错等级配置。
    fn compile_example() {
        // 构造 M/Q/H 等级映射之外仍受组件校验的二维码。
        let _qrcode = embed(
            // 以业务值创建二维码组件。
            QRCode::new("https://example.com")
                // 设置逻辑像素尺寸。
                .size(128.0)
                // 设置公开的零到三级纠错等级。
                .error_level(2),
        );
    }
}

// 隔离 data-result 围栏中的成功结果页。
mod data_result {
    // 引入文档承诺的结果页公开 prelude。
    use uix_app::prelude::*;

    // 编译具备文字语义的成功结果页。
    fn compile_example() {
        // 构造成功状态及其标题、说明和操作文字。
        let _result = embed(
            // 创建成功类型结果页。
            ResultView::new(ResultType::Success)
                // 设置可读标题。
                .title("操作成功")
                // 设置补充说明。
                .subtitle("你的更改已保存")
                // 设置附加操作文字。
                .extra_text("返回首页"),
        );
    }
}

// 隔离 data-table-pagination 围栏中的远程分页契约。
mod data_table_pagination {
    // 引入文档承诺的表格公开 prelude。
    use uix_app::prelude::*;

    // 编译由业务层提供当前页数据的远程分页表格。
    fn compile_example() {
        // 准备当前页的业务行快照。
        let rows = vec![vec!["小贝".to_string()]];
        // 构造带远程分页回调的表格视图。
        let _table = embed(
            // 创建表格并声明姓名列。
            Table::new()
                // 设置公开列定义。
                .columns(vec![TableColumn::new("姓名", 200.0)])
                // 注入当前页业务行。
                .rows(rows)
                // 声明应用层拥有的分页事实与回调。
                .pagination(TablePagination {
                    // 当前页从一开始计数。
                    current: 1,
                    // 声明远程数据总行数。
                    total: 100,
                    // 声明每页大小。
                    page_size: 10,
                    // 接收用户请求的新页码。
                    on_change: |_page| {},
                })
                // 将公开表格组件构建为 View。
                .build(),
        );
    }
}

// 隔离 data-table-row-click 围栏中的行业务回调。
mod data_table_row_click {
    // 引入文档承诺的表格公开 prelude。
    use uix_app::prelude::*;

    // 编译接收行快照与行索引的点击回调。
    fn compile_example() {
        // 准备一行外部业务数据。
        let rows = vec![vec!["小贝".to_string()]];
        // 构造带行点击观察器的表格视图。
        let _table = embed(
            // 创建表格并声明姓名列。
            Table::new()
                // 设置公开列定义。
                .columns(vec![TableColumn::new("姓名", 200.0)])
                // 注入业务行快照。
                .rows(rows)
                // 接收同一行按下与释放产生的业务事件。
                .on_row_click(|_row, _row_index| {
                    // 回调只读取行事实，示例不执行副作用。
                })
                // 将公开表格组件构建为 View。
                .build(),
        );
    }
}

// 隔离 data-table-span 围栏中的业务行跨度函数。
mod data_table_span {
    // 引入文档承诺的表格公开 prelude。
    use uix_app::prelude::*;

    // 编译由行内容决定跨度的合并单元格。
    fn compile_example() {
        // 准备触发行合并的业务数据。
        let rows = vec![
            // 第一行声明合并类别。
            vec!["合并".to_string(), "一".to_string()],
            // 第二行提供被覆盖的相邻内容。
            vec!["合并".to_string(), "二".to_string()],
        ];
        // 构造带行跨度函数的表格视图。
        let _table = embed(
            // 创建表格组件。
            Table::new()
                // 声明类别列与数值列。
                .columns(vec![
                    // 类别列根据业务行首格决定跨度。
                    TableColumn::new("类别", 120.0).row_span(|row, _col| {
                        // 合并标记覆盖两行，其余记录保持单行。
                        if row[0] == "合并" { 2 } else { 1 }
                    }),
                    // 数值列保持默认单元格跨度。
                    TableColumn::new("数值", 120.0),
                ])
                // 注入外部业务行。
                .rows(rows)
                // 将公开表格组件构建为 View。
                .build(),
        );
    }
}

// 隔离 data-transfer-advanced 围栏中的穿梭框扩展契约。
mod data_transfer_advanced {
    // 引入文档承诺的穿梭框公开 prelude。
    use uix_app::prelude::*;

    // 编译源目标列表、自定义行视图与迁移回调。
    fn compile_example() {
        // 声明源列表、目标列表并开启搜索。
        let transfer = Transfer::new()
            // 注入源列表稳定业务项。
            .source(vec![TransferItem::new("draft", "草稿")])
            // 注入目标列表稳定业务项。
            .target(vec![TransferItem::new("published", "已发布")])
            // 开启组件内搜索过滤。
            .searchable(true);
        // 自定义每一行的 View，并在迁移完成后接收最新列表。
        let transfer = transfer
            // 只读取条目快照来构造行子视图。
            .render_item(|item| label(item.title.clone()))
            // 观察组件发布的源、目标与迁移方向。
            .on_change(|_source, _target, _direction| {});
        // 将组件嵌入声明式 View 树。
        let _transfer = embed(transfer);
    }
}
