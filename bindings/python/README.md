# lnd-sdk

`lnd-sdk` 是 `lnd` 的 Python 高层绑定. 它对外暴露 `Client`, `DiscoveryFilter`, `AnnounceSpec`, `AnnounceHandle`, `WatchHandle` 等对象接口, 调用方不需要直接接触底层 C ABI.

当前实现采用 mixed project 布局:

- 外层 `bindings/python` 放置 `pyproject.toml` 和 Python 高层包 `lnd/`
- 内层 `bindings/python/native` 是 workspace 子 crate, 负责 `maturin + pyo3` 原生扩展

这样可以满足几个目标:

- 主 Rust crate `lnd` 本身不引入 `pyo3`
- Python wheel 可以自包含分发, 不要求用户额外准备 `liblnd`
- 通过 `abi3-py310` 构建单个 wheel, 可覆盖 Python 3.10 及以上版本

`pyo3` 只出现在 `bindings/python/native` 这个附属 crate 中, 不污染主 crate.

## 构建 wheel

```bash
cd bindings/python
maturin build --release
```

## 安装

```bash
pip install target/wheels/lnd_sdk-0.1.0-*.whl
```

## 运行时行为

安装 wheel 后, Python 代码直接调用内置的 `lnd._native`.

调用方不需要额外准备 `liblnd.so`, `liblnd.dylib` 或 `lnd.dll`.
