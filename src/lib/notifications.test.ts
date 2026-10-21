import { describe, expect, it } from "vitest";
import { notify } from "./notifications";

describe("browser notifications", () => {
  it("returns false when the browser notification API is unavailable", async () => {
    expect(await notify("Job Tracker", "Test")).toBe(false);
  });
});
