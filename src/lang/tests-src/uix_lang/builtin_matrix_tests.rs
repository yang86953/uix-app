//! `uix-lang-compiler/uix_lang/builtin_matrix.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use crate::lang::compiler::projection_schema::{RegistrationStatus, UI_PROJECTION_SCHEMA};

use super::supported_builtin_hint;

#[test]
fn app_and_supported_hint_share_projection_schema() {
    let app = UI_PROJECTION_SCHEMA.component("App").expect("App 应已登记");
    assert_eq!(app.status, RegistrationStatus::Available);
    let names = supported_builtin_hint().split('、').count();
    assert_eq!(names, 112);
}
