# Local Network Discovery agent notes

LND 就是 Local Network Discovery 的缩写.

## Goal

这个仓库最常见的问题不是单点实现错误, 而是协议, 核心实现, CLI, 绑定层, 文档, 示例, 测试之间出现局部不一致.

做改动时, 优先保证整体语义一致, 不要只修一个表层入口.

## Design principles

### 1. Public contract first

只要对外暴露的语法, 字段, 默认值, 错误语义, 匹配语义, 生命周期语义发生变化, 就应把它视为 public contract change.

遇到这类改动时, 不要只改某一个入口. 需要同时检查:

- 核心协议模型
- 核心 client / server 语义
- CLI 行为
- 各语言绑定
- 文档和示例
- 关键测试

### 2. Do not assume all bindings are implemented the same way

不同语言接入层的实现方式可能不同.

- 有的绑定可能直接复用核心实现
- 有的绑定可能只是薄封装
- 有的绑定可能是独立重写的协议 client

因此不能假设主实现改了, 其他语言就会自动正确. 每个绑定都要单独确认语义是否仍然等价.

### 3. Behavior parity matters more than API name parity

跨语言等价不只是函数名或字段名对应, 更重要的是行为一致, 包括:

- 默认值
- 自动推导策略
- 重试和重连语义
- 过滤和匹配规则
- 错误条件
- 输出形状

如果某种语言由于生态差异不能完全照搬接口形状, 至少要保持能力和语义尽量等价, 并在文档中明确差异.

### 4. Respect the difference between bindings and protocol reimplementations

仓库中的跨语言接入层不一定都属于同一类:

- `bindings` 可能是建立在 Rust 核心之上的绑定封装
- `impls` 可能是直接重实现协议的独立 SDK

这两类入口的同步风险不同, 但都属于 public surface.

因此当 discovery 协议, 默认值, 自动推导规则, watch / announce 语义发生变化时, 不要只检查一种接入层.
要先判断这次变更会影响:

- Rust 核心直接复用型绑定
- 纯协议重实现型 SDK
- 两者同时

尤其不能假设某个纯协议 SDK 会随着 Rust 改动自动保持正确.

### 5. Generated artifacts are not the source of truth

如果仓库中存在生成产物, 应修改其真正的源头, 然后重新生成或校验产物.

不要长期手工维护派生文件, 除非项目已经明确把它们当作手写源文件使用.

### 6. Docs, examples, and tests are part of the product

推荐用法一旦改变, 文档, 示例, 测试也要一起更新.

否则用户看到的系统会分裂成两套:

- 代码里真正实现的一套
- 文档和示例里仍然展示的旧一套

这类不一致和代码 bug 一样严重.

### 7. Prefer stable semantics over clever local fixes

涉及 identity, discovery, filtering, replay, lease, auto detection 这类基础语义时, 优先追求:

- 可解释
- 可移植
- 可跨语言复现
- 可长期演进

不要为了局部方便引入很难在其他绑定或其他平台复现的隐式规则.

### 8. Keep the core clean

跨语言接入层应该尽量是非侵入式的.

如果某个特定语言的打包, 运行时, 或生态约束只属于该语言, 优先把它放在附属层 (子 crate) 处理, 不要无必要污染核心实现.

### 9. Validate semantics, not only compilation

编译通过只能说明接口表面大致对齐, 不能说明行为真的一致.

做协议或语义改动时, 至少要验证:

- 关键路径是否仍然可用
- 新旧默认行为是否符合预期
- 绑定层是否真的能产生与核心一致的结果

如果某项能力跨多个入口暴露, 最好至少覆盖一个核心测试和一个绑定层或 CLI 层面的验证.

### 10. This file must evolve with the architecture

AGENTS.md 不是一次性文档.

当仓库的公开面, 绑定架构, 生成链路, 推荐发现模型, 或身份模型发生变化时, 应同步更新本文件, 让它继续描述当前项目真正需要遵守的原则.

更新本文件时:

- 优先保留长期稳定的原则
- 删除已经过时的约束
- 补上最近实际踩过的坑
- 不要把它写成固定文件清单或短期操作手册

目标是: 它应该帮助未来的修改者发现"哪些地方容易局部不一致", 而不是替代探索代码本身.

### 11. Release

仓库不需要发布二进制可执行文件.
也不需要根据 commit 和工作区决定版本号的后缀.
