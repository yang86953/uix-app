// 引入公开 UIX 消费者预lude。
use uix::prelude::*;

// 验证状态伪类只依赖公开 View、State 与事件 API。
#[test]
fn pseudo_styles_compile_for_real_consumer() {
    // 展开基础样式与三个状态差异层。
    let _stateful: ViewNode = uix::uix!(
        r##"
        stateful { padding: 8px; color: #colorText; }
        stateful:hover { backgroundColor: #colorFillTertiary; }
        stateful:checked { borderColor: #colorPrimary; borderWidth: 2px; }
        stateful:disabled { opacity: 0.5; }
        <Component name="Stateful" state="checked: bool = false, disabled: bool = false">
          <Checkbox class="stateful" text="状态" checked={checked} disabled={disabled} />
        </Component>
        <Stateful />
        "##
    );
}

// 验证完整类型主题 token 与颜色、数值引用通过公开 App API。
#[test]
fn full_theme_tokens_compile_for_real_consumer() {
    // 展开代表全部运行时字段类型的主题声明。
    let _app: App = uix::uix_app!(
        r#"
        @theme ocean {
          primaryColor: #336699;
          backgroundColor: #101820;
          colorText: #eef4ff;
          colorBgRaised: rgba(20,30,40,0.9);
          fontFamily: 'Segoe UI';
          fontSize: 16px;
          padding: 18px;
          borderRadius: 9px;
          motionDurationFast: 0.12;
          motionEasingDefault: 'linear';
          screenMD: 800px;
          boxShadow: 0 2px 8px rgba(0,0,0,0.1), 0 4px 16px rgba(0,0,0,0.08), 0 8px 24px rgba(0,0,0,0.06);
          isDark: true;
        }
        <App title="消费者" size="640x480" theme="ocean">
          <Button style="color: #colorText; backgroundColor: #backgroundColor; fontSize: #fontSize; padding: #padding;">主题</Button>
        </App>
        "#
    );
}

// 验证静态与伪状态 transform 通过公开 View 仿射入口完成真实宏展开。
#[test]
fn transform_styles_compile_for_real_consumer() {
    // 展开基础旋转缩放与悬停平移倾斜的完整样式级联。
    let _transformed: ViewNode = uix::uix!(
        r#"
        transformed { transform: rotate(15deg) scale(1.1); transformOrigin: top left; }
        transformed:hover { transform: translate(4px, -2px) skew(3deg); transformOrigin: 25% 12px 0; }
        <Text class="transformed">变换</Text>
        "#
    );
}

// 验证 cursor 基础与伪状态值通过公开 View 光标入口完成真实宏展开。
#[test]
fn cursor_styles_compile_for_real_consumer() {
    // 展开基础手形与悬停文字光标的完整样式级联。
    let _cursor: ViewNode = uix::uix!(
        r#"
        cursorTarget { cursor: pointer; }
        cursorTarget:hover { cursor: text; }
        <Text class="cursorTarget">光标</Text>
        "#
    );
}

// 验证五种 borderStyle 值通过公开 Style 与 BorderStyle 契约完成真实宏展开。
#[test]
fn border_style_values_compile_for_real_consumer() {
    // 展开基础虚线、悬停实线及其余三个线型的完整样式级联。
    let _border_styles: ViewNode = uix::uix!(
        // 保留所有规范关键字以验证真实消费者可见的枚举路径。
        r##"
        bordered { borderColor: #colorBorder; borderWidth: 3px; borderStyle: dashed; }
        bordered:hover { borderStyle: solid; }
        noBorder { borderWidth: 3px; borderStyle: none; }
        dottedBorder { borderWidth: 3px; borderStyle: dotted; }
        doubleBorder { borderWidth: 6px; borderStyle: double; }
        <Container>
          <Text class="bordered">虚线</Text>
          <Text class="noBorder">无绘制</Text>
          <Text class="dottedBorder">圆点</Text>
          <Text class="doubleBorder">双线</Text>
        </Container>
        "##
    );
}

// 验证 lineHeight 倍率与像素值通过公开 Style 和 LineHeight 契约完成真实宏展开。
#[test]
fn line_height_styles_compile_for_real_consumer() {
    // 展开 Text 倍率行高与 Typography 固定像素行高。
    let _line_heights: ViewNode = uix::uix!(
        // 同时覆盖普通文本标签与语义排版组件的适配路径。
        r#"
        readable { lineHeight: 1.5; }
        fixedLine { lineHeight: 24px; }
        <Container>
          <Text class="readable">倍率行高</Text>
          <Typography class="fixedLine">固定行高</Typography>
        </Container>
        "#
    );
}
