import { DashboardService } from "./dashboard_service";

describe("DashboardService", () => {
  let dashboard: DashboardService;

  beforeEach(() => {
    dashboard = new DashboardService();
  });

  it("returns healthy status initially", () => {
    const data = dashboard.getDashboard();
    expect(data.status).toBe("healthy");
    expect(data.metrics.system).toBeDefined();
  });

  it("detects degraded state from errors", () => {
    for (let i = 0; i < 20; i++) {
      dashboard.recordError();
    }

    const data = dashboard.getDashboard();
    expect(data.status).toBe("degraded");
  });

  it("detects critical state from errors", () => {
    for (let i = 0; i < 100; i++) {
      dashboard.recordError();
    }

    const data = dashboard.getDashboard();
    expect(data.status).toBe("critical");
  });

  it("detects degraded state from resources", () => {
    dashboard.recordResources(85, 1024); // 85% CPU

    const data = dashboard.getDashboard();
    expect(data.status).toBe("degraded");
  });
});