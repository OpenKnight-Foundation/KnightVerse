import { AnomalyDetector, Activity, AnomalyReport } from "./anomaly_detector";

const detector = new AnomalyDetector();

export function trackActivity(
  userId: string, 
  action: string, 
  metadata?: Partial<Omit<Activity, 'userId' | 'action' | 'timestamp'>>
): void {
  detector.record({
    userId,
    action,
    timestamp: Date.now(),
    ...metadata,
  });
}

export function checkUser(userId: string): AnomalyReport {
  return detector.analyze(userId);
}

export function resetDetector(): void {
  // In a real app, we might want to clear state for testing
  (detector as any).activityLog = [];
}