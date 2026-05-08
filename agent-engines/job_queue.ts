import { GPUWorker, GPUJob } from "./gpu_worker";

export class JobQueue {
  private queue: GPUJob[] = [];
  private worker: GPUWorker;
  private processing = false;

  constructor() {
    this.worker = new GPUWorker();
  }

  enqueue(job: GPUJob) {
    this.queue.push(job);
    this.processNext();
  }

  private async processNext() {
    if (this.processing) return;
    if (this.queue.length === 0) return;

    this.processing = true;

    const job = this.queue.shift();

    if (!job) return;

    try {
      const result = await this.worker.process(job);
      console.log("Processed:", result);
    } catch (err) {
      console.error("Job failed:", err);
    }

    this.processing = false;
    this.processNext();
  }
}