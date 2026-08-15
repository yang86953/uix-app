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

// 验证 userSelect 四值通过公开 View 与 UserSelect 契约完成真实宏展开。
#[test]
fn user_select_styles_compile_for_real_consumer() {
    // 展开祖先禁选、后代文字选择、完整子树选择与组件默认策略。
    let _selection: ViewNode = uix::uix!(
        // 同时覆盖 Container 继承边界与三种文字组件消费路径。
        r#"
        noSelection { userSelect: none; }
        selectableGroup { userSelect: text; }
        selectAll { userSelect: all; }
        autoSelection { userSelect: auto; }
        <Container>
          <Container class="noSelection"><Text>禁止选择</Text></Container>
          <Container class="selectableGroup">
            <Text>允许选择</Text>
            <Typography class="selectAll">整组选择</Typography>
          </Container>
          <Text class="autoSelection">组件默认</Text>
        </Container>
        "#
    );
}

// 验证五种 position 模式与四边差异通过公开 View 定位契约完成真实宏展开。
#[test]
fn position_styles_compile_for_real_consumer() {
    // 展开正常流、相对、绝对、固定和粘滞定位及 hover 单边 auto。
    let _positioned: ViewNode = uix::uix!(
        // 同时覆盖基础 class、伪类差异与所有公开定位枚举。
        r#"
        staticItem { position: static; top: 99px; }
        relativeItem { position: relative; left: 4px; bottom: -2px; }
        absoluteItem { position: absolute; top: 8px; right: 12px; }
        absoluteItem:hover { top: auto; left: 3px; }
        fixedItem { position: fixed; right: 6px; bottom: 7px; }
        stickyItem { position: sticky; top: 5px; }
        <Container>
          <Text class="staticItem">正常流</Text>
          <Text class="relativeItem">相对定位</Text>
          <Text class="absoluteItem">绝对定位</Text>
          <Text class="fixedItem">固定定位</Text>
          <Text class="stickyItem">粘滞定位</Text>
        </Container>
        "#
    );
}

// 验证背景来源、定位与重复通过公开 Style 契约完成真实宏展开。
#[test]
fn background_styles_compile_for_real_consumer() {
    // 展开本地图片、主题双色渐变、二维定位与四种重复方式。
    let _backgrounds: ViewNode = uix::uix!(
        // 同时覆盖基础 class、伪类差异、主题颜色端点与公开背景枚举。
        r##"
        imageBackdrop {
          backgroundColor: #101820;
          backgroundImage: url('assets/pattern.png');
          backgroundPosition: 50% 25%;
          backgroundRepeat: repeat-x;
          opacity: 0.8;
        }
        imageBackdrop:hover {
          backgroundImage: linear-gradient(#colorPrimary, rgba(0,0,0,0.5));
          backgroundPosition: center;
          backgroundRepeat: no-repeat;
        }
        radialBackdrop {
          backgroundImage: radial-gradient(white, transparent);
          backgroundPosition: bottom right;
          backgroundRepeat: repeat-y;
        }
        repeatingBackdrop {
          backgroundImage: none;
          backgroundRepeat: repeat;
        }
        <Container>
          <Text class="imageBackdrop">图片背景</Text>
          <Text class="radialBackdrop">径向背景</Text>
          <Text class="repeatingBackdrop">显式默认值</Text>
        </Container>
        "##
    );
}

// 验证 float/clear 的推荐 Flex 与 Grid 替代写法可由真实消费者宏展开。
#[test]
fn float_clear_flex_alternatives_compile_for_real_consumer() {
    // 用行容器对齐和显式新行分组表达浮动与清除的产品意图。
    let _layout: ViewNode = uix::uix!(
        // 同时覆盖 row、主轴两端分布、flexGrow 与后继 Column 分组。
        r#"
        floatingRow { display: flex; flexDirection: row; justifyContent: space-between; }
        leadingItem { flexGrow: 1; }
        clearedRow { display: flex; flexDirection: column; }
        <Column>
          <Container class="floatingRow">
            <Text class="leadingItem">起点内容</Text>
            <Text>终点内容</Text>
          </Container>
          <Column class="clearedRow"><Text>新行内容</Text></Column>
        </Column>
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

// 验证 textDecoration 四值通过公开 Style 和 TextDecoration 契约完成真实宏展开。
#[test]
fn text_decoration_styles_compile_for_real_consumer() {
    // 展开普通文本与语义排版组件的全部闭合装饰值。
    let _decorations: ViewNode = uix::uix!(
        // 同时覆盖显式 none、下划线、上划线与删除线。
        r#"
        noDecoration { textDecoration: none; }
        underlined { textDecoration: underline; }
        overlined { textDecoration: overline; }
        struck { textDecoration: line-through; }
        <Container>
          <Text class="noDecoration">无装饰</Text>
          <Text class="underlined">下划线</Text>
          <Typography class="overlined">上划线</Typography>
          <Typography class="struck">删除线</Typography>
        </Container>
        "#
    );
}

// 验证 textAlign 四值通过公开 Style 和 TextAlign 契约完成真实宏展开。
#[test]
fn text_align_styles_compile_for_real_consumer() {
    // 展开普通文本与语义排版组件的全部闭合对齐值。
    let _alignments: ViewNode = uix::uix!(
        // 同时覆盖显式左、右、居中与段落两端对齐。
        r#"
        leftAligned { width: 160px; textAlign: left; }
        rightAligned { width: 160px; textAlign: right; }
        centered { width: 160px; textAlign: center; }
        justified { width: 160px; textAlign: justify; }
        <Container>
          <Text class="leftAligned">左对齐</Text>
          <Text class="rightAligned">右对齐</Text>
          <Typography class="centered">居中</Typography>
          <Typography class="justified">两端对齐段落文本</Typography>
        </Container>
        "#
    );
}

// 验证 fontWeight 关键字与数值通过公开 Style 和 FontWeight 契约完成真实宏展开。
#[test]
fn font_weight_styles_compile_for_real_consumer() {
    // 展开普通文本与语义排版组件的关键字、边界和精确区间值。
    let _font_weights: ViewNode = uix::uix!(
        // 同时覆盖显式 normal、bold、区间内任意整数与上下边界。
        r#"
        normalWeight { fontWeight: normal; }
        boldWeight { fontWeight: bold; }
        minimumWeight { fontWeight: 100; }
        exactWeight { fontWeight: 550; }
        maximumWeight { fontWeight: 900; }
        <Container>
          <Text class="normalWeight">常规</Text>
          <Text class="boldWeight">粗体</Text>
          <Text class="minimumWeight">最细</Text>
          <Typography class="exactWeight">精确数值</Typography>
          <Typography class="maximumWeight">最粗</Typography>
        </Container>
        "#
    );
}

// 验证 fontFamily 有序列表通过公开 Style 和 FontFamily 契约完成真实宏展开。
#[test]
fn font_family_styles_compile_for_real_consumer() {
    // 展开普通文本与语义排版组件的已加载名称和通用族回退列表。
    let _font_families: ViewNode = uix::uix!(
        // 同时覆盖带空格名称、缺失名称、裸名称与通用字体族。
        r#"
        registeredFallback { fontFamily: 'Missing Family', 'Segoe UI', Arial, sans-serif; }
        genericFallback { fontFamily: monospace; }
        <Container>
          <Text class="registeredFallback">已加载字体回退</Text>
          <Typography class="genericFallback">系统字体回退</Typography>
        </Container>
        "#
    );
}
