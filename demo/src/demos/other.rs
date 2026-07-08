//! 组件库页面 — page_other（Transfer / Upload / DesignTokens）。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

pub fn page_other(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        .section("Transfer")
        .push(labeled_row(
            tk,
            200.0,
            "Transfer",
            Transfer::new().source(vec![
                TransferItem {
                    key: "a".into(),
                    title: "选项 A".into(),
                    selected: false,
                },
                TransferItem {
                    key: "b".into(),
                    title: "选项 B".into(),
                    selected: false,
                },
                TransferItem {
                    key: "c".into(),
                    title: "选项 C".into(),
                    selected: true,
                },
            ]),
        ))
        .section("Upload")
        .push(labeled_row(
            tk,
            120.0,
            "Upload",
            Upload::new().accept(".png,.jpg").drag(true).multiple(true),
        ))
        .section("QRCode / Watermark")
        .push(demo_row(80.0).child(QRCode::new("https://uix.dev")))
        .push(demo_row(40.0).child(Watermark::new("UIX Demo")))
        .section("DesignTokens — 语义色")
        .push(
            demo_row(44.0)
                .child(
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(tk.color_primary_bg)
                        .rounded(4.0),
                )
                .child(
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(tk.color_success_bg)
                        .rounded(4.0),
                )
                .child(
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(tk.color_warning_bg)
                        .rounded(4.0),
                )
                .child(
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(tk.color_error_bg)
                        .rounded(4.0),
                )
                .child(
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(tk.color_info_bg)
                        .rounded(4.0),
                ),
        )
        .section("Fill / Border 层级")
        .push(
            demo_row(24.0)
                .child(
                    Container::new()
                        .size(50.0, 18.0)
                        .bg(tk.color_fill)
                        .rounded(2.0),
                )
                .child(
                    Container::new()
                        .size(50.0, 18.0)
                        .bg(tk.color_fill_secondary)
                        .rounded(2.0),
                )
                .child(
                    Container::new()
                        .size(50.0, 18.0)
                        .bg(tk.color_fill_tertiary)
                        .rounded(2.0),
                )
                .child(
                    Container::new()
                        .size(50.0, 18.0)
                        .bg(tk.color_border)
                        .rounded(2.0),
                ),
        )
        .build()
}
