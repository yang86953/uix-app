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
    // 保存音量滑块 f64 字段。
    volume: f64,
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

// 提供接收类型化反馈关闭事实的 Rust 侧回调。
#[cfg(feature = "feedback")]
fn on_feedback_closed(
    // 接收稳定 key 与实际关闭原因。
    _closed: FeedbackClosed,
) {
}

// 验证两类 keyed 反馈声明在真实公开 API 消费者中通过类型检查。
#[cfg(feature = "feedback")]
#[test]
fn feedback_declarations_compile_against_public_uix_api() {
    // 动态秒数经运行时有限非负边界验证。
    let message_seconds = 2.0_f64;
    // 展开两个零布局声明节点与类型化关闭事件。
    let _view: ViewNode = uix!(
        r#"<Container><Message key="saved" type="success" content="保存成功" duration={message_seconds} closable @close="on_feedback_closed($event)" /><Notification key="sync" title="同步完成" content="数据已更新" /></Container>"#
    );
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

// 验证声明式关键帧动画在真实公开宏消费者中通过类型检查。
#[test]
fn declarative_keyframe_animation_compiles_against_public_uix_api() {
    // 展开六类首批可动画字段与完整播放简写。
    let _view: ViewNode = crate::uix!(
        r#"
        @keyframes reveal {
          from {
            width: 24px;
            height: 32px;
            borderRadius: 2;
            opacity: 0;
            color: rgb(20, 30, 40);
            backgroundColor: rgba(10, 20, 30, 0.5);
          }
          to {
            width: 80px;
            height: 96px;
            borderRadius: 16;
            opacity: 1;
            color: rgb(240, 240, 240);
            backgroundColor: rgb(30, 40, 50);
          }
        }
        revealCard {
          animation: reveal 600ms ease-in-out 50ms 2 alternate both;
        }
        <Text class="revealCard">声明式动画</Text>
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
    // 编译真实两层导入图并验证公开宏生成结果类型。
    let _imported: ViewNode = uix!("tests/fixtures/uix_lang/imports/root.uix");
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
    // 提供 Skeleton 动态尺寸表达式。
    let skeleton_width = 240.0_f32;
    // 高度使用独立 f32 值验证公开构建器类型。
    let skeleton_height = 48.0_f32;
    // 验证形状、动态尺寸和自动化标识只依赖公开 prelude。
    let _skeleton: ViewNode = crate::uix!(
        r#"<Skeleton shape="text" width={skeleton_width} height={skeleton_height} automationId="loading" />"#
    );
    // 提供 Empty 动态描述文本。
    let empty_description = String::from("暂无数据");
    // 提供 Empty 动态图标名。
    let empty_icon = String::from("inbox");
    // 验证动态内容和公共属性只依赖公开 prelude。
    let _empty: ViewNode = crate::uix!(
        r#"<Empty description={empty_description} icon={empty_icon} width="240px" automationId="empty" />"#
    );
    // 提供 ResultView 动态标题文本。
    let result_title = String::from("操作成功");
    // 提供 ResultView 动态辅助文字。
    let result_action = String::from("返回首页");
    // 验证结果类型、动态文本和公共属性只依赖公开 prelude。
    let _result_view: ViewNode = crate::uix!(
        r#"<ResultView status="success" title={result_title} extraText={result_action} width="320px" automationId="result" />"#
    );
    // 提供 Tag 动态颜色。
    let tag_color = Color::hex("#336699");
    // 提供 Tag 动态关闭能力。
    let tag_closable = true;
    // 提供 Tag 动态可勾选能力。
    let tag_checkable = true;
    // 提供 Tag 正文插值。
    let tag_label = "已完成";
    // 验证颜色、能力、正文插值和公共属性只依赖公开 prelude。
    let _tag: ViewNode = crate::uix!(
        r#"<Tag color={tag_color} closable={tag_closable} checkable={tag_checkable} width="160px" automationId="tag">状态：{tag_label}</Tag>"#
    );
    // 提供 Card 动态标题。
    let card_title = String::from("用户信息");
    // 提供 Card 公开构建器消费的操作项字符串集合。
    let card_actions = vec!["编辑", "删除"];
    // 验证 Card 专有属性、异质子树和公共属性只依赖公开 prelude。
    let _card: ViewNode = crate::uix!(
        r#"<Card title={card_title} actions={card_actions} width="320px" automationId="profile-card"><Text>姓名</Text><Button>编辑</Button></Card>"#
    );
    // 提供 Descriptions 公开运行时消费的 Vec 类型化数据。
    let description_items = vec![
        // 创建第一项公开描述数据。
        DescriptionsItem::new("姓名", "Ada"),
        // 创建第二项公开描述数据。
        DescriptionsItem::new("角色", "管理员"),
    ];
    // 提供由 Rust 类型系统核对的动态 usize 列数。
    let description_columns = 2usize;
    // 验证类型化数据、动态列数与公共属性只依赖公开 prelude。
    let _descriptions: ViewNode = crate::uix!(
        r#"<Descriptions data={description_items} columns={description_columns} width="480px" automationId="profile-details" />"#
    );
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
        // 提供位于声明范围内的音量初始值。
        volume: 35.0,
    });
    // 提供 FormSelectItem 消费的字符串候选集合。
    let level_options = ["初级", "中级", "高级"];
    // 提供 FormRadioItem 消费的字符串候选集合。
    let channel_options = ["邮件", "短信"];
    // 提供 FormSwitchItem 消费的动态禁用状态。
    let notifications_locked = false;
    // 提供 FormSliderItem 消费的动态 f64 步长。
    let volume_step = 5.0_f64;
    // 提供返回类型化业务结果的提交回调。
    let on_profile_submit = |_profile: ProfileForm| Ok::<(), String>(());
    // 验证模型投影、规则和 submitForm 只依赖公开 prelude。
    let _form: ViewNode = crate::uix!(
        r#"<Form model={profile} @submit="on_profile_submit"><FormInputItem field="email" label="邮箱" rules="required,email" /><FormSelectItem field="level" label="等级" options={level_options} rules="required" searchable placeholder="请选择" /><FormCheckboxItem field="accepted" label="协议确认" text="我已阅读并同意" rules="required" /><FormRadioItem field="channel" label="通知渠道" options={channel_options} rules="required" vertical /><FormSwitchItem field="notifications" label="启用通知" rules="required" disabled={notifications_locked} /><FormSliderItem field="volume" label="音量" min="0" max="100" step={volume_step} /><Button type="primary" @click="submitForm">提交</Button></Form>"#
    );
}

// Image 路径构建器只在 image-codecs capability 启用时参与消费测试。
#[cfg(feature = "image-codecs")]
// 验证 Image 生成物在真实公开 API 消费者中通过类型检查。
#[test]
// 声明 Image 独立 capability 消费测试。
fn image_compiles_against_public_uix_api() {
    // 提供调用方继续持有的图片来源字符串。
    let image_src = String::from("assets/hero.png");
    // 提供调用方继续持有的替代文本。
    let image_alt = String::from("首页横幅");
    // 提供调用方继续持有的加载失败文字。
    let image_fallback = String::from("图片加载失败");
    // 提供动态固有宽度。
    let image_width = 128.0_f32;
    // 提供动态固有高度。
    let image_height = 88.0_f32;
    // 提供动态圆角。
    let image_radius = 8.0_f32;
    // 提供动态预览开关。
    let image_preview = true;
    // 提供动态缩放策略。
    let image_fit = true;
    // 提供动态延迟加载开关。
    let image_lazy = false;
    // 验证来源、文本、尺寸、运行时开关和公共属性只依赖公开 prelude。
    let _image: ViewNode = crate::uix!(
        // 使用完整 Image 声明覆盖所有已登记专有属性。
        r#"<Image src={image_src} alt={image_alt} fallback={image_fallback} width={image_width} height={image_height} radius={image_radius} preview={image_preview} fit={image_fit} lazy={image_lazy} automationId="hero-image" />"#
    );
    // 生成代码只能临时借用调用方持有的来源。
    assert_eq!(image_src, "assets/hero.png");
    // 生成代码只能临时借用调用方持有的替代文本。
    assert_eq!(image_alt, "首页横幅");
    // 生成代码只能临时借用调用方持有的失败文字。
    assert_eq!(image_fallback, "图片加载失败");
    // 编译成功即证明启用 capability 时公开 Image 消费契约闭合。
}

// ImageGroup 路径集合构建器只在 image-codecs capability 启用时参与消费测试。
#[cfg(feature = "image-codecs")]
// 验证 ImageGroup 生成物在真实公开 API 消费者中通过类型检查。
#[test]
// 声明 ImageGroup 独立 capability 消费测试。
fn image_group_compiles_against_public_uix_api() {
    // 提供调用方拥有且元素可转换为 String 的图片路径数组。
    let gallery_images = ["assets/first.png", "assets/second.png"];
    // 提供动态 usize 初始索引。
    let gallery_start_index = 1_usize;
    // 提供只观察运行时索引文本的同步回调。
    let on_gallery_change = |_index: &str| {};
    // 验证集合、初始索引、变化事件和公共属性只依赖公开 prelude。
    let _gallery: ViewNode = crate::uix!(
        // 使用完整 ImageGroup 声明覆盖所有已登记专有属性。
        r#"<ImageGroup images={gallery_images} startIndex={gallery_start_index} @change="on_gallery_change($event)" width="320px" automationId="gallery" />"#
    );
    // 可复制路径数组证明生成代码按值取得集合而不借用调用方生命周期。
    assert_eq!(gallery_images[0], "assets/first.png");
    // 编译成功即证明启用 capability 时公开 ImageGroup 消费契约闭合。
}

// 验证 List 生成物在真实公开 API 消费者中通过类型检查。
#[test]
// 声明 List 独立消费测试。
fn list_compiles_against_public_uix_api() {
    // 提供调用方拥有且元素可转换为 String 的文本数组。
    let list_items = ["视觉基线", "语义断言", "交互回归"];
    // 提供调用方继续持有的动态页首文本。
    let list_header = String::from("质量清单");
    // 提供调用方继续持有的动态加载更多文字。
    let load_more_text = String::from("加载更多");
    // 验证集合、字符串槽位和公共属性只依赖公开 prelude。
    let _list: ViewNode = crate::uix!(
        // 使用完整 List 声明覆盖所有已登记专有属性。
        r#"<List data={list_items} header={list_header} footer="共 3 项" loadMore={load_more_text} width="320px" automationId="quality-list" />"#
    );
    // 可复制文本数组证明生成代码按值取得集合而不借用调用方生命周期。
    assert_eq!(list_items[0], "视觉基线");
    // 生成代码只能临时借用调用方持有的页首文本。
    assert_eq!(list_header, "质量清单");
    // 生成代码只能临时借用调用方持有的加载更多文字。
    assert_eq!(load_more_text, "加载更多");
    // 编译成功即证明公开 List 消费契约闭合。
}

// TreeSelect 公开类型只在 tree-widgets capability 启用时参与消费测试。
#[cfg(feature = "tree-widgets")]
// 验证 tree-widgets 生成物在真实公开 API 消费者中通过类型检查。
#[test]
// 声明 TreeSelect 独立 capability 消费测试。
fn tree_select_compiles_against_public_uix_api() {
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
    // 编译成功即证明启用 capability 时保留原有 TreeSelect 消费覆盖。
}

// Spin 公开类型只在 feedback capability 启用时参与消费测试。
#[cfg(feature = "feedback")]
// 验证 feedback 生成物在真实公开 API 消费者中通过类型检查。
#[test]
// 声明 Spin 独立 capability 消费测试。
fn spin_compiles_against_public_uix_api() {
    // 提供调用方继续持有的加载提示文字。
    let loading_text = String::from("正在加载");
    // 提供动态加载状态。
    let loading = true;
    // 验证配置、提示文字、遮罩子树和公共属性只依赖公开 prelude。
    let _spin: ViewNode = crate::uix!(
        r#"<Spin spinning={loading} text={loading_text} width="240px" automationId="loading"><Text>内容</Text></Spin>"#
    );
    // 生成代码只能临时借用调用方持有的提示文字。
    assert_eq!(loading_text, "正在加载");
    // 编译成功即证明启用 capability 时公开 Spin 消费契约闭合。
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

// 验证组件内 setStyle、恢复语义与鼠标进入离开在公开 API 消费者中通过类型检查。
#[test]
fn dynamic_style_compiles_against_public_uix_api() {
    // 展开闭合目标类、内联作者覆盖与进入离开恢复组合。
    let _dynamic: ViewNode = uix!(
        r##"
        normalCard { padding: 4px; backgroundColor: #ffffff; }
        hoverCard { padding: 12px; backgroundColor: #eeeeee; }
        <Widget name="HoverCard">
          <Text class="normalCard" style="color: #1677ff;" @click="setStyle('hoverCard')" @mouseEnter="setStyle('hoverCard')" @mouseLeave="setStyle('')">Hover</Text>
        </Widget>
        <HoverCard />
        "##
    );
}

// 验证状态伪类的自动 hover 与既有 disabled/checked 事实通过公开 API 类型检查。
#[test]
fn pseudo_styles_compile_against_public_uix_api() {
    // 展开基础样式与三个只含差异字段的状态叠加层。
    let _stateful: ViewNode = uix!(
        r##"
        stateful { padding: 8px; color: #colorText; }
        stateful:hover { backgroundColor: #colorFillTertiary; }
        stateful:checked { borderColor: #colorPrimary; borderWidth: 2px; }
        stateful:disabled { opacity: 0.5; }
        <Widget name="Stateful" state="checked: bool = false, disabled: bool = false">
          <Checkbox class="stateful" text="状态" checked={checked} disabled={disabled} />
        </Widget>
        <Stateful />
        "##
    );
}

// 验证组件私有状态、共享状态、回调与组合在真实公开 API 中通过类型检查。
#[test]
fn generated_widgets_compile_against_public_uix_api() {
    // 创建 Rust 侧持有的共享 number 状态。
    let shared_count = State::new(4_f64);
    // 展开多层组件并要求最终结果为公开 ViewNode。
    let _view: ViewNode = uix_derive::__uix_view_internal!(
        r#"
        <Widget name="Counter" props="label: String, onConfirm: () -> bool" state="count: 0">
          <Column>
            <Text>{label}: {count}</Text>
            <Button @click="setState(count: count + 1)">+1</Button>
            <Button @click="onConfirm()">Confirm</Button>
          </Column>
        </Widget>
        <Widget name="Panel" props="title: String, onClose: () -> bool">
          <Counter label={title} onConfirm={onClose} />
        </Widget>
        <Widget name="SharedCounter" props="count: State<number>">
          <Column>
            <Text>Shared: {count}</Text>
            <Button @click="setState(count: count + 1)">Share +1</Button>
          </Column>
        </Widget>
        <Widget name="Message" state="text: 'A'">
          <Button @click="setState(text: 'B')">{text}</Button>
        </Widget>
        <Widget name="SearchAction" props="onSearch: (String)">
          <Button @click="onSearch('needle')">Search</Button>
        </Widget>
        <Column>
          <Panel title="Clicks" onClose={confirm_delete} />
          <SharedCounter count={shared_count} />
          <Message />
          <SearchAction onSearch={submit_search} />
        </Column>
        "#
    );
}

// 验证同一数据绑定可被多个组件重复消费（codegen 采用 clone 语义）。
#[test]
fn shared_data_bindings_compile_against_public_uix_api() {
    // 创建两个下拉共享的结构化选项绑定。
    let shared_options = vec![
        // 中文文案绑定稳定地区代码。
        SelectOption::new("中国", "cn"),
        // 另一项用于覆盖多个结构化选项。
        SelectOption::new("美国", "us"),
    ];
    // 创建第一个下拉的受控状态。
    let city_a = State::new(String::from("cn"));
    // 创建第二个下拉的受控状态。
    let city_b = State::new(String::from("us"));
    // 展开两个 Select 并要求最终结果为公开 ViewNode。
    let _view: ViewNode = uix!(
        r#"<Column><Select value={city_a} options={shared_options} width="200px" /><Select value={city_b} options={shared_options} width="200px" /></Column>"#
    );
    // 绑定必须按值 clone 消费，调用方集合保持不变。
    assert_eq!(shared_options.len(), 2);
}

// 反馈与导航 capability 启用时验证句柄位绑定与类型化私有 state。
#[cfg(all(feature = "feedback", feature = "navigation"))]
// 验证受控组件、双向绑定与显式类型注解在公开 API 消费者中通过类型检查。
#[test]
fn generated_handle_bindings_and_typed_states_compile_against_public_uix_api() {
    // 创建 Steps 类型化数据绑定。
    let steps_data = vec![Step::new("注册"), Step::new("验证")];
    // 展开由类型化私有 state 完全控制的组件并要求公开 ViewNode。
    let _view: ViewNode = uix!(
        r#"
        // 声明组件读取调用点步骤数据的外部依赖。
        <Widget name="Controlled" state="feedback_open: bool = false, checked: bool = true, low: number = 20, high: number = 80, rating: u32 = 7, current: usize = 1, page: usize = 1" external="steps_data">
          <Column>
            <Modal open={feedback_open} title="受控">
              <Button @click="setState(feedback_open: false)">关闭</Button>
            </Modal>
            <Switch checked={checked} />
            <RangeSlider value={{ start: low, end: high }} min="0" max="100" />
            <Rate value={rating} count="5" />
            <Steps current={current} items={steps_data} />
            <Pagination total="100" current={page} />
            <Button @click="setState(feedback_open: true)">打开</Button>
            <Button @click="setState(rating: rating + 1)">评分 +1</Button>
            <Button @click="setState(current: 2)">跳到第 3 步</Button>
          </Column>
        </Widget>
        <Controlled />
        "#
    );
}

// 验证语言面数据字面量与数据类型构造链在消费者中通过公开 API 类型检查。
#[cfg(all(feature = "navigation", feature = "tree-widgets"))]
#[test]
fn data_literals_and_constructor_chains_compile_against_public_uix_api() {
    // 创建级联路径与树 key 的受控状态。
    let region = State::new(CascaderValue {
        // 初始显示路径为空。
        labels: Vec::new(),
        // 初始稳定值路径为空。
        values: Vec::new(),
    });
    // 创建树选择稳定 key 状态。
    let tree_key = State::new(String::from("member"));
    // 展开覆盖八类数据类型构造、数组字面量与枚举语义值的组件。
    let _view: ViewNode = uix!(
        r#"
        // 声明组件读取调用点级联与树选择状态的外部依赖。
        <Widget name="Data" state="city: 'cn', gender: 'female', steps: usize = 1" external="region, tree_key">
          <Column>
            <Select value={city} options={[SelectOption('中国', 'cn'), SelectOption('美国', 'us')]} width="200px" />
            <Cascader value={region} options={[CascaderOption('浙江', 'zj').children([CascaderOption('杭州', 'hz')])]} width="240px" />
            <TreeSelect value={tree_key} options={[TreeNode('部门', 'dept').children([TreeNode('成员', 'member')])]} width="240px" />
            <Radio value={gender} options={['female', 'male']} width="320px" />
            <Descriptions data={[DescriptionsItem('姓名', 'Ada')]} columns="2" width="480px" />
            <Timeline items={[TimelineItem('创建').description('2026-08-01')]} pending reverse />
            <Steps current={steps} items={[Step('注册').status('finish'), Step('验证').status('process')]} />
            <Breadcrumb items={[BreadcrumbItem('首页'), BreadcrumbItem('导航').active()]} />
            <Anchor items={[AnchorItem('基础', '#basic')]} />
          </Column>
        </Widget>
        <Data />
        "#
    );
}

// 在模块级展开 items，并隔离本文件已有的同名手写 ProfileForm。
#[cfg(all(feature = "feedback", feature = "navigation", feature = "tree-widgets"))]
mod generated_semantic_items {
    // 生成语言面 <Record> 声明的模块级业务模型。
    crate::uix_items!(
        r#"
        <Record name="ProfileForm" fields="email: String, accepted: bool, volume: number" />
        <Widget name="Semantic" state="selected_date: Date = Date(2026, 8, 10), selected_time: Time = Time(14, 30), selected_color: Color = Color('#1677ff'), scroll: Point = Point(0, 0), region: CascaderValue = { labels: [], values: [] }, cities: HashSet<String> = [], profile: ProfileForm = { email: 'a@b.com', accepted: true, volume: 30.0 }">
          <Column>
            <DatePicker value={selected_date} width="200px" />
            <TimePicker value={selected_time} width="160px" />
            <ColorPicker value={selected_color} width="160px" />
            <Cascader value={region} options={[CascaderOption('浙江', 'zj')]} width="240px" />
            <Select value={cities} options={[SelectOption('中国', 'cn')]} multiple width="200px" />
            <Form model={profile} @submit="submit_semantic">
              <FormInputItem field="email" label="邮箱" rules="required" />
            </Form>
          </Column>
        </Widget>
        <Semantic />
        "#
    );
}

// 验证 uix_items! 生成的 record 与语言面语义类型状态在消费者中通过公开 API 类型检查。
#[cfg(all(feature = "feedback", feature = "navigation", feature = "tree-widgets"))]
#[test]
fn record_items_and_semantic_states_compile_against_public_uix_api() {
    // Rust 侧回调引用语言面声明的 record 类型。
    fn submit_semantic(model: generated_semantic_items::ProfileForm) -> Result<(), String> {
        // 输出关键字段供演示日志核对。
        let _ = (model.email, model.accepted, model.volume);
        // 演示提交始终成功。
        Ok(())
    }
}

// 验证 transition 生成物只依赖公开 UIX 运行时契约并可在真实消费者中类型检查。
#[test]
fn declarative_transition_compiles_against_public_uix_api() {
    // 展开同时改变透明度与背景色的 hover 状态过渡。
    let _view: ViewNode = uix!(
        r#"
        card {
          opacity: 1;
          backgroundColor: rgb(10, 20, 30);
          transition: all 250ms ease-in-out 50ms;
        }
        card:hover {
          opacity: 0.5;
          backgroundColor: rgb(30, 40, 50);
        }
        <Widget name="TransitionCard">
          <Text class="card">平滑卡片</Text>
        </Widget>
        <TransitionCard />
        "#
    );
}

// 验证通用组件新登记属性只调用公开 ButtonBuilder、ControlSize 与 Label API。
#[test]
fn general_widget_properties_compile_against_public_uix_api() {
    // 展开带组件私有布尔状态的按钮和可选择标签。
    let _view: ViewNode = uix!(
        r#"
        <Widget name="CommonProperties" state="busy: bool = true, can_select: bool = true">
          <Column>
            <Button size="large" loading={busy}>处理中</Button>
            <Label selectable={can_select}>可选择文字</Label>
          </Column>
        </Widget>
        <CommonProperties />
        "#
    );
}

// 验证 Container 第一批简写只依赖公开 View、StyleExt 与裁剪契约。
#[test]
fn container_shorthands_compile_against_public_uix_api() {
    // 展开覆盖布局、视觉、可见性和溢出裁剪的静态 Container。
    let _view: ViewNode = uix!(
        r##"<Container alignSelf="center" bg="#112233" radius="8px" w="120px" h="64px" opacity="0.75" visible overflow="hidden"><Text>内容</Text></Container>"##
    );
}

// 验证 setTheme 作为框架内置操作生成主题请求通道调用，不依赖调用方同名函数。
#[test]
fn set_theme_builtin_compiles_without_caller_function() {
    // 展开使用 setTheme 内置操作的按钮，作用域内不定义任何 setTheme 函数。
    let _view: ViewNode = uix!(r#"<Button @click="setTheme('dark')">暗色主题</Button>"#);
}

// 验证 uix_app! 生成物通过公开 App、主题与 View API 类型检查。
#[test]
fn generated_app_compiles_against_public_uix_api() {
    // 展开带完整类型主题 token 与语义引用的应用入口。
    let _app: App = uix_derive::__uix_app_internal!(
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
        <App title="消费者" size="640x480" theme="ocean" settings="settings.toml">
          <Button style="color: #colorText; backgroundColor: #backgroundColor; fontSize: #fontSize; padding: #padding;" @click="setTheme('dark')">切换</Button>
        </App>
        "#
    );
}
