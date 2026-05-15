# lnd Go SDK

这是 `lnd` 的 Go 高层 SDK. 它直接实现 `lnd` 的 HTTP(S) + REST + SSE 协议, 不依赖 cgo, 因此可以作为纯 Go module 使用.

这意味着只要仓库和 tag 可见, 外部项目可以直接:

```bash
go get github.com/azazo1/lnd/bindings/go
```

当前前提:

- 仓库路径已经是 `github.com/azazo1/lnd`
- 需要发布对应 tag, 才适合给外部项目稳定引用

SDK 公开对象:

- `Client`
- `DiscoveryFilter`
- `AnnounceSpec`
- `AddressSelection`
- `AnnounceHandle`
- `WatchHandle`

自动 `network_id` 相关接口:

- `client.ResolveNetworkID()`
- `client.ListNetworkIDCandidates()`
- `ResolveNetworkIDWithSelection(selection)`
- `ListNetworkIDCandidates(selection)`

当前自动推导策略与 Rust 保持一致, 基于本机子网前缀指纹, 不依赖网关 MAC 或 cgo.

最小示例:

```go
client := lnd.NewClient("http://127.0.0.1:8765", "dev-token")
networkID, err := client.ResolveNetworkID()
if err != nil {
	return err
}
nodes, err := client.Discover(
	context.Background(),
	lnd.NewDiscoveryFilter(networkID).WithService("_demo._tcp").AddTag("stable"),
)
if err != nil {
	return err
}
_ = nodes
```

它的 discover, announce, watch 语义与 Rust SDK 对齐, 包括:

- client 侧自动 LAN 地址解析
- `AddressSelection` 控制 loopback, IPv6, 接口白名单和黑名单
- SSE watch 的断线重连, cursor 恢复和 `reset` 后快照重同步
