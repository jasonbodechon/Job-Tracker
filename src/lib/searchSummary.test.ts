import { describe, expect, it } from "vitest";
import { describeSearchResult, describeSourceSummary, noResultsGuidance } from "./searchSummary";

describe("search result summary", () => {
  it("reports fetched and excluded posting counts", () => {
    expect(describeSearchResult({ imported: 2, inactive: 1, fetched: 12, outsideLocation: 4, belowMinimumScore: 6 })).toBe("2 matching jobs imported (12 fetched; 4 outside the remote/location settings; 6 below the minimum score).");
  });

  it("reports when pagination reaches the configured source limit", () => {
    expect(describeSearchResult({ imported: 20, inactive: 0, fetched: 5000, available: 6200, truncatedSources: ["Amazon (5000 of 6200)"] })).toContain("source limit reached: Amazon (5000 of 6200)");
  });

  it("directs a no-result search to the limiting profile rule", () => {
    expect(noResultsGuidance({ imported: 0, inactive: 0, fetched: 4, belowMinimumScore: 4 })).toContain("lowering Minimum score");
  });

  it("directs location exclusions to the distance and work-style controls", () => {
    const guidance = noResultsGuidance({ imported: 0, inactive: 0, fetched: 4, outsideLocation: 4 });
    expect(guidance).toContain("Distance and work-style eligibility");
    expect(guidance).not.toContain("Remote only");
  });

  it("surfaces source failures before profile advice", () => {
    expect(noResultsGuidance({ imported: 0, inactive: 0, sourceErrors: ["Top Hat: request failed"] })).toContain("Top Hat: request failed");
  });

  it("explains what each source returned", () => {
    const base = { sourceId: "directory-amazon", company: "Amazon", kind: "Amazon", available: 1000, fetched: 1000, imported: 0, outsideLocation: 985, belowMinimumScore: 15 };
    expect(describeSourceSummary(base)).toBe("Amazon: 1,000 jobs checked, 0 imported (985 outside your location settings, 15 below your minimum score).");
    expect(describeSourceSummary({ ...base, available: 8412, imported: 3, outsideLocation: 990, belowMinimumScore: 7 })).toBe("Amazon: 1,000 of 8,412 jobs checked (source limit reached), 3 imported (990 outside your location settings, 7 below your minimum score).");
    expect(describeSourceSummary({ ...base, fetched: 0, available: 0, outsideLocation: 0, belowMinimumScore: 0 })).toBe("Amazon: the source returned no jobs.");
    expect(describeSourceSummary({ ...base, error: "request timed out" })).toBe("Amazon: failed. request timed out");
    expect(describeSourceSummary({ ...base, imported: 2, outsideLocation: 998, belowMinimumScore: 0, warning: "[PARTIAL_REFRESH] Kept 1000 jobs" })).toBe("Amazon: 1,000 jobs checked, 2 imported (998 outside your location settings). Warning: [PARTIAL_REFRESH] Kept 1000 jobs");
  });
});
