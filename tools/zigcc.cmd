:: 使用 PATH 中的 Zig 作为 Linux GNU linker 包装器。
@echo off
:: 让 Zig 进入 C 编译器兼容子命令并选择 Linux GNU 目标。
zig.exe cc -target x86_64-linux-gnu %*
