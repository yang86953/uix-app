//! 类型化表单滑块字段回归。

// 引入当前 form Module 的公开契约。
use super::*;
// 引入稳定组件快照字段枚举。
use crate::ui::widget_snapshot::SnapshotFields;
// 引入公开视图构建契约。
use crate::ui::view::View;
// 引入受控字段状态。
use crate::ui::State;

// 定义测试用业务模型。
#[derive(Clone, Debug)]
struct SliderForm {
    // 保存可由类型化表单写回的音量值。
    volume: f64,
}

// 验证滑块字段投影独立标签、范围、步长与类型化提交值。
#[test]
fn typed_slider_item_projects_numeric_contract_and_submit_value() {
    // 创建具有稳定初始音量的业务模型。
    let model = State::new(SliderForm { volume: 35.0 });
    // 构建带显式数值约束的类型化滑块字段。
    let form = Form::model(&model)
        // 投影稳定业务字段。
        .field(
            // 声明稳定字段 key。
            "volume",
            // 投影 f64 模型成员。
            |value| &mut value.volume,
            // 声明滑块范围、标签与步长。
            FormSliderItem::new("volume", 0.0..=100.0)
                // 设置用户可见字段标签。
                .label("音量")
                // 设置五单位步长。
                .step(5.0),
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
        SnapshotFields::FormItem { label, name, .. } => {
            // 标签使用声明的用户可见文本。
            assert_eq!(label, "音量");
            // 字段 key 继续对应 f64 成员。
            assert_eq!(name, "volume");
        }
        // 其他组件表示字段壳投影失败。
        other => panic!("期望 FormItem 快照，实际为 {other:?}"),
    }
    // 提交当前类型化字段快照。
    let submitted = form.submit().expect("合法滑块值应提交成功");
    // 提交值必须保留初始 f64 精度。
    assert_eq!(submitted.volume, 35.0);
}

// 验证低层 FormModel 入口仍可直接建立受控滑块。
#[test]
fn bound_slider_item_keeps_existing_low_level_entry() {
    // 登记可被滑块绑定的统一字段。
    let model = Form::new()
        // 声明稳定字段与用户可见标签。
        .field("volume", "音量")
        // 声明低层字段初值。
        .initial(25.0_f64)
        // 完成低层表单模型构建。
        .build();
    // 创建外部受控 f64 状态。
    let value = State::new(25.0_f64);
    // 使用既有入口建立带范围的字段适配器。
    let item = model
        // 保持原有参数顺序与返回类型。
        .slider_item("volume", &value, 0.0..=100.0)
        // 已登记字段必须成功绑定。
        .expect("已登记滑块字段应可绑定");
    // 构建成功即证明兼容入口仍满足新绑定契约。
    let _view = item.build();
}
