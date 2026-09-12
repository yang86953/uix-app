//! 自 src/draw/geometry/spatial/unit.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/draw/geometry/spatial/unit.rs:100）：
/// ```rust
/// use uix_app::draw::geometry::spatial::PhysicalUnitExt;
/// let w = 10.0.mm();
/// let h = 5.0.cm();
/// let fs = 12.0.pt();
/// ```
pub struct DocExample1;


/// 迁出的文档样例 2（原 src/draw/geometry/spatial/unit.rs:168）：
/// ```rust
/// use uix_app::draw::geometry::spatial::AngleExt;
/// let a = 45.0.deg();  // 度 → 弧度
/// let b = 0.5.rad();   // 弧度
/// ```
pub struct DocExample2;
