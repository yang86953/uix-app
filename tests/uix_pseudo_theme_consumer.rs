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
