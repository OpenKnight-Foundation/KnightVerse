export type NodeStatus = "online" | "offline" | "degraded";

export interface NodeInfo {
  nodeId: string;
  address: string;
  status: NodeStatus;
  load: number;
  lastSeen: string;
}

export interface ClusterState {
  nodes: NodeInfo[];
  totalLoad: number;
  activeNodes: number;
  inactiveNodes: number;
  averageLoad: number;
}

export class OrchestratorService {
  private nodes: Map<string, NodeInfo> = new Map();

  constructor() {
    // This service can be extended to use cluster discovery, remote health checks, or a control plane.
  }

  registerNode(node: NodeInfo): void {
    this.nodes.set(node.nodeId, node);
  }

  unregisterNode(nodeId: string): void {
    this.nodes.delete(nodeId);
  }

  updateNode(node: Partial<NodeInfo> & { nodeId: string }): void {
    if (!this.nodes.has(node.nodeId)) {
      throw new Error(`Node ${node.nodeId} is not registered`);
    }

    const existing = this.nodes.get(node.nodeId)!;
    this.nodes.set(node.nodeId, {
      ...existing,
      ...node,
    });
  }

  private getOnlineNodes(): NodeInfo[] {
    return Array.from(this.nodes.values()).filter((node) => node.status === "online");
  }

  getClusterState(): ClusterState {
    const nodes = Array.from(this.nodes.values());
    const online = this.getOnlineNodes();
    const totalLoad = nodes.reduce((sum, node) => sum + node.load, 0);
    const activeNodes = online.length;
    const averageLoad = nodes.length ? Number((totalLoad / nodes.length).toFixed(3)) : 0;

    return {
      nodes,
      totalLoad,
      activeNodes,
      inactiveNodes: nodes.length - activeNodes,
      averageLoad,
    };
  }

  async dispatchTask(taskId: string, payload: any): Promise<{ nodeId: string; status: string }> {
    const onlineNodes = this.getOnlineNodes();
    if (onlineNodes.length === 0) {
      throw new Error("No active nodes in cluster");
    }

    const effectiveLoad = (node: NodeInfo) => {
      const demand = typeof payload?.cpuDemand === "number" ? payload.cpuDemand : 0;
      return node.load + demand;
    };

    const bestNode = onlineNodes.reduce((best, node) => {
      return effectiveLoad(node) < effectiveLoad(best) ? node : best;
    }, onlineNodes[0]);

    console.log(`Dispatching task ${taskId} to node ${bestNode.nodeId}`);

    return {
      nodeId: bestNode.nodeId,
      status: "dispatched",
    };
  }
}
