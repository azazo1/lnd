package io.github.azazo1.lnd;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * watch 流中的标准事件模型.
 *
 * <p>事件类型:
 *
 * <ul>
 *   <li>`SNAPSHOT`: 首次快照或 reset 后重同步快照
 *   <li>`UPSERT`: 节点上线或续租
 *   <li>`REMOVE`: 节点下线或过期
 *   <li>`RESET`: cursor 无法恢复, 调用方应准备接受新快照
 *   <li>`KEEPALIVE`: 服务端保活事件
 * </ul>
 */
public final class DiscoveryEvent {
    /**
     * 事件类型枚举.
     */
    public enum Type {
        SNAPSHOT("snapshot"),
        UPSERT("upsert"),
        REMOVE("remove"),
        RESET("reset"),
        KEEPALIVE("keepalive");

        private final String wireName;

        Type(String wireName) {
            this.wireName = wireName;
        }

        /**
         * 返回协议层字符串形式.
         *
         * @return 线协议名称
         */
        public String wireName() {
            return wireName;
        }

        static Type fromWireName(String wireName) throws LndException {
            for (Type value : values()) {
                if (value.wireName.equals(wireName)) {
                    return value;
                }
            }
            throw new LndException("unsupported discovery event type: " + wireName);
        }
    }

    private final Type type;
    private final List<DiscoveredNode> nodes;
    private final DiscoveredNode node;

    private DiscoveryEvent(Type type, List<DiscoveredNode> nodes, DiscoveredNode node) {
        this.type = type;
        this.nodes = Collections.unmodifiableList(nodes);
        this.node = node;
    }

    /**
     * 创建 snapshot 事件.
     *
     * @param nodes 快照节点列表
     * @return snapshot 事件
     */
    public static DiscoveryEvent snapshot(List<DiscoveredNode> nodes) {
        return new DiscoveryEvent(Type.SNAPSHOT, new ArrayList<DiscoveredNode>(nodes), null);
    }

    /**
     * 创建 upsert 事件.
     *
     * @param node 被 upsert 的节点
     * @return upsert 事件
     */
    public static DiscoveryEvent upsert(DiscoveredNode node) {
        return new DiscoveryEvent(Type.UPSERT, Collections.<DiscoveredNode>emptyList(), node);
    }

    /**
     * 创建 remove 事件.
     *
     * @param node 被移除的节点
     * @return remove 事件
     */
    public static DiscoveryEvent remove(DiscoveredNode node) {
        return new DiscoveryEvent(Type.REMOVE, Collections.<DiscoveredNode>emptyList(), node);
    }

    /**
     * 创建 reset 事件.
     *
     * @return reset 事件
     */
    public static DiscoveryEvent reset() {
        return new DiscoveryEvent(Type.RESET, Collections.<DiscoveredNode>emptyList(), null);
    }

    /**
     * 创建 keepalive 事件.
     *
     * @return keepalive 事件
     */
    public static DiscoveryEvent keepalive() {
        return new DiscoveryEvent(Type.KEEPALIVE, Collections.<DiscoveredNode>emptyList(), null);
    }

    /**
     * 返回事件类型.
     *
     * @return 事件类型
     */
    public Type getType() {
        return type;
    }

    /**
     * 返回 snapshot 节点列表.
     *
     * <p>注意事项:
     *
     * <ul>
     *   <li>仅 `SNAPSHOT` 事件有值
     *   <li>其他事件返回空列表
     * </ul>
     *
     * @return 不可变列表
     */
    public List<DiscoveredNode> getNodes() {
        return nodes;
    }

    /**
     * 返回单节点事件的节点对象.
     *
     * <p>注意事项:
     *
     * <ul>
     *   <li>仅 `UPSERT` 和 `REMOVE` 事件有值
     *   <li>其他事件返回 `null`
     * </ul>
     *
     * @return 节点对象或 `null`
     */
    public DiscoveredNode getNode() {
        return node;
    }

    @Override
    public String toString() {
        return "DiscoveryEvent{"
            + "type=" + type
            + ", nodes=" + nodes
            + ", node=" + node
            + '}';
    }
}
