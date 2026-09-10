// 声明本文件只编译布局使用文档，不启动窗口或原生资源。
#![allow(dead_code)]

// 隔离 layout-grid 围栏中的显式与响应式网格声明。
mod layout_grid {
    // 引入文档承诺的公开布局 prelude。
    use uix_app::prelude::*;

    // 编译显式轨道与响应式二十四栅格两种公开写法。
    fn compile_example() {
        // 构造带两个比例轨道和两个子项的显式网格。
        let _explicit_grid = embed(tree! {
            // 通过公开 Grid builder 声明列轨和间距。
            Grid::new()
                // 声明一比二的比例列轨。
                .columns(vec![GridTrack::Fr(1.0), GridTrack::Fr(2.0)])
                // 声明统一轨道间距。
                .gap(8.0) => [
                    // 放入第一列公开文本节点。
                    label("1fr"),
                    // 放入第二列公开文本节点。
                    label("2fr"),
                ]
        });

        // 构造使用断点列宽的响应式网格组件。
        let _responsive_grid = embed(
            // 创建公开二十四栅格 builder。
            Grid::responsive()
                // 为两个源顺序子项声明断点跨度。
                .cols(vec![
                    // 声明第一个子项从全宽收敛到四分之一宽。
                    Col::new().span(24).sm(12).md(8).lg(6),
                    // 声明第二个子项使用相同断点跨度。
                    Col::new().span(24).sm(12).md(8).lg(6),
                ])
                // 声明响应式网格间距。
                .gap(4.0),
        );
    }
}

// 隔离 layout-splitter 围栏中的面板配置。
mod layout_splitter {
    // 引入文档承诺的公开布局 prelude。
    use uix_app::prelude::*;

    // 编译双面板 Splitter 的公开 builder 链。
    fn compile_example() {
        // 构造可拖拽的横向双面板分隔组件。
        let _splitter = embed(
            // 创建默认 Splitter。
            Splitter::new()
                // 声明面板数量。
                .panels(2)
                // 声明首个面板最小尺寸。
                .min_size(0, 120.0)
                // 保持横向分隔方向。
                .vertical(false),
        );
    }
}

// 隔离 layout-affix 围栏中的固钉与回顶组件。
mod layout_affix {
    // 引入文档承诺的公开布局 prelude。
    use uix_app::prelude::*;

    // 编译带子节点的 Affix 与独立 BackTop 公开写法。
    fn compile_example() {
        // 构造包含操作按钮的固钉节点。
        let _affix = embed(ViewNode::new(
            // 声明顶部偏移与当前滚动位置。
            Affix::new(12.0).scroll_y(180.0),
            // 把公开按钮转换为固钉直接子节点。
            vec![button("回到操作").into()],
        ));

        // 构造超过公开阈值后出现的回顶组件。
        let _back_top = embed(BackTop::new().visibility_height(400.0));
    }
}

// 隔离 layout-scroll 围栏中的普通与虚拟滚动声明。
mod layout_scroll {
    // 引入文档承诺的公开布局 prelude。
    use uix_app::prelude::*;

    // 编译普通滚动容器与固定行高虚拟列表。
    fn compile_example() {
        // 构造包含一百行文本的普通滚动容器。
        let _scroll = scroll(column(
            // 把有限序列映射为公开文本节点集合。
            (0..100)
                // 为每个索引创建对应行节点。
                .map(|index| label(format!("行 {index}")))
                // 收集为 column 接受的公开 View 集合。
                .collect::<Vec<_>>(),
        ))
        // 声明固定视口高度。
        .height(300.0);

        // 构造只物化可见行的虚拟滚动节点。
        let _virtual_scroll = embed(
            // 创建公开虚拟滚动组件。
            VirtualScroll::new()
                // 声明十万行逻辑数据。
                .item_count(100_000)
                // 声明稳定固定行高。
                .item_height(24.0)
                // 按绝对索引惰性创建顺序不变的行节点。
                .render(|index| label(format!("行 {index}")).height(24.0))
                // 构建带公开 renderer 的声明节点。
                .build(),
        );
    }
}

// 隔离 layout-app-shell 围栏中的应用壳辅助函数。
mod layout_app_shell {
    // 引入文档承诺的公开布局 prelude。
    use uix_app::prelude::*;

    // 为文档中的 cards 调用提供最小拥有型业务卡片集合。
    fn cards() -> Vec<ViewNode> {
        // 返回可由响应式 Grid 直接拥有的公开节点。
        vec![label("卡片")]
    }

    // 编译应用壳、响应式栅格与可折叠侧栏组合。
    fn app_shell() -> ViewNode {
        // 横向组合侧栏与主内容区域。
        row((
            // 嵌入可折叠的固定宽度侧栏。
            embed(Sider::new(200.0).collapsible(true)),
            // 纵向组合标题栏和内容区。
            column((
                // 嵌入带标题的固定高度 Header。
                embed(Header::new(48.0).title("工作台")),
                // 嵌入持有响应式网格子树的 Content。
                embed(
                    Content::new().child(
                        // 创建公开响应式网格。
                        Grid::responsive()
                            // 声明三个从全宽收敛到四分之一宽的卡片列。
                            .cols(vec![
                                // 声明第一列断点跨度。
                                Col::new().span(24).sm(12).md(8).lg(6),
                                // 声明第二列断点跨度。
                                Col::new().span(24).sm(12).md(8).lg(6),
                                // 声明第三列断点跨度。
                                Col::new().span(24).sm(12).md(8).lg(6),
                            ])
                            // 把业务卡片集合交给 Grid 拥有。
                            .children(cards()),
                    ),
                ),
            ))
            // 让主内容区域消费侧栏之外的剩余横向空间。
            .flex_grow(1.0),
        ))
    }
}
