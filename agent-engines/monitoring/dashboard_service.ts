import { MetricsCollector } from "./metrics_collector";
import { AIHealthAnalyzer } from "./ai_health";

export class DashboardService {
  private collector = new MetricsCollector();
  private analyzer = new AIHealthAnalyzer(this.collector);

  recordLatency(ms: number) {
    this.collector.record("latency", ms);
  }

  recordError() {
    this.collector.record("errors", 1);
  }

  recordThroughput(count: number) {
    this.collector.record("throughput", count);
  }

  recordResources(cpuPct: number, memoryMb: number) {
    this.collector.record("cpu_usage", cpuPct);
    this.collector.record("memory_usage_mb", memoryMb);
  }

  getDashboard() {
    const health = this.analyzer.getHealthStatus();
    return {
      ...health,
      summary: {
        total_requests: this.collector.getSum("throughput"),
        total_errors: this.collector.getSum("errors"),
        avg_latency: Math.round(this.collector.getAverage("latency")),
      },
      time_series: {
        latency: this.collector.getMetrics("latency").slice(-20),
        errors: this.collector.getMetrics("errors").slice(-20),
      }
    };
  }
}