//! 其他组件演示 —— Transfer / Upload / QRCode / Watermark / canvas / DesignTokens / Accessibility。

use uix::prelude::*;

use crate::common::page::PageBuilder;
use crate::demos::context::DemoCtx;

/// 语义色色块 + 标签。
fn color_swatch(tk: &DesignTokens, label_text: &str, bg: impl Into<Color>) -> impl View {
    column((
        column(())
            .width(72.0)
            .height(28.0)
            .bg(bg)
            .radius(tk.border_radius_sm),
        label(label_text)
            .font_size(11.0)
            .fg(tk.color_text_secondary),
    ))
    .gap(4.0)
    .align(AlignItems::Center)
}

/// Fill / Border 层级色块 + 标签。
fn fill_swatch(tk: &DesignTokens, label_text: &str, bg: impl Into<Color>) -> impl View {
    column((
        column(()).width(56.0).height(20.0).bg(bg).radius(2.0),
        label(label_text).font_size(10.0).fg(tk.color_text_tertiary),
    ))
    .gap(2.0)
    .align(AlignItems::Center)
}

pub fn page_other(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        // ── Transfer ──
        .section("Transfer")
        .push(
            Transfer::new()
                .source(vec![
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
                ])
                .target(vec![]),
        )
        // ── Upload ──
        .section("Upload")
        .push(
            Upload::new()
                .accept(".jpg,.png,.pdf")
                .max_size(10 * 1024 * 1024)
                .on_select(|files| {
                    // Vec<SelectedFile>
                }),
        )
        // ── QRCode / Watermark ──
        .section("QRCode / Watermark")
        .push(
            row((
                column((
                    label("QRCode").font_size(11.0).fg(tk.color_text_secondary),
                    QRCode::new("https://uix.dev").size(96.0),
                ))
                .gap(4.0),
                column((
                    label("Watermark")
                        .font_size(11.0)
                        .fg(tk.color_text_secondary),
                    Watermark::new("UIX Demo"),
                ))
                .gap(4.0),
            ))
            .gap(24.0),
        )
        // ── canvas — 轻量绘制 ──
        .section("canvas — 轻量绘制")
        .push(
            row((
                canvas(48.0, 48.0, move |frame, ctx| {
                    let center = frame.center();
                    ctx.fill_circle(center.x, center.y, 20.0, Color::from_rgb(64, 150, 255));
                }),
                canvas(48.0, 48.0, move |frame, ctx| {
                    let center = frame.center();
                    ctx.fill_circle(center.x, center.y, 20.0, Color::from_rgb(82, 196, 26));
                }),
                canvas(48.0, 48.0, move |frame, ctx| {
                    let center = frame.center();
                    ctx.fill_circle(center.x, center.y, 20.0, Color::from_rgb(250, 173, 20));
                }),
                canvas(48.0, 48.0, move |frame, ctx| {
                    let center = frame.center();
                    ctx.fill_circle(center.x, center.y, 20.0, Color::from_rgb(255, 77, 79));
                }),
            ))
            .gap(12.0),
        )
        // ── DesignTokens — 语义色 ──
        .section("DesignTokens — 语义色")
        .push(
            row((
                color_swatch(tk, "Primary", tk.color_primary_bg),
                color_swatch(tk, "Success", tk.color_success_bg),
                color_swatch(tk, "Warning", tk.color_warning_bg),
                color_swatch(tk, "Error", tk.color_error_bg),
                color_swatch(tk, "Info", tk.color_info_bg),
            ))
            .gap(12.0),
        )
        // ── Fill / Border 层级 ──
        .section("Fill / Border 层级")
        .push(
            row((
                fill_swatch(tk, "Fill", tk.color_fill),
                fill_swatch(tk, "Fill2", tk.color_fill_secondary),
                fill_swatch(tk, "Fill3", tk.color_fill_tertiary),
                fill_swatch(tk, "Border", tk.color_border),
            ))
            .gap(12.0),
        )
        // ── 无障碍 — Accessibility ──
        .section("无障碍 — Accessibility")
        .push(
            column((
                label("带有无障碍标注的按钮：")
                    .font_size(12.0)
                    .fg(tk.color_text_secondary),
                button("提交表单")
                    .primary()
                    .role(AccessibilityRole::Button)
                    .aria(AriaAttribute::Label, "提交表单"),
            ))
            .gap(8.0),
        )
        .build()
}
