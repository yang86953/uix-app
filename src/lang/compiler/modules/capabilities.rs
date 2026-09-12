//! 应用模块工具链能力说明，由编译器集中提供给 CLI 与编辑器。

#[derive(Debug, Clone, Copy)]
pub struct ModuleCapability {
    pub name: &'static str,
    pub feature: &'static str,
    pub summary: &'static str,
}

/// 返回本前端支持的业务能力，不把旧 UI 全量组件登记冒充动态支持。
pub fn capabilities() -> &'static [ModuleCapability] {
    &[
        ModuleCapability {
            name: "aot-module",
            feature: "uix-modules",
            summary: "uix_module! 生成真实 Rust 函数；函数、状态、效果、View 与异步片段共享检查。",
        },
        ModuleCapability {
            name: "dynamic-module",
            feature: "uix-dynamic",
            summary: "load_module/load_module_file 冻结类型化产物；必须由宿主显式启用，不向现有 UI 宏或 Scheme 自动回退。",
        },
        ModuleCapability {
            name: "module-import",
            feature: "uix-modules",
            summary: "包内显式 Import、精确版本、导出和结构类型；根实例拥有别名隔离状态，宿主端口按限定名绑定。64文件/实例、16层、4MiB源码闭包。",
        },
        ModuleCapability {
            name: "module-view",
            feature: "uix-modules",
            summary: "根 View 支持 Column/Row/Text/Input/Button、If/ElseIf/Else 与强制稳定 key 的 For；只提交快照，layout/paint 不运行模块。业务库 Import 不挂载 View。",
        },
        ModuleCapability {
            name: "module-async",
            feature: "uix-modules",
            summary: "AsyncCommand 通过 ModuleWorker 真正挂起；await 仅等待声明的异步宿主端口。段提交、revision/代际冲突、取消/替换/撤权/关闭终态，不重放外部效果。",
        },
    ]
}

#[derive(Debug, Clone, Copy)]
pub struct ModuleSyntax {
    pub tag: &'static str,
    pub attributes: &'static [&'static str],
    pub summary: &'static str,
}

/// 模块编辑使用同一前端的窄语法说明，不建议未经实现的完整 Rust 或 UI 节点。
pub fn syntax() -> &'static [ModuleSyntax] {
    &[
        ModuleSyntax {
            tag: "Module",
            attributes: &["name", "version", "schema"],
            summary: "可移植应用模块入口；AOT需uix-modules，运行期源码需uix-dynamic。",
        },
        ModuleSyntax {
            tag: "Import",
            attributes: &["from", "as", "version"],
            summary: "显式相对 .uix 路径与精确版本；限定名只能调用公开导出。",
        },
        ModuleSyntax {
            tag: "Data",
            attributes: &["name", "fields", "export"],
            summary: "拥有型结构数据；export=true 暴露类型名，不引入名义类型或 Rust 对象。",
        },
        ModuleSyntax {
            tag: "State",
            attributes: &["name", "type", "value"],
            summary: "实例拥有的状态；类型化常量初值；修改使用 setState。",
        },
        ModuleSyntax {
            tag: "Port",
            attributes: &["name", "params", "returns", "effect", "async"],
            summary: "宿主显式授权与签名匹配的端口；无隐式 I/O。",
        },
        ModuleSyntax {
            tag: "Function",
            attributes: &["name", "params", "returns", "body", "export"],
            summary: "纯函数，不读状态或调用宿主；默认私有。",
        },
        ModuleSyntax {
            tag: "Query",
            attributes: &["name", "params", "returns", "body", "export"],
            summary: "只读业务查询，可读实例状态及查询端口，不能写状态。",
        },
        ModuleSyntax {
            tag: "Command",
            attributes: &["name", "params", "returns", "body", "export"],
            summary: "同步命令，状态原子提交；宿主已产生的外部效果不回滚。",
        },
        ModuleSyntax {
            tag: "AsyncCommand",
            attributes: &["name", "params", "returns", "body", "export"],
            summary: "有界 worker 异步命令；let value = await port(...); 仅等待本模块声明端口。",
        },
        ModuleSyntax {
            tag: "View",
            attributes: &[],
            summary: "唯一根视图；基础节点与有界控制绑定，不是完整 UIX/Rust。",
        },
        ModuleSyntax {
            tag: "Column",
            attributes: &["key", "gap", "padding"],
            summary: "垂直布局，子节点共享拥有型快照。",
        },
        ModuleSyntax {
            tag: "Row",
            attributes: &["key", "gap", "padding"],
            summary: "水平布局。",
        },
        ModuleSyntax {
            tag: "Text",
            attributes: &["key", "text"],
            summary: "动态只读文本属性。",
        },
        ModuleSyntax {
            tag: "Input",
            attributes: &["key", "value", "placeholder", "onChange"],
            summary: "输入绑定与 $event.value；事件提交到当前实例 worker。",
        },
        ModuleSyntax {
            tag: "Button",
            attributes: &["key", "text", "disabled", "onClick"],
            summary: "显式点击事件，可调用同步或异步命令；禁用状态拒绝事件。",
        },
        ModuleSyntax {
            tag: "If",
            attributes: &[],
            summary: "<If {condition}> 的 Bool 条件。",
        },
        ModuleSyntax {
            tag: "ElseIf",
            attributes: &[],
            summary: "紧随 If 的后续 Bool 条件。",
        },
        ModuleSyntax {
            tag: "Else",
            attributes: &[],
            summary: "条件链最后的兜底分支。",
        },
        ModuleSyntax {
            tag: "For",
            attributes: &[],
            summary: "<For {item} {index} in {items} key={item.id}>；强制 String/Int 稳定 key。",
        },
    ]
}
