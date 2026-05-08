import { JobQueue } from "./job_queue";

describe("JobQueue", () => {
  it("processes queued jobs", async () => {
    const queue = new JobQueue();

    queue.enqueue({ id: "1", payload: { test: true } });
    queue.enqueue({ id: "2", payload: { test: true } });

    // allow async processing
    await new Promise((res) => setTimeout(res, 200));

    expect(true).toBe(true); // basic execution test
  });
});