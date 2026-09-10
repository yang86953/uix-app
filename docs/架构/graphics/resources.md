# resources 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 graphics System 的 `resources` Module 及字体、文本布局、图像生命周期。[geometry](geometry.md)值与[backend](backend.md)上传端口由 graphics System 编排注入，Module 间不直接持有实例。导出：`FontService`、`ImageService` 及资源句柄。

> **当前实现线索**：主要位于 `src/draw/resources/`。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `FontService` | struct | face/fallback、layout、glyph cache 与 raster |
| `TextBackend` | trait | 字体后端布局与光栅协议 |
| `TextLayout` / `LineInfo` / `LineMetrics` / `PositionedGlyph` | struct | 保留源字符索引的布局结果 |
| `GlyphRaster` / `GlyphCache` | struct | 字形位图/轮廓缓存 |
| `ImageService` / `ImageSlot` | struct | 解码、槽位与 backend 资源协调 |
| `BitmapHandle` / `ImageHandle` / `FontHandle` | value | 带失效边界的资源身份 |

## 组件：FontService

布局按实际字体覆盖拆段，再统一应用换行和垂直对齐；CRLF/CR/LF、末尾空行、Tab、软断点和不可断空白保留源字符索引语义。

### 跨平台字体一致性边界

文本布局、字形选择、光栅化与绘制必须消费同一个 `FontService` 结果，图形 Adapter 不得重新选择系统字体或修改字形度量。`system-ui` 与平台默认字体表达原生外观偏好，Windows、Linux 和 macOS 可能解析到不同字体文件，因此不提供像素级一致性保证。

平台字体发现只提供有序路径候选，不能仅凭字体文件可解析或字符存在于 cmap 就发布主字体。`FontService` 必须在接纳系统正文与 CJK 回退前，使用当前 `TextBackend` 完成探针字符的布局和光栅化；映射成功但没有可见 coverage/outline 的候选必须卸载并继续下一项。该探针只约束平台自动发现的正文候选，应用显式加载的图标或符号字体仍按其窄用途保留。

要求跨系统保持文字宽度、换行、基线和像素外观一致的产品，必须随应用分发同一组已授权的正文与 CJK 字体数据，并在首次布局前按固定顺序加载；平台字体发现只能作为明确允许原生差异的兼容策略。验收证据必须记录字体文件身份、回退顺序、shaping 结果和关键文本快照，不能只比较字体族字符串。

`FontBundle` 是该约束的公开配置值：主字体与 fallback 均保存字体文件数据，fallback 声明顺序就是缺字选择顺序。`App::font_bundle` 在创建首个窗口前把完整字体包安装到唯一 `FontService`；配置存在时不得调用平台字体发现，任一字体无法解析时启动失败，不能静默改用系统字体。安装后组合根释放配置字节引用，字体数据生命周期只归 `FontService`。

```rust
use uix_app::prelude::*;

let fonts = FontBundle::from_static("Product Sans", include_bytes!("assets/ProductSans.ttf"))
    .with_static_fallback("Product CJK", include_bytes!("assets/ProductCJK.otf"));

let app = App::new().font_bundle(fonts);
```

示例名称不指定具体字体产品；仓库或应用必须另行核验所选字体的再分发许可、字形覆盖和包体预算。
`include_bytes!` 等进程期静态资产使用 `from_static` / `with_static_fallback`，字体后端可直接借用二进制只读段；运行时读取或临时生成的数据继续使用 `new` / `with_fallback`，由字体包取得共享所有权。

## 组件：ImageService

组件保存 handle 而非 texture；解码失败返回 typed error，释放或设备重建使旧 generation 失效。UI `Style` 的本地背景路径复用同一 `ImageService` 路径缓存与固有尺寸，不建立第二套图片资源所有权；加载失败由 UI 绘制适配降级为不绘制图片，同时保留已有背景色和可观察失败。

路径和图片字节都按不可信输入处理：解码前限制格式、尺寸、像素预算和总字节，规范化本地路径并服从应用授予的文件访问范围；资源服务不因路径或元数据隐式发起网络请求。错误报告不得泄漏超出诊断策略允许范围的完整用户路径。

## 所有权与模块不变量

- `FontService` / `ImageService` 拥有缓存、槽位和 backend 资源；组件句柄只标识资源及 generation，不延长服务、窗口或设备生命周期。
- 共享 CPU 资源和逐设备 GPU 资源必须分层持有；设备替换只失效对应 generation，不污染仍有效的其他窗口/设备缓存。
- 字体/图像 I/O、解码、glyph/image cache 和上传队列都有容量、取消与生命周期边界；资源服务不登记无条件逐帧工作。
- 关闭后新加载立即失败，in-flight 工作按取消契约收束；晚到解码或上传结果不得复活已释放槽位。
