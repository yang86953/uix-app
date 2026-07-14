//! 图表页面 — Bar / Line / Pie 全覆盖（使用 使用.md 新 API）。

use uix::prelude::*;

use crate::common::page::PageBuilder;
use crate::demos::context::DemoCtx;

pub fn page_charts(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;
    let inner_w = crate::common::page::INNER_W;

    PageBuilder::new(tk)
        .gap()
        .section("BarChart — 月活跃用户")
        .push(
            BarChart::new()
                .data(&[
                    ("Jan", 420.0),
                    ("Feb", 380.0),
                    ("Mar", 530.0),
                    ("Apr", 490.0),
                    ("May", 620.0),
                    ("Jun", 580.0),
                ])
                .size(inner_w, 180.0),
        )
        .section("LineChart — 多系列")
        .push(
            LineChart::new()
                .series(vec![
                    Series::new(
                        "收入",
                        &[
                            ("Jan", 120.0),
                            ("Feb", 150.0),
                            ("Mar", 130.0),
                            ("Apr", 170.0),
                            ("May", 200.0),
                            ("Jun", 190.0),
                        ],
                    ),
                    Series::new(
                        "支出",
                        &[
                            ("Jan", 80.0),
                            ("Feb", 90.0),
                            ("Mar", 110.0),
                            ("Apr", 100.0),
                            ("May", 120.0),
                            ("Jun", 140.0),
                        ],
                    ),
                ])
                .size(inner_w, 200.0),
        )
        .section("PieChart — 实心 / 环形")
        .push(
            row((
                PieChart::new()
                    .data(&[
                        ("Chrome", 65.0),
                        ("Firefox", 15.0),
                        ("Safari", 10.0),
                        ("Other", 10.0),
                    ])
                    .size(180.0),
                PieChart::new()
                    .data(&[("A", 40.0), ("B", 30.0), ("C", 20.0), ("D", 10.0)])
                    .size(180.0)
                    .donut(0.45),
            ))
            .gap(16.0),
        )
        .build()
}
