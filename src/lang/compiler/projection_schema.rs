//! UIX Lang 可枚举 UI 投影事实的唯一登记入口。

/// 当前 schema 的独立版本；缓存与工具协议必须把它计入身份。
pub const SCHEMA_VERSION: u32 = 4;
/// 当前语言基线版本。
pub const LANGUAGE_VERSION: &str = "0.0.3";

/// 声明一个登记项当前是否可用于正式 AOT。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationStatus {
    /// 已有确定 Rust 映射并允许生成。
    Available,
    /// 已保留语言身份，但尚未开放当前入口。
    Planned,
}

/// 声明内置组件所属的参考文档类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentCategory {
    General,
    Layout,
    Input,
    Display,
    Feedback,
    Navigation,
    Chart,
    Application,
}

impl ComponentCategory {
    /// 返回面向诊断和文档的稳定中文类别名。
    pub const fn label(self) -> &'static str {
        match self {
            Self::General => "通用组件",
            Self::Layout => "布局组件",
            Self::Input => "输入组件",
            Self::Display => "展示组件",
            Self::Feedback => "反馈组件",
            Self::Navigation => "导航组件",
            Self::Chart => "图表组件",
            Self::Application => "标签语法 / App 应用入口",
        }
    }
}

/// 描述一个内置标签的稳定身份、生成入口与 capability。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub category: ComponentCategory,
    pub status: RegistrationStatus,
    pub emitter: Option<&'static str>,
    pub capability: Option<&'static str>,
}

/// 描述一个事件名当前公开的 `$event` 顶层字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub fields: &'static [&'static str],
}

/// 描述一个语言数据构造器到公开 Rust API 的确定映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataConstructorSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub rust_path: &'static str,
    pub method: &'static str,
    pub normalize_numbers: bool,
    pub capability: Option<&'static str>,
}

/// 区分 UI 投影输入在语言面的值形状。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionValueKind {
    String,
    Number,
    Boolean,
    Enumeration,
    Expression,
    InlineStyle,
    Data,
    StateHandle,
    Any,
}

/// 描述全部 View 通用属性的稳定输入契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttributeSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub value_kind: ProjectionValueKind,
}

/// 描述一项已支持样式属性及其值类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub value_kind: ProjectionValueKind,
}

/// 区分运行时设计 token 的闭合字段类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeTokenKind {
    Color,
    String,
    Number,
    Shadow,
    Boolean,
}

/// 描述 `@theme` 可写和样式可引用的设计 token。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeTokenSpec {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub kind: ThemeTokenKind,
    #[serde(default)]
    pub alias_for: Option<String>,
    pub key: String,
    pub unit: String,
    #[serde(default)]
    pub allow_px: bool,
    #[serde(default)]
    pub minimum: Option<f32>,
    #[serde(default)]
    pub shadow_layers: Option<usize>,
    #[serde(default)]
    pub typography: Vec<String>,
    #[serde(default)]
    pub fallback: f32,
}

/// 描述必须接收 `State<T>` 而不是普通值的组件属性位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleSpec {
    pub id: &'static str,
    pub component: &'static str,
    pub attribute: &'static str,
    pub value_type: &'static str,
}

/// 描述内置组件拥有的命名子节点插槽。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotSpec {
    pub id: &'static str,
    pub component: &'static str,
    pub name: &'static str,
    pub multiple: bool,
}

/// 描述 Cargo feature 与 UI 投影能力的稳定对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilitySpec {
    pub id: &'static str,
    pub name: &'static str,
    pub cargo_feature: &'static str,
}

/// 描述只有出现特定组件属性时才需要的 capability。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttributeCapabilitySpec {
    pub id: &'static str,
    pub component: &'static str,
    pub attribute: &'static str,
    pub capability: &'static str,
}

/// 区分已登记状态值类型的形状类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueTypeShape {
    /// 基础标量类型。
    Scalar,
    /// 公开语义构造类型。
    Semantic,
    /// 容器集合类型。
    Collection,
    /// 可空包装类型。
    Optional,
    /// 组件拥有的专用载荷类型。
    ComponentPayload,
}

/// 描述 props、state 注解与 record 字段可登记的一个值类型。
///
/// 核心语言只登记通用标量、语义、集合与可空形状；组件专属载荷
/// 类型由拥有它的组件类别登记，并按需挂接 capability 门禁，
/// 保证组件数据面不会泄漏进核心语言清单。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueTypeSpec {
    pub id: &'static str,
    /// 语言面书写名称，容器类型保留完整泛型拼写。
    pub name: &'static str,
    pub shape: ValueTypeShape,
    /// 是否允许作为 props 与回调参数的基础值类型。
    pub allowed_in_props: bool,
    /// 是否允许作为 `State<T>` prop 的泛型内类型。
    ///
    /// Rust 侧驱动的整数索引与集合句柄需要精确类型传入共享状态，
    /// 但它们不构成拥有型值 prop，也不进入回调参数白名单。
    pub allowed_in_state_props: bool,
    /// 拥有该类型的参考文档类别；`None` 表示核心语言通用类型。
    pub owner_category: Option<ComponentCategory>,
    pub capability: Option<&'static str>,
}

/// 提供 Compiler System 唯一拥有的只读 UI 投影 schema。
#[derive(Debug, Clone, Copy, Default)]
pub struct UiProjectionSchema;

/// 全部入口共享的 schema 实例。
pub const UI_PROJECTION_SCHEMA: UiProjectionSchema = UiProjectionSchema;

impl UiProjectionSchema {
    /// 返回当前 schema 版本。
    pub const fn version(self) -> u32 {
        SCHEMA_VERSION
    }

    /// 返回当前语言版本。
    pub const fn language_version(self) -> &'static str {
        LANGUAGE_VERSION
    }

    /// 返回完整组件登记表。
    pub const fn components(self) -> &'static [ComponentSpec] {
        COMPONENTS
    }

    /// 按精确标签名查询组件登记。
    pub fn component(self, name: &str) -> Option<&'static ComponentSpec> {
        COMPONENTS.iter().find(|entry| entry.name == name)
    }

    /// 生成当前 AOT 可用标签的确定性诊断清单。
    pub fn supported_component_hint(self) -> String {
        COMPONENTS
            .iter()
            .filter(|entry| entry.status == RegistrationStatus::Available)
            .map(|entry| entry.name)
            .collect::<Vec<_>>()
            .join("、")
    }

    /// 返回完整事件载荷登记表。
    pub const fn events(self) -> &'static [EventSpec] {
        EVENTS
    }

    /// 按精确事件属性名查询载荷字段。
    pub fn event(self, name: &str) -> Option<&'static EventSpec> {
        EVENTS.iter().find(|entry| entry.name == name)
    }

    /// 返回完整数据构造登记表。
    pub const fn data_constructors(self) -> &'static [DataConstructorSpec] {
        DATA_CONSTRUCTORS
    }

    /// 按精确语言类型名查询数据构造映射。
    pub fn data_constructor(self, name: &str) -> Option<&'static DataConstructorSpec> {
        DATA_CONSTRUCTORS.iter().find(|entry| entry.name == name)
    }

    /// 返回全部 View 共用属性。
    pub const fn common_attributes(self) -> &'static [AttributeSpec] {
        COMMON_ATTRIBUTES
    }

    /// 按名称查询 View 共用属性。
    pub fn common_attribute(self, name: &str) -> Option<&'static AttributeSpec> {
        COMMON_ATTRIBUTES.iter().find(|entry| entry.name == name)
    }

    /// 返回完整已支持样式属性表。
    pub const fn style_properties(self) -> &'static [StyleSpec] {
        STYLE_PROPERTIES
    }

    /// 按名称查询已支持样式属性。
    pub fn style_property(self, name: &str) -> Option<&'static StyleSpec> {
        STYLE_PROPERTIES.iter().find(|entry| entry.name == name)
    }

    /// Returns token declarations from the current library catalog.
    pub fn theme_tokens(self) -> Vec<ThemeTokenSpec> {
        super::components::current()
            .theme_tokens
            .values()
            .cloned()
            .collect()
    }
    pub fn theme_token(self, name: &str) -> Option<ThemeTokenSpec> {
        super::components::current().theme_tokens.get(name).cloned()
    }

    /// 返回全部 State 句柄位。
    pub const fn handle_slots(self) -> &'static [HandleSpec] {
        HANDLE_SLOTS
    }

    /// 查询一个组件属性是否为 State 句柄位。
    pub fn handle_slot(self, component: &str, attribute: &str) -> Option<&'static HandleSpec> {
        HANDLE_SLOTS
            .iter()
            .find(|entry| entry.component == component && entry.attribute == attribute)
    }

    /// 返回全部内置命名插槽。
    pub const fn slots(self) -> &'static [SlotSpec] {
        SLOTS
    }

    /// 查询组件的命名插槽。
    pub fn slot(self, component: &str, name: &str) -> Option<&'static SlotSpec> {
        SLOTS
            .iter()
            .find(|entry| entry.component == component && entry.name == name)
    }

    /// 返回全部可独立门禁的语言能力。
    pub const fn capabilities(self) -> &'static [CapabilitySpec] {
        CAPABILITIES
    }

    /// 按语言能力名查询 Cargo feature。
    pub fn capability(self, name: &str) -> Option<&'static CapabilitySpec> {
        CAPABILITIES.iter().find(|entry| entry.name == name)
    }

    /// 返回全部属性级 capability 门禁。
    pub const fn attribute_capabilities(self) -> &'static [AttributeCapabilitySpec] {
        ATTRIBUTE_CAPABILITIES
    }

    /// 查询一个组件属性是否要求独立 capability。
    pub fn attribute_capability(
        self,
        component: &str,
        attribute: &str,
    ) -> Option<&'static AttributeCapabilitySpec> {
        ATTRIBUTE_CAPABILITIES
            .iter()
            .find(|entry| entry.component == component && entry.attribute == attribute)
    }

    /// 返回完整状态值类型登记表。
    pub const fn value_types(self) -> &'static [ValueTypeSpec] {
        VALUE_TYPES
    }

    /// 按语言面书写名称（含容器泛型拼写）查询值类型登记。
    pub fn value_type(self, name: &str) -> Option<&'static ValueTypeSpec> {
        VALUE_TYPES.iter().find(|entry| entry.name == name)
    }

    /// 生成 state 注解与 record 字段当前登记类型的确定性诊断清单。
    pub fn supported_value_type_hint(self) -> String {
        let mut names = self
            .value_types()
            .iter()
            .map(|entry| entry.name)
            .collect::<Vec<_>>()
            .join("、");
        // record 引用是解析器按声明动态识别的补充类别。
        names.push_str("，或文档内声明的 record 名");
        names
    }
}

macro_rules! component {
    ($name:literal, $category:ident, $emitter:literal) => {
        component!($name, $category, $emitter, None)
    };
    ($name:literal, $category:ident, $emitter:literal, $capability:expr) => {
        ComponentSpec {
            id: concat!("component.", $name),
            name: $name,
            category: ComponentCategory::$category,
            status: RegistrationStatus::Available,
            emitter: Some($emitter),
            capability: $capability,
        }
    };
}

// 顺序保持现有诊断清单与参考分类稳定。
const COMPONENTS: &[ComponentSpec] = &[
    component!("KernelView", General, "generate_kernel_view"),
    component!("KernelHost", General, "generate_kernel_host"),
    component!("Canvas", General, "generate_canvas"),
    component!("FocusTrap", General, "generate_focus_trap"),
    component!("App", Application, "generate_document_app"),
];

macro_rules! attribute {
    ($name:literal, $kind:ident) => {
        AttributeSpec {
            id: concat!("attribute.common.", $name),
            name: $name,
            value_kind: ProjectionValueKind::$kind,
        }
    };
}

// 与 ViewNode 公共生成入口逐项对齐；组件专属属性由对应 Component Module 追加。
const COMMON_ATTRIBUTES: &[AttributeSpec] = &[
    attribute!("gap", Number),
    attribute!("padding", Number),
    attribute!("margin", Number),
    attribute!("width", Number),
    attribute!("height", Number),
    attribute!("flexGrow", Number),
    attribute!("flexShrink", Number),
    attribute!("color", Any),
    attribute!("backgroundColor", Any),
    attribute!("fontSize", Any),
    attribute!("automationId", String),
    attribute!("key", Any),
    attribute!("align", Enumeration),
    attribute!("justify", Enumeration),
    attribute!("class", String),
    attribute!("style", InlineStyle),
];

macro_rules! style {
    ($name:literal, $kind:ident) => {
        StyleSpec {
            id: concat!("style.", $name),
            name: $name,
            value_kind: ProjectionValueKind::$kind,
        }
    };
}

// 本表是 style Gate 接受属性名的唯一公开事实；顺序按布局、盒模型、文字与视觉分组。
const STYLE_PROPERTIES: &[StyleSpec] = &[
    style!("display", Enumeration),
    style!("flexDirection", Enumeration),
    style!("justifyContent", Enumeration),
    style!("alignItems", Enumeration),
    style!("alignSelf", Enumeration),
    style!("gap", Number),
    style!("flexGrow", Number),
    style!("flexShrink", Number),
    style!("flexWrap", Boolean),
    style!("gridTemplateColumns", Data),
    style!("gridTemplateRows", Data),
    style!("gridColumnGap", Number),
    style!("gridRowGap", Number),
    style!("gridColumnSpan", Number),
    style!("gridRowSpan", Number),
    style!("width", Any),
    style!("height", Any),
    style!("minWidth", Any),
    style!("maxWidth", Any),
    style!("minHeight", Any),
    style!("maxHeight", Any),
    style!("margin", Data),
    style!("padding", Data),
    style!("marginTop", Number),
    style!("marginRight", Number),
    style!("marginBottom", Number),
    style!("marginLeft", Number),
    style!("paddingTop", Number),
    style!("paddingRight", Number),
    style!("paddingBottom", Number),
    style!("paddingLeft", Number),
    style!("borderColor", Any),
    style!("borderWidth", Data),
    style!("borderStyle", Enumeration),
    style!("borderTopWidth", Number),
    style!("borderRightWidth", Number),
    style!("borderBottomWidth", Number),
    style!("borderLeftWidth", Number),
    style!("borderRadius", Number),
    style!("color", Any),
    style!("fontSize", Any),
    style!("fontFamily", String),
    style!("fontWeight", Any),
    style!("lineHeight", Any),
    style!("textAlign", Enumeration),
    style!("textDecoration", Enumeration),
    style!("backgroundColor", Any),
    style!("backgroundImage", String),
    style!("backgroundPosition", Any),
    style!("backgroundRepeat", Enumeration),
    style!("backgroundSize", Any),
    style!("backgroundColor:hover", Any),
    style!("backgroundColor:focus", Any),
    style!("backgroundColor:active", Any),
    style!("opacity", Number),
    style!("overflow", Enumeration),
    style!("boxShadow", Data),
    style!("visible", Boolean),
    style!("z-index", Number),
    style!("transform", Data),
    style!("transformOrigin", Data),
    style!("cursor", Enumeration),
    style!("userSelect", Enumeration),
    style!("position", Enumeration),
    style!("top", Any),
    style!("right", Any),
    style!("bottom", Any),
    style!("left", Any),
];

// Library-owned handles and slots are queried from the active catalog.
const HANDLE_SLOTS: &[HandleSpec] = &[];
const SLOTS: &[SlotSpec] = &[];
const CAPABILITIES: &[CapabilitySpec] = &[];
const ATTRIBUTE_CAPABILITIES: &[AttributeCapabilitySpec] = &[];

const EVENTS: &[EventSpec] = &[
    EventSpec {
        id: "event.click",
        name: "@click",
        fields: &["x", "y"],
    },
    EventSpec {
        id: "event.change",
        name: "@change",
        fields: &["value"],
    },
    EventSpec {
        id: "event.select",
        name: "@select",
        fields: &["value"],
    },
    EventSpec {
        id: "event.submit",
        name: "@submit",
        fields: &["value"],
    },
    EventSpec {
        id: "event.close",
        name: "@close",
        fields: &["value"],
    },
    EventSpec {
        id: "event.keyDown",
        name: "@keyDown",
        fields: &["key", "code", "mods"],
    },
    EventSpec {
        id: "event.keyUp",
        name: "@keyUp",
        fields: &["key", "code", "mods"],
    },
    EventSpec {
        id: "event.mouseEnter",
        name: "@mouseEnter",
        fields: &[],
    },
    EventSpec {
        id: "event.mouseLeave",
        name: "@mouseLeave",
        fields: &[],
    },
    EventSpec {
        id: "event.confirm",
        name: "@confirm",
        fields: &[],
    },
    EventSpec {
        id: "event.cancel",
        name: "@cancel",
        fields: &[],
    },
    EventSpec {
        id: "event.ok",
        name: "@ok",
        fields: &[],
    },
];

macro_rules! data_constructor {
    ($name:literal, $path:literal) => {
        data_constructor!($name, $path, "new", false, None)
    };
    ($name:literal, $path:literal, $method:literal, $normalize:expr, $capability:expr) => {
        DataConstructorSpec {
            id: concat!("data.", $name),
            name: $name,
            rust_path: $path,
            method: $method,
            normalize_numbers: $normalize,
            capability: $capability,
        }
    };
}

macro_rules! value_type {
    ($name:literal, $shape:ident, $in_props:expr, $owner:expr) => {
        value_type!($name, $shape, $in_props, $owner, None)
    };
    ($name:literal, $shape:ident, $in_props:expr, $owner:expr, $capability:expr) => {
        value_type!($name, $shape, $in_props, $owner, $capability, $in_props)
    };
    ($name:literal, $shape:ident, $in_props:expr, $owner:expr, $capability:expr, $in_state_props:expr) => {
        ValueTypeSpec {
            id: concat!("valueType.", $name),
            name: $name,
            shape: ValueTypeShape::$shape,
            allowed_in_props: $in_props,
            allowed_in_state_props: $in_state_props,
            owner_category: $owner,
            capability: $capability,
        }
    };
}

const DATA_CONSTRUCTORS: &[DataConstructorSpec] = &[
    data_constructor!("Color", "::uix_app::prelude::Color", "hex", false, None),
    data_constructor!("Point", "::uix_app::prelude::Point", "new", true, None),
];

// 核心通用类型在前，组件拥有的类型紧随其后；顺序保持诊断清单稳定。
const VALUE_TYPES: &[ValueTypeSpec] = &[
    value_type!("String", Scalar, true, None),
    value_type!("number", Scalar, true, None),
    value_type!("bool", Scalar, true, None),
    value_type!("u32", Scalar, false, None, None, true),
    value_type!("usize", Scalar, false, None, None, true),
    value_type!("f32", Scalar, false, None, None, true),
    value_type!("i32", Scalar, false, None, None, true),
    value_type!("Color", Semantic, true, None),
    value_type!("Point", Semantic, true, None),
    value_type!("HashSet<String>", Collection, false, None),
    value_type!("Vec<String>", Collection, false, None, None, true),
    value_type!("Vec<number>", Collection, false, None, None, true),
    value_type!("Option<String>", Optional, true, None),
];

#[cfg(test)]
#[path = "../tests-src/projection_schema_tests.rs"]
mod tests;
