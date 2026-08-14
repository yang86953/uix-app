# UIX 生成文件诊断夹具

该独立 Cargo 工程故意引用未定义的 `missing_value`。执行：

```powershell
cargo +stable-x86_64-pc-windows-gnullvm check --manifest-path tests/fixtures/uix_lang/generated_error/Cargo.toml
```

预期编译失败位置位于 `$OUT_DIR/uix-lang-gen/main-*.rs`，错误行之前包含 `// [uix-lang] src/main.uix:3:10` 映射注释。该夹具不属于根 workspace，避免正常测试被故意的编译错误阻断。
