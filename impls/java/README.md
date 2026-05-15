# lnd Java SDK

这是 `lnd` 的纯 Java 协议客户端实现. 它不依赖 `JNI`, `JNA` 或 Rust 动态库, 直接实现 `lnd` 的 `HTTP(S) + REST + SSE` 协议.

这使它更适合下面两类场景:

- 常规 JVM 应用直接接入
- Android 应用通过 `Java` 或 `Kotlin` 使用

## 为什么这里用纯 Java

现有 Rust 核心理论上可以直接在 Android 中调用, 方式通常是:

1. 把 Rust 交叉编译成 Android 各 ABI 的 `.so`
2. 通过 `JNI` 或 `C ABI` 暴露接口
3. 在 `Java` 或 `Kotlin` 层再包一层易用 API

但当前仓库并没有提供这些 Android 专用内容:

- `NDK` 构建链路
- `JNI` 封装层
- Android `AAR` 打包
- 多 ABI 发布与示例工程

所以对 Android 接入来说, 纯 Java 协议重实现更直接, 也更容易分发.

## 当前能力

Java SDK 现在覆盖:

- `Client`
- `DiscoveryFilter`
- `AnnounceSpec`
- `AddressSelection`
- `AnnounceHandle`
- `WatchHandle`
- `resolveNetworkId()`
- `listNetworkIdCandidates()`
- `listReachabilityScopes()`
- `resolveAnnounceAddrs()`
- `resolveReachabilityScopes()`
- `discover()`
- `discoverWithAutoScopeOverlap()`
- `announceOnce()`
- `announce()`
- `watch()`
- `watchWithAutoScopeOverlap()`

协议语义与 Rust / Go 保持一致:

- `network_id` 可选
- `reachability_scopes` 采用 overlap 匹配
- 自动 LAN 地址收集
- 自动子网前缀推导
- `watch` 的断线重连
- `cursor` 恢复
- `reset` 后自动补快照

## 构建

如果你有 Gradle 环境, 可以直接在这个目录构建:

```bash
gradle jar
```

如果只是本地快速验证, 也可以直接使用仓库根目录的 `just`:

```bash
just java-build
just example-java
```

## 最小示例

```java
import io.github.azazo1.lnd.Client;
import io.github.azazo1.lnd.DiscoveryFilter;

import java.util.List;

public final class Main {
    public static void main(String[] args) throws Exception {
        Client client = new Client("http://127.0.0.1:8765", "dev-token");
        String networkId = client.resolveNetworkId();

        DiscoveryFilter filter = new DiscoveryFilter()
            .withNetworkId(networkId)
            .withService("_demo._tcp")
            .addTag("stable");

        List<String> scopes = client.listReachabilityScopes();
        for (String scope : scopes) {
            filter.addReachabilityScope(scope);
        }

        System.out.println(client.discover(filter));
    }
}
```

完整示例见 [examples/sdk/java/Main.java](/Users/azazo1/pjs/rust/lnd/examples/sdk/java/Main.java).

## Android 接入建议

推荐顺序如下:

1. 优先直接把这个 `Java` SDK 作为源码模块或 Maven 产物接入 Android 工程
2. 如果你的 Android 侧已经大量使用 `Kotlin`, 可以在外层再包一层 `Kotlin` API
3. 只有在你必须复用 Rust 现有内部实现时, 才考虑单独补 `JNI + NDK` 路线

当前这份 Java 代码故意避免依赖额外三方库, 主要为了:

- 降低 Android 集成门槛
- 避免为了一个发现协议再引入额外 `SSE` 或 `JSON` 运行时
- 让协议行为更容易和 Rust / Go 对齐

## 发布建议

当前仓库里已经放好了独立的 Gradle 子项目. 后续如果要面向外部发布, 更推荐:

- 发布到 Maven 仓库, 供 JVM / Android 项目直接依赖
- 在 Android 工程里通过 `includeBuild` 或源码模块方式接入

相比之下, 让 Android 业务层直接调用底层 `C ABI` 并不友好.

## 许可证

Java SDK 跟随仓库根目录的 [MIT License](/Users/azazo1/pjs/rust/lnd/LICENSE).
