import { SecurityGuard } from "./security_guard";
import { JobQueue } from "./job_queue";
import { trackActivity, checkUser } from "./anomaly_service";
import { DashboardService } from "./monitoring/dashboard_service";
import { OrchestratorService, NodeInfo } from "./orchestrator_service";
import { PersonalityService, PersonalityTraits } from "./personality_service";

const guard = new SecurityGuard();
const orchestrator = new OrchestratorService();
const personality = new PersonalityService();

export function processPrompt(input: string) {
  if (!guard.validatePrompt(input)) {
    throw new Error("Unsafe prompt detected");
  }

  const sanitized = guard.sanitize(input);

  return {
    processed: sanitized,
  };
}


const queue = new JobQueue();

export function runAIAnalysis(payload: any) {
  queue.enqueue({
    id: Date.now().toString(),
    payload,
  });

  return {
    status: "queued",
  };
}


export function handleUserAction(userId: string, action: string, metadata?: any) {
  trackActivity(userId, action, metadata);

  const analysis = checkUser(userId);

  if (analysis.isBot) {
    return {
      blocked: true,
      riskLevel: analysis.riskLevel,
      score: analysis.score,
      reasons: analysis.reasons,
      findings: analysis.findings,
    };
  }

  return {
    blocked: false,
    riskLevel: analysis.riskLevel,
    score: analysis.score,
  };
}


const dashboard = new DashboardService();

export function trackAIRequest(duration: number, success: boolean) {
  dashboard.recordLatency(duration);
  dashboard.recordThroughput(1);

  if (!success) {
    dashboard.recordError();
  }
}

// Background resource sampling
setInterval(() => {
  const mem = process.memoryUsage();
  // Simplified CPU metric for health analysis
  const cpu = process.cpuUsage();
  const totalCpu = (cpu.user + cpu.system) / 1000000; // seconds
  dashboard.recordResources(totalCpu % 100, mem.heapUsed / (1024 * 1024));
}, 5000);

export function getAIHealthDashboard() {
  return dashboard.getDashboard();
}

export function registerOrchestratorNode(node: NodeInfo) {
  orchestrator.registerNode(node);
}

export function getClusterHealth() {
  return orchestrator.getClusterState();
}

export async function dispatchAITask(taskId: string, payload: any) {
  return orchestrator.dispatchTask(taskId, payload);
}

export async function startPersonalityTraining(agentId: string, traits: PersonalityTraits) {
  return personality.startTraining(agentId, traits);
}

export function getTrainingStatus(jobId: string) {
  return personality.getJobStatus(jobId);
}