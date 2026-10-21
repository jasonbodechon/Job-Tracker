import { describe, expect, it } from "vitest";
import { connectorDefaults, effectiveSourceTuning } from "./sourceSettings";

describe("source connector settings", () => {
  it("uses conservative Amazon defaults", () => expect(connectorDefaults.Amazon).toEqual({ requestDelayMs: 250, maxRetries: 4, maxResults: 1000, pageSize: 100, maxCalls: 10, hydrationLimit: 0 }));
  it("uses the current Google page size", () => expect(connectorDefaults.Google).toEqual({ requestDelayMs: 250, maxRetries: 3, maxResults: 1000, pageSize: 20, maxCalls: 50, hydrationLimit: 0 }));
  it("uses rate-safe Microsoft defaults and hydrates fifty changed jobs", () => expect(connectorDefaults.Microsoft).toEqual({ requestDelayMs: 1000, maxRetries: 4, maxResults: 1000, pageSize: 10, maxCalls: 100, hydrationLimit: 50 }));
  it("uses automatic defaults for zero and applies explicit overrides", () => expect(effectiveSourceTuning({ kind: "Amazon", requestDelayMs: 500, maxRetries: 0, maxResults: 250, pageSize: 0, maxCalls: 3, hydrationLimit: 0 })).toEqual({ requestDelayMs: 500, maxRetries: 4, maxResults: 250, pageSize: 100, maxCalls: 3, hydrationLimit: 0 }));
});
