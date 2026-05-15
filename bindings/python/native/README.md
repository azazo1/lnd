# Python native crate

这个目录是 Python 绑定的 Rust 子 crate.

职责:

- 提供 `lnd._native` 原生扩展
- 作为 workspace member 参与仓库统一构建
- 由外层 [bindings/python/pyproject.toml](/Users/azazo1/pjs/rust/lnd/bindings/python/pyproject.toml) 引用
