// 导入应用、主题、视图与 RichText 公开门面。
use uix::prelude::*;

// 返回仓库内确定性验收图片的绝对路径。
fn acceptance_image_path() -> String {
    // 从当前演示包目录解析仓库共享图片资源。
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        // 跨过 demo 子目录并进入仓库资源目录。
        .join("../../assets/images/acceptance-1461-before.png")
        // 在 Windows 下保留可交给 ImageService 的完整路径。
        .to_string_lossy()
        // 返回独立拥有的路径字符串。
        .into_owned()
}

// 返回稳定不存在的图片路径以覆盖失败与 alt fallback。
fn missing_image_path() -> String {
    // 使用与成功资源相同的仓库根定位方式。
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        // 指向仓库中有意不存在的文件名。
        .join("../../assets/images/missing-rich-text-inline-image.png")
        // 转换为 Windows 文件系统路径字符串。
        .to_string_lossy()
        // 返回独立拥有的失败路径。
        .into_owned()
}

// 构造可复用的内联图片公开段。
fn image_segment(
    // 接收无障碍、选择复制与 fallback 共用的替代文本。
    alt: &str,
    // 接收本地资源路径。
    src: String,
    // 接收可选逻辑宽度覆盖。
    width: Option<f32>,
    // 接收可选逻辑高度覆盖。
    height: Option<f32>,
    // 接收保持比例或拉伸策略。
    fit: bool,
    // 接收可选圆角半径。
    radius: Option<f32>,
) -> RichTextSegment {
    // 返回完整图片原子契约。
    RichTextSegment::Image {
        // 保存可读替代文本。
        alt: alt.to_owned(),
        // 保存本地路径。
        src,
        // 保存宽度覆盖。
        width,
        // 保存高度覆盖。
        height,
        // 保存适配策略。
        fit,
        // 保存圆角策略。
        radius,
    }
}

// 构造带稳定标题、边框和内容宽度的验收卡片。
fn acceptance_card(
    // 接收截图内可读的契约标题。
    title: &'static str,
    // 接收卡片外部宽度。
    card_width: f32,
    // 接收 RichText 的实际宽度约束。
    content_width: f32,
    // 接收待验收段列表。
    segments: Vec<RichTextSegment>,
) -> ViewNode {
    // 局部卡片必须保持固有高度，避免 flex 剩余空间影响矩阵。
    column_fit((
        // 标明当前卡片覆盖的验收条件。
        label(title),
        // 使用真实 RichText 组件承载图片原子。
        embed(
            // 构造富文本组件。
            RichText::new()
                // 安装当前验收内容。
                .content(segments)
                // 开启真实选择生命周期供既有交互契约继续生效。
                .selectable(true)
                // 使用十六像素默认字号形成二十四像素占位行高。
                .font_size(16.0),
        )
        // 固定内容宽度以覆盖宽窄折行矩阵。
        .width(content_width),
    ))
    // 标题与内容之间保留稳定间距。
    .gap(12.0)
    // 卡片四周提供清晰边界。
    .padding(18.0)
    // 固定卡片宽度便于两列比较。
    .width(card_width)
    // 禁止父列默认 Stretch 覆盖卡片显式宽度。
    .align_self(AlignItems::Start)
    // 使用主题容器背景验证明暗主题下的 fallback 对比。
    .bg(ColorValue::neutral(NeutralRole::BgContainer))
    // 使用主题次级边框明确裁剪范围。
    .border(1.0, ColorValue::neutral(NeutralRole::BorderSecondary))
    // 使用统一圆角区分卡片边界与图片圆角。
    .radius(8.0)
}

// 构造高度覆盖、固有比例与跨字号垂直对齐样本。
fn baseline_segments() -> Vec<RichTextSegment> {
    // 读取确定性成功图片路径。
    let image = acceptance_image_path();
    // 返回小字、图片和大字共享一行的段序列。
    vec![
        // 图片前使用十四像素正文。
        RichTextSegment::Text {
            // 标明前侧字号。
            content: "14px 前文  ".into(),
            // 设置小于默认值的字号。
            style: RichTextStyle {
                // 使用十四像素。
                font_size: Some(14.0),
                // 其余样式沿用默认值。
                ..RichTextStyle::default()
            },
        },
        // 仅覆盖高度，让固有比例和当前设备比例共同决定宽度。
        image_segment("成功图片", image, None, Some(72.0), true, Some(10.0)),
        // 图片后使用二十八像素正文。
        RichTextSegment::Text {
            // 标明后侧字号。
            content: "  28px 后文".into(),
            // 设置显著更大的字号。
            style: RichTextStyle {
                // 使用二十八像素。
                font_size: Some(28.0),
                // 其余样式沿用默认值。
                ..RichTextStyle::default()
            },
        },
    ]
}

// 构造窄宽度下的多图片原子折行样本。
fn wrapping_segments() -> Vec<RichTextSegment> {
    // 读取确定性成功图片路径。
    let image = acceptance_image_path();
    // 返回文本与三个图片原子组成的窄宽序列。
    vec![
        // 使用短前缀验证图片与相邻文字共同参与折行。
        RichTextSegment::Text {
            // 提供可见前缀。
            content: "窄宽：".into(),
            // 沿用默认样式。
            style: RichTextStyle::default(),
        },
        // 第一张固定尺寸图片留在首行。
        image_segment("图片一", image.clone(), Some(82.0), Some(54.0), true, Some(6.0)),
        // 插入可选择空格作为原子间距。
        RichTextSegment::Text {
            // 使用两个 ASCII 空格。
            content: "  ".into(),
            // 沿用默认样式。
            style: RichTextStyle::default(),
        },
        // 第二张图片在剩余宽度允许时保持同一行。
        image_segment("图片二", image.clone(), Some(82.0), Some(54.0), false, None),
        // 插入第二个稳定间距。
        RichTextSegment::Text {
            // 使用两个 ASCII 空格。
            content: "  ".into(),
            // 沿用默认样式。
            style: RichTextStyle::default(),
        },
        // 第三张图片必须作为不可拆分原子换到下一行。
        image_segment("图片三", image, Some(82.0), Some(54.0), true, Some(18.0)),
    ]
}

// 构造 fit、fill 与圆角对比样本。
fn fit_segments() -> Vec<RichTextSegment> {
    // 读取确定性成功图片路径。
    let image = acceptance_image_path();
    // 返回两个等尺寸但策略不同的图片原子。
    vec![
        // 保持比例并使用十二像素圆角。
        image_segment("fit 保持比例", image.clone(), Some(150.0), Some(88.0), true, Some(12.0)),
        // 使用可见文字间距隔开两种策略。
        RichTextSegment::Text {
            // 输出策略分隔文字。
            content: "  fit / fill  ".into(),
            // 沿用默认样式。
            style: RichTextStyle::default(),
        },
        // 拉伸填满并保持直角。
        image_segment("fill 拉伸", image, Some(150.0), Some(88.0), false, None),
    ]
}

// 构造失败 fallback、代码与链接相邻样本。
fn fallback_segments() -> Vec<RichTextSegment> {
    // 返回代码、失败图片和链接共享一行的段序列。
    vec![
        // 图片前使用真实代码段。
        RichTextSegment::Code {
            // 说明左侧元素类型。
            content: "code_before".into(),
        },
        // 使用可见间距避免元素边界粘连。
        RichTextSegment::Text {
            // 插入两个空格。
            content: "  ".into(),
            // 沿用默认样式。
            style: RichTextStyle::default(),
        },
        // 不存在的资源必须稳定显示主题占位、边框与完整 alt。
        image_segment(
            // alt 同时用于截图、选择复制与无障碍。
            "加载失败：只显示 alt，不泄漏 src",
            // 使用确定性失败路径。
            missing_image_path(),
            // 固定足够宽度显示 fallback。
            Some(210.0),
            // 固定高度形成稳定失败占位。
            Some(58.0),
            // fallback 仍保存 fit 策略。
            true,
            // 使用八像素圆角验证失败边框裁剪。
            Some(8.0),
        ),
        // 使用可见间距隔开右侧链接。
        RichTextSegment::Text {
            // 插入两个空格。
            content: "  ".into(),
            // 沿用默认样式。
            style: RichTextStyle::default(),
        },
        // 图片后使用真实可提交链接段。
        RichTextSegment::Link {
            // 提供可见链接文字。
            content: "link_after".into(),
            // 使用稳定测试 URL。
            url: "https://example.test/rich-text-image".into(),
        },
    ]
}

// 构造完整 RichText 内联图片真窗验收视图。
fn acceptance_view() -> ViewNode {
    // 左列覆盖固有比例、跨字号与窄宽多图折行。
    let left = column_fit((
        // 高度覆盖样本触发固有比例和设备比例换算。
        acceptance_card(
            // 标明基线和 DPI 相关契约。
            "A. 高度覆盖 + 固有比例 + 当前显示器 DPI；14px / 图片 / 28px 垂直居中",
            // 使用宽卡片容纳跨字号单行。
            520.0,
            // 内容宽度保留卡片内边距。
            484.0,
            // 安装跨字号样本。
            baseline_segments(),
        ),
        // 窄宽样本强制第三张图片原子换行。
        acceptance_card(
            // 标明不可拆分折行与裁剪契约。
            "B. 264px 窄内容区：多图片作为不可拆分原子折行",
            // 直接收窄父卡片，让 RichText 收到真实有限交叉轴约束。
            300.0,
            // 卡片内边距扣除后保留二百六十四像素内容区。
            264.0,
            // 安装多图片样本。
            wrapping_segments(),
        ),
    ))
    // 左列卡片之间保留稳定间距。
    .gap(18.0);
    // 右列覆盖 fit/fill、圆角和失败 fallback。
    let right = column_fit((
        // 同尺寸图片用于比较保持比例与拉伸策略。
        acceptance_card(
            // 标明两种图片绘制策略。
            "C. 150×88：fit 圆角保持比例 / fill 直角拉伸",
            // 使用四百二十像素卡片。
            420.0,
            // 内容宽度容纳两个图片和策略标签。
            384.0,
            // 安装 fit/fill 对比样本。
            fit_segments(),
        ),
        // 失败图片与代码、链接相邻，覆盖 fallback 可读性和邻接几何。
        acceptance_card(
            // 标明失败状态和相邻元素契约。
            "D. 失败 alt fallback + Code / Link 相邻；明暗主题均应可读",
            // 使用四百二十像素卡片。
            420.0,
            // 内容宽度迫使必要时按原子边界折行。
            384.0,
            // 安装稳定失败样本。
            fallback_segments(),
        ),
    ))
    // 右列卡片之间保留稳定间距。
    .gap(18.0);
    // 页面使用标题、判定说明和两列矩阵。
    column((
        // 提供截图内可读的验收主题。
        label("UIX RichText Markdown 内联图片真窗验收").font_size(24.0),
        // 说明静态截图覆盖范围与动态契约边界。
        label("截图判定：成功位图、原子折行、fit/fill、圆角、失败 alt、主题可读性；选择复制与 capability gate 由自动化契约证明。"),
        // 并排安装两列验收卡片。
        row((left, right)).gap(20.0),
    ))
    // 页面各区域保持稳定间距。
    .gap(20.0)
    // 为窗口边界提供统一留白。
    .padding(28.0)
    // 使用主题布局背景突出图片占位与卡片边界。
    .bg(ColorValue::neutral(NeutralRole::BgLayout))
    // 填满窗口客户区。
    .flex_grow(1.0)
}

// 启动支持明暗主题切换参数的独立验收应用。
fn main() {
    // 从稳定命令行参数读取暗色主题请求。
    let dark = std::env::args().any(|argument| argument == "--dark");
    // 根据验收模式选择窗口标题。
    let title = if dark {
        // 暗色窗口使用独立标题便于精确定位。
        "UIX RichText 内联图片视觉验收（暗色）"
    } else {
        // 亮色窗口使用独立标题便于精确定位。
        "UIX RichText 内联图片视觉验收（亮色）"
    };
    // 根据验收模式选择完整 Ant Design 主题。
    let theme = if dark {
        // 暗色验收使用权威暗色令牌。
        Theme::antd_dark()
    } else {
        // 亮色验收使用权威亮色令牌。
        Theme::antd_light()
    };
    // 使用公开 App 组合根启动唯一 UI 管线。
    App::new()
        // 设置可被自动化精确定位的窗口标题。
        .title(title)
        // 固定逻辑尺寸以容纳完整四项矩阵。
        .size(1040, 720)
        // 明确选择 Windows D3D11 参考后端。
        .graphics_backend(GraphicsBackend::Direct3D11)
        // 安装当前明暗主题。
        .theme(theme)
        // 根工厂构造真实 RichText 图片验收视图。
        .root(acceptance_view)
        // 进入现有原生窗口事件循环。
        .run();
}
