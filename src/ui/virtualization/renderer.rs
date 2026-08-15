// 导入公开 View 构建契约与声明节点类型。
use crate::ui::view::{View, ViewNode};

// 区分绝对索引与业务键身份，避免 renderer 模式切换时意外串用旧状态。
pub(crate) enum VirtualScrollItemIdentity {
    // 顺序不可变列表按绝对索引拥有当前逻辑项。
    Index(usize),
    // 可排序、插入或删除的列表按调用方业务键拥有逻辑项。
    Business(String),
}

// 为树内 keyed reconcile 与组件私有状态生成同一规范字符串。
impl VirtualScrollItemIdentity {
    // 消费类型化身份并生成不会跨模式碰撞的运行时 key。
    pub(crate) fn into_runtime_key(self) -> String {
        // 每种身份模式使用互斥前缀并保留原始值供诊断。
        match self {
            // 保持既有普通 renderer 的绝对索引后备 key。
            Self::Index(index) => format!("virtual-scroll-item:{index}"),
            // 为业务键增加专用前缀，避免与任何索引后备文本碰撞。
            Self::Business(key) => format!("virtual-scroll-business:{key}"),
        }
    }
}

// 在用户行工厂执行前产生协调与私有状态共用的类型化身份。
pub(crate) type VirtualScrollKeyRenderer =
    Box<dyn FnMut(usize) -> VirtualScrollItemIdentity + 'static>;

/// 保存于树级 keyed sidecar 中的应用行工厂。
pub(crate) type VirtualScrollItemRenderer = Box<dyn FnMut(usize) -> ViewNode + 'static>;

// 把稳定键工厂与行工厂绑定为同一份 VirtualScroll renderer 注册。
pub(crate) struct VirtualScrollRenderer {
    // 保存必须先于行构建执行的稳定键工厂。
    pub(crate) key: VirtualScrollKeyRenderer,
    // 保存只为当前物化窗口构建声明 View 的行工厂。
    pub(crate) item: VirtualScrollItemRenderer,
}

/// 同时拥有声明式 `VirtualScroll` 与应用 renderer 的构建器。
pub struct VirtualScrollBuilder {
    // 保存唯一拥有滚动与物化状态的运行时组件。
    pub(super) scroll: super::virtual_scroll::VirtualScroll,
    // 保存由树 sidecar 管理的应用键工厂与行工厂。
    pub(super) renderer: VirtualScrollRenderer,
}

// 提供普通索引身份与业务稳定键两种明确的 renderer 构造入口。
impl super::virtual_scroll::VirtualScroll {
    /// 为进入物化范围的索引声明普通 View 子树。
    pub fn render<V>(self, mut renderer: impl FnMut(usize) -> V + 'static) -> VirtualScrollBuilder
    where
        // 普通行工厂继续接受所有公开 View 实现。
        V: View,
    {
        // 把滚动组件与索引身份 renderer 一起移交给 builder。
        VirtualScrollBuilder {
            // 保留 VirtualScroll 的滚动、测量与物化状态。
            scroll: self,
            // 普通 renderer 明确按绝对索引拥有协调身份和私有状态。
            renderer: VirtualScrollRenderer {
                // 在用户工厂前生成不会与业务键语义混用的索引身份。
                key: Box::new(VirtualScrollItemIdentity::Index),
                // 只在对应索引进入物化窗口时构建实际行 View。
                item: Box::new(move |index| renderer(index).build()),
            },
        }
    }

    /// 为可排序、插入或删除的数据声明业务稳定键与惰性行工厂。
    pub fn render_keyed<K, V>(
        // 消费 VirtualScroll 声明并进入 renderer builder 阶段。
        self,
        // 在行工厂执行前按绝对索引取得稳定业务键。
        mut key: impl FnMut(usize) -> K + 'static,
        // 按绝对索引构建当前物化行。
        mut renderer: impl FnMut(usize) -> V + 'static,
        // 返回同时拥有键工厂与行工厂的声明 builder。
    ) -> VirtualScrollBuilder
    where
        // 业务键统一转换成 WidgetTree 协调使用的字符串身份。
        K: ToString,
        // 行工厂继续接受所有公开 View 实现。
        V: View,
    {
        // 把两个应用闭包一起移交给树外 sidecar 生命周期。
        VirtualScrollBuilder {
            // 保留 VirtualScroll 的滚动、测量与物化状态。
            scroll: self,
            // 绑定必须按顺序消费的稳定键与行工厂。
            renderer: VirtualScrollRenderer {
                // 先计算业务键，使动态 state 命名空间与节点身份一致。
                key: Box::new(move |index| {
                    // 用类型化模式隔离业务键与绝对索引后备键。
                    VirtualScrollItemIdentity::Business(key(index).to_string())
                }),
                // 再惰性构建对应业务项的声明 View。
                item: Box::new(move |index| renderer(index).build()),
            },
        }
    }
}
