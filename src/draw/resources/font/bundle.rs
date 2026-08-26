//! 跨平台确定性字体包值契约。

// 使用共享只读字节保存应用字体资产，克隆配置时不复制大字体文件。
use std::sync::Arc;

// 引入统一的参数错误类型。
use crate::core::{Errc, Error, Result};

// 限制单个字体资产的最大字节数，覆盖常见 CJK 字体同时约束不可信输入。
const MAX_FONT_ASSET_BYTES: usize = 64 * 1024 * 1024;
// 限制一条确定性回退链的字体数量，避免启动配置无限放大资源占用。
const MAX_FALLBACK_FONTS: usize = 16;
// 限制用于注册表匹配和诊断的字体族名称长度。
const MAX_FAMILY_BYTES: usize = 256;

/// 应用随包提供的主字体与有序 fallback 字体数据。
///
/// 配置该值后，Application 不再查询平台字体；任一资产无效都会在首次窗口创建前
/// 返回启动失败，避免 Windows 与 Linux 静默选择不同字体。
#[derive(Clone)]
pub struct FontBundle {
    // 保存必须首先参与布局和度量的主字体。
    primary: FontAsset,
    // 按声明顺序保存只在主字体缺字时参与选择的回退字体。
    fallbacks: Vec<FontAsset>,
}

// 保存字体包内部的一项不可变字体资产。
#[derive(Clone)]
pub(crate) struct FontAsset {
    // 保存供样式字体族解析使用的稳定名称。
    family: String,
    // 保存跨平台完全相同的字体文件字节。
    bytes: FontAssetBytes,
}

// 区分需要拥有的动态数据与可随进程存活的静态嵌入数据。
#[derive(Clone)]
enum FontAssetBytes {
    // 动态或临时输入复制到共享分配，保持既有公开构造契约。
    Shared(Arc<[u8]>),
    // include_bytes! 等静态资产直接借用二进制只读段，不制造启动期大分配。
    Static(&'static [u8]),
}

// 向 FontService 暴露字体来源事实，不泄漏可变所有权。
pub(crate) enum FontAssetSource<'a> {
    // 静态资产可以由支持该能力的文本后端直接借用。
    Static(&'static [u8]),
    // 动态资产仍以共享所有权交接给文本后端。
    Shared(&'a Arc<[u8]>),
}

// 为公开字体包提供无平台依赖的组装入口。
impl FontBundle {
    /// 使用主字体创建确定性字体包。
    pub fn new(family: impl Into<String>, bytes: impl AsRef<[u8]>) -> Self {
        // 主字体在构造时取得字节所有权，调用方临时缓冲区无需继续存活。
        Self {
            // 保存主字体的名称与不可变数据。
            primary: FontAsset::new(family, bytes),
            // 新字体包默认没有额外回退字体。
            fallbacks: Vec::new(),
        }
    }

    /// 使用进程期静态主字体创建零复制确定性字体包。
    ///
    /// 该入口适用于 `include_bytes!` 与其他 `&'static [u8]`；动态缓冲区继续使用
    /// [`Self::new`]，避免把短生命周期借用错误地保留到应用运行期。
    pub fn from_static(family: impl Into<String>, bytes: &'static [u8]) -> Self {
        Self {
            // 静态数据地址在整个进程期稳定，可由字体后端安全建立借用视图。
            primary: FontAsset::from_static(family, bytes),
            // 新字体包默认没有额外回退字体。
            fallbacks: Vec::new(),
        }
    }

    /// 在回退链尾部追加一项字体，声明顺序就是缺字选择顺序。
    pub fn with_fallback(
        // 消费当前字体包以保持 builder 链式调用。
        mut self,
        // 接收回退字体的注册族名。
        family: impl Into<String>,
        // 接收回退字体的完整文件数据。
        bytes: impl AsRef<[u8]>,
    ) -> Self {
        // 保留调用方声明的确定性回退顺序。
        self.fallbacks.push(FontAsset::new(family, bytes));
        // 返回可继续追加 fallback 的同一字体包。
        self
    }

    /// 在回退链尾部追加一项进程期静态字体，不复制其文件数据。
    pub fn with_static_fallback(
        // 消费当前字体包以保持 builder 链式调用。
        mut self,
        // 接收回退字体的注册族名。
        family: impl Into<String>,
        // 只接受由类型系统证明覆盖应用运行期的静态数据。
        bytes: &'static [u8],
    ) -> Self {
        // 保留声明顺序并记录静态来源，安装时由文本后端选择零复制能力。
        self.fallbacks.push(FontAsset::from_static(family, bytes));
        // 返回可继续追加 fallback 的同一字体包。
        self
    }

    /// 返回主字体的注册族名。
    pub fn primary_family(&self) -> &str {
        // 只暴露名称，不泄漏或复制字体数据。
        self.primary.family()
    }

    /// 返回显式回退字体数量。
    pub fn fallback_count(&self) -> usize {
        // Bitmap tofu 不属于应用声明的字体回退链。
        self.fallbacks.len()
    }

    // 按主字体后接 fallback 的顺序返回全部内部资产。
    pub(crate) fn assets(&self) -> impl Iterator<Item = &FontAsset> {
        // 单项主字体与回退切片共享同一稳定迭代顺序。
        std::iter::once(&self.primary).chain(self.fallbacks.iter())
    }

    // 在任何字体进入文本后端前验证完整配置。
    pub(crate) fn validate(&self) -> Result<()> {
        // 过长回退链必须作为配置错误拒绝，不能截断后改变缺字语义。
        if self.fallbacks.len() > MAX_FALLBACK_FONTS {
            // 返回不依赖平台实现的稳定错误分类。
            return Err(Error::new(
                // 字体包超限属于调用方参数错误。
                Errc::InvalidArgument,
                // 诊断包含上限但不输出字体数据。
                format!("font bundle exceeds {MAX_FALLBACK_FONTS} fallback fonts"),
            ));
        }
        // 主字体必须先通过同一项级约束。
        self.primary.validate("primary", 0)?;
        // 按确定性顺序验证全部 fallback。
        for (index, asset) in self.fallbacks.iter().enumerate() {
            // 错误索引从零开始，仅用于稳定定位配置项。
            asset.validate("fallback", index)?;
        }
        // 全部资产都可以安全交给字体解析后端。
        Ok(())
    }
}

// 隐藏字体资产的存储细节，只向 FontService 提供窄读取接口。
impl FontAsset {
    // 创建一项拥有字体数据的内部资产。
    fn new(family: impl Into<String>, bytes: impl AsRef<[u8]>) -> Self {
        // 把借用字节复制到稳定的共享只读分配中。
        Self {
            // 保存调用方声明的字体族名称。
            family: family.into(),
            // 保证 App builder 离开后字体数据仍然有效。
            bytes: FontAssetBytes::Shared(Arc::from(bytes.as_ref())),
        }
    }

    // 创建一项直接借用进程期静态字体数据的内部资产。
    fn from_static(family: impl Into<String>, bytes: &'static [u8]) -> Self {
        Self {
            // 字体族名称仍由配置值拥有。
            family: family.into(),
            // 静态切片不需要额外 Arc 分配或数据复制。
            bytes: FontAssetBytes::Static(bytes),
        }
    }

    // 返回注册表使用的字体族名称。
    pub(crate) fn family(&self) -> &str {
        // 名称只读借用自字体包 owner。
        &self.family
    }

    // 返回文本后端加载的完整字体文件数据。
    pub(crate) fn bytes(&self) -> &[u8] {
        // 两种来源都只向验证和解析阶段开放不可变字节。
        match &self.bytes {
            FontAssetBytes::Shared(bytes) => bytes.as_ref(),
            FontAssetBytes::Static(bytes) => bytes,
        }
    }

    // 返回安装阶段需要的来源事实，让后端保持静态借用或共享所有权。
    pub(crate) fn source(&self) -> FontAssetSource<'_> {
        match &self.bytes {
            // Arc 只在实际安装时克隆引用，不复制字体数据。
            FontAssetBytes::Shared(bytes) => FontAssetSource::Shared(bytes),
            // 静态切片原样交给支持零复制的文本后端。
            FontAssetBytes::Static(bytes) => FontAssetSource::Static(bytes),
        }
    }

    // 验证单项字体资产的名称、数据和资源预算。
    fn validate(&self, role: &str, index: usize) -> Result<()> {
        // 空白名称、控制字符或过长名称都无法成为稳定注册身份。
        if self.family.trim().is_empty()
            || self.family.chars().any(char::is_control)
            || self.family.len() > MAX_FAMILY_BYTES
        {
            // 拒绝无效字体族名称且不回显不可信内容。
            return Err(Error::new(
                // 名称违反公开字体包参数约束。
                Errc::InvalidArgument,
                // 使用角色和索引稳定定位失败项。
                format!("font bundle {role}[{index}] has an invalid family name"),
            ));
        }
        // 空数据或超出单项预算的数据不能进入字体解析器。
        let byte_len = self.bytes().len();
        if byte_len == 0 || byte_len > MAX_FONT_ASSET_BYTES {
            // 返回显式资源边界错误而不尝试平台 fallback。
            return Err(Error::new(
                // 字节载荷违反公开参数约束。
                Errc::InvalidArgument,
                // 报告允许的大小范围，不记录资产内容。
                format!(
                    "font bundle {role}[{index}] bytes must be within 1..={MAX_FONT_ASSET_BYTES}"
                ),
            ));
        }
        // 当前字体资产满足进入后端前的结构约束。
        Ok(())
    }
}
