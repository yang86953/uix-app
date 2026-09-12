//! 文件形式 UIX 入口只包含构建期生成物；它们是普通声明宏，不是过程宏。

#[macro_export]
macro_rules! uix {
    ($path:literal) => {{ #[allow(unused_braces)] { include!(concat!(env!("OUT_DIR"), "/uix/view/", $path, ".rs")) } }};
}

#[macro_export]
macro_rules! uix_app {
    ($path:literal) => { include!(concat!(env!("OUT_DIR"), "/uix/app/", $path, ".rs")) };
}

#[macro_export]
macro_rules! uix_items {
    ($path:literal) => { include!(concat!(env!("OUT_DIR"), "/uix/items/", $path, ".rs")); };
}

#[macro_export]
macro_rules! uix_module {
    ($path:literal) => { include!(concat!(env!("OUT_DIR"), "/uix/module/", $path, ".rs")) };
}
