// 引入公开 UIX 消费者 prelude。
use uix::prelude::*;

// 验证阶段 5 四类语法通过真实公开 API 类型检查。
#[test]
fn phase5_language_features_compile_for_real_consumer() {
    // 键盘处理器接收公开 KeyCode 值。
    fn on_key(_key: KeyCode) {}
    // 变更处理器接收现有字符串载荷借用。
    fn on_change(_value: &str) {}
    // 先生成文档内 record 类型，供同一测试中的 View 宏引用。
    uix::uix_items!(
        r#"
        <Record name="Address" fields="city: String" />
        <Record name="Profile" fields="name: String, address: Address, tags: Vec<String>, scores: Vec<number>, addresses: Vec<Address>, alias: Option<String>" />
        <Component name="PhaseFive" props="label: String = '默认值'" external="on_key, on_change" state="ready: bool = false, fallback: bool = true, input: String = '初始', names: Vec<String> = ['甲', '乙'], scores: Vec<number> = [1, 2.5], addresses: Vec<Address> = [{ city: '上海' }], selected: Option<String> = Some('active'), profile: Profile = { name: '用户', address: { city: '北京' }, tags: ['稳定'], scores: [3], addresses: [{ city: '深圳' }], alias: Some('owner') }">
          <Column>
            <Button @keyDown="on_key($event.key)">{label}</Button>
            <Input value={input} @change="on_change($event.value)" />
            <If {ready}><Text>主分支</Text></If>
            <ElseIf {fallback}><Text>后备分支</Text></ElseIf>
            <Else><Text>兜底分支</Text></Else>
          </Column>
        </Component>
        <PhaseFive />
        "#
    );
    // 展开同一文档并要求最终值满足公开 ViewNode 契约。
    let _view: ViewNode = uix::uix!(
        r#"
        <Record name="Address" fields="city: String" />
        <Record name="Profile" fields="name: String, address: Address, tags: Vec<String>, scores: Vec<number>, addresses: Vec<Address>, alias: Option<String>" />
        <Component name="PhaseFive" props="label: String = '默认值'" external="on_key, on_change" state="ready: bool = false, fallback: bool = true, input: String = '初始', names: Vec<String> = ['甲', '乙'], scores: Vec<number> = [1, 2.5], addresses: Vec<Address> = [{ city: '上海' }], selected: Option<String> = Some('active'), profile: Profile = { name: '用户', address: { city: '北京' }, tags: ['稳定'], scores: [3], addresses: [{ city: '深圳' }], alias: Some('owner') }">
          <Column>
            <Button @keyDown="on_key($event.key)">{label}</Button>
            <Input value={input} @change="on_change($event.value)" />
            <If {ready}><Text>主分支</Text></If>
            <ElseIf {fallback}><Text>后备分支</Text></ElseIf>
            <Else><Text>兜底分支</Text></Else>
          </Column>
        </Component>
        <PhaseFive />
        "#
    );
}
