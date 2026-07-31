# Rust 架构组合性能对照

该独立基准复现 Diagnostics System 的相同行为，并比较：

- 泛型实现的编译期组合；
- `Box<dyn Trait>` 实现的运行时组合；
- 不经过 System / Module / Component 的扁平直接实现。

所有实现都预分配相同容量、产生相同拥有型条目，并在计时区外校验完整结果。

```sh
cargo run --release
```
