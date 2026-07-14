//! 组件库页面 — page_other（Transfer / Upload / DesignTokens）。

use uix::prelude::*;

use crate::common::page::PageBuilder;
use crate::demos::context::DemoCtx;

pub fn page_other(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        .block(
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
        )
        .block(
            "Upload",
            Upload::new().accept(".png,.jpg").drag(true).multiple(true),
        )
        .block(
            "QRCode / Watermark",
            row((
                column((
                    label("QRCode").font_size(11.0).fg(tk.color_text_tertiary),
                    QRCode::new("https://uix.dev"),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Watermark")
                        .font_size(11.0)
                        .fg(tk.color_text_tertiary),
                    Watermark::new("UIX Demo"),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
            ))
            .height(96.0)
            .wrap(true)
            .align(AlignItems::Start)
            .gap(24.0),
        )
        .block(
            "DesignTokens — 语义色",
            row((
                column((
                    label("Primary").font_size(11.0).fg(tk.color_text_tertiary),
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(tk.color_primary_bg)
                        .rounded(4.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Success").font_size(11.0).fg(tk.color_text_tertiary),
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(tk.color_success_bg)
                        .rounded(4.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Warning").font_size(11.0).fg(tk.color_text_tertiary),
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(tk.color_warning_bg)
                        .rounded(4.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Error").font_size(11.0).fg(tk.color_text_tertiary),
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(tk.color_error_bg)
                        .rounded(4.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Info").font_size(11.0).fg(tk.color_text_tertiary),
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(tk.color_info_bg)
                        .rounded(4.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
            ))
            .height(56.0)
            .wrap(true)
            .align(AlignItems::Start)
            .gap(24.0),
        )
        .block(
            "Fill / Border 层级",
            row((
                column((
                    label("Fill").font_size(11.0).fg(tk.color_text_tertiary),
                    Container::new()
                        .size(56.0, 20.0)
                        .bg(tk.color_fill)
                        .rounded(2.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Fill2").font_size(11.0).fg(tk.color_text_tertiary),
                    Container::new()
                        .size(56.0, 20.0)
                        .bg(tk.color_fill_secondary)
                        .rounded(2.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Fill3").font_size(11.0).fg(tk.color_text_tertiary),
                    Container::new()
                        .size(56.0, 20.0)
                        .bg(tk.color_fill_tertiary)
                        .rounded(2.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
                column((
                    label("Border").font_size(11.0).fg(tk.color_text_tertiary),
                    Container::new()
                        .size(56.0, 20.0)
                        .bg(tk.color_border)
                        .rounded(2.0),
                ))
                .align(AlignItems::Start)
                .gap(4.0),
            ))
            .height(48.0)
            .wrap(true)
            .align(AlignItems::Start)
            .gap(24.0),
        )
        .build()
}
