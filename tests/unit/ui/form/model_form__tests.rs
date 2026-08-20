// 引入当前类型化表单实现。
use super::*;
// 引入稳定组件快照字段枚举。
use crate::ui::widget_snapshot::SnapshotFields;

// 定义测试用业务模型。
#[derive(Clone, Debug)]
struct ContactForm {
    // 保存邮箱字段值。
    email: String,
    // 保存选择字段值。
    level: String,
    // 保存协议确认状态。
    accepted: bool,
    // 保存单选组字段值。
    channel: String,
    // 保存通知开关状态。
    notifications: bool,
}

// 验证字段 key 与用户可见标签保持独立。
#[test]
fn typed_input_item_projects_explicit_label_to_form_item() {
    // 创建带初始邮箱的受控模型。
    let model = State::new(ContactForm {
        // 提供合法值，避免规则影响结构测试。
        email: "owner@example.com".to_string(),
        // 提供稳定的选择字段初值。
        level: "中级".to_string(),
        // 提供默认未勾选状态。
        accepted: false,
        // 提供稳定单选初值。
        channel: "邮件".to_string(),
        // 提供默认关闭的通知状态。
        notifications: false,
    });
    // 构建带独立展示标签的类型化字段。
    let form = Form::model(&model)
        // 字段 key 继续对应 Rust 模型成员。
        .field(
            "email",
            |value| &mut value.email,
            FormInputItem::new("email").label("电子邮箱"),
        )
        // 完成表单句柄构建。
        .build();
    // 生成真实字段 View 树。
    let view = form.view();
    // 读取首个 FormItem 的稳定快照字段。
    let fields = view.children[0].widget.snapshot_fields();
    // 快照必须同时保留字段 key 和独立标签。
    match fields {
        // 核对 FormItem 公开语义字段。
        SnapshotFields::FormItem { label, name, .. } => {
            // 标签使用文档声明的用户可见文本。
            assert_eq!(label, "电子邮箱");
            // 字段 key 仍稳定指向业务模型成员。
            assert_eq!(name, "email");
        }
        // 任何其他组件类型都表示字段壳投影失败。
        other => panic!("期望 FormItem 快照，实际为 {other:?}"),
    }
}

// 验证选择字段同样投影独立标签。
#[test]
fn typed_select_item_projects_explicit_label_to_form_item() {
    // 创建带初始等级的受控模型。
    let model = State::new(ContactForm {
        // 提供合法邮箱，保持模型完整。
        email: "owner@example.com".to_string(),
        // 提供当前选择值。
        level: "中级".to_string(),
        // 提供默认未勾选状态。
        accepted: false,
        // 提供稳定单选初值。
        channel: "邮件".to_string(),
        // 提供默认关闭的通知状态。
        notifications: false,
    });
    // 构建带选项与独立标签的类型化选择字段。
    let form = Form::model(&model)
        // 字段 key 继续对应 Rust 模型成员。
        .field(
            // 声明稳定字段 key。
            "level",
            // 投影业务模型成员。
            |value| &mut value.level,
            // 配置用户可见标签与候选项。
            FormSelectItem::new("level")
                // 设置独立标签。
                .label("等级")
                // 设置可选值。
                .options(["初级", "中级", "高级"]),
        )
        // 完成表单句柄构建。
        .build();
    // 生成真实字段 View 树。
    let view = form.view();
    // 读取首个 FormItem 的稳定快照字段。
    let fields = view.children[0].widget.snapshot_fields();
    // 快照必须同时保留字段 key 和独立标签。
    match fields {
        // 核对 FormItem 公开语义字段。
        SnapshotFields::FormItem { label, name, .. } => {
            // 标签使用声明的用户可见文本。
            assert_eq!(label, "等级");
            // 字段 key 仍稳定指向业务模型成员。
            assert_eq!(name, "level");
        }
        // 任何其他组件类型都表示字段壳投影失败。
        other => panic!("期望 FormItem 快照，实际为 {other:?}"),
    }
}

// 验证布尔字段的独立表单标签与必须勾选规则。
#[test]
fn typed_checkbox_item_projects_label_and_requires_checked_value() {
    // 创建默认未接受协议的业务模型。
    let model = State::new(ContactForm {
        // 提供合法邮箱，保持模型完整。
        email: "owner@example.com".to_string(),
        // 提供稳定选择字段，保持模型完整。
        level: "中级".to_string(),
        // 初始状态故意保持未勾选。
        accepted: false,
        // 单选字段在本测试中保持完整即可。
        channel: "邮件".to_string(),
        // 通知开关在本测试中保持默认关闭。
        notifications: false,
    });
    // 构建带独立字段标签和控件文字的类型化布尔字段。
    let form = Form::model(&model)
        // 投影稳定业务字段。
        .field(
            // 声明稳定字段 key。
            "accepted",
            // 投影 bool 模型成员。
            |value| &mut value.accepted,
            // 声明复选字段配置。
            FormCheckboxItem::new("accepted")
                // 设置 FormItem 标签。
                .field_label("协议确认")
                // 保留复选框自身文字契约。
                .label("我已阅读并同意")
                // 要求提交前完成勾选。
                .required(true),
        )
        // 完成类型化表单构建。
        .build();
    // 生成真实字段 View 并建立值绑定。
    let view = form.view();
    // 读取外层 FormItem 快照。
    let fields = view.children[0].widget.snapshot_fields();
    // 核对稳定 key 与独立标签。
    match fields {
        // 解构 FormItem 公开语义字段。
        SnapshotFields::FormItem {
            label,
            name,
            required,
            ..
        } => {
            // 外层标签使用独立字段文本。
            assert_eq!(label, "协议确认");
            // 字段 key 继续对应 bool 成员。
            assert_eq!(name, "accepted");
            // FormItem 必须公开必选状态。
            assert!(required);
        }
        // 其他组件表示字段壳投影失败。
        other => panic!("期望 FormItem 快照，实际为 {other:?}"),
    }
    // 未勾选状态必须被运行时统一校验拒绝。
    let errors = form.submit().expect_err("未勾选协议必须失败");
    // 返回稳定布尔必选错误文案。
    assert_eq!(errors[0].message(), "必须勾选");
}

// 验证开关字段投影独立标签并要求开启态。
#[test]
fn typed_switch_item_projects_label_and_requires_enabled_value() {
    // 创建通知功能尚未开启的业务模型。
    let model = State::new(ContactForm {
        // 提供合法邮箱，保持模型完整。
        email: "owner@example.com".to_string(),
        // 提供稳定等级，保持模型完整。
        level: "中级".to_string(),
        // 协议状态不参与本测试。
        accepted: true,
        // 单选字段在本测试中保持完整即可。
        channel: "邮件".to_string(),
        // 关闭状态用于触发必须开启规则。
        notifications: false,
    });
    // 构建带独立标签的类型化开关字段。
    let form = Form::model(&model)
        // 投影稳定业务字段。
        .field(
            // 声明稳定字段 key。
            "notifications",
            // 投影 bool 模型成员。
            |value| &mut value.notifications,
            // 声明开关字段配置。
            FormSwitchItem::new("notifications")
                // 设置用户可见字段标签。
                .label("启用通知")
                // 要求提交前开启该开关。
                .required(true),
        )
        // 完成类型化表单构建。
        .build();
    // 生成真实字段 View 并建立值绑定。
    let view = form.view();
    // 读取外层 FormItem 快照。
    let fields = view.children[0].widget.snapshot_fields();
    // 核对稳定 key、独立标签与必填状态。
    match fields {
        // 解构 FormItem 公开语义字段。
        SnapshotFields::FormItem {
            label,
            name,
            required,
            ..
        } => {
            // 标签使用声明的用户可见文本。
            assert_eq!(label, "启用通知");
            // 字段 key 继续对应 bool 成员。
            assert_eq!(name, "notifications");
            // FormItem 必须公开必选状态。
            assert!(required);
        }
        // 其他组件表示字段壳投影失败。
        other => panic!("期望 FormItem 快照，实际为 {other:?}"),
    }
    // 关闭状态必须被运行时统一校验拒绝。
    let errors = form.submit().expect_err("未开启通知必须失败");
    // 返回稳定开关必选错误文案。
    assert_eq!(errors[0].message(), "必须开启");
}

// 验证单选组投影独立标签并复用统一必填校验。
#[test]
fn typed_radio_item_projects_label_and_requires_selection() {
    // 创建尚未选择通知渠道的业务模型。
    let model = State::new(ContactForm {
        // 提供合法邮箱，保持模型完整。
        email: "owner@example.com".to_string(),
        // 提供稳定等级，保持模型完整。
        level: "中级".to_string(),
        // 协议状态不参与本测试。
        accepted: true,
        // 空字符串用于触发统一 required 规则。
        channel: String::new(),
        // 通知开关不参与本测试。
        notifications: false,
    });
    // 构建带独立标签与候选集合的类型化单选字段。
    let form = Form::model(&model)
        // 投影稳定业务字段。
        .field(
            // 声明稳定字段 key。
            "channel",
            // 投影 String 模型成员。
            |value| &mut value.channel,
            // 声明单选组字段配置。
            FormRadioItem::new("channel")
                // 设置用户可见字段标签。
                .label("通知渠道")
                // 设置按源码顺序排列的候选项。
                .options(["邮件", "短信"])
                // 启用统一必填规则。
                .required(true),
        )
        // 完成类型化表单构建。
        .build();
    // 生成真实字段 View 并建立值绑定。
    let view = form.view();
    // 读取外层 FormItem 快照。
    let fields = view.children[0].widget.snapshot_fields();
    // 核对稳定 key、独立标签与必填状态。
    match fields {
        // 解构 FormItem 公开语义字段。
        SnapshotFields::FormItem {
            label,
            name,
            required,
            ..
        } => {
            // 标签使用声明的用户可见文本。
            assert_eq!(label, "通知渠道");
            // 字段 key 继续对应 String 成员。
            assert_eq!(name, "channel");
            // FormItem 必须公开必填状态。
            assert!(required);
        }
        // 其他组件表示字段壳投影失败。
        other => panic!("期望 FormItem 快照，实际为 {other:?}"),
    }
    // 空字符串必须被统一 required 规则拒绝。
    let errors = form.submit().expect_err("未选择渠道必须失败");
    // 返回类型化表单既有必填错误文案。
    assert_eq!(errors[0].message(), "必填");
}
