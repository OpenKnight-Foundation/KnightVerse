import { expect } from "chai";
import { AnomalyDetector, AnomalyRiskLevel } from "./anomaly_detector";

describe("AnomalyDetector", () => {
  let detector: AnomalyDetector;

  beforeEach(() => {
    detector = new AnomalyDetector();
  });

  it("should detect high frequency activity from a single user", () => {
    const userId = "user-1";
    for (let i = 0; i < 25; i++) {
      detector.record({
        userId,
        timestamp: Date.now(),
        action: "search",
      });
    }

    const report = detector.analyze(userId);
    expect(report.isBot).to.be.true;
    expect(report.reasons).to.include("User activity rate exceeded threshold");
    expect(report.riskLevel).to.equal(AnomalyRiskLevel.CRITICAL);
  });

  it("should detect many users sharing the same IP hash", () => {
    const ipHash = "shared-ip-123";
    for (let i = 0; i < 6; i++) {
      detector.record({
        userId: `user-${i}`,
        timestamp: Date.now(),
        action: "login",
        ipHash,
      });
    }

    const report = detector.analyze("user-0");
    expect(report.score).to.be.at.least(60);
    expect(report.reasons).to.include("Multiple users sharing same ipHash");
    expect(report.riskLevel).to.be.oneOf([AnomalyRiskLevel.HIGH, AnomalyRiskLevel.CRITICAL]);
  });

  it("should detect synchronized bursts", () => {
    const timestamp = Date.now();
    for (let i = 0; i < 10; i++) {
      detector.record({
        userId: `user-burst-${i}`,
        timestamp,
        action: "request",
      });
    }

    const report = detector.analyze("user-burst-0");
    expect(report.reasons).to.include("Synchronized burst from multiple users");
  });

  it("should detect repeated search profiles", () => {
    const fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    for (let i = 0; i < 7; i++) {
      detector.record({
        userId: `user-bot-${i}`,
        timestamp: Date.now(),
        action: "analyze",
        fen,
        searchParams: { depth: 20 },
      });
    }

    const report = detector.analyze("user-bot-0");
    expect(report.reasons).to.include("Many users repeating the same search profile");
  });

  it("should prune old events outside the sliding window", () => {
    const userId = "old-user";
    const oldTimestamp = Date.now() - 600_000; // 10 mins ago

    detector.record({
      userId,
      timestamp: oldTimestamp,
      action: "old-action",
    });

    // Record a new event to trigger pruning
    detector.record({
      userId: "new-user",
      timestamp: Date.now(),
      action: "new-action",
    });

    const report = detector.analyze(userId);
    expect(report.isBot).to.be.false;
    expect(report.score).to.equal(0);
  });
});