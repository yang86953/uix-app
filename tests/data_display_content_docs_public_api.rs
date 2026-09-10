// 声明本文件只编译内容型数据展示文档，不启动窗口、计时器或外部 I/O。
#![allow(dead_code)]

// 隔离 data-typography 围栏中的排版类型入口。
mod data_typography {
    // 引入文档承诺的排版组件公开 prelude。
    use uix_app::prelude::*;

    // 编译标题、段落与可复制文本配置。
    fn compile_example() {
        // 构造二级标题。
        let _heading = embed(Typography::heading("标题", 2));
        // 构造段落正文。
        let _paragraph = embed(Typography::paragraph("段落正文"));
        // 构造可复制的普通文本。
        let _text = embed(Typography::text("可复制文本").copyable(true));
    }
}

// 隔离 data-typography-title 围栏中的一级标题入口。
mod data_typography_title {
    // 引入文档承诺的排版组件公开 prelude。
    use uix_app::prelude::*;

    // 编译页面标题配置。
    fn compile_example() {
        // 构造一级页面标题。
        let _title = embed(Typography::title("页面标题"));
    }
}

// 隔离 data-typography-text 围栏中的强调与标记入口。
mod data_typography_text {
    // 引入文档承诺的排版组件公开 prelude。
    use uix_app::prelude::*;

    // 编译加粗且高亮的普通文本。
    fn compile_example() {
        // 构造同时使用 strong 与 mark 的正文。
        let _text = embed(Typography::text("正文").strong().mark());
    }
}

// 隔离 data-typography-paragraph 围栏中的段落装饰入口。
mod data_typography_paragraph {
    // 引入文档承诺的排版组件公开 prelude。
    use uix_app::prelude::*;

    // 编译斜体且带下划线的段落。
    fn compile_example() {
        // 构造同时使用 italic 与 underline 的段落。
        let _paragraph = embed(Typography::paragraph("段落").italic().underline());
    }
}

// 隔离 data-descriptions 围栏中的描述列表与标签。
mod data_descriptions {
    // 引入文档承诺的描述列表与标签公开 prelude。
    use uix_app::prelude::*;

    // 编译两列描述列表和带文字语义的成功标签。
    fn compile_example() {
        // 构造两列短结构化信息列表。
        let _descriptions = embed(
            // 创建描述列表。
            Descriptions::new()
                // 注入标签和值的业务快照。
                .items(vec![
                    // 声明姓名信息。
                    DescriptionsItem::new("姓名", "Ada"),
                    // 声明角色信息。
                    DescriptionsItem::new("角色", "管理员"),
                ])
                // 设置两列布局。
                .column(2),
        );
        // 构造不只依赖颜色表达语义的成功标签。
        let _tag = embed(Tag::new("成功").color(TagColor::Success));
    }
}

// 隔离 data-timeline 围栏中的有序事件快照。
mod data_timeline {
    // 引入文档承诺的时间轴公开 prelude。
    use uix_app::prelude::*;

    // 编译按顺序展示标题与说明的时间轴。
    fn compile_example() {
        // 构造包含创建与评审事件的时间轴。
        let _timeline = embed(
            // 创建时间轴并注入有序事件。
            Timeline::new().items(vec![
                // 声明创建事件及日期说明。
                TimelineItem::new("创建").description("2026-07-01"),
                // 声明评审事件及日期说明。
                TimelineItem::new("评审").description("2026-07-15"),
            ]),
        );
    }
}

// 隔离 data-progress-advanced 围栏中的进度显示变体。
mod data_progress_advanced {
    // 引入文档承诺的进度组件与颜色公开 prelude。
    use uix_app::prelude::*;

    // 编译确定进度、渐变、步骤与仪表盘配置。
    fn compile_example() {
        // 构造普通确定进度条。
        let _plain = embed(ProgressBar::new().progress(0.45));
        // 构造蓝绿渐变进度条。
        let _gradient = embed(
            // 为确定进度应用双色渐变。
            ProgressBar::new()
                // 设置当前进度。
                .progress(0.6)
                // 设置起止颜色。
                .gradient(Color::BLUE, Color::GREEN),
        );
        // 构造八步骤进度条。
        let _steps = embed(ProgressBar::new().progress(0.3).steps(8));
        // 构造仪表盘形态进度条。
        let _dashboard = embed(ProgressBar::new().progress(0.7).dashboard());
    }
}

// 隔离 data-skeleton-advanced 围栏中的骨架形态。
mod data_skeleton_advanced {
    // 引入文档承诺的骨架屏公开 prelude。
    use uix_app::prelude::*;

    // 编译矩形、圆形与活动段落骨架。
    fn compile_example() {
        // 构造指定尺寸的矩形骨架。
        let _rect = embed(
            // 创建骨架并选择矩形形态。
            Skeleton::new()
                // 使用矩形外观。
                .shape(SkeletonShape::Rect)
                // 设置宽高。
                .size(120.0, 16.0),
        );
        // 构造指定尺寸的圆形骨架。
        let _circle = embed(
            // 创建骨架并选择圆形形态。
            Skeleton::new()
                // 使用圆形外观。
                .shape(SkeletonShape::Circle)
                // 设置正方形包围尺寸。
                .size(40.0, 40.0),
        );
        // 构造三行且启用 shimmer 的文字骨架。
        let _paragraph = embed(Skeleton::new().paragraph(3).active(true));
    }
}

// 隔离 data-watermark-advanced 围栏中的水印绘制配置。
mod data_watermark_advanced {
    // 引入文档承诺的水印公开 prelude。
    use uix_app::prelude::*;

    // 编译字号、透明度、旋转角和间距配置。
    fn compile_example() {
        // 构造平铺在内容区域上方但不抢占布局的水印。
        let _watermark = embed(
            // 以可读文字创建水印。
            Watermark::new("内部资料")
                // 设置文字字号。
                .font_size(16.0)
                // 设置低透明度。
                .opacity(0.12)
                // 设置逆时针旋转角。
                .rotate(-20.0)
                // 设置水平和垂直平铺间距。
                .gap(220.0, 160.0),
        );
    }
}

// 隔离 data-table-model 围栏中的类型化行模型。
mod data_table_model {
    // 引入文档承诺的类型化表格公开 prelude。
    use uix_app::prelude::*;

    // 声明由应用层拥有的类型化业务行。
    #[derive(Clone, PartialEq)]
    // 使用单行字段声明保持示例模型紧凑。
    struct UserRow {
        // 提供稳定业务身份。
        id: u64,
        // 提供姓名列数据。
        name: String,
        // 提供邮箱列数据。
        email: String,
    }

    // 编译稳定行 key 和三列类型化投影。
    fn compile_example() {
        // 构造外部业务行快照。
        let rows = vec![UserRow {
            // 设置唯一业务标识。
            id: 1,
            // 设置姓名。
            name: "Ada".into(),
            // 设置邮箱。
            email: "ada@example.com".into(),
        }];
        // 构造校验唯一行 key 的类型化表格。
        let _table = embed(
            // 从业务行和稳定 key 投影创建数据表。
            Table::data(rows.clone(), |row| row.id.to_string())
                // 重复 key 必须作为类型化错误暴露。
                .expect("row key 必须唯一")
                // 声明三列文本投影。
                .columns(vec![
                    // 投影业务标识。
                    TableColumn::new("ID", 80.0).bind(|row: &UserRow| row.id.to_string()),
                    // 投影姓名。
                    TableColumn::new("姓名", 200.0).bind(|row: &UserRow| row.name.clone()),
                    // 投影邮箱。
                    TableColumn::new("邮箱", 260.0).bind(|row: &UserRow| row.email.clone()),
                ])
                // 将类型化表格构建为 View。
                .build(),
        );
    }
}

// 隔离 data-table-virtualized 围栏中的自定义单元格与虚拟化。
mod data_table_virtualized {
    // 引入文档承诺的类型化表格与标签公开 prelude。
    use uix_app::prelude::*;

    // 声明由应用层拥有的类型化业务行。
    #[derive(Clone, PartialEq)]
    // 保留稳定 id、名称与邮箱事实。
    struct UserRow {
        // 提供稳定业务身份。
        id: u64,
        // 提供自定义单元格的可读名称。
        name: String,
        // 提供状态列的确定性文字快照。
        email: String,
    }

    // 编译任意 View 单元格和声明式虚拟化开关。
    fn compile_example() {
        // 构造外部业务行快照。
        let rows = vec![UserRow {
            // 设置唯一业务标识。
            id: 1,
            // 设置可读名称。
            name: "Ada".into(),
            // 设置确定性文字快照。
            email: "ada@example.com".into(),
        }];
        // 构造只物化可视区的类型化表格。
        let _table = embed(
            // 从业务行和稳定 key 投影创建数据表。
            Table::data(rows.clone(), |row| row.id.to_string())
                // 重复 key 必须作为类型化错误暴露。
                .expect("row key 必须唯一")
                // 声明带自定义 View 的状态列。
                .columns(vec![
                    // 先绑定确定性文字快照，再声明任意 View 渲染器。
                    TableColumn::new("状态", 120.0)
                        // 使用邮箱作为确定性快照值。
                        .bind(|row: &UserRow| row.email.clone())
                        // 用可读成功标签渲染单元格。
                        .render(|row| embed(Tag::new(&row.name).color(TagColor::Success))),
                ])
                // 大数据集只物化可视区及 overscan。
                .virtualized(true)
                // 将类型化表格构建为 View。
                .build(),
        );
    }
}
