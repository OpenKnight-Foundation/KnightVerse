import { GPUWorker } from "./gpu_worker";

describe("GPUWorker", () => {
  const worker = new GPUWorker();

  it("processes job successfully", async () => {
    const result = await worker.process({
      id: "1",
      payload: { text: "hello" },
    });

    expect(result).toHaveProperty("result");
    expect(result).toHaveProperty("mode");
  });

  it("handles large payload", async () => {
    const large = { data: "x".repeat(10000) };

    const result = await worker.process({
      id: "2",
      payload: large,
    });

    expect(result.result.processed).toBe(true);
  });
});