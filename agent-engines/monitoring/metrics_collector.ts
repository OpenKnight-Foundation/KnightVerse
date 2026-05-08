export type Metric = {
  name: string;
  value: number;
  timestamp: number;
};

export class MetricsCollector {
  private metrics: Metric[] = [];
  private readonly MAX_AGE_MS = 3600_000; // 1 hour
  private readonly MAX_METRICS = 1000;

  record(name: string, value: number) {
    this.metrics.push({
      name,
      value,
      timestamp: Date.now(),
    });
    this.prune();
  }

  private prune() {
    const now = Date.now();
    const cutoff = now - this.MAX_AGE_MS;
    
    // Time-based pruning
    while (this.metrics.length > 0 && this.metrics[0].timestamp < cutoff) {
      this.metrics.shift();
    }

    // Size-based pruning
    if (this.metrics.length > this.MAX_METRICS) {
      this.metrics = this.metrics.slice(-this.MAX_METRICS);
    }
  }

  getMetrics(name?: string): Metric[] {
    if (!name) return this.metrics;
    return this.metrics.filter((m) => m.name === name);
  }

  getAverage(name: string): number {
    const data = this.getMetrics(name);
    if (data.length === 0) return 0;
    return data.reduce((sum, m) => sum + m.value, 0) / data.length;
  }

  getSum(name: string): number {
    return this.getMetrics(name).reduce((sum, m) => sum + m.value, 0);
  }

  getMax(name: string): number {
    const data = this.getMetrics(name);
    if (data.length === 0) return 0;
    return Math.max(...data.map(m => m.value));
  }

  clear() {
    this.metrics = [];
  }
}