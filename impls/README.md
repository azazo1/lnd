# impls

`impls/` 用来放置不依赖 Rust `C ABI` 的协议重实现.

这类实现直接对接 `lnd` 的 `HTTP(S) + REST + SSE` 协议, 目标是:

- 更贴近目标语言自身生态
- 便于该语言的包管理和分发
- 避免为每种语言都引入 `JNI`, `cgo` 或 `FFI` 装配层

当前新增的 `Java` SDK 位于 [java](/Users/azazo1/pjs/rust/lnd/impls/java).
当前 `Go` SDK 位于 [go](/Users/azazo1/pjs/rust/lnd/impls/go).

说明:

- `bindings/` 用来放置直接建立在 Rust 核心之上的绑定封装
- `impls/` 用来放置直接重实现协议的 SDK

这两个目录都属于公开接入面的一部分. 做协议语义变更时, 两边都应检查.
