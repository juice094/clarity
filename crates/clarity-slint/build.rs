//! `clarity-slint` 构建脚本：编译 Slint UI 文件并注册 lucide-slint 图标库。

use std::{collections::HashMap, path::PathBuf};

fn main() {
    // 注册 lucide-slint 图标库，供 .slint 通过 @lucide 引入。
    let library = HashMap::from([("lucide".to_string(), PathBuf::from(lucide_slint::lib()))]);
    let config = slint_build::CompilerConfiguration::new().with_library_paths(library);

    // 编译 ui/ 目录下的所有 .slint 文件为 Rust 模块。
    // 当前阶段仅包含最小桥接验证界面；后续按阶段扩展。
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint UI compilation failed");
}
