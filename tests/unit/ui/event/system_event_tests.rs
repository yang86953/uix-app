// ui/event 事件系统专项测试。
// 覆盖 SystemEvent 构造/kind 映射/克隆、SemanticEvent 载荷与传播控制、
// HandlerTable 注册/分派顺序/一次性/谓词/移除语义。

// 引入被测的事件类型与分发表。
use super::*;
// 引入基础几何与输入类型。
use crate::core::{ComponentId, Point};
use crate::platform::windowing::{KeyCode, KeyMod, MouseButton};

// ── SystemEvent kind 映射与克隆 ───────────────────────────────────────────

// 带载荷指针事件的 kind 必须映射到对应变体。
#[test]
fn pointer_events_map_to_pointer_kinds() {
    // 指针按下。
    let event = SystemEvent::PointerDown {
        pos: Point::new(1.0, 2.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(event.kind(), SystemEventKind::PointerDown);
    // 指针双击。
    let event = SystemEvent::PointerDoubleClick {
        pos: Point::new(1.0, 2.0),
        button: MouseButton::Right,
        mods: KeyMod::SHIFT,
    };
    assert_eq!(event.kind(), SystemEventKind::PointerDoubleClick);
    // 指针抬起。
    let event = SystemEvent::PointerUp {
        pos: Point::new(1.0, 2.0),
        button: MouseButton::Middle,
        mods: KeyMod::NONE,
    };
    assert_eq!(event.kind(), SystemEventKind::PointerUp);
    // 指针移动（无按钮载荷）。
    let event = SystemEvent::PointerMove {
        pos: Point::new(3.0, 4.0),
        mods: KeyMod::CTRL,
    };
    assert_eq!(event.kind(), SystemEventKind::PointerMove);
    // 滚轮。
    let event = SystemEvent::Wheel {
        pos: Point::new(0.0, 0.0),
        delta: Point::new(0.0, -1.0),
    };
    assert_eq!(event.kind(), SystemEventKind::Wheel);
}

// 键盘事件的 kind 必须映射到对应变体。
#[test]
fn key_events_map_to_key_kinds() {
    // 按下。
    let event = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::ALT,
    };
    assert_eq!(event.kind(), SystemEventKind::KeyDown);
    // 抬起。
    let event = SystemEvent::KeyUp {
        key: KeyCode::Escape,
        mods: KeyMod::NONE,
    };
    assert_eq!(event.kind(), SystemEventKind::KeyUp);
    // 文本输入。
    let event = SystemEvent::TextInput {
        text: "abc".to_owned(),
    };
    assert_eq!(event.kind(), SystemEventKind::TextInput);
}

// 剪贴板与输入法事件的 kind 必须映射到对应变体。
#[test]
fn clipboard_and_ime_events_map_to_kinds() {
    // 剪贴板三件套。
    assert_eq!(SystemEvent::Copy.kind(), SystemEventKind::Copy);
    assert_eq!(SystemEvent::Cut.kind(), SystemEventKind::Cut);
    assert_eq!(
        SystemEvent::Paste {
            text: "x".to_owned()
        }
        .kind(),
        SystemEventKind::Paste
    );
    // 输入法三件套。
    assert_eq!(
        SystemEvent::ImeCompositionStart.kind(),
        SystemEventKind::ImeCompositionStart
    );
    assert_eq!(
        SystemEvent::ImeCompositionUpdate {
            text: "拼".to_owned()
        }
        .kind(),
        SystemEventKind::ImeCompositionUpdate
    );
    assert_eq!(
        SystemEvent::ImeCompositionEnd {
            text: "拼".to_owned()
        }
        .kind(),
        SystemEventKind::ImeCompositionEnd
    );
}

// 焦点与窗口事件的 kind 必须映射到对应变体。
#[test]
fn focus_and_window_events_map_to_kinds() {
    // 焦点进出。
    assert_eq!(SystemEvent::FocusIn.kind(), SystemEventKind::FocusIn);
    assert_eq!(SystemEvent::FocusOut.kind(), SystemEventKind::FocusOut);
    // 指针进出。
    assert_eq!(SystemEvent::PointerEnter.kind(), SystemEventKind::PointerEnter);
    assert_eq!(SystemEvent::PointerLeave.kind(), SystemEventKind::PointerLeave);
    // 主题与区域变更。
    assert_eq!(
        SystemEvent::ThemeChanged { is_dark: true }.kind(),
        SystemEventKind::ThemeChanged
    );
    assert_eq!(
        SystemEvent::LocaleChanged {
            locale: "zh-CN".to_owned()
        }
        .kind(),
        SystemEventKind::LocaleChanged
    );
    // 窗口尺寸与状态。
    assert_eq!(
        SystemEvent::Resize {
            width: 800.0,
            height: 600.0
        }
        .kind(),
        SystemEventKind::Resize
    );
    assert_eq!(SystemEvent::WindowMaximize.kind(), SystemEventKind::WindowMaximize);
    assert_eq!(SystemEvent::WindowMinimize.kind(), SystemEventKind::WindowMinimize);
    assert_eq!(SystemEvent::WindowRestore.kind(), SystemEventKind::WindowRestore);
    assert_eq!(SystemEvent::WindowFocus.kind(), SystemEventKind::WindowFocus);
    assert_eq!(SystemEvent::WindowBlur.kind(), SystemEventKind::WindowBlur);
}

// 定时器与拖放事件的 kind 必须映射到对应变体。
#[test]
fn timer_and_drag_events_map_to_kinds() {
    // 定时器。
    assert_eq!(
        SystemEvent::Timer { id: 7 }.kind(),
        SystemEventKind::Timer
    );
    // 文件拖入。
    assert_eq!(
        SystemEvent::FileDrop {
            files: vec!["a.png".to_owned()],
            position: Point::new(1.0, 1.0),
        }
        .kind(),
        SystemEventKind::FileDrop
    );
    // 拖拽三段式。
    let drag_start = SystemEvent::DragStart {
        pos: Point::new(1.0, 1.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(drag_start.kind(), SystemEventKind::DragStart);
    let drag_move = SystemEvent::DragMove {
        pos: Point::new(2.0, 2.0),
        delta: Point::new(1.0, 1.0),
        mods: KeyMod::NONE,
    };
    assert_eq!(drag_move.kind(), SystemEventKind::DragMove);
    let drag_end = SystemEvent::DragEnd {
        pos: Point::new(3.0, 3.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(drag_end.kind(), SystemEventKind::DragEnd);
}

// 相同载荷构造的事件必须可克隆且字段一致。
#[test]
fn system_event_clones_are_equal() {
    // 构造带载荷的指针事件。
    let original = SystemEvent::PointerDown {
        pos: Point::new(10.0, 20.0),
        button: MouseButton::Left,
        mods: KeyMod::CTRL | KeyMod::SHIFT,
    };
    // 克隆必须保留全部载荷字段。
    match original.clone() {
        SystemEvent::PointerDown { pos, button, mods } => {
            assert_eq!(pos, Point::new(10.0, 20.0));
            assert_eq!(button, MouseButton::Left);
            assert_eq!(mods, KeyMod::CTRL | KeyMod::SHIFT);
        }
        _ => panic!("克隆必须保持指针按下变体"),
    }
    // 载荷携带字符串的事件也必须可克隆。
    let text = SystemEvent::Paste {
        text: "你好".to_owned(),
    };
    match text.clone() {
        SystemEvent::Paste { text: cloned } => assert_eq!(cloned, "你好"),
        _ => panic!("克隆必须保持粘贴变体"),
    }
    // 不同载荷的 kind 仍相同，但内容不同。
    let other = SystemEvent::Paste {
        text: "再见".to_owned(),
    };
    assert_eq!(text.kind(), other.kind());
}

// ── SemanticEvent 构造与载荷 ──────────────────────────────────────────────

// 语义事件构造器必须初始化目标与载荷。
#[test]
fn semantic_event_constructors_fill_target_and_payload() {
    // 点击事件。
    let click = SemanticEvent::click(
        ComponentId::new(1),
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::new(5.0, 5.0),
            modifiers: KeyMod::NONE,
        },
    );
    assert_eq!(click.kind, SemanticKind::Click);
    assert_eq!(click.target, ComponentId::new(1));
    assert_eq!(click.current_target, ComponentId::new(1));
    assert_eq!(click.click_payload().map(|payload| payload.button), Some(MouseButton::Left));
    // 文本载荷事件。
    let text = SemanticEvent::text_input(ComponentId::new(2), "abc");
    assert_eq!(text.kind, SemanticKind::TextInput);
    assert_eq!(text.text_payload(), Some("abc"));
    // 文件拖放事件。
    let files = SemanticEvent::file_drop(
        ComponentId::new(3),
        vec!["a.png".to_owned()],
        Point::new(1.0, 1.0),
    );
    assert_eq!(files.kind, SemanticKind::FileDrop);
    let (paths, position) = files.file_drop_payload().expect("必须有拖放载荷");
    assert_eq!(paths, &["a.png".to_owned()]);
    assert_eq!(position, Point::new(1.0, 1.0));
}

// 自定义载荷必须按类型往返。
#[test]
fn custom_payload_round_trips_typed_value() {
    // 构造携带自定义结构的语义事件。
    let event = SemanticEvent::custom(
        ComponentId::new(4),
        CustomPayload { value: 42, tag: "x".to_owned() },
    );
    // kind 必须携带自定义载荷的类型标识。
    assert_eq!(event.kind, SemanticKind::Custom(std::any::TypeId::of::<CustomPayload>()));
    // 载荷必须按原类型取回。
    let payload = event.custom_payload::<CustomPayload>().expect("必须可取回载荷");
    assert_eq!(payload.value, 42);
    assert_eq!(payload.tag, "x");
    // 类型不符时必须返回 None。
    assert!(event.custom_payload::<u32>().is_none());
    // 非自定义事件的载荷访问器必须返回 None。
    let click = SemanticEvent::click(
        ComponentId::new(1),
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::new(0.0, 0.0),
            modifiers: KeyMod::NONE,
        },
    );
    assert!(click.custom_payload::<CustomPayload>().is_none());
    assert!(click.text_payload().is_none());
}

// 传播停止与默认行为阻止必须按标志记录。
#[test]
fn propagation_and_default_flags_are_controllable() {
    // 构造新事件（默认未停止）。
    let mut event = SemanticEvent::change(ComponentId::new(1), "v");
    assert!(!event.propagation_stopped());
    assert!(!event.default_prevented());
    // 停止传播后标志必须置位。
    event.stop_propagation();
    assert!(event.propagation_stopped());
    // 阻止默认行为后标志必须置位。
    event.prevent_default();
    assert!(event.default_prevented());
    // 主键点击识别只对左键有效。
    let primary = SemanticEvent::click(
        ComponentId::new(1),
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::new(0.0, 0.0),
            modifiers: KeyMod::NONE,
        },
    );
    assert!(primary.is_primary_click());
    let secondary = SemanticEvent::click(
        ComponentId::new(1),
        ClickEvent {
            button: MouseButton::Right,
            pos: Point::new(0.0, 0.0),
            modifiers: KeyMod::NONE,
        },
    );
    assert!(!secondary.is_primary_click());
}

// ── HandlerTable 注册与分派 ───────────────────────────────────────────────

// 按路径顺序分派必须依次命中并返回 Handled。
#[test]
fn dispatch_path_follows_path_order() {
    // 构造三个组件路径。
    let parent = ComponentId::new(1);
    let middle = ComponentId::new(2);
    let leaf = ComponentId::new(3);
    let mut table = HandlerTable::new();
    // 用共享容器记录触发顺序。
    let order = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    table.on(parent, SemanticKind::Click, {
        let order = order.clone();
        move |_event| order.borrow_mut().push("parent")
    });
    table.on(middle, SemanticKind::Click, {
        let order = order.clone();
        move |_event| order.borrow_mut().push("middle")
    });
    table.on(leaf, SemanticKind::Click, {
        let order = order.clone();
        move |_event| order.borrow_mut().push("leaf")
    });
    // 沿 父→中→子 路径分派。
    let mut event = SemanticEvent::click(
        leaf,
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::new(0.0, 0.0),
            modifiers: KeyMod::NONE,
        },
    );
    let result = table.dispatch_path(&[parent, middle, leaf], &mut event);
    // 必须按路径顺序触发。
    assert_eq!(*order.borrow(), vec!["parent", "middle", "leaf"]);
    // 有处理器命中必须返回 Handled。
    assert_eq!(result, EventResult::Handled);
    // 分派过程必须更新当前目标。
    assert_eq!(event.current_target, leaf);
}

// 无处理器命中必须返回 NotHandled。
#[test]
fn dispatch_path_without_handlers_returns_not_handled() {
    let mut table = HandlerTable::new();
    // 构造无人注册的路径。
    let mut event = SemanticEvent::click(
        ComponentId::new(9),
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::new(0.0, 0.0),
            modifiers: KeyMod::NONE,
        },
    );
    let result = table.dispatch_path(&[ComponentId::new(9)], &mut event);
    // 无处理器必须返回 NotHandled。
    assert_eq!(result, EventResult::NotHandled);
}

// 处理器种类不匹配时不得触发。
#[test]
fn dispatch_skips_mismatched_kind() {
    let mut table = HandlerTable::new();
    let component = ComponentId::new(1);
    // 只注册 Change 处理器（共享计数）。
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    table.on(component, SemanticKind::Change, {
        let calls = calls.clone();
        move |_event| calls.set(calls.get() + 1)
    });
    // 分派 Click 事件。
    let mut event = SemanticEvent::click(
        component,
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::new(0.0, 0.0),
            modifiers: KeyMod::NONE,
        },
    );
    let result = table.dispatch_path(&[component], &mut event);
    // 种类不匹配不得触发。
    assert_eq!(calls.get(), 0);
    assert_eq!(result, EventResult::NotHandled);
}

// 一次性处理器触发后必须自动移除。
#[test]
fn once_handler_is_removed_after_first_dispatch() {
    let mut table = HandlerTable::new();
    let component = ComponentId::new(1);
    // 注册一次性处理器（共享计数）。
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    table.register(
        component,
        HandlerRegistration::with_options(
            SemanticKind::Change,
            HandlerOptions::once(),
            Box::new({
                let calls = calls.clone();
                move |_event| calls.set(calls.get() + 1)
            }),
        ),
    );
    // 第一次分派必须触发。
    let mut event = SemanticEvent::change(component, "v");
    table.dispatch_path(&[component], &mut event);
    assert_eq!(calls.get(), 1);
    // 第二次分派必须不再触发。
    let mut event = SemanticEvent::change(component, "v");
    table.dispatch_path(&[component], &mut event);
    assert_eq!(calls.get(), 1);
}

// when 谓词为假时处理器不得触发。
#[test]
fn when_predicate_gates_dispatch() {
    let mut table = HandlerTable::new();
    let component = ComponentId::new(1);
    // 注册带谓词的处理器：只接受文本 "yes"。
    let calls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    table.register(
        component,
        HandlerRegistration::with_options(
            SemanticKind::Change,
            HandlerOptions::when(|event| event.text_payload() == Some("yes")),
            Box::new({
                let calls = calls.clone();
                move |event| {
                    calls.borrow_mut().push(event.text_payload().unwrap_or("").to_owned());
                }
            }),
        ),
    );
    // 不满足谓词的载荷不得触发。
    let mut event = SemanticEvent::change(component, "no");
    table.dispatch_path(&[component], &mut event);
    assert!(calls.borrow().is_empty());
    // 满足谓词的载荷必须触发。
    let mut event = SemanticEvent::change(component, "yes");
    table.dispatch_path(&[component], &mut event);
    assert_eq!(*calls.borrow(), vec!["yes"]);
}

// 停止传播必须中断后续路径分派。
#[test]
fn stopped_propagation_breaks_path() {
    let mut table = HandlerTable::new();
    let parent = ComponentId::new(1);
    let leaf = ComponentId::new(2);
    // 父节点停止传播。
    table.on(parent, SemanticKind::Click, |event| {
        event.stop_propagation();
    });
    // 子节点正常计数（共享容器）。
    let leaf_calls = std::rc::Rc::new(std::cell::Cell::new(0));
    table.on(leaf, SemanticKind::Click, {
        let leaf_calls = leaf_calls.clone();
        move |_event| leaf_calls.set(leaf_calls.get() + 1)
    });
    // 沿 父→子 分派。
    let mut event = SemanticEvent::click(
        leaf,
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::new(0.0, 0.0),
            modifiers: KeyMod::NONE,
        },
    );
    table.dispatch_path(&[parent, leaf], &mut event);
    // 父节点触发后子节点不得再触发。
    assert_eq!(leaf_calls.get(), 0);
    // 停止传播后当前目标停留在停止点。
    assert_eq!(event.current_target, parent);
}

// remove 必须按句柄移除处理器。
#[test]
fn remove_unregisters_handler_by_id() {
    let mut table = HandlerTable::new();
    let component = ComponentId::new(1);
    // 注册两个处理器（共享计数）。
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let id = table.on(component, SemanticKind::Change, {
        let calls = calls.clone();
        move |_event| calls.set(calls.get() + 1)
    });
    table.on(component, SemanticKind::Change, {
        let calls = calls.clone();
        move |_event| calls.set(calls.get() + 1)
    });
    // 移除第一个。
    table.remove(component, id);
    // 分派后只剩第二个触发。
    let mut event = SemanticEvent::change(component, "v");
    table.dispatch_path(&[component], &mut event);
    assert_eq!(calls.get(), 1);
    // 移除不存在的句柄必须无害。
    table.remove(component, HandlerId(999));
}

// clear_component 必须清空该组件全部处理器。
#[test]
fn clear_component_removes_all_handlers_of_component() {
    let mut table = HandlerTable::new();
    let component = ComponentId::new(1);
    // 注册两个种类的处理器（共享计数）。
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    table.on(component, SemanticKind::Change, {
        let calls = calls.clone();
        move |_event| calls.set(calls.get() + 1)
    });
    table.on(component, SemanticKind::Submit, {
        let calls = calls.clone();
        move |_event| calls.set(calls.get() + 1)
    });
    // 清空组件。
    table.clear_component(component);
    // 两种事件都不得再触发。
    let mut event = SemanticEvent::change(component, "v");
    table.dispatch_path(&[component], &mut event);
    let mut event = SemanticEvent::submit(component, "v");
    table.dispatch_path(&[component], &mut event);
    assert_eq!(calls.get(), 0);
}

// 便捷注册器必须只透传对应载荷。
#[test]
fn convenience_handlers_extract_payloads() {
    let mut table = HandlerTable::new();
    let component = ComponentId::new(1);
    // 记录点击载荷（共享容器）。
    let clicked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    table.on_click(component, {
        let clicked = clicked.clone();
        move |payload| clicked.borrow_mut().push(payload.button)
    });
    // 记录变更文本。
    let changed = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    table.on_change(component, {
        let changed = changed.clone();
        move |value| changed.borrow_mut().push(value.to_owned())
    });
    // 记录自定义载荷。
    let customs = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    table.on_custom::<CustomPayload>(component, {
        let customs = customs.clone();
        move |payload| customs.borrow_mut().push(payload.value)
    });
    // 分派点击。
    let mut event = SemanticEvent::click(
        component,
        ClickEvent {
            button: MouseButton::Right,
            pos: Point::new(0.0, 0.0),
            modifiers: KeyMod::NONE,
        },
    );
    table.dispatch_path(&[component], &mut event);
    // 点击便捷器必须收到按钮。
    assert_eq!(*clicked.borrow(), vec![MouseButton::Right]);
    // 分派变更。
    let mut event = SemanticEvent::change(component, "新值");
    table.dispatch_path(&[component], &mut event);
    assert_eq!(*changed.borrow(), vec!["新值"]);
    // 分派自定义载荷。
    let mut event = SemanticEvent::custom(component, CustomPayload { value: 7, tag: "t".to_owned() });
    table.dispatch_path(&[component], &mut event);
    assert_eq!(*customs.borrow(), vec![7]);
}

// ── 处理器签名与捕获指纹 ──────────────────────────────────────────────────

// 签名必须编码种类、选项与可选代次/指纹。
#[test]
fn handler_signature_encodes_options_and_captures() {
    // 默认注册无代次无指纹。
    let registration = HandlerRegistration::new(
        SemanticKind::Click,
        Box::new(|_event| {}),
    );
    let signature = registration.signature();
    assert_eq!(signature.kind, SemanticKind::Click);
    assert!(!signature.options.once);
    assert!(!signature.options.when);
    assert_eq!(signature.generation, None);
    assert_eq!(signature.capture_fingerprint, None);
    // 显式代次必须进入签名。
    let registration = registration.with_generation(3);
    assert_eq!(registration.signature().generation, Some(3));
    // 捕获指纹存在而代次缺失时，author 签名必须补零代次。
    let registration = HandlerRegistration::new(
        SemanticKind::Click,
        Box::new(|_event| {}),
    )
    .with_capture_fingerprint(0xDEAD_BEEF);
    let authored = registration.authored_signature();
    assert_eq!(authored.capture_fingerprint, Some(0xDEAD_BEEF));
    assert_eq!(authored.generation, Some(0));
}

// 窗口捕获指纹必须对窗口 id 稳定且区分不同窗口。
#[test]
fn window_capture_fingerprint_distinguishes_windows() {
    // 相同窗口的指纹必须一致。
    let first = window_capture_fingerprint(WindowId::new(1));
    let second = window_capture_fingerprint(WindowId::new(1));
    assert_eq!(first, second);
    // 不同窗口的指纹必须不同。
    assert_ne!(first, window_capture_fingerprint(WindowId::new(2)));
}

/// 自定义载荷测试结构。
#[derive(Debug, Clone, PartialEq)]
struct CustomPayload {
    value: i32,
    tag: String,
}
