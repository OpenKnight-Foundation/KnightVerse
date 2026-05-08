export interface PersonalityTraits {
  aggressiveness: number;
  positionalStyle: number;
  riskTolerance: number;
  tone: "neutral" | "aggressive" | "humorous" | "formal";
}

export interface TrainingJob {
  jobId: string;
  agentId: string;
  targetTraits: PersonalityTraits;
  status: "queued" | "training" | "validating" | "completed" | "failed";
  progress: number;
  startedAt?: string;
  completedAt?: string;
  nodeId?: string;
}

export class PersonalityService {
  private jobs: Map<string, TrainingJob> = new Map();

  async startTraining(agentId: string, traits: PersonalityTraits): Promise<string> {
    const jobId = Math.random().toString(36).substring(7);
    const job: TrainingJob = {
      jobId,
      agentId,
      targetTraits: traits,
      status: "queued",
      progress: 0,
    };

    this.jobs.set(jobId, job);
    
    // Simulate background training process
    this.simulateTraining(jobId);
    
    return jobId;
  }

  getJobStatus(jobId: string): TrainingJob | undefined {
    return this.jobs.get(jobId);
  }

  listJobs(): TrainingJob[] {
    return Array.from(this.jobs.values());
  }

  private async simulateTraining(jobId: string) {
    const job = this.jobs.get(jobId);
    if (!job) return;

    job.status = "training";
    job.startedAt = new Date().toISOString();

    for (let i = 0; i <= 100; i += 20) {
      job.progress = i;
      if (i === 80) job.status = "validating";
      await new Promise((resolve) => setTimeout(resolve, 500));
    }

    job.status = "completed";
    job.completedAt = new Date().toISOString();
  }
}
