import { spawnSync } from "node:child_process";
import { describe, expect, it } from "vitest";

const run = (...args: string[]) => spawnSync(process.execPath, ["deployment/local-tools.mjs", ...args], { cwd: process.cwd(), encoding: "utf8" });

describe("local-tools helper for the local CMD launcher", () => {
  it("generates unique 32-byte hex secrets", () => {
    const first = run("secret");
    const second = run("secret");
    expect(first.status).toBe(0);
    expect(first.stdout.trim()).toMatch(/^[0-9a-f]{64}$/);
    expect(second.stdout.trim()).not.toBe(first.stdout.trim());
  });

  it("prints a file-name-safe backup timestamp", () => {
    const result = run("timestamp");
    expect(result.status).toBe(0);
    expect(result.stdout.trim()).toMatch(/^\d{8}-\d{6}$/);
  });

  it("checks the minimum Node.js major version", () => {
    const current = Number.parseInt(process.versions.node.split(".")[0], 10);
    expect(run("check-node", String(current)).status).toBe(0);
    expect(run("check-node", String(current + 1)).status).toBe(1);
    expect(run("check-node", String(current)).stdout.trim()).toBe(process.versions.node);
    expect(run("check-node").status).toBe(2);
    expect(run("unknown").status).toBe(2);
  });
});
