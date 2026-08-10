// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 表示消费者循环中的拥有所有权数据项。
#[derive(Clone)]
struct ConsumerItem {
    // 保存稳定行身份。
    id: u64,
    // 保存可插值名称。
    name: String,
}

// 定义 UIX Form 首批使用的类型化业务模型。
#[derive(Clone)]
struct ProfileForm {
    // 保存邮箱字段。
    email: String,
    // 保存等级选择字段。
    level: String,
    // 保存协议确认布尔字段。
    accepted: bool,
    // 保存通知渠道单选组字段。
    channel: String,
    // 保存通知开关布尔字段。
    notifications: bool,
}

// 提供无事件参数的 Rust 侧回调。
fn on_confirm() {}

// 提供带 bool 返回值的组件回调 prop。
fn confirm_delete() -> bool {
    // 返回确定的确认结果。
    true
}

// 提供带 String 参数的组件回调 prop。
fn submit_search(
    // 接收组件传出的拥有所有权字符串。
    _query: String,
) {
}

// 提供接收点击坐标的 Rust 侧回调。
fn on_point(
    // 接收点击 x 坐标。
    _x: f32,
    // 接收点击 y 坐标。
    _y: f32,
) {
}

// 验证核心生成物在真实 uix 公开 API 消费者中通过类型检查。
#[test]
fn generated_view_compiles_against_public_uix_api() {
    // 提供文本插值变量。
    let count = 2_i32;
    // 提供表达式布尔属性变量。
    let busy = false;
    // 提供 If 条件变量。
    let visible = true;
    // 提供 For 数据源。
    let items = vec![ConsumerItem {
        // 设置稳定行身份。
        id: 7,
        // 设置文本插值内容。
        name: "Alpha".to_string(),
    }];
    // 在真实 crate 中展开并要求结果类型为公开 ViewNode。
    let _view: ViewNode = uix_derive::__uix_view_internal!(
        r#"
        <Column gap="8px">
          <Text fontSize="heading2" key={count}>Count: {count}</Text>
          <Button type="primary" disabled={busy} @click="on_confirm()">Save</Button>
          <Button @click="on_point($event.x, $event.y)">Point</Button>
          <If {visible}><Icon name="star" size="16px" /></If>
          <For {item} {index} in {items} key={item.id}>
            <Container direction="row"><Text>{index}</Text><Text>{item.name}</Text></Container>
          </For>
        </Column>
    "#
    );
}

// 验证 crate 根与 prelude 导出的公开 uix! 内嵌及文件入口。
#[test]
fn public_uix_macro_compiles_inline_and_file_entries() {
    // 通过 prelude 导入的公开宏展开内嵌 UIX 源码。
    let _inline: ViewNode = uix!(r#"<Text>Inline entry</Text>"#);
    // 以调用 crate 清单目录为基准读取并展开 .uix 文件。
    let _file: ViewNode = uix!("tests/fixtures/uix_lang/public_entry.uix");
    // 通过 crate 根路径再次验证宏导出。
    let _root_export: ViewNode = crate::uix!(r#"<Button>Root export</Button>"#);
    // 验证通用组件文档中的 Label 别名真实生成。
    let _label: ViewNode = crate::uix!(r#"<Label>Documented label</Label>"#);
    // 提供 Divider 动态文字属性的消费者值。
    let divider_text = "Documented divider".to_string();
    // 提供 Divider 动态虚线属性的消费者值。
    let divider_dashed = true;
    // 验证 Divider 专有属性与公共样式只依赖公开 prelude API。
    let _divider: ViewNode = crate::uix!(
        r#"<Divider text={divider_text} dashed={divider_dashed} direction="vertical" margin="8px" />"#
    );
    // 提供 Space 动态间距属性的消费者值。
    let space_gap = 12_f32;
    // 提供 Space 动态换行属性的消费者值。
    let space_wrap = true;
    // 验证 Space 专有属性、有序子节点与公共样式只依赖公开 prelude API。
    let _space: ViewNode = crate::uix!(
        r#"<Space direction="vertical" gap={space_gap} wrap={space_wrap} margin="4px"><Label>First</Label><Button>Second</Button></Space>"#
    );
    // 提供 Typography 动态层级属性的 u8 消费者值。
    let typography_level = 2_u8;
    // 提供 Typography 动态标记属性的消费者值。
    let typography_marked = true;
    // 提供 Typography 动态复制属性的消费者值。
    let typography_copyable = true;
    // 验证 Typography 专有属性、主题语义色与文本插值只依赖公开 prelude API。
    let _typography: ViewNode = crate::uix!(
        r#"<Typography level={typography_level} type="danger" mark={typography_marked} underline="true" copyable={typography_copyable}>Level {typography_level}</Typography>"#
    );
    // 验证 ThemeToggle 叶组件只依赖公开 prelude API。
    let _theme_toggle: ViewNode = crate::uix!(r#"<ThemeToggle />"#);
    // 提供 ButtonGroup 子按钮的动态禁用状态。
    let group_disabled = false;
    // 验证 ButtonGroup 保留 Button 专有属性、事件与公共样式的公开消费路径。
    let _button_group: ViewNode = crate::uix!(
        r#"<ButtonGroup margin="4px"><Button type="primary" disabled={group_disabled} @click="on_confirm()">Left</Button><Button style="padding: 8px;">Middle</Button><Button type="ghost">Right</Button></ButtonGroup>"#
    );
    // 提供 WindowControl 动态显示属性的消费者值。
    let show_maximize = false;
    // 验证 WindowControl 组合与动态布尔属性只依赖公开 prelude API。
    let _window_controls: ViewNode = crate::uix!(
        r#"<WindowControl showMinimize="true" showMaximize={show_maximize} showClose="false" />"#
    );
    // 验证 Row/Col 24 栅格只依赖公开 GridBuilder 与 Col 配置。
    let _responsive_grid: ViewNode = crate::uix!(
        r#"<Row gap="16px"><Col span={12} lg={8}><Text>Left</Text></Col><Col span={12} lg={16}><Text>Right</Text></Col></Row>"#
    );
    // 验证 Grid/Col 显式轨道、独立间距与跨轨道样式只依赖公开 prelude。
    let _explicit_grid: ViewNode = crate::uix!(
        r#"<Grid columns="1fr 200px auto" rows="auto" gap="16px" colGap="8px" rowGap="4px" padding="12px"><Col span={2}><Text>Wide</Text></Col><Col gridColumnSpan={1} gridRowSpan={1}><Text>Narrow</Text></Col></Grid>"#
    );
    // 提供 ScrollView 双向绑定的滚动位置状态。
    let scroll_offset = State::new(Point::new(0.0, 0.0));
    // 验证 ScrollView 方向、偏移绑定与单一内容 View 只依赖公开 prelude。
    let _scroll_view: ViewNode = crate::uix!(
        r#"<ScrollView direction="both" offset={scroll_offset} height="320px"><Column><Text>First</Text><Text>Second</Text></Column></ScrollView>"#
    );
    // 提供 VirtualScroll 拥有所有权的数据快照来源。
    let virtual_items = vec![ConsumerItem {
        // 设置稳定业务身份。
        id: 11,
        // 设置可插值行内容。
        name: "Virtual".to_string(),
    }];
    // 验证 VirtualScroll 的数据、行高、索引、key 与公共样式只依赖公开 prelude。
    let _virtual_scroll: ViewNode = crate::uix!(
        r#"<VirtualScroll data={virtual_items} rowHeight="32px" item={item} height="160px"><For {item} {index} in {virtual_items} key={item.id}><Container direction="row"><Text>{index}</Text><Text>{item.name}</Text></Container></For></VirtualScroll>"#
    );
    // 提供 Affix 声明式滚动位置状态。
    let affix_scroll_y = State::new(24_f32);
    // 提供 Affix 动态顶部偏移。
    let affix_offset_top = 8_f32;
    // 验证 Affix 的状态读取、动态偏移、唯一内容与公共样式只依赖公开 prelude。
    let _affix: ViewNode = crate::uix!(
        r#"<Affix offsetTop={affix_offset_top} scrollY={affix_scroll_y} width="320px"><Container><Text>Sticky toolbar</Text></Container></Affix>"#
    );
    // 提供 BackTop 双向滚动位置状态。
    let back_top_scroll_y = State::new(480_f32);
    // 验证 BackTop 的阈值、状态回写入口与公共样式只依赖公开 prelude。
    let _back_top: ViewNode =
        crate::uix!(r#"<BackTop threshold="400" scrollY={back_top_scroll_y} margin="8px" />"#);
    // 提供 Splitter 动态初始比例。
    let splitter_ratio = 0.3_f32;
    // 验证 Splitter 的双面板、方向、比例与公共样式只依赖公开 prelude。
    let _splitter: ViewNode = crate::uix!(
        r#"<Splitter direction="horizontal" defaultRatio={splitter_ratio} height="320px"><Container><Text>Navigation</Text></Container><Container><Text>Content</Text></Container></Splitter>"#
    );
    // 验证完整应用布局壳、布尔简写和嵌套区域只依赖公开 prelude。
    let _app_layout: ViewNode = crate::uix!(
        r#"<Layout direction="row"><Sider collapsible><Text>Navigation</Text></Sider><Layout><Header><Text>Header</Text></Header><Content><Text>Content</Text></Content><Footer><Text>Footer</Text></Footer></Layout></Layout>"#
    );
    // 提供 Input 双向绑定的文本状态。
    let input_name = State::new(String::from("Belldandy"));
    // 提供读取 Change 文本载荷的消费者回调。
    let on_name_change = |_value: &str| {};
    // 验证 Input 的密码类型、绑定、禁用状态、事件载荷与公共样式只依赖公开 prelude。
    let _input: ViewNode = crate::uix!(
        r#"<Input value={input_name} placeholder="请输入名称" type="password" disabled @change="on_name_change($event)" width="240px" />"#
    );
    // 提供 InputNumber 泛型整数双向绑定状态。
    let input_number = State::new(50_i32);
    // 验证数值绑定、范围、步长、精度和公共样式只依赖公开 prelude。
    let _input_number: ViewNode = crate::uix!(
        r#"<InputNumber value={input_number} min="0" max="100" step="5" precision="0" width="160px" />"#
    );
    // 提供 InputGroup 文本双向绑定状态。
    let grouped_amount = State::new(String::from("12.50"));
    // 验证前后附加文本、状态绑定和公共样式只依赖公开 prelude。
    let _input_group: ViewNode = crate::uix!(
        r#"<InputGroup addonBefore="¥" value={grouped_amount} addonAfter="元" width="200px" />"#
    );
    // 提供 Slider f64 双向绑定状态。
    let slider_volume = State::new(35_f64);
    // 验证范围、步长、状态绑定和公共样式只依赖公开 prelude。
    let _slider: ViewNode =
        crate::uix!(r#"<Slider value={slider_volume} min="0" max="100" step="5" width="240px" />"#);
    // 提供 RangeSlider 起点双向绑定状态。
    let range_start = State::new(20_f64);
    // 提供 RangeSlider 终点双向绑定状态。
    let range_end = State::new(80_f64);
    // 验证对象区间绑定、范围、步长和公共样式只依赖公开 prelude。
    let _range_slider: ViewNode = crate::uix!(
        r#"<RangeSlider value={{ start: range_start, end: range_end }} min="0" max="100" step="5" width="260px" />"#
    );
    // 提供 Rate u32 双向绑定状态；启用半星时一个单位代表半星。
    let rating = State::new(7_u32);
    // 验证星数、半星、状态绑定和公共样式只依赖公开 prelude。
    let _rate: ViewNode =
        crate::uix!(r#"<Rate value={rating} count="5" allowHalf width="180px" />"#);
    // 提供 Checkbox 双向勾选状态。
    let accepted = State::new(true);
    // 验证标签、禁用、勾选绑定和公共样式只依赖公开 prelude。
    let _checkbox: ViewNode = crate::uix!(
        r#"<Checkbox checked={accepted} text="同意协议" disabled="false" width="180px" />"#
    );
    // 提供 Switch 双向开关状态。
    let switch_enabled = State::new(false);
    // 提供 Switch 动态禁用状态。
    let switch_locked = false;
    // 验证禁用、开关绑定和公共样式只依赖公开 prelude。
    let _switch: ViewNode = crate::uix!(
        r#"<Switch checked={switch_enabled} disabled={switch_locked} width="180px" />"#
    );
    // 提供 Radio 当前选中值的双向状态。
    let gender = State::new(String::from("female"));
    // 提供 Radio 可迭代字符串选项数据。
    let gender_options = vec![String::from("female"), String::from("male")];
    // 验证选项数据、值绑定和公共样式只依赖公开 prelude。
    let _radio: ViewNode = crate::uix!(
        r#"<Radio value={gender} options={gender_options} width="240px" automationId="gender" />"#
    );
    // 提供 Segmented 当前选中视图的双向状态。
    let view_mode = State::new(String::from("grid"));
    // 提供 Segmented 可迭代字符串选项数据。
    let view_options = vec![String::from("grid"), String::from("list")];
    // 验证构造器选项、值绑定和公共样式只依赖公开 prelude。
    let _segmented: ViewNode = crate::uix!(
        r#"<Segmented value={view_mode} options={view_options} width="240px" automationId="view-mode" />"#
    );
    // 提供 Select 当前稳定值的单选双向状态。
    let city = State::new(String::from("cn"));
    // 提供 Select 独立显示文案与稳定值的结构化选项。
    let city_options = vec![
        // 中文文案绑定稳定地区代码。
        SelectOption::new("中国", "cn"),
        // 另一项用于覆盖多个结构化选项。
        SelectOption::new("美国", "us"),
    ];
    // 提供动态可搜索开关。
    let can_search = true;
    // 验证结构化选项、单选绑定、搜索和占位文本只依赖公开 prelude。
    let _select: ViewNode = crate::uix!(
        r#"<Select value={city} options={city_options} searchable={can_search} placeholder="请选择城市" width="240px" />"#
    );
    // 提供 Select 多选稳定值集合状态。
    let cities = State::new(::std::collections::HashSet::<String>::new());
    // 为多选消费者准备独立拥有所有权的结构化选项。
    let multiple_city_options = vec![
        // 首个多选项保留独立身份。
        SelectOption::new("中国", "cn"),
        // 第二个多选项保留独立身份。
        SelectOption::new("美国", "us"),
    ];
    // 验证 multiple 简写在编译期约束集合状态类型。
    let _multiple_select: ViewNode = crate::uix!(
        r#"<Select value={cities} options={multiple_city_options} multiple width="240px" />"#
    );
    // 提供 Cascader 当前完整选中路径的双向状态。
    let region = State::new(CascaderValue {
        // 初始显示路径保持为空。
        labels: Vec::new(),
        // 初始稳定值路径保持为空。
        values: Vec::new(),
    });
    // 提供 Cascader 层级选项树。
    let region_options = vec![
        // 省级根项包含城市叶项。
        CascaderOption::new("浙江", "zj").children(vec![
            // 城市叶项提交完整路径。
            CascaderOption::new("杭州", "hz"),
        ]),
    ];
    // 验证选项树、路径绑定和公共样式只依赖公开 prelude。
    let _cascader: ViewNode = crate::uix!(
        r#"<Cascader value={region} options={region_options} width="240px" automationId="region" />"#
    );
    // 提供 TreeSelect 当前选中节点的稳定 key 状态。
    let selected_tree_key = State::new(String::from("member"));
    // 提供与 Tree 共享数据结构的层级节点集合。
    let tree_nodes = vec![
        // 根节点包含一个可选成员叶节点。
        TreeNode::new("部门", "dept").children(vec![
            // 叶节点的稳定 key 与初始受控状态一致。
            TreeNode::new("成员", "member"),
        ]),
    ];
    // 验证节点树、稳定 key 绑定和公共属性只依赖公开 prelude。
    let _tree_select: ViewNode = crate::uix!(
        r#"<TreeSelect value={selected_tree_key} options={tree_nodes} width="240px" automationId="tree" />"#
    );
    // 提供 AutoComplete 当前输入与选中结果的受控文本。
    let query = State::new(String::from("ap"));
    // 提供可迭代的字符串候选集合。
    let suggestions = vec![String::from("apple"), String::from("apricot")];
    // 验证候选、文本绑定、占位文本和公共属性只依赖公开 prelude。
    let _autocomplete: ViewNode = crate::uix!(
        r#"<AutoComplete value={query} options={suggestions} placeholder="请输入" width="240px" automationId="search" />"#
    );
    // 提供 Mentions 包含正文和活动提及的受控完整文本。
    let mention_text = State::new(String::from("请联系 @a"));
    // 提供提及输入使用的字符串候选集合。
    let mention_suggestions = vec![String::from("alice"), String::from("adam")];
    // 验证候选、完整文本绑定、占位文本和公共属性只依赖公开 prelude。
    let _mentions: ViewNode = crate::uix!(
        r#"<Mentions value={mention_text} suggestions={mention_suggestions} placeholder="提及成员" width="240px" automationId="mentions" />"#
    );
    // 提供 DatePicker 当前选中日期的受控状态。
    let selected_date = State::new(Date::new(2026, 8, 10));
    // 验证日期绑定、选择粒度和公共属性只依赖公开 prelude。
    let _date_picker: ViewNode = crate::uix!(
        r#"<DatePicker value={selected_date} mode="month" width="240px" automationId="date" />"#
    );
    // 提供 DateRangePicker 当前范围的起点日期状态。
    let range_start = State::new(Date::new(2026, 8, 1));
    // 终点状态与起点状态保持独立所有权。
    let range_end = State::new(Date::new(2026, 8, 31));
    // 验证结构化双端点绑定和公共属性只依赖公开 prelude。
    let _date_range_picker: ViewNode = crate::uix!(
        r#"<DateRangePicker value={{ start: range_start, end: range_end }} width="280px" automationId="date-range" />"#
    );
    // 提供 TimePicker 当前选中时间的受控状态。
    let selected_time = State::new(Time::new(14, 30));
    // 验证时间绑定和公共属性只依赖公开 prelude。
    let _time_picker: ViewNode =
        crate::uix!(r#"<TimePicker value={selected_time} width="200px" automationId="time" />"#);
    // 提供 ColorPicker 当前选中颜色的受控状态。
    let selected_color = State::new(Color::hex("#1677ff"));
    // 验证颜色绑定和公共属性只依赖公开 prelude。
    let _color_picker: ViewNode =
        crate::uix!(r#"<ColorPicker value={selected_color} width="200px" automationId="color" />"#);
    // 提供 Form 绑定的类型化业务状态。
    let profile = State::new(ProfileForm {
        // 提供通过邮箱规则的初始值。
        email: "owner@example.com".to_string(),
        // 提供已包含在候选集合中的初始等级。
        level: "中级".to_string(),
        // 提供已勾选的协议确认状态。
        accepted: true,
        // 提供已包含在单选组候选集合中的初始值。
        channel: "邮件".to_string(),
        // 提供已开启的通知初始值。
        notifications: true,
    });
    // 提供 FormSelectItem 消费的字符串候选集合。
    let level_options = ["初级", "中级", "高级"];
    // 提供 FormRadioItem 消费的字符串候选集合。
    let channel_options = ["邮件", "短信"];
    // 提供 FormSwitchItem 消费的动态禁用状态。
    let notifications_locked = false;
    // 提供返回类型化业务结果的提交回调。
    let on_profile_submit = |_profile: ProfileForm| Ok::<(), String>(());
    // 验证模型投影、规则和 submitForm 只依赖公开 prelude。
    let _form: ViewNode = crate::uix!(
        r#"<Form model={profile} @submit="on_profile_submit"><FormInputItem field="email" label="邮箱" rules="required,email" /><FormSelectItem field="level" label="等级" options={level_options} rules="required" searchable placeholder="请选择" /><FormCheckboxItem field="accepted" label="协议确认" text="我已阅读并同意" rules="required" /><FormRadioItem field="channel" label="通知渠道" options={channel_options} rules="required" vertical /><FormSwitchItem field="notifications" label="启用通知" rules="required" disabled={notifications_locked} /><Button type="primary" @click="submitForm">提交</Button></Form>"#
    );
}

// 验证已映射内联样式在真实公开 API 消费者中通过类型检查。
#[test]
fn mapped_inline_styles_compile_against_public_uix_api() {
    // 展开覆盖布局、Grid、盒模型、状态色、阴影和可见性的代表样式。
    let _styled: ViewNode = uix!(
        // 只使用样式参考中当前标记为已映射的值。
        r##"<Column style="display: grid; gridTemplateColumns: 1fr 120px; gridTemplateRows: auto; gridColumnGap: 8px; gridRowGap: 4px; flexWrap: true; justifyContent: space-between; alignItems: center; margin: 1px 2px 3px 4px; padding: 8px 12px; borderColor: #336699; borderWidth: 1px; borderRadius: 6px; width: 320px; height: auto; backgroundColor: rgba(10,20,30,0.5); backgroundColor:hover: #fff; opacity: 0.9; overflow: hidden; boxShadow: 0 2px 4px rgba(0,0,0,0.2); visible: true;"><Text style="color: blue; fontSize: heading3; flexGrow: 1; flexShrink: 0; alignSelf: flex-start; gridColumnSpan: 2; gridRowSpan: 1;">Styled</Text></Column>"##
    );
}

// 验证样式类继承和内联优先级在公开宏消费者中通过类型检查。
#[test]
fn style_classes_compile_against_public_uix_api() {
    // 展开父类、子类、多 class 与内联覆盖组合。
    let _styled: ViewNode = uix!(
        // 后声明类覆盖前一类，内联样式具有最高优先级。
        r##"baseCard { padding: 4px; color: red; } elevatedCard { extends: baseCard; boxShadow: 0 2px 4px rgba(0,0,0,0.2); } blueCard { color: blue; } <Text class="elevatedCard blueCard" style="padding: 8px;">Class style</Text>"##
    );
}

// 验证组件私有状态、共享状态、回调与组合在真实公开 API 中通过类型检查。
#[test]
fn generated_components_compile_against_public_uix_api() {
    // 创建 Rust 侧持有的共享 number 状态。
    let shared_count = State::new(4_f64);
    // 展开多层组件并要求最终结果为公开 ViewNode。
    let _view: ViewNode = uix_derive::__uix_view_internal!(
        r#"
        <Component name="Counter" props="label: String, onConfirm: () -> bool" state="count: 0">
          <Column>
            <Text>{label}: {count}</Text>
            <Button @click="setState(count: count + 1)">+1</Button>
            <Button @click="onConfirm()">Confirm</Button>
          </Column>
        </Component>
        <Component name="Panel" props="title: String, onClose: () -> bool">
          <Counter label={title} onConfirm={onClose} />
        </Component>
        <Component name="SharedCounter" props="count: State<number>">
          <Column>
            <Text>Shared: {count}</Text>
            <Button @click="setState(count: count + 1)">Share +1</Button>
          </Column>
        </Component>
        <Component name="Message" state="text: 'A'">
          <Button @click="setState(text: 'B')">{text}</Button>
        </Component>
        <Component name="SearchAction" props="onSearch: (String)">
          <Button @click="onSearch('needle')">Search</Button>
        </Component>
        <Column>
          <Panel title="Clicks" onClose={confirm_delete} />
          <SharedCounter count={shared_count} />
          <Message />
          <SearchAction onSearch={submit_search} />
        </Column>
        "#
    );
}
