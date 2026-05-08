import { MetricsCollector } from "./metrics_collector";

export type HealthStatus = "healthy" | "degraded" | "critical";

export class AIHealthAnalyzer {
  constructor(private collector: MetricsCollector) {}

  getHealthStatus() {
    const latency = this.collector.getAverage("latency");
    const errors = this.collector.getSum("errors");
    const throughput = this.collector.getSum("throughput");
    const cpuUsage = this.collector.getMax("cpu_usage");
    const memoryUsage = this.collector.getMax("memory_usage_mb");

    let status: HealthStatus = "healthy";

    // Health thresholds
    const CRITICAL_ERROR_THRESHOLD = 50;
    const DEGRADED_ERROR_THRESHOLD = 10;
    const CRITICAL_LATENCY_MS = 2000;
    const DEGRADED_LATENCY_MS = 1000;
    const CRITICAL_CPU_PCT = 95;
    const DEGRADED_CPU_PCT = 80;

    if (errors > CRITICAL_ERROR_THRESHOLD || latency > CRITICAL_LATENCY_MS || cpuUsage > CRITICAL_CPU_PCT) {
      status = "critical";
    } else if (errors > DEGRADED_ERROR_THRESHOLD || latency > DEGRADED_LATENCY_MS || cpuUsage > DEGRADED_CPU_PCT) {
      status = "degraded";
    }

    return {
      status,
      timestamp: Date.now(),
      metrics: {
        latency: Math.round(latency),
        errors,
        throughput,
        system: {
          cpu_usage_pct: Math.round(cpuUsage * 100) / 100,
          memory_usage_mb: Math.round(memoryUsage * 100) / 100,
        }
      },
    };
  }
}