// 复用被测私有选择策略与具体 API 身份。
use super::{GraphicsApi, GraphicsSelection};

#[test]
// 验证自动配置不会重新进入具体 API 枚举。
fn automatic_config_parses_as_private_selection_strategy() {
    // 空配置保持历史上的自动选择语义。
    assert_eq!("".parse(), Ok(GraphicsSelection::Automatic));
    // 显式 auto 文本同样只构造私有策略。
    assert_eq!("auto".parse(), Ok(GraphicsSelection::Automatic));
    // 自动策略诊断名称保持稳定。
    assert_eq!(GraphicsSelection::Automatic.to_string(), "auto");
}

#[test]
// 验证具体配置只产生显式 API 请求。
fn concrete_config_parses_as_explicit_api_selection() {
    // 常见 D3D11 别名归一为同一个具体 API。
    assert_eq!(
        "Direct3D-11".parse(),
        Ok(GraphicsSelection::Explicit(GraphicsApi::D3d11))
    );
    // OpenGL ES 简写归一为同一个具体 API。
    assert_eq!(
        "gles".parse(),
        Ok(GraphicsSelection::Explicit(GraphicsApi::OpenGlEs))
    );
    // 显式策略诊断只展示所请求的具体 API。
    assert_eq!(
        GraphicsSelection::Explicit(GraphicsApi::OpenGlEs).to_string(),
        "opengles"
    );
}
