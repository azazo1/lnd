# C native crate

这个目录提供 `liblnd` 的 C ABI 动态库构建入口.

职责:

- 导出稳定的 C ABI
- 生成仓库根目录的 `include/lnd.h`
- 作为 workspace member 参与统一构建和测试

Rust 核心实现保留在根 crate `lnd`, 这个子 crate 只负责 C 侧产物和头文件.
