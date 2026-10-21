import { describe, expect, it } from "vitest";
import { describeSearchResult, noResultsGuidance } from "./searchSummary";

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
});
