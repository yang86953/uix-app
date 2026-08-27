// 引入文件读取与目录遍历能力。
use std::fs;
// 引入稳定路径类型。
use std::path::{Path, PathBuf};

// 引入与公开 uix!、uix_app! 一致的完整文档解析和生成测试入口。
use super::{Diagnostic, generate_document_app, generate_test_document_view, parse_document};

// 返回按路径排序的 Markdown 文件。
fn markdown_paths(markdown_root: &Path) -> Vec<PathBuf> {
    // 读取权威 Markdown 目录。
    let entries = fs::read_dir(markdown_root).unwrap_or_else(|error| {
        // 缺失文档目录时保留具体文件系统错误。
        panic!("无法读取 {}：{error}", markdown_root.display())
    });
    // 只保留直接位于该目录下的 Markdown 文件。
    let mut paths = entries
        // 目录项读取失败应立即暴露治理环境问题。
        .map(|entry| entry.expect("读取组件参考目录项失败").path())
        // 非 Markdown 资产不属于本门禁输入。
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        // 收集为可稳定排序的拥有型路径。
        .collect::<Vec<_>>();
    // 平台目录枚举顺序不进入测试结果。
    paths.sort();
    // 返回确定顺序的权威页面集合。
    paths
}

// 提取 Markdown 中所有标记为 uix 的代码围栏及其内容起始行。
fn fenced_uix_examples(source: &str, display_path: &str) -> Vec<(usize, String)> {
    // 保存已经闭合的示例。
    let mut examples = Vec::new();
    // 保存当前围栏内容的首行；空值表示位于普通 Markdown 中。
    let mut start_line = None;
    // 保存当前围栏的原始内容行。
    let mut buffer = Vec::new();
    // 按原始行序扫描文档。
    for (index, line) in source.lines().enumerate() {
        // 进入明确标记为 uix 的围栏。
        if start_line.is_none() && line.trim() == "```uix" {
            // 下一行是一基索引的内容起始行。
            start_line = Some(index + 2);
            // 新围栏不得继承旧内容。
            buffer.clear();
            // 起始标记本身不属于 UIX 源码。
            continue;
        }
        // 当前 uix 围栏遇到普通闭合标记时完成提取。
        if let Some(start) = start_line.filter(|_| line.trim() == "```") {
            // 保留原始换行结构供解析器产生准确相对位置。
            examples.push((start, buffer.join("\n")));
            // 返回普通 Markdown 状态。
            start_line = None;
            // 清空已交付的围栏内容。
            buffer.clear();
            // 闭合标记不属于 UIX 源码。
            continue;
        }
        // 只在 uix 围栏内部收集源码。
        if start_line.is_some() {
            // 保留当前原始内容行。
            buffer.push(line);
        }
    }
    // 未闭合围栏必须点名具体页面。
    assert!(
        start_line.is_none(),
        "{display_path} 存在未闭合的 uix 代码围栏"
    );
    // 返回当前页面全部示例。
    examples
}

// 验证每个组件参考 UIX 示例都能经过完整解析与 View 生成。
#[test]
fn widget_reference_examples_generate_as_documents() {
    // 从过程宏 crate 定位仓库根目录。
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    // 组件参考目录是本门禁的唯一事实输入。
    let reference_root = repository_root.join("docs/uix-lang/参考/组件");
    // 枚举所有权威组件参考页面。
    let paths = markdown_paths(&reference_root);
    // 空目录不能伪装成门禁成功。
    assert!(!paths.is_empty(), "组件参考目录没有 Markdown 页面");
    // 记录跨页面示例总数，防止全部页面意外失去围栏。
    let mut total_examples = 0_usize;
    // 按确定路径顺序验证每个页面。
    for path in paths {
        // 使用仓库相对路径生成跨机器稳定诊断。
        let display_path = path
            // 参考页面必定位于仓库根目录之下。
            .strip_prefix(&repository_root)
            // 防御性回退仍保留可读完整路径。
            .unwrap_or(&path)
            // 转成平台原生展示文本。
            .display()
            // 保存为后续 panic 可拥有的字符串。
            .to_string();
        // 读取 UTF-8 Markdown 权威文本。
        let source = fs::read_to_string(&path)
            // 文件错误必须点名具体页面。
            .unwrap_or_else(|error| panic!("无法读取 {display_path}：{error}"));
        // 提取当前页面全部 uix 示例。
        let examples = fenced_uix_examples(&source, &display_path);
        // 每个组件参考页面至少应提供一个可执行形状示例。
        assert!(!examples.is_empty(), "{display_path} 没有 uix 代码围栏");
        // 累加实际被验证的示例数量。
        total_examples += examples.len();
        // 分别生成每个示例以保留精确来源行号。
        for (start_line, example) in examples {
            // 统一容器允许文档用多个顶层兄弟展示同类组件。
            let document = format!("<Column>\n{example}\n</Column>");
            // 走与公开宏相同的完整解析、声明校验和 View 生成路径。
            generate_test_document_view(&document).unwrap_or_else(|diagnostic| {
                // 失败必须同时报告页面、围栏起始行与结构化诊断。
                panic!("{display_path}:{start_line} UIX 示例生成失败：{diagnostic:?}")
            });
        }
    }
    // 至少一个示例实际进入完整生成路径。
    assert!(total_examples > 0, "没有验证任何组件参考 UIX 示例");
}

// 验证样式与事件参考中的可执行 UIX 示例持续通过完整生成。
#[test]
fn style_and_event_reference_examples_generate_as_documents() {
    // 从过程宏 crate 定位仓库根目录。
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    // 明确列出不属于组件目录的两个权威参考页面。
    let reference_paths = [
        // 样式页包含顶层样式声明与少量元素示例。
        repository_root.join("docs/uix-lang/参考/样式属性.md"),
        // 事件页包含公开事件绑定示例。
        repository_root.join("docs/uix-lang/参考/事件.md"),
    ];
    // 记录跨页面示例总数，防止输入意外退化为空。
    let mut total_examples = 0_usize;
    // 按声明顺序验证两个权威页面。
    for path in reference_paths {
        // 使用仓库相对路径生成跨机器稳定诊断。
        let display_path = path
            // 两个页面都必须位于仓库根目录下。
            .strip_prefix(&repository_root)
            // 防御性回退仍保留可读完整路径。
            .unwrap_or(&path)
            // 转成平台原生展示文本。
            .display()
            // 保存为后续 panic 可拥有的字符串。
            .to_string();
        // 读取 UTF-8 Markdown 权威文本。
        let source = fs::read_to_string(&path)
            // 文件错误必须点名具体页面。
            .unwrap_or_else(|error| panic!("无法读取 {display_path}：{error}"));
        // 只提取明确标记为可执行 uix 的围栏。
        let examples = fenced_uix_examples(&source, &display_path);
        // 每个参考页面至少应保留一个可执行示例。
        assert!(!examples.is_empty(), "{display_path} 没有 uix 代码围栏");
        // 累加实际进入完整生成路径的示例数量。
        total_examples += examples.len();
        // 分别生成每个示例以保留精确来源行号。
        for (start_line, example) in examples {
            // 判断围栏是否已经包含元素根或元素片段。
            let contains_element = example
                // 逐行检查以避开颜色、比较符或普通文本中的尖括号。
                .lines()
                // 只有以元素起始标记开头的行才视为元素片段。
                .any(|line| line.trim_start().starts_with('<'));
            // 判断首个非空行是否直接从元素片段开始。
            let starts_with_element = example
                // 忽略围栏开头的排版空行。
                .lines()
                // 取得首个承载实际源码的行。
                .find(|line| !line.trim().is_empty())
                // 只有直接元素片段才需要补稳定父根。
                .is_some_and(|line| line.trim_start().starts_with('<'));
            // 为两类参考片段建立可生成的稳定单根文档。
            let document = if starts_with_element {
                // 元素片段允许用多个兄弟展示同一能力。
                format!("<Column>\n{example}\n</Column>")
            } else if contains_element {
                // 顶层声明后已有根元素的完整文档保持原始结构。
                example
            } else {
                // 顶层样式、主题或关键帧声明后追加无业务语义的根 View。
                format!("{example}\n<Container />")
            };
            // 走与公开宏相同的完整解析、声明校验和 View 生成路径。
            generate_test_document_view(&document).unwrap_or_else(|diagnostic| {
                // 失败必须同时报告页面、围栏起始行与结构化诊断。
                panic!("{display_path}:{start_line} UIX 示例生成失败：{diagnostic:?}")
            });
        }
    }
    // 至少一个非组件参考示例实际进入完整生成路径。
    assert!(total_examples > 0, "没有验证任何样式或事件参考 UIX 示例");
}

// 依据真实根元素选择公开宏对应的生成入口。
fn generate_document_source(document: &str) -> Result<(), Diagnostic> {
    // 解析已经补成唯一根形状的完整文档。
    let parsed = parse_document(document)?;
    // App 根必须经过 uix_app! 使用的完整 App builder 生成路径。
    if parsed.root.name == "App" {
        // 丢弃令牌文本，只保留结构化成功或诊断结果。
        generate_document_app(&parsed).map(|_| ())
    } else {
        // 普通 View 与补根后的组件片段经过 uix! 使用的完整 View 生成路径。
        generate_test_document_view(document).map(|_| ())
    }
}

// 为指南中的完整 App、普通 View 与阶段性 Widget 片段补齐文档形状。
fn generate_guide_example(example: &str) -> Result<(), Diagnostic> {
    // 仅声明 Widget 的教程步骤缺少文档根，需要补无业务语义的稳定 View 根。
    let document = if example.contains("<Widget") && !example.contains("<App") {
        // 保留全部声明并追加独立空容器，以验证组件声明的完整生成。
        format!("{example}\n<Container />")
    } else {
        // 已经包含普通 View 或 App 根的示例保持原始契约。
        example.to_owned()
    };
    // 按补齐后的真实根形状走完整生成路径。
    generate_document_source(&document)
}

// 验证快速开始与连续教程中的全部 UIX 示例持续通过对应完整生成。
#[test]
fn guide_examples_generate_through_public_entry_shapes() {
    // 从过程宏 crate 定位仓库根目录。
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    // 明确列出承担入门契约的两份权威指南。
    let guide_paths = [
        // 快速开始覆盖首个 View、计数器 App 及主题 App。
        repository_root.join("docs/uix-lang/指南/快速开始.md"),
        // 教程覆盖逐步 Widget 片段与最终 App。
        repository_root.join("docs/uix-lang/指南/教程.md"),
    ];
    // 记录跨指南示例总数，防止输入意外退化为空。
    let mut total_examples = 0_usize;
    // 按声明顺序验证两份权威指南。
    for path in guide_paths {
        // 使用仓库相对路径生成跨机器稳定诊断。
        let display_path = path
            // 两份指南都必须位于仓库根目录下。
            .strip_prefix(&repository_root)
            // 防御性回退仍保留可读完整路径。
            .unwrap_or(&path)
            // 转成平台原生展示文本。
            .display()
            // 保存为后续 panic 可拥有的字符串。
            .to_string();
        // 读取 UTF-8 Markdown 权威文本。
        let source = fs::read_to_string(&path)
            // 文件错误必须点名具体指南。
            .unwrap_or_else(|error| panic!("无法读取 {display_path}：{error}"));
        // 提取当前指南全部标记为可执行的 uix 示例。
        let examples = fenced_uix_examples(&source, &display_path);
        // 每份指南至少应保留一个可执行示例。
        assert!(!examples.is_empty(), "{display_path} 没有 uix 代码围栏");
        // 累加实际进入完整生成路径的示例数量。
        total_examples += examples.len();
        // 分别生成每个示例以保留精确来源行号。
        for (start_line, example) in examples {
            // 依据真实根形状走与公开 uix! 或 uix_app! 相同的生成路径。
            generate_guide_example(&example).unwrap_or_else(|diagnostic| {
                // 失败必须同时报告页面、围栏起始行与结构化诊断。
                panic!("{display_path}:{start_line} UIX 指南示例生成失败：{diagnostic:?}")
            });
        }
    }
    // 当前两份指南应恰好覆盖快速开始三个与教程六个 uix 围栏。
    assert_eq!(total_examples, 9, "UIX 指南示例数量发生未审查变化");
}

// 把规范中的声明、元素片段与完整文档规范化为唯一根文档。
fn materialize_specification_example(example: &str) -> String {
    // 查找首个承载实际源码且不是单行注释的行。
    let first_code_line = example
        // 按原始源码行序扫描。
        .lines()
        // 去除仅用于排版的首尾空白。
        .map(str::trim)
        // 忽略空行与位于示例开头的说明注释。
        .find(|line| !line.is_empty() && !line.starts_with("//"))
        // uix 围栏不得是空源码。
        .expect("规范 uix 围栏不得为空");
    // App 根必须保持在文档顶层，不能被普通容器包裹。
    if first_code_line.starts_with("<App") {
        // 返回完整 App 原文。
        return example.to_owned();
    }
    // Widget、Record 与 Visual 是顶层声明，不能被普通容器包裹。
    let starts_with_declaration = first_code_line.starts_with("<Widget")
        // Record 同样属于根元素之前的声明。
        || first_code_line.starts_with("<Record")
        // Visual 同样属于根元素之前的模块级声明。
        || first_code_line.starts_with("<Visual")
        // 主题和模块指令使用 @ 前缀。
        || first_code_line.starts_with('@')
        // 样式类声明以标识符开头而不是元素标记。
        || !first_code_line.starts_with('<');
    // 普通元素片段统一放进 Column，以容纳一个或多个兄弟元素。
    if !starts_with_declaration {
        // Column 只提供稳定根形状，不改变被测元素自身语义。
        return format!("<Column>\n{example}\n</Column>");
    }
    // 已经包含顶层声明和唯一根的完整文档保持原样。
    if parse_document(example).is_ok() {
        // 返回已经满足唯一根契约的源码。
        return example.to_owned();
    }
    // 纯声明示例追加无业务语义的空容器，以触发全部声明生成。
    format!("{example}\n<Container />")
}

// 验证规范目录中全部标记为可执行的 UIX 围栏持续通过完整生成。
#[test]
fn specification_examples_generate_as_documents() {
    // 从过程宏 crate 定位仓库根目录。
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    // 规范目录是语言当前契约示例的唯一事实输入。
    let specification_root = repository_root.join("docs/uix-lang/规范");
    // 枚举全部规范页面，让新增页面自动进入治理范围。
    let paths = markdown_paths(&specification_root);
    // 空目录不能伪装成门禁成功。
    assert!(!paths.is_empty(), "UIX 规范目录没有 Markdown 页面");
    // 记录全部当前可执行围栏数量，防止分类未经审查地改变。
    let mut total_examples = 0_usize;
    // 按确定路径顺序验证每份规范页面。
    for path in paths {
        // 使用仓库相对路径生成跨机器稳定诊断。
        let display_path = path
            // 规范页面必定位于仓库根目录之下。
            .strip_prefix(&repository_root)
            // 防御性回退仍保留可读完整路径。
            .unwrap_or(&path)
            // 转成平台原生展示文本。
            .display()
            // 保存为后续 panic 可拥有的字符串。
            .to_string();
        // 读取 UTF-8 Markdown 权威文本。
        let source = fs::read_to_string(&path)
            // 文件错误必须点名具体规范页面。
            .unwrap_or_else(|error| panic!("无法读取 {display_path}：{error}"));
        // 只提取明确标记为当前可执行契约的 uix 围栏。
        let examples = fenced_uix_examples(&source, &display_path);
        // 累加当前页面实际进入完整生成路径的示例数量。
        total_examples += examples.len();
        // 分别生成每个示例以保留精确来源行号。
        for (start_line, example) in examples {
            // 把声明或元素片段补成可交给公开宏的唯一根文档。
            let document = materialize_specification_example(&example);
            // 依据真实根形状走完整 View 或 App 生成路径。
            generate_document_source(&document).unwrap_or_else(|diagnostic| {
                // 失败必须同时报告页面、围栏起始行与结构化诊断。
                panic!("{display_path}:{start_line} UIX 规范示例生成失败：{diagnostic:?}")
            });
        }
    }
    // 当前规范应有三十一个明确标记为可执行的示例进入完整生成路径。
    // 2026-08-27 审查：语法 · Widget 成员由单一属性形式拆为块级与属性两个
    // 等价围栏（净 +1）；两种形式的解析等价性由 widget_member_block_tests 锁定。
    assert_eq!(total_examples, 31, "UIX 规范示例数量发生未审查变化");
}

// 验证已宣告完整实现的目标设计示例持续通过当前完整生成路径。
#[test]
fn completed_target_design_examples_generate_as_documents() {
    // 从过程宏 crate 定位仓库根目录。
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    // 目标设计页面承载已经完成并回填的七项设计事实。
    let target_design_path = repository_root.join("docs/uix-lang/变更/目标设计.md");
    // 使用仓库相对路径生成跨机器稳定诊断。
    let display_path = target_design_path
        // 目标设计页面必定位于仓库根目录之下。
        .strip_prefix(&repository_root)
        // 防御性回退仍保留可读完整路径。
        .unwrap_or(&target_design_path)
        // 转成平台原生展示文本。
        .display()
        // 保存为后续 panic 可拥有的字符串。
        .to_string();
    // 读取 UTF-8 Markdown 权威文本。
    let source = fs::read_to_string(&target_design_path)
        // 文件错误必须点名具体目标设计页面。
        .unwrap_or_else(|error| panic!("无法读取 {display_path}：{error}"));
    // 提取全部仍标记为当前可执行契约的 uix 围栏。
    let examples = fenced_uix_examples(&source, &display_path);
    // 当前已完成目标设计应稳定保留六个可执行示例。
    assert_eq!(examples.len(), 6, "UIX 目标设计示例数量发生未审查变化");
    // 分别生成每个示例以保留精确来源行号。
    for (start_line, example) in examples {
        // 复用规范示例规则补齐声明或元素片段的唯一根。
        let document = materialize_specification_example(&example);
        // 依据真实根形状走完整 View 或 App 生成路径。
        generate_document_source(&document).unwrap_or_else(|diagnostic| {
            // 失败必须同时报告页面、围栏起始行与结构化诊断。
            panic!("{display_path}:{start_line} UIX 目标设计示例生成失败：{diagnostic:?}")
        });
    }
}
