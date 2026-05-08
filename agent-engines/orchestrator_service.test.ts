import { expect } from "chai";
import { NodeInfo, OrchestratorService } from "./orchestrator_service";

describe("OrchestratorService", () => {
  let service: OrchestratorService;

  beforeEach(() => {
    service = new OrchestratorService();
  });

  it("should register and unregister nodes", () => {
    const node: NodeInfo = {
      nodeId: "node-1",
      address: "localhost",
      status: "online",
      load: 0.5,
      lastSeen: new Date().toISOString(),
    };

    service.registerNode(node);

    let state = service.getClusterState();
    expect(state.nodes).to.have.lengthOf(1);
    expect(state.activeNodes).to.equal(1);
    expect(state.inactiveNodes).to.equal(0);
    expect(state.averageLoad).to.equal(0.5);

    service.unregisterNode("node-1");
    state = service.getClusterState();
    expect(state.nodes).to.have.lengthOf(0);
    expect(state.activeNodes).to.equal(0);
    expect(state.inactiveNodes).to.equal(0);
    expect(state.averageLoad).to.equal(0);
  });

  it("should calculate cluster state correctly", () => {
    service.registerNode({
      nodeId: "n1",
      address: "addr1",
      status: "online",
      load: 0.2,
      lastSeen: "",
    });
    service.registerNode({
      nodeId: "n2",
      address: "addr2",
      status: "offline",
      load: 0.8,
      lastSeen: "",
    });

    const state = service.getClusterState();
    expect(state.totalLoad).to.equal(1.0);
    expect(state.activeNodes).to.equal(1);
    expect(state.inactiveNodes).to.equal(1);
    expect(state.averageLoad).to.equal(0.5);
  });

  it("should dispatch tasks to the least loaded online node", async () => {
    service.registerNode({
      nodeId: "busy",
      address: "addr1",
      status: "online",
      load: 0.9,
      lastSeen: "",
    });
    service.registerNode({
      nodeId: "idle",
      address: "addr2",
      status: "online",
      load: 0.1,
      lastSeen: "",
    });

    const result = await service.dispatchTask("task-1", {});
    expect(result.nodeId).to.equal("idle");
    expect(result.status).to.equal("dispatched");
  });

  it("should dispatch with cpu demand to the least loaded node", async () => {
    service.registerNode({
      nodeId: "almost-idle",
      address: "addr3",
      status: "online",
      load: 0.3,
      lastSeen: "",
    });
    service.registerNode({
      nodeId: "mostly-free",
      address: "addr4",
      status: "online",
      load: 0.4,
      lastSeen: "",
    });

    const result = await service.dispatchTask("task-2", { cpuDemand: 0.05 });
    expect(result.nodeId).to.equal("almost-idle");
  });

  it("should ignore offline nodes when dispatching tasks", async () => {
    service.registerNode({
      nodeId: "offline-node",
      address: "addr5",
      status: "offline",
      load: 0.0,
      lastSeen: "",
    });
    service.registerNode({
      nodeId: "online-node",
      address: "addr6",
      status: "online",
      load: 0.7,
      lastSeen: "",
    });

    const result = await service.dispatchTask("task-3", {});
    expect(result.nodeId).to.equal("online-node");
  });

  it("should throw error if no active nodes", async () => {
    try {
      await service.dispatchTask("t1", {});
      expect.fail("Should have thrown error");
    } catch (e: any) {
      expect(e.message).to.equal("No active nodes in cluster");
    }
  });

  it("should update existing node status", () => {
    service.registerNode({
      nodeId: "n3",
      address: "addr7",
      status: "degraded",
      load: 0.5,
      lastSeen: "",
    });

    service.updateNode({ nodeId: "n3", status: "online", load: 0.2 });

    const state = service.getClusterState();
    expect(state.activeNodes).to.equal(1);
    expect(state.nodes.find((node) => node.nodeId === "n3")?.status).to.equal("online");
  });
});
