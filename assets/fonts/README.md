# 字体资源

UIX 不内置完整正文字体。应用使用系统字体发现，或显式注入 FontBundle。
`lucide.ttf` 是图标字体，遵循 ISC，不能用于正文。
`tests/fixtures/fonts/uix-test-body.ttf` 是独立 OFL 测试子集，不作为运行期默认字体。
具体归属和完整许可见 [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md)。

无窗口 AgentWorkspace 没有可加载正文字体时返回 typed 错误；窗口 App 保留已有系统字体后备行为。
