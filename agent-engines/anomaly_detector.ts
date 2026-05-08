export enum AnomalyRiskLevel {
  LOW = "low",
  MODERATE = "moderate",
  HIGH = "high",
  CRITICAL = "critical",
}

export interface Activity {
  userId: string;
  timestamp: number;
  action: string;
  ipHash?: string;
  deviceHash?: string;
  fen?: string;
  searchParams?: any;
}

export interface AnomalyFinding {
  riskLevel: AnomalyRiskLevel;
  score: number;
  reason: string;
  affectedUserIds: string[];
  evidence: any;
}

export interface AnomalyReport {
  userId: string;
  riskLevel: AnomalyRiskLevel;
  score: number;
  findings: AnomalyFinding[];
  isBot: boolean;
  reasons: string[];
}

export class AnomalyDetector {
  private activityLog: Activity[] = [];
  
  // Configuration
  private WINDOW_MS = 300_000; // 5 minutes
  private MAX_EVENTS = 10000;
  private BUCKET_SIZE_MS = 10_000; // 10 seconds for burst detection
  
  // Thresholds
  private SHARED_HASH_THRESHOLD = 5;
  private BURST_THRESHOLD = 8;
  private REPEATED_PROFILE_THRESHOLD = 5;
  private RATE_THRESHOLD = 20;

  record(activity: Activity) {
    this.activityLog.push(activity);
    this.prune();
  }

  analyze(userId: string): AnomalyReport {
    const currentEvents = this.activityLog;
    const findings: AnomalyFinding[] = [];

    // 1. Shared Hash Detection (IP/Device)
    findings.push(...this.findSharedHashes(currentEvents, "ipHash"));
    findings.push(...this.findSharedHashes(currentEvents, "deviceHash"));

    // 2. Synchronized Burst Detection
    findings.push(...this.findSynchronizedBursts(currentEvents));

    // 3. Repeated Profile Analysis
    findings.push(...this.findRepeatedProfiles(currentEvents));

    // 4. Individual Rate Spike Detection
    findings.push(...this.findRateSpikes(currentEvents, userId));

    // Aggregate results
    const score = this.calculateAggregateScore(findings);
    const riskLevel = this.getRiskLevel(score);

    return {
      userId,
      riskLevel,
      score,
      findings,
      isBot: score >= 60,
      reasons: findings.map((f) => f.reason),
    };
  }

  private prune() {
    const now = Date.now();
    const cutoff = now - this.WINDOW_MS;
    
    // Time-based pruning
    while (this.activityLog.length > 0 && this.activityLog[0].timestamp < cutoff) {
      this.activityLog.shift();
    }

    // Size-based pruning
    if (this.activityLog.length > this.MAX_EVENTS) {
      this.activityLog = this.activityLog.slice(-this.MAX_EVENTS);
    }
  }

  private findSharedHashes(events: Activity[], field: "ipHash" | "deviceHash"): AnomalyFinding[] {
    const hashToUsers: Record<string, Set<string>> = {};
    for (const event of events) {
      const hash = event[field];
      if (hash) {
        if (!hashToUsers[hash]) hashToUsers[hash] = new Set();
        hashToUsers[hash].add(event.userId);
      }
    }

    const findings: AnomalyFinding[] = [];
    for (const hash in hashToUsers) {
      const userCount = hashToUsers[hash].size;
      if (userCount >= this.SHARED_HASH_THRESHOLD) {
        const score = Math.min(100, 45 + userCount * 5);
        findings.push({
          riskLevel: this.getRiskLevel(score),
          score,
          reason: `Multiple users sharing same ${field}`,
          affectedUserIds: Array.from(hashToUsers[hash]),
          evidence: { hash, userCount },
        });
      }
    }
    return findings;
  }

  private findSynchronizedBursts(events: Activity[]): AnomalyFinding[] {
    const bucketToUsers: Record<number, Set<string>> = {};
    for (const event of events) {
      const bucket = Math.floor(event.timestamp / this.BUCKET_SIZE_MS);
      if (!bucketToUsers[bucket]) bucketToUsers[bucket] = new Set();
      bucketToUsers[bucket].add(event.userId);
    }

    const findings: AnomalyFinding[] = [];
    for (const bucket in bucketToUsers) {
      const userCount = bucketToUsers[bucket].size;
      if (userCount >= this.BURST_THRESHOLD) {
        const score = Math.min(100, 30 + userCount * 4);
        findings.push({
          riskLevel: this.getRiskLevel(score),
          score,
          reason: "Synchronized burst from multiple users",
          affectedUserIds: Array.from(bucketToUsers[bucket]),
          evidence: { bucket, userCount },
        });
      }
    }
    return findings;
  }

  private findRepeatedProfiles(events: Activity[]): AnomalyFinding[] {
    const profileToUsers: Record<string, Set<string>> = {};
    for (const event of events) {
      if (event.fen) {
        const profile = `${event.fen}|${JSON.stringify(event.searchParams || {})}`;
        if (!profileToUsers[profile]) profileToUsers[profile] = new Set();
        profileToUsers[profile].add(event.userId);
      }
    }

    const findings: AnomalyFinding[] = [];
    for (const profile in profileToUsers) {
      const userCount = profileToUsers[profile].size;
      if (userCount >= this.REPEATED_PROFILE_THRESHOLD) {
        const score = Math.min(100, 30 + userCount * 6);
        findings.push({
          riskLevel: this.getRiskLevel(score),
          score,
          reason: "Many users repeating the same search profile",
          affectedUserIds: Array.from(profileToUsers[profile]),
          evidence: { profile, userCount },
        });
      }
    }
    return findings;
  }

  private findRateSpikes(events: Activity[], userId: string): AnomalyFinding[] {
    const userEvents = events.filter((e) => e.userId === userId);
    if (userEvents.length >= this.RATE_THRESHOLD) {
      const score = Math.min(100, 25 + userEvents.length * 3);
      return [{
        riskLevel: this.getRiskLevel(score),
        score,
        reason: "User activity rate exceeded threshold",
        affectedUserIds: [userId],
        evidence: { eventCount: userEvents.length },
      }];
    }
    return [];
  }

  private calculateAggregateScore(findings: AnomalyFinding[]): number {
    if (findings.length === 0) return 0;
    const maxScore = Math.max(...findings.map((f) => f.score));
    const bonus = Math.min(15, 5 * (findings.length - 1));
    return Math.min(100, maxScore + bonus);
  }

  private getRiskLevel(score: number): AnomalyRiskLevel {
    if (score >= 85) return AnomalyRiskLevel.CRITICAL;
    if (score >= 60) return AnomalyRiskLevel.HIGH;
    if (score >= 25) return AnomalyRiskLevel.MODERATE;
    return AnomalyRiskLevel.LOW;
  }
}