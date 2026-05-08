import { expect } from "chai";
import { PersonalityService, PersonalityTraits } from "./personality_service";

describe("PersonalityService", () => {
  let service: PersonalityService;

  beforeEach(() => {
    service = new PersonalityService();
  });

  it("should start a training job and reach completion", async function() {
    this.timeout(5000); // Increase timeout for simulation
    const traits: PersonalityTraits = {
      aggressiveness: 0.8,
      positionalStyle: 0.2,
      riskTolerance: 0.9,
      tone: "aggressive",
    };

    const jobId = await service.startTraining("test-agent", traits);
    expect(jobId).to.be.a("string");

    let status = service.getJobStatus(jobId);
    expect(status?.status).to.be.oneOf(["queued", "training"]);

    // Wait for simulation to finish
    await new Promise((resolve) => setTimeout(resolve, 3500));

    status = service.getJobStatus(jobId);
    expect(status?.status).to.equal("completed");
    expect(status?.progress).to.equal(100);
  });

  it("should list all jobs", async () => {
    await service.startTraining("agent-1", { aggressiveness: 0.5, positionalStyle: 0.5, riskTolerance: 0.5, tone: "neutral" });
    await service.startTraining("agent-2", { aggressiveness: 0.7, positionalStyle: 0.3, riskTolerance: 0.8, tone: "humorous" });

    const jobs = service.listJobs();
    expect(jobs).to.have.lengthOf(2);
  });
});
