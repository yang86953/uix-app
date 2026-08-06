//! manifest 条目分组：导航、图表与其他组件（条目 72-92）。
//! 由 manifest.rs 以 `#[macro_use]` 引入，展开为分组库存数组字面量，由 manifest.rs 合并为总库存。

// 导航、图表与其他组件（条目 72-92）。
macro_rules! manifest_cases_extra {
    () => {
    &[    widget(
        "breadcrumb",
        "Breadcrumb",
        "导航",
        &["default", "active", "long-content"],
    ),
    widget(
        "dropdown",
        "Dropdown",
        "导航",
        &["closed", "open", "items", "focus"],
    ),
    widget(
        "menu",
        "Menu",
        "导航",
        &[
            "horizontal",
            "vertical",
            "active",
            "disabled",
            "hover",
            "focus",
        ],
    ),
    widget(
        "nav-item",
        "NavItem",
        "导航",
        &["default", "selected", "compact", "hover", "focus"],
    ),
    composite(
        "nav-group",
        "NavGroup",
        "导航",
        &["items", "selected", "hover", "focus"],
    ),
    composite(
        "navigation",
        "Navigation",
        "导航",
        &["default", "selected", "compact", "hover", "focus"],
    ),
    widget(
        "pagination",
        "Pagination",
        "导航",
        &["default", "active", "disabled-edge", "focus"],
    ),
    widget(
        "steps",
        "Steps",
        "导航",
        &["wait", "process", "finish", "error"],
    ),
    widget(
        "tabs",
        "Tabs",
        "导航",
        &["top", "bottom", "active", "focus"],
    ),
    widget(
        "bar-chart",
        "BarChart",
        "图表与其他",
        &["positive-data", "labels", "grid", "compact"],
    ),
    widget(
        "line-chart",
        "LineChart",
        "图表与其他",
        &["positive-data", "dots", "grid", "compact"],
    ),
    widget(
        "pie-chart",
        "PieChart",
        "图表与其他",
        &["pie", "donut", "legend", "compact"],
    ),
    widget(
        "qr-code",
        "QRCode",
        "图表与其他",
        &["default", "dense-data", "compact"],
    ),
    widget(
        "transfer",
        "Transfer",
        "图表与其他",
        &["source", "selected", "target", "empty", "focus"],
    ),
    widget(
        "upload",
        "Upload",
        "图表与其他",
        &[
            "drop-zone",
            "drag",
            "file-list",
            "image-preview",
            "manual",
            "pending",
            "uploading",
            "done",
            "error",
            "focus",
        ],
    ),
    widget(
        "watermark",
        "Watermark",
        "图表与其他",
        &["default", "long-content", "compact"],
    ),
    provider(
        "config-provider",
        "ConfigProvider",
        &["inherited-size", "component-override", "empty-render"],
    ),
    provider(
        "locale-provider",
        "LocaleProvider",
        &["zh-cn", "en-us", "fallback"],
    ),
    widget(
        "chart-placeholder",
        "ChartPlaceholder",
        "图表与其他",
        &["empty", "title", "subtitle", "responsive"],
    ),
    widget(
        "float-button-group",
        "FloatButtonGroup",
        "通用",
        &["collapsed", "hover", "focus", "lucide-icons"],
    ),
    widget(
        "image-group",
        "ImageGroup",
        "数据展示",
        &["gallery", "thumbnails", "open", "navigation", "focus"],
    )
]
    };
}
