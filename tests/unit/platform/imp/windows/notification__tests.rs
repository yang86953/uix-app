// 引入被测转义组件。
use super::escape_notification_xml;

// 所有 XML 元字符都必须被实体化。
#[test]
fn notification_text_is_xml_escaped() {
    // 同时覆盖五种 XML 元字符和普通 Unicode 文本。
    let escaped = escape_notification_xml("UIX <&> \"完成\" '好'");
    // 断言载荷不能注入标签或属性边界。
    assert_eq!(escaped, "UIX &lt;&amp;&gt; &quot;完成&quot; &apos;好&apos;");
}
