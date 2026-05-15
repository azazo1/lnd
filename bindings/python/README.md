# lnd-sdk

`lnd-sdk` 是 `lnd` 的 Python 高层绑定. 它对外暴露 `Client`, `DiscoveryFilter`, `AnnounceSpec`, `AnnounceHandle`, `WatchHandle` 等对象接口, 调用方不需要直接接触底层 C ABI.

当前实现基于 `ctypes` 加载 `liblnd`, 因此 wheel 本身是纯 Python 包, 但运行时仍需要能找到 `lnd` 动态库.

## 构建 wheel

```bash
cd bindings/python
uv run python -m build
```

## 安装

```bash
pip install dist/lnd_sdk-0.1.0-py3-none-any.whl
```

运行前需要确保动态库可见:

- Linux: `LD_LIBRARY_PATH`
- macOS: `DYLD_LIBRARY_PATH`
- Windows: `PATH`

也可以通过下面两种方式显式指定动态库:

- 创建 `Client` 时传 `library_path=...`
- 设置环境变量 `LND_LIBRARY_PATH=/path/to/liblnd.so`
